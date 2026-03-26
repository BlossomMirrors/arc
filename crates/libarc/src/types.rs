use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Provider {
    Flatpak,
    Native,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub provider: Provider,
    pub installed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransactionType {
    Install,
    Remove,
    Update,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransactionStatus {
    Pending,
    Running,
    Success,
    Failed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: Uuid,
    pub transaction_type: TransactionType,
    pub status: TransactionStatus,
    pub progress: u8,
    pub package_id: String,
    pub provider: Provider,
}

impl Transaction {
    pub fn new(transaction_type: TransactionType, package_id: String, provider: Provider) -> Self {
        Self {
            id: Uuid::new_v4(),
            transaction_type,
            status: TransactionStatus::Pending,
            progress: 0,
            package_id,
            provider,
        }
    }
}
