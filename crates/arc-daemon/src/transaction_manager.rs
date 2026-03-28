use libarc::{Provider, Transaction, TransactionStatus, TransactionType};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

pub struct TransactionManager {
    // rwlock lets many readers run at the same time but only one writer,
    // useful here because reads (get, list) are way more common than writes
    transactions: Arc<RwLock<HashMap<Uuid, Transaction>>>,
}

impl TransactionManager {
    pub fn new() -> Self {
        Self {
            transactions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn create(
        &self,
        t_type: TransactionType,
        pkg_id: String,
        provider: Provider,
    ) -> Transaction {
        let tx = Transaction::new(t_type, pkg_id, provider);
        let mut map = self.transactions.write().await;
        map.insert(tx.id, tx.clone());
        tx
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
        if let Some(tx) = map.get_mut(&id) {
            tx.progress = 100;
            tx.status = if success {
                TransactionStatus::Success
            } else {
                TransactionStatus::Failed(message)
            };
        }
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
}

impl Default for TransactionManager {
    fn default() -> Self {
        Self::new()
    }
}
