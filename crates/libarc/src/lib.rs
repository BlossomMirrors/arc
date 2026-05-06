pub mod errors;
pub mod events;
pub mod settings;
pub mod types;

pub use errors::ArcError;
pub use events::ArcEvent;
pub use settings::Settings;
pub use types::{Package, Provider, Transaction, TransactionStatus, TransactionType};

use anyhow::Result;
use zbus::{proxy, Connection};

// this macro generates a strongly typed rust client for our daemon's dbus api,
// method calls become normal async functions and signals become streams
#[proxy(
    interface = "dev.arc.ArcDaemon1",
    default_service = "dev.arc.ArcDaemon1",
    default_path = "/dev/arc/ArcDaemon1"
)]
pub trait ArcDaemon {
    async fn install_package(&self, package_id: &str) -> zbus::Result<String>;
    async fn install_flatpakref(&self, url: &str) -> zbus::Result<String>;
    async fn remove_package(&self, package_id: &str) -> zbus::Result<String>;
    async fn search(&self, query: &str) -> zbus::Result<String>;
    async fn search_category(&self, category: &str) -> zbus::Result<String>;
    async fn get_app_info(&self, package_id: &str) -> zbus::Result<String>;
    async fn list_installed(&self) -> zbus::Result<String>;
    async fn list_updates(&self) -> zbus::Result<String>;
    async fn update_package(&self, package_id: &str) -> zbus::Result<String>;
    async fn get_transaction(&self, transaction_id: &str) -> zbus::Result<String>;
    async fn refresh_cache(&self) -> zbus::Result<bool>;
    async fn run_package(&self, package_id: &str) -> zbus::Result<String>;

    // signals are one way messages the daemon sends to all connected clients
    // without them having to ask, fire and forget from the daemon side
    #[zbus(signal)]
    fn transaction_started(&self, transaction_id: String, package_id: String) -> zbus::Result<()>;

    #[zbus(signal)]
    fn transaction_progress(&self, transaction_id: String, progress: u8) -> zbus::Result<()>;

    #[zbus(signal)]
    fn transaction_finished(
        &self,
        transaction_id: String,
        success: bool,
        message: String,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    fn updates_available(&self, count: u32) -> zbus::Result<()>;
}

// connects to the session bus (per user, not system wide) and returns a proxy
// you can call methods on like normal async functions
pub async fn connect() -> Result<ArcDaemonProxy<'static>> {
    let conn = Connection::session().await?;
    let proxy = ArcDaemonProxy::new(&conn).await?;
    Ok(proxy)
}
