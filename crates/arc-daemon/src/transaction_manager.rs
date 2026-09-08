use libarc::cache::JsonCache;
use libarc::{Provider, Transaction, TransactionStatus, TransactionType};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, OnceLock};
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
        report_launcher_progress(&map);
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

        report_launcher_progress(&map);
        drop(map);

        let mut hist = self.history.write().await;
        hist.push_back(tx);
        while hist.len() > HISTORY_LIMIT {
            hist.pop_front();
        }
        let snapshot: Vec<Transaction> = hist.iter().cloned().collect();
        drop(hist);
        save_history(&snapshot);
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
        save_history(&[]);
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

// averages progress across every still-running transaction and pushes it to
// the taskbar/dock icon via the Unity LauncherEntry protocol; called with
// the transactions map already locked so it always sees a consistent view
fn report_launcher_progress(map: &HashMap<Uuid, Transaction>) {
    let active: Vec<u8> = map
        .values()
        .filter(|tx| tx.status == TransactionStatus::Pending || tx.status == TransactionStatus::Running)
        .map(|tx| tx.progress)
        .collect();
    if active.is_empty() {
        crate::launcher_progress::update(false, 0.0);
        return;
    }
    let average = active.iter().map(|&p| p as f64).sum::<f64>() / active.len() as f64 / 100.0;
    crate::launcher_progress::update(true, average);
}

fn history_store() -> &'static JsonCache<Vec<Transaction>> {
    static STORE: OnceLock<JsonCache<Vec<Transaction>>> = OnceLock::new();
    STORE.get_or_init(|| JsonCache::new("daemon", "transactions.json"))
}

fn load_history() -> VecDeque<Transaction> {
    history_store().load().map(VecDeque::from).unwrap_or_default()
}

fn save_history(items: &[Transaction]) {
    history_store().store(&items.to_vec());
}
