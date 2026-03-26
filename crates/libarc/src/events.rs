use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ArcEvent {
    TransactionStarted { transaction_id: Uuid, package_id: String },
    TransactionProgress { transaction_id: Uuid, progress: u8 },
    TransactionFinished { transaction_id: Uuid, success: bool, message: String },
    UpdatesAvailable { count: u32 },
    PackageListRefreshed,
}
