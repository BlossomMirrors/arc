use crate::providers::flatpak::FlatpakProvider;
use crate::providers::MultiProvider;
use crate::providers::PackageProvider;
use crate::transaction_manager::TransactionManager;
use libarc::{Provider, TransactionType};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::Semaphore;

// flatpak ids look like "org.gimp.GIMP" (reverse dns, dots, no slashes or semicolons).
// distrobox ids look like "distrobox:container:name:type" or are file paths for installs.
// lutris ids look like "lutris:<slug>".
fn provider_from_id(package_id: &str) -> Provider {
    if package_id.starts_with("pwa:") {
        return Provider::Pwa;
    }
    if package_id.starts_with("lutris:") {
        return Provider::Lutris;
    }
    if package_id.starts_with("appimage:") || package_id.to_lowercase().ends_with(".appimage") {
        return Provider::AppImage;
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

use tr::tr;
use tracing::{error, info, warn};
use zbus::interface;
use zbus::object_server::SignalEmitter;

// AppStreamDb's cold parse can take well over a minute; fail fast instead of
// making a UI click hang for that whole window — the caller can just retry
// once the background warm-up (kicked off at daemon startup) finishes.
const APP_INFO_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

pub struct ArcDaemonInterface {
    pub provider: Arc<MultiProvider>,
    pub transaction_manager: Arc<TransactionManager>,
    pub download_semaphore: Arc<Semaphore>,
    pub download_permits: Arc<AtomicUsize>,
    // package whose detail page the frontend currently shows so its
    // transactions skip the job notification
    pub foreground_package: Arc<tokio::sync::RwLock<String>>,
}

impl ArcDaemonInterface {
    async fn kio_hidden(&self, package_id: &str) -> bool {
        *self.foreground_package.read().await == package_id
    }
}

#[interface(name = "org.blossomos.arc.daemon")]
impl ArcDaemonInterface {
    async fn install_package(
        &self,
        package_id: String,
        // zbus injects this automatically, it is how we push events back to
        // all listening clients without them polling us
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> String {
        info!("InstallPackage: {}", package_id);
        let (tx, cancel_token) = self
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
        let semaphore = self.download_semaphore.clone();
        // emitter is tied to this request's lifetime so we have to own it
        // before spawning otherwise the borrow checker will not allow the move
        let emitter = emitter.to_owned();

        // spawn so we return the tx id to the caller right away and do the
        // actual install in the background, progress comes via signals
        let kio_hidden = self.kio_hidden(&package_id).await;
        tokio::spawn(async move {
            let _ =
                Self::transaction_started(&emitter, tx_id.to_string(), package_id.clone()).await;
            let kio = crate::kio::KioJob::start(
                &tr!("Installing application"),
                &package_id,
                Some(cancel_token.clone()),
                kio_hidden,
            );

            // Wait for a download slot; allow cancellation while queued
            let _permit = tokio::select! {
                result = semaphore.acquire_owned() => {
                    match result { Ok(p) => p, Err(_) => return }
                }
                _ = cancel_token.cancelled() => {
                    tm.complete(tx_id, false, "Cancelled".to_string()).await;
                    kio.finish(false, &tr!("Cancelled"));
                    let _ = Self::transaction_finished(&emitter, tx_id.to_string(), false, "Cancelled".to_string()).await;
                    return;
                }
            };

            let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel::<crate::providers::Progress>();

            // Forward GLib progress signals to DBus as they arrive
            let emitter_fwd = emitter.clone();
            let tm_fwd = tm.clone();
            let cancel_token_fwd = cancel_token.clone();
            let kio_fwd = kio.clone();
            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        _ = cancel_token_fwd.cancelled() => {
                            // Cancelled, stop forwarding progress
                            break;
                        }
                        result = progress_rx.recv() => {
                            match result {
                                Some(p) => {
                                    tm_fwd.update_progress(tx_id, p.percent).await;
                                    kio_fwd.progress(p.percent);
                                    let _ =
                                        Self::transaction_progress(&emitter_fwd, tx_id.to_string(), p.percent).await;
                                    let _ =
                                        Self::transaction_stats(&emitter_fwd, tx_id.to_string(), p.bytes_done, p.bytes_total).await;
                                }
                                None => break, // Channel closed
                            }
                        }
                    }
                }
            });

            tokio::select! {
                _ = cancel_token.cancelled() => {
                    info!("Transaction {} cancelled", tx_id);
                    tm.complete(tx_id, false, "Cancelled".to_string()).await;
                    kio.finish(false, &tr!("Cancelled"));
                    let _ = Self::transaction_finished(
                        &emitter,
                        tx_id.to_string(),
                        false,
                        "Cancelled".to_string(),
                    )
                    .await;
                }
                result = provider.install_with_progress(&package_id, progress_tx, cancel_token.clone()) => {
                    match result {
                        Ok(()) => {
                            provider.invalidate_package_cache().await;
                            tm.complete(tx_id, true, "Installation successful".to_string())
                                .await;
                            kio.finish(true, "");
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
                            kio.finish(false, &e.to_string());
                            let _ = Self::transaction_finished(
                                &emitter,
                                tx_id.to_string(),
                                false,
                                e.to_string(),
                            )
                            .await;
                        }
                    }
                }
            }
        });

        tx_id.to_string()
    }

    async fn install_flatpakref(
        &self,
        url: String,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> String {
        info!("InstallFlatpakref: {}", url);
        let (tx, cancel_token) = self
            .transaction_manager
            .create(
                TransactionType::Install,
                url.clone(),
                libarc::Provider::Flatpak,
            )
            .await;
        let tx_id = tx.id;

        let provider = self.provider.clone();
        let tm = self.transaction_manager.clone();
        let semaphore = self.download_semaphore.clone();
        let emitter = emitter.to_owned();

        let kio_hidden = self.kio_hidden(&url).await;
        tokio::spawn(async move {
            let _ = Self::transaction_started(&emitter, tx_id.to_string(), url.clone()).await;
            let kio = crate::kio::KioJob::start(
                &tr!("Installing application"),
                &url,
                Some(cancel_token.clone()),
                kio_hidden,
            );

            let _permit = tokio::select! {
                result = semaphore.acquire_owned() => {
                    match result { Ok(p) => p, Err(_) => return }
                }
                _ = cancel_token.cancelled() => {
                    tm.complete(tx_id, false, "Cancelled".to_string()).await;
                    kio.finish(false, &tr!("Cancelled"));
                    let _ = Self::transaction_finished(&emitter, tx_id.to_string(), false, "Cancelled".to_string()).await;
                    return;
                }
            };

            let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel::<crate::providers::Progress>();

            let emitter_fwd = emitter.clone();
            let tm_fwd = tm.clone();
            let cancel_token_fwd = cancel_token.clone();
            let kio_fwd = kio.clone();
            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        _ = cancel_token_fwd.cancelled() => {
                            // Cancelled, stop forwarding progress
                            break;
                        }
                        result = progress_rx.recv() => {
                            match result {
                                Some(p) => {
                                    tm_fwd.update_progress(tx_id, p.percent).await;
                                    kio_fwd.progress(p.percent);
                                    let _ =
                                        Self::transaction_progress(&emitter_fwd, tx_id.to_string(), p.percent).await;
                                    let _ =
                                        Self::transaction_stats(&emitter_fwd, tx_id.to_string(), p.bytes_done, p.bytes_total).await;
                                }
                                None => break, // Channel closed
                            }
                        }
                    }
                }
            });

            tokio::select! {
                _ = cancel_token.cancelled() => {
                    info!("Transaction {} cancelled", tx_id);
                    tm.complete(tx_id, false, "Cancelled".to_string()).await;
                    kio.finish(false, &tr!("Cancelled"));
                    let _ = Self::transaction_finished(
                        &emitter,
                        tx_id.to_string(),
                        false,
                        "Cancelled".to_string(),
                    )
                    .await;
                }
                result = provider.install_flatpakref_with_progress(&url, progress_tx, cancel_token.clone()) => {
                    match result {
                        Ok(()) => {
                            provider.invalidate_package_cache().await;
                            tm.complete(tx_id, true, "Installation successful".to_string())
                                .await;
                            kio.finish(true, "");
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
                            error!("InstallFlatpakref failed: {}", e);
                            tm.complete(tx_id, false, e.to_string()).await;
                            kio.finish(false, &e.to_string());
                            let _ = Self::transaction_finished(
                                &emitter,
                                tx_id.to_string(),
                                false,
                                e.to_string(),
                            )
                            .await;
                        }
                    }
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
        let (tx, _cancel_token) = self
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

        let kio_hidden = self.kio_hidden(&package_id).await;
        tokio::spawn(async move {
            let _ =
                Self::transaction_started(&emitter, tx_id.to_string(), package_id.clone()).await;
            let kio =
                crate::kio::KioJob::start(&tr!("Removing application"), &package_id, None, kio_hidden);

            tm.update_progress(tx_id, 10).await;
            kio.progress(10);
            let _ = Self::transaction_progress(&emitter, tx_id.to_string(), 10).await;

            match provider.remove(&package_id).await {
                Ok(()) => {
                    provider.invalidate_package_cache().await;
                    tm.complete(tx_id, true, "Removal successful".to_string())
                        .await;
                    kio.finish(true, "");
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
                    kio.finish(false, &e.to_string());
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

    async fn remove_package_with_data(
        &self,
        package_id: String,
        delete_data: bool,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> String {
        info!("RemovePackageWithData: {} delete_data={}", package_id, delete_data);
        let (tx, _cancel_token) = self
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

        let kio_hidden = self.kio_hidden(&package_id).await;
        tokio::spawn(async move {
            let _ =
                Self::transaction_started(&emitter, tx_id.to_string(), package_id.clone()).await;
            let kio =
                crate::kio::KioJob::start(&tr!("Removing application"), &package_id, None, kio_hidden);
            tm.update_progress(tx_id, 10).await;
            kio.progress(10);
            let _ = Self::transaction_progress(&emitter, tx_id.to_string(), 10).await;

            match provider.remove(&package_id).await {
                Ok(()) => {
                    if delete_data {
                        if let Some(home) = std::env::var_os("HOME") {
                            let home = std::path::PathBuf::from(home);
                            let flatpak_dir = home.join(".var/app").join(&package_id);
                            if flatpak_dir.exists() {
                                let _ = std::fs::remove_dir_all(&flatpak_dir);
                            }
                            if let Some(appid) = package_id.strip_prefix("pwa:") {
                                let pwa_dir = home
                                    .join(".local/share/blossomos-webapps")
                                    .join(appid);
                                if pwa_dir.exists() {
                                    let _ = std::fs::remove_dir_all(&pwa_dir);
                                }
                            }
                        }
                    }
                    provider.invalidate_package_cache().await;
                    tm.complete(tx_id, true, "Removal successful".to_string()).await;
                    kio.finish(true, "");
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
                    kio.finish(false, &e.to_string());
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
        let (tx, cancel_token) = self
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
        let semaphore = self.download_semaphore.clone();
        let emitter = emitter.to_owned();

        let kio_hidden = self.kio_hidden(&package_id).await;
        tokio::spawn(async move {
            let _ =
                Self::transaction_started(&emitter, tx_id.to_string(), package_id.clone()).await;
            let kio = crate::kio::KioJob::start(
                &tr!("Updating application"),
                &package_id,
                Some(cancel_token.clone()),
                kio_hidden,
            );

            let _permit = tokio::select! {
                result = semaphore.acquire_owned() => {
                    match result { Ok(p) => p, Err(_) => return }
                }
                _ = cancel_token.cancelled() => {
                    tm.complete(tx_id, false, "Cancelled".to_string()).await;
                    kio.finish(false, &tr!("Cancelled"));
                    let _ = Self::transaction_finished(&emitter, tx_id.to_string(), false, "Cancelled".to_string()).await;
                    return;
                }
            };

            let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel::<crate::providers::Progress>();

            let emitter_fwd = emitter.clone();
            let tm_fwd = tm.clone();
            let cancel_token_fwd = cancel_token.clone();
            let kio_fwd = kio.clone();
            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        _ = cancel_token_fwd.cancelled() => {
                            // Cancelled, stop forwarding progress
                            break;
                        }
                        result = progress_rx.recv() => {
                            match result {
                                Some(p) => {
                                    tm_fwd.update_progress(tx_id, p.percent).await;
                                    kio_fwd.progress(p.percent);
                                    let _ =
                                        Self::transaction_progress(&emitter_fwd, tx_id.to_string(), p.percent).await;
                                    let _ =
                                        Self::transaction_stats(&emitter_fwd, tx_id.to_string(), p.bytes_done, p.bytes_total).await;
                                }
                                None => break, // Channel closed
                            }
                        }
                    }
                }
            });

            tokio::select! {
                _ = cancel_token.cancelled() => {
                    info!("Transaction {} cancelled", tx_id);
                    tm.complete(tx_id, false, "Cancelled".to_string()).await;
                    kio.finish(false, &tr!("Cancelled"));
                    let _ = Self::transaction_finished(
                        &emitter,
                        tx_id.to_string(),
                        false,
                        "Cancelled".to_string(),
                    )
                    .await;
                }
                result = provider.update_with_progress(&package_id, progress_tx, cancel_token.clone()) => {
                    match result {
                        Ok(()) => {
                            provider.invalidate_package_cache().await;
                            tm.complete(tx_id, true, "Update successful".to_string())
                                .await;
                            kio.finish(true, "");
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
                            kio.finish(false, &e.to_string());
                            let _ = Self::transaction_finished(
                                &emitter,
                                tx_id.to_string(),
                                false,
                                e.to_string(),
                            )
                            .await;
                        }
                    }
                }
            }
        });

        tx_id.to_string()
    }

    // the frontend reports which app its detail page currently shows so
    // transactions for it run without a job notification
    async fn set_foreground_package(&self, package_id: String) {
        *self.foreground_package.write().await = package_id;
    }

    async fn set_concurrent_downloads(&self, count: u32) {
        let target = count.max(1) as usize;
        let current = self.download_permits.swap(target, Ordering::SeqCst);
        if target > current {
            self.download_semaphore.add_permits(target - current);
        } else if target < current {
            self.download_semaphore.forget_permits(current - target);
        }
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
        match tokio::time::timeout(APP_INFO_TIMEOUT, self.provider.get_app_info(&package_id)).await
        {
            Ok(Ok(Some(package))) => {
                serde_json::to_string(&Some(package)).unwrap_or_else(|_| "null".to_string())
            }
            Ok(Ok(None)) => "null".to_string(),
            Ok(Err(e)) => {
                error!("GetAppInfo failed: {}", e);
                "null".to_string()
            }
            Err(_) => {
                warn!("GetAppInfo timed out for {package_id} (appstream still warming up)");
                "null".to_string()
            }
        }
    }

    async fn get_app_metadata(&self, package_id: String) -> String {
        info!("GetAppMetadata: {}", package_id);
        if package_id.starts_with("pwa:") {
            return self.provider.pwa.get_metadata_json(&package_id).await;
        }

        let fast_id = package_id.clone();
        if let Ok(Some(json)) = tokio::task::spawn_blocking(move || {
            crate::appstream_db::load_local_metainfo(&fast_id).and_then(|e| serde_json::to_string(&e).ok())
        })
        .await
        {
            return json;
        }

        // resolve_now checks the already-loaded db first then decompresses
        // and scans just the catalog files not yet folded in
        // stays well under APP_INFO_TIMEOUT instead of waiting on the full parse
        let resolve_id = package_id.clone();
        let fetch = tokio::task::spawn_blocking(move || {
            crate::appstream_db::resolve_now(&resolve_id)
                .or_else(|| crate::appstream_db::AppStreamDb::try_get().load_from_exported_metainfo(&resolve_id))
        });
        let entry = tokio::time::timeout(APP_INFO_TIMEOUT, fetch)
            .await
            .ok()
            .and_then(|r| r.ok())
            .flatten();
        if let Some(entry) = entry {
            if let Ok(json) = serde_json::to_string(&entry) {
                return json;
            }
        }

        // id missing from every catalog file
        // fall back to the disk-seeded package cache for a partial answer
        match self.provider.cached_package(&package_id).await {
            Some(pkg) => serde_json::to_string(&crate::appstream_db::partial_entry_from_package(&pkg))
                .unwrap_or_else(|_| "null".to_string()),
            None => "null".to_string(),
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

    async fn list_transactions(&self) -> String {
        info!("ListTransactions");
        let txs = self.transaction_manager.list().await;
        serde_json::to_string(&txs).unwrap_or_else(|_| "[]".to_string())
    }

    async fn clear_transaction_history(&self) {
        info!("ClearTransactionHistory");
        self.transaction_manager.clear_history().await;
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

    async fn cancel_transaction(
        &self,
        transaction_id: String,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> bool {
        info!("CancelTransaction: {}", transaction_id);
        match transaction_id.parse::<uuid::Uuid>() {
            Ok(id) => {
                let cancelled = self.transaction_manager.cancel(id).await;
                if cancelled {
                    let _ = Self::transaction_finished(
                        &emitter,
                        transaction_id.clone(),
                        false,
                        "Cancelled".to_string(),
                    )
                    .await;
                }
                cancelled
            }
            Err(_) => false,
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

    async fn get_home_apps(&self, popular_count: u32, recent_count: u32) -> String {
        tokio::task::spawn_blocking(move || {
            let db = crate::appstream_db::AppStreamDb::try_get();
            let popular = db.get_popular_apps(popular_count as usize);
            let recent = db.get_recent_apps(recent_count as usize);
            serde_json::json!({ "popular": popular, "recent": recent }).to_string()
        })
        .await
        .unwrap_or_else(|_| r#"{"popular":[],"recent":[]}"#.to_string())
    }

    async fn list_extensions(&self, app_id: String) -> String {
        info!("ListExtensions: {}", app_id);
        match self.provider.list_extensions(&app_id).await {
            Ok(packages) => serde_json::to_string(&packages).unwrap_or_else(|_| "[]".to_string()),
            Err(e) => {
                error!("ListExtensions failed: {}", e);
                "[]".to_string()
            }
        }
    }

    async fn list_remotes(&self) -> String {
        info!("ListRemotes");
        tokio::task::spawn_blocking(FlatpakProvider::list_remotes)
            .await
            .ok()
            .and_then(|v| serde_json::to_string(&v).ok())
            .unwrap_or_else(|| "[]".to_string())
    }

    async fn add_remote(&self, name: String, url: String) -> bool {
        info!("AddRemote: {} {}", name, url);
        tokio::task::spawn_blocking(move || FlatpakProvider::add_remote_from_url(&name, &url))
            .await
            .map(|r| r.is_ok())
            .unwrap_or(false)
    }

    async fn remove_remote(&self, name: String) -> bool {
        info!("RemoveRemote: {}", name);
        tokio::task::spawn_blocking(move || FlatpakProvider::remove_remote(&name))
            .await
            .map(|r| r.is_ok())
            .unwrap_or(false)
    }

    async fn add_flatpakrepo(&self, content: String) -> bool {
        info!("AddFlatpakrepo");
        tokio::task::spawn_blocking(move || FlatpakProvider::add_remote_from_flatpakrepo(&content))
            .await
            .map(|r| r.is_ok())
            .unwrap_or(false)
    }

    async fn install_flatpak_bundle(
        &self,
        path: String,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> String {
        info!("InstallFlatpakBundle: {}", path);
        let (tx, cancel_token) = self
            .transaction_manager
            .create(TransactionType::Install, path.clone(), Provider::Flatpak)
            .await;
        let tx_id = tx.id;

        let provider = self.provider.clone();
        let tm = self.transaction_manager.clone();
        let semaphore = self.download_semaphore.clone();
        let emitter = emitter.to_owned();

        let kio_hidden = self.kio_hidden(&path).await;
        tokio::spawn(async move {
            let _ = Self::transaction_started(&emitter, tx_id.to_string(), path.clone()).await;
            let kio = crate::kio::KioJob::start(
                &tr!("Installing application"),
                &path,
                Some(cancel_token.clone()),
                kio_hidden,
            );

            let _permit = tokio::select! {
                result = semaphore.acquire_owned() => {
                    match result { Ok(p) => p, Err(_) => return }
                }
                _ = cancel_token.cancelled() => {
                    tm.complete(tx_id, false, "Cancelled".to_string()).await;
                    kio.finish(false, &tr!("Cancelled"));
                    let _ = Self::transaction_finished(&emitter, tx_id.to_string(), false, "Cancelled".to_string()).await;
                    return;
                }
            };

            let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel::<crate::providers::Progress>();
            let emitter_fwd = emitter.clone();
            let tm_fwd = tm.clone();
            let cancel_token_fwd = cancel_token.clone();
            let kio_fwd = kio.clone();
            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        _ = cancel_token_fwd.cancelled() => break,
                        result = progress_rx.recv() => match result {
                            Some(p) => {
                                tm_fwd.update_progress(tx_id, p.percent).await;
                                kio_fwd.progress(p.percent);
                                let _ = Self::transaction_progress(&emitter_fwd, tx_id.to_string(), p.percent).await;
                                let _ = Self::transaction_stats(&emitter_fwd, tx_id.to_string(), p.bytes_done, p.bytes_total).await;
                            }
                            None => break,
                        }
                    }
                }
            });

            tokio::select! {
                _ = cancel_token.cancelled() => {
                    tm.complete(tx_id, false, "Cancelled".to_string()).await;
                    kio.finish(false, &tr!("Cancelled"));
                    let _ = Self::transaction_finished(&emitter, tx_id.to_string(), false, "Cancelled".to_string()).await;
                }
                result = provider.install_bundle_with_progress(&path, progress_tx, cancel_token.clone()) => {
                    match result {
                        Ok(()) => {
                            provider.invalidate_package_cache().await;
                            tm.complete(tx_id, true, "Installation successful".to_string()).await;
                            kio.finish(true, "");
                            let _ = Self::transaction_progress(&emitter, tx_id.to_string(), 100).await;
                            let _ = Self::transaction_finished(&emitter, tx_id.to_string(), true, "Installation successful".to_string()).await;
                        }
                        Err(e) => {
                            error!("Bundle install failed: {}", e);
                            tm.complete(tx_id, false, e.to_string()).await;
                            kio.finish(false, &e.to_string());
                            let _ = Self::transaction_finished(&emitter, tx_id.to_string(), false, e.to_string()).await;
                        }
                    }
                }
            }
        });

        tx_id.to_string()
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
    async fn transaction_stats(
        signal_emitter: &SignalEmitter<'_>,
        transaction_id: String,
        bytes_done: u64,
        bytes_total: u64,
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
