pub mod errors;
pub mod events;
pub mod launcher;
pub mod search;
pub mod settings;
pub mod types;

pub use errors::ArcError;
pub use events::ArcEvent;
pub use search::{is_subsequence, score_field, score_package, search_and_rank};
pub use settings::Settings;
pub use types::{AppMetadata, Package, Provider, RemoteInfo, Transaction, TransactionStatus, TransactionType};

use anyhow::Result;
use zbus::{proxy, Connection};

pub const BUS_NAME: &str = "org.blossomos.arc.daemon";
pub const OBJECT_PATH: &str = "/org/blossomos/arc/daemon";

#[proxy(
    interface = "org.blossomos.arc.daemon",
    default_service = "org.blossomos.arc.daemon",
    default_path = "/org/blossomos/arc/daemon"
)]
pub trait ArcDaemon {
    async fn install_package(&self, package_id: &str) -> zbus::Result<String>;
    async fn install_flatpakref(&self, url: &str) -> zbus::Result<String>;
    async fn remove_package(&self, package_id: &str) -> zbus::Result<String>;
    async fn remove_package_with_data(&self, package_id: &str, delete_data: bool) -> zbus::Result<String>;
    async fn search(&self, query: &str) -> zbus::Result<String>;
    async fn search_category(&self, category: &str) -> zbus::Result<String>;
    async fn get_app_info(&self, package_id: &str) -> zbus::Result<String>;
    async fn get_app_metadata(&self, package_id: &str) -> zbus::Result<String>;
    async fn list_installed(&self) -> zbus::Result<String>;
    async fn list_updates(&self) -> zbus::Result<String>;
    async fn update_package(&self, package_id: &str) -> zbus::Result<String>;
    async fn get_transaction(&self, transaction_id: &str) -> zbus::Result<String>;
    async fn list_transactions(&self) -> zbus::Result<String>;
    async fn clear_transaction_history(&self) -> zbus::Result<()>;
    async fn refresh_cache(&self) -> zbus::Result<bool>;
    async fn set_foreground_package(&self, package_id: &str) -> zbus::Result<()>;
    async fn run_package(&self, package_id: &str) -> zbus::Result<String>;
    async fn cancel_transaction(&self, transaction_id: &str) -> zbus::Result<bool>;
    async fn get_home_apps(&self, popular_count: u32, recent_count: u32) -> zbus::Result<String>;
    async fn list_extensions(&self, app_id: &str) -> zbus::Result<String>;
    async fn list_remotes(&self) -> zbus::Result<String>;
    async fn add_remote(&self, name: &str, url: &str) -> zbus::Result<bool>;
    async fn remove_remote(&self, name: &str) -> zbus::Result<bool>;
    async fn add_flatpakrepo(&self, content: &str) -> zbus::Result<bool>;
    async fn install_flatpak_bundle(&self, path: &str) -> zbus::Result<String>;
    async fn set_concurrent_downloads(&self, count: u32) -> zbus::Result<()>;

    #[zbus(signal)]
    fn transaction_started(&self, transaction_id: String, package_id: String) -> zbus::Result<()>;

    #[zbus(signal)]
    fn transaction_progress(&self, transaction_id: String, progress: u8) -> zbus::Result<()>;

    #[zbus(signal)]
    fn transaction_stats(
        &self,
        transaction_id: String,
        bytes_done: u64,
        bytes_total: u64,
    ) -> zbus::Result<()>;

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

pub async fn connect() -> Result<ArcDaemonProxy<'static>> {
    let conn = Connection::session().await?;
    let proxy = ArcDaemonProxy::new(&conn).await?;
    Ok(proxy)
}

impl ArcDaemonProxy<'_> {
    pub async fn search_packages(&self, query: &str) -> Result<Vec<Package>> {
        Ok(serde_json::from_str(&self.search(query).await?)?)
    }

    pub async fn search_category_packages(&self, category: &str) -> Result<Vec<Package>> {
        Ok(serde_json::from_str(&self.search_category(category).await?)?)
    }

    pub async fn installed_packages(&self) -> Result<Vec<Package>> {
        Ok(serde_json::from_str(&self.list_installed().await?)?)
    }

    pub async fn updates_packages(&self) -> Result<Vec<Package>> {
        Ok(serde_json::from_str(&self.list_updates().await?)?)
    }

    pub async fn transactions(&self) -> Result<Vec<Transaction>> {
        Ok(serde_json::from_str(&self.list_transactions().await?)?)
    }

    pub async fn remotes(&self) -> Result<Vec<RemoteInfo>> {
        Ok(serde_json::from_str(&self.list_remotes().await?)?)
    }

    pub async fn extensions(&self, app_id: &str) -> Result<Vec<Package>> {
        Ok(serde_json::from_str(&self.list_extensions(app_id).await?)?)
    }

    pub async fn app_info(&self, package_id: &str) -> Result<Option<Package>> {
        Ok(serde_json::from_str(&self.get_app_info(package_id).await?)?)
    }

    pub async fn app_metadata(&self, package_id: &str) -> Result<AppMetadata> {
        Ok(serde_json::from_str(&self.get_app_metadata(package_id).await?)?)
    }
}

pub fn clear_foreground_blocking() {
    if let Ok(conn) = zbus::blocking::Connection::session() {
        let _ = conn.call_method(
            Some(BUS_NAME),
            OBJECT_PATH,
            Some(BUS_NAME),
            "SetForegroundPackage",
            &("",),
        );
    }
}
