use libarc::{Provider, Transaction, TransactionStatus, TransactionType};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub struct TransactionManager {
    // rwlock lets many readers run at the same time but only one writer,
    // useful here because reads (get, list) are way more common than writes
    transactions: Arc<RwLock<HashMap<Uuid, Transaction>>>,
    // cancellation tokens for running transactions
    cancellation_tokens: Arc<RwLock<HashMap<Uuid, CancellationToken>>>,
}

impl TransactionManager {
    pub fn new() -> Self {
        Self {
            transactions: Arc::new(RwLock::new(HashMap::new())),
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
            tx.progress = progress;
            tx.status = TransactionStatus::Running;
        }
    }

    pub async fn complete(&self, id: Uuid, success: bool, message: String) {
        let mut map = self.transactions.write().await;
        let mut tokens = self.cancellation_tokens.write().await;
        if let Some(tx) = map.get_mut(&id) {
            tx.progress = 100;
            tx.status = if success {
                TransactionStatus::Success
            } else {
                TransactionStatus::Failed(message)
            };
        }
        // Remove the cancellation token as the transaction is complete
        tokens.remove(&id);
    }

    pub async fn get(&self, id: Uuid) -> Option<Transaction> {
        let map = self.transactions.read().await;
        map.get(&id).cloned()
    }

    #[allow(dead_code)]
    pub async fn list(&self) -> Vec<Transaction> {
        let map = self.transactions.read().await;
        map.values().cloned().collect()
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
