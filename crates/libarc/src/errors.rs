use thiserror::Error;

#[derive(Debug, Error)]
pub enum ArcError {
    #[error("Package not found: {0}")]
    PackageNotFound(String),
    #[error("Transaction failed: {0}")]
    TransactionFailed(String),
    #[error("Provider error: {0}")]
    ProviderError(String),
    #[error("D-Bus error: {0}")]
    DbusError(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}
