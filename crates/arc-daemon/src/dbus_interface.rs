use crate::providers::MultiProvider;
use crate::providers::PackageProvider;
use crate::transaction_manager::TransactionManager;
use libarc::{Provider, TransactionType};
use std::sync::Arc;

// flatpak ids look like "org.gimp.GIMP" (reverse dns, dots, no slashes or semicolons).
// distrobox ids look like "distrobox:container:name:type" or are file paths for installs.
// lutris ids look like "lutris:<slug>".
fn provider_from_id(package_id: &str) -> Provider {
    if package_id.starts_with("lutris:") {
        return Provider::Lutris;
    }
    let looks_like_flatpak = !package_id.contains('/')
        && !package_id.contains(';')
        && !package_id.starts_with("distrobox:")
        && package_id.matches('.').count() >= 2;
    if looks_like_flatpak {
        Provider::Flatpak
    } else {
        Provider::Distrobox
    }
}

use tracing::{error, info};
use zbus::interface;
use zbus::object_server::SignalEmitter;

pub struct ArcDaemonInterface {
    pub provider: Arc<MultiProvider>,
    pub transaction_manager: Arc<TransactionManager>,
}

#[interface(name = "dev.arc.ArcDaemon1")]
impl ArcDaemonInterface {
    async fn install_package(
        &self,
        package_id: String,
        // zbus injects this automatically, it is how we push events back to
        // all listening clients without them polling us
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> String {
        info!("InstallPackage: {}", package_id);
        let tx = self
            .transaction_manager
            .create(
                TransactionType::Install,
                package_id.clone(),
                provider_from_id(&package_id),
            )
            .await;
        let tx_id = tx.id;

        let provider = self.provider.clone();
        let tm = self.transaction_manager.clone();
        // emitter is tied to this request's lifetime so we have to own it
        // before spawning otherwise the borrow checker will not allow the move
        let emitter = emitter.to_owned();

        // spawn so we return the tx id to the caller right away and do the
        // actual install in the background, progress comes via signals
        tokio::spawn(async move {
            let _ =
                Self::transaction_started(&emitter, tx_id.to_string(), package_id.clone()).await;

            let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel::<u8>();

            // Forward GLib progress signals to DBus as they arrive
            let emitter_fwd = emitter.clone();
            let tm_fwd = tm.clone();
            tokio::spawn(async move {
                while let Some(pct) = progress_rx.recv().await {
                    tm_fwd.update_progress(tx_id, pct).await;
                    let _ =
                        Self::transaction_progress(&emitter_fwd, tx_id.to_string(), pct).await;
                }
            });

            match provider.install_with_progress(&package_id, progress_tx).await {
                Ok(()) => {
                    tm.complete(tx_id, true, "Installation successful".to_string())
                        .await;
                    let _ = Self::transaction_progress(&emitter, tx_id.to_string(), 100).await;
                    let _ = Self::transaction_finished(
                        &emitter,
                        tx_id.to_string(),
                        true,
                        "Installation successful".to_string(),
                    )
                    .await;
                }
                Err(e) => {
                    error!("Install failed: {}", e);
                    tm.complete(tx_id, false, e.to_string()).await;
                    let _ = Self::transaction_finished(
                        &emitter,
                        tx_id.to_string(),
                        false,
                        e.to_string(),
                    )
                    .await;
                }
            }
        });

        tx_id.to_string()
    }

    async fn remove_package(
        &self,
        package_id: String,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> String {
        info!("RemovePackage: {}", package_id);
        let tx = self
            .transaction_manager
            .create(
                TransactionType::Remove,
                package_id.clone(),
                provider_from_id(&package_id),
            )
            .await;
        let tx_id = tx.id;

        let provider = self.provider.clone();
        let tm = self.transaction_manager.clone();
        let emitter = emitter.to_owned();

        tokio::spawn(async move {
            let _ =
                Self::transaction_started(&emitter, tx_id.to_string(), package_id.clone()).await;

            tm.update_progress(tx_id, 10).await;
            let _ = Self::transaction_progress(&emitter, tx_id.to_string(), 10).await;

            match provider.remove(&package_id).await {
                Ok(()) => {
                    tm.complete(tx_id, true, "Removal successful".to_string())
                        .await;
                    let _ = Self::transaction_progress(&emitter, tx_id.to_string(), 100).await;
                    let _ = Self::transaction_finished(
                        &emitter,
                        tx_id.to_string(),
                        true,
                        "Removal successful".to_string(),
                    )
                    .await;
                }
                Err(e) => {
                    error!("Remove failed: {}", e);
                    tm.complete(tx_id, false, e.to_string()).await;
                    let _ = Self::transaction_finished(
                        &emitter,
                        tx_id.to_string(),
                        false,
                        e.to_string(),
                    )
                    .await;
                }
            }
        });

        tx_id.to_string()
    }

    async fn update_package(
        &self,
        package_id: String,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> String {
        info!("UpdatePackage: {}", package_id);
        let tx = self
            .transaction_manager
            .create(
                TransactionType::Update,
                package_id.clone(),
                provider_from_id(&package_id),
            )
            .await;
        let tx_id = tx.id;

        let provider = self.provider.clone();
        let tm = self.transaction_manager.clone();
        let emitter = emitter.to_owned();

        tokio::spawn(async move {
            let _ =
                Self::transaction_started(&emitter, tx_id.to_string(), package_id.clone()).await;

            let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel::<u8>();

            let emitter_fwd = emitter.clone();
            let tm_fwd = tm.clone();
            tokio::spawn(async move {
                while let Some(pct) = progress_rx.recv().await {
                    tm_fwd.update_progress(tx_id, pct).await;
                    let _ =
                        Self::transaction_progress(&emitter_fwd, tx_id.to_string(), pct).await;
                }
            });

            match provider.update_with_progress(&package_id, progress_tx).await {
                Ok(()) => {
                    tm.complete(tx_id, true, "Update successful".to_string())
                        .await;
                    let _ = Self::transaction_progress(&emitter, tx_id.to_string(), 100).await;
                    let _ = Self::transaction_finished(
                        &emitter,
                        tx_id.to_string(),
                        true,
                        "Update successful".to_string(),
                    )
                    .await;
                }
                Err(e) => {
                    error!("Update failed: {}", e);
                    tm.complete(tx_id, false, e.to_string()).await;
                    let _ = Self::transaction_finished(
                        &emitter,
                        tx_id.to_string(),
                        false,
                        e.to_string(),
                    )
                    .await;
                }
            }
        });

        tx_id.to_string()
    }

    async fn refresh_cache(&self) -> bool {
        info!("RefreshCache");
        match self.provider.refresh_cache().await {
            Ok(()) => true,
            Err(e) => {
                error!("RefreshCache failed: {}", e);
                false
            }
        }
    }

    // returns json because dbus does not have a native list type that maps
    // cleanly to our structs, easier to serialize and let the client deserialize
    async fn search(&self, query: String) -> String {
        info!("Search: {}", query);
        match self.provider.search(&query).await {
            Ok(packages) => serde_json::to_string(&packages).unwrap_or_else(|_| "[]".to_string()),
            Err(e) => {
                error!("Search failed: {}", e);
                format!("{{\"error\":\"{}\"}}", e)
            }
        }
    }

    async fn search_category(&self, category: String) -> String {
        info!("SearchCategory: {}", category);
        match self.provider.search_category(&category).await {
            Ok(packages) => serde_json::to_string(&packages).unwrap_or_else(|_| "[]".to_string()),
            Err(e) => {
                error!("SearchCategory failed: {}", e);
                "[]".to_string()
            }
        }
    }

    async fn get_app_info(&self, package_id: String) -> String {
        info!("GetAppInfo: {}", package_id);
        // Go directly to provider for all app types
        match self.provider.get_app_info(&package_id).await {
            Ok(Some(package)) => {
                serde_json::to_string(&Some(package)).unwrap_or_else(|_| "null".to_string())
            }
            Ok(None) => "null".to_string(),
            Err(e) => {
                error!("GetAppInfo failed: {}", e);
                "null".to_string()
            }
        }
    }

    async fn list_installed(&self) -> String {
        info!("ListInstalled");
        match self.provider.list_installed().await {
            Ok(packages) => serde_json::to_string(&packages).unwrap_or_else(|_| "[]".to_string()),
            Err(e) => {
                error!("ListInstalled failed: {}", e);
                format!("{{\"error\":\"{}\"}}", e)
            }
        }
    }

    async fn list_updates(&self, #[zbus(signal_emitter)] emitter: SignalEmitter<'_>) -> String {
        info!("ListUpdates");
        match self.provider.list_updates().await {
            Ok(packages) => {
                let count = packages.len() as u32;
                // fire the signal so any notification daemon listening can
                // show a badge or popup without polling list_updates itself
                if count > 0 {
                    let _ = Self::updates_available(&emitter, count).await;
                }
                serde_json::to_string(&packages).unwrap_or_else(|_| "[]".to_string())
            }
            Err(e) => {
                error!("ListUpdates failed: {}", e);
                "[]".to_string()
            }
        }
    }

    async fn get_transaction(&self, transaction_id: String) -> String {
        info!("GetTransaction: {}", transaction_id);
        match transaction_id.parse::<uuid::Uuid>() {
            Ok(id) => match self.transaction_manager.get(id).await {
                Some(tx) => serde_json::to_string(&tx).unwrap_or_else(|_| "null".to_string()),
                None => "null".to_string(),
            },
            Err(_) => "null".to_string(),
        }
    }

    async fn run_package(&self, package_id: String) -> String {
        info!("RunPackage: {}", package_id);
        match self.provider.run(&package_id).await {
            Ok(()) => serde_json::json!({ "success": true }).to_string(),
            Err(e) => {
                error!("RunPackage failed: {}", e);
                serde_json::json!({ "success": false, "error": e.to_string() }).to_string()
            }
        }
    }

    // the next four are signal declarations, zbus generates the actual emit
    #[zbus(signal)]
    async fn transaction_started(
        signal_emitter: &SignalEmitter<'_>,
        transaction_id: String,
        package_id: String,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn transaction_progress(
        signal_emitter: &SignalEmitter<'_>,
        transaction_id: String,
        progress: u8,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn transaction_finished(
        signal_emitter: &SignalEmitter<'_>,
        transaction_id: String,
        success: bool,
        message: String,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn updates_available(signal_emitter: &SignalEmitter<'_>, count: u32) -> zbus::Result<()>;
}
