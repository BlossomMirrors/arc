use libarc::{Provider, Transaction, TransactionStatus, TransactionType};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

const HISTORY_LIMIT: usize = 50;

pub struct TransactionManager {
    // rwlock lets many readers run at the same time but only one writer,
    // useful here because reads (get, list) are way more common than writes
    transactions: Arc<RwLock<HashMap<Uuid, Transaction>>>,
    history: Arc<RwLock<VecDeque<Transaction>>>,
    // cancellation tokens for running transactions
    cancellation_tokens: Arc<RwLock<HashMap<Uuid, CancellationToken>>>,
}

impl TransactionManager {
    pub fn new() -> Self {
        Self {
            transactions: Arc::new(RwLock::new(HashMap::new())),
            history: Arc::new(RwLock::new(load_history())),
            cancellation_tokens: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn create(
        &self,
        t_type: TransactionType,
        pkg_id: String,
        provider: Provider,
    ) -> (Transaction, CancellationToken) {
        let tx = Transaction::new(t_type, pkg_id, provider);
        let cancel_token = CancellationToken::new();
        let mut map = self.transactions.write().await;
        let mut tokens = self.cancellation_tokens.write().await;
        map.insert(tx.id, tx.clone());
        tokens.insert(tx.id, cancel_token.clone());
        (tx, cancel_token)
    }

    pub async fn update_progress(&self, id: Uuid, progress: u8) {
        let mut map = self.transactions.write().await;
        if let Some(tx) = map.get_mut(&id) {
            if matches!(tx.status, TransactionStatus::Success | TransactionStatus::Failed(_)) {
                return;
            }
            tx.progress = progress;
            tx.status = TransactionStatus::Running;
        }
    }

    pub async fn complete(&self, id: Uuid, success: bool, message: String) {
        let mut map = self.transactions.write().await;
        let mut tokens = self.cancellation_tokens.write().await;
        tokens.remove(&id);
        let Some(mut tx) = map.remove(&id) else {
            return;
        };
        tx.progress = 100;
        tx.status = if success {
            TransactionStatus::Success
        } else {
            TransactionStatus::Failed(message)
        };

        let mut hist = self.history.write().await;
        hist.push_back(tx);
        while hist.len() > HISTORY_LIMIT {
            hist.pop_front();
        }
        let snapshot: Vec<Transaction> = hist.iter().cloned().collect();
        drop(hist);
        tokio::task::spawn_blocking(move || save_history(&snapshot));
    }

    pub async fn get(&self, id: Uuid) -> Option<Transaction> {
        if let Some(tx) = self.transactions.read().await.get(&id).cloned() {
            return Some(tx);
        }
        self.history.read().await.iter().find(|t| t.id == id).cloned()
    }

    pub async fn list(&self) -> Vec<Transaction> {
        let map = self.transactions.read().await;
        let hist = self.history.read().await;
        let mut out: Vec<Transaction> = map.values().cloned().collect();
        out.extend(hist.iter().cloned());
        out
    }

    pub async fn cancel(&self, id: Uuid) -> bool {
        let mut map = self.transactions.write().await;
        let mut tokens = self.cancellation_tokens.write().await;
        if let Some(tx) = map.get_mut(&id) {
            // Only allow cancelling pending or running transactions
            if tx.status == TransactionStatus::Pending || tx.status == TransactionStatus::Running {
                tx.status = TransactionStatus::Failed("Cancelled".to_string());
                if let Some(token) = tokens.remove(&id) {
                    token.cancel();
                    return true;
                }
            }
        }
        false
    }

    pub async fn clear_history(&self) {
        self.history.write().await.clear();
        tokio::task::spawn_blocking(|| save_history(&[]));
    }

    pub async fn cancel_all(&self) {
        let mut map = self.transactions.write().await;
        let mut tokens = self.cancellation_tokens.write().await;
        for (id, tx) in map.iter_mut() {
            if tx.status == TransactionStatus::Pending || tx.status == TransactionStatus::Running {
                tx.status = TransactionStatus::Failed("Cancelled".to_string());
                if let Some(token) = tokens.remove(id) {
                    token.cancel();
                }
            }
        }
    }
}

impl Default for TransactionManager {
    fn default() -> Self {
        Self::new()
    }
}

fn history_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".cache/arc-daemon/transaction_history.json"))
}

fn load_history() -> VecDeque<Transaction> {
    let Some(path) = history_path() else {
        return VecDeque::new();
    };
    let Ok(bytes) = std::fs::read(&path) else {
        return VecDeque::new();
    };
    serde_json::from_slice(&bytes).unwrap_or_default()
}

fn save_history(items: &[Transaction]) {
    let Some(path) = history_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let Ok(bytes) = serde_json::to_vec(items) else {
        return;
    };
    let tmp = path.with_extension("json.tmp");
    if std::fs::write(&tmp, &bytes).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}
