use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteInfo {
    pub name: String,
    pub url: String,
    pub protected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Provider {
    Flatpak,
    Distrobox,
    Lutris,
    AppImage,
    Pwa,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub provider: Provider,
    pub installed: bool,
    pub icon_url: Option<String>,
    pub remote: Option<String>,
    #[serde(default)]
    pub screenshots: Vec<String>,
    #[serde(default)]
    pub developer_name: Option<String>,
    #[serde(default)]
    pub homepage_url: Option<String>,
    #[serde(default)]
    pub content_rating: Option<String>,
    #[serde(default)]
    pub is_runtime: bool,
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
    // carries the error message so the client knows what went wrong
    Failed(String),
}

// we create this before the work starts so the frontend can track progress
// and look up status by id at any point
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
