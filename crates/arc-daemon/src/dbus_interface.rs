use crate::providers::flatpak::FlatpakProvider;
use crate::providers::MultiProvider;
use crate::providers::PackageProvider;
use crate::transaction_manager::TransactionManager;
use libarc::{Package, Provider, TransactionType};
use std::env;
use std::fs;
use std::sync::Arc;
use tokio::spawn;
use tokio::sync::{Mutex, Semaphore};
use tokio::task::spawn_blocking;

fn proxy_icon_url(pkg: &mut Package) {
    crate::media::proxy_icon_url_field(&mut pkg.icon_url, &pkg.id);
    crate::media::proxy_screenshot_urls(&mut pkg.screenshots);
}

fn proxy_icon_urls(packages: &mut [Package]) {
    for pkg in packages {
        proxy_icon_url(pkg);
    }
}

// flatpak ids look like "org.gimp.GIMP" (reverse dns, dots, no slashes or semicolons).
// distrobox ids look like "distrobox:container:name:type" or are file paths for installs.
// lutris ids look like "lutris:<slug>".
pub async fn run_auto_updates(
    provider: Arc<MultiProvider>,
    transaction_manager: Arc<TransactionManager>,
    emitter: SignalEmitter<'static>,
) {
    loop {
        if libarc::Settings::load().auto_updates {
            info!("Auto-update: checking for updates...");
            match provider.list_updates().await {
                Err(e) => warn!("Auto-update: list_updates failed: {}", e),
                Ok(updates) if updates.is_empty() => {
                    info!("Auto-update: nothing to update");
                }
                Ok(updates) => {
                    info!("Auto-update: updating {} package(s)", updates.len());
                    let mut any_succeeded = false;
                    for pkg in updates {
                        any_succeeded |=
                            auto_update_one(&provider, &transaction_manager, &emitter, &pkg.id)
                                .await;
                    }

                    if any_succeeded {
                        provider.invalidate_package_cache().await;
                    }
                    info!("Auto-update: done");
                    let remaining =
                        provider.list_updates().await.map(|u| u.len() as u32).unwrap_or(0);
                    let _ = ArcDaemonInterface::updates_available(&emitter, remaining).await;
                }
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
    }
}

async fn auto_update_one(
    provider: &MultiProvider,
    tm: &TransactionManager,
    emitter: &SignalEmitter<'static>,
    package_id: &str,
) -> bool {
    info!("Auto-update: updating {}", package_id);
    let (tx, _cancel_token) = tm
        .create_automatic(
            TransactionType::Update,
            package_id.to_string(),
            provider_from_id(package_id),
        )
        .await;
    let tx_id = tx.id;

    let _ = ArcDaemonInterface::transaction_started(
        emitter,
        tx_id.to_string(),
        package_id.to_string(),
    )
    .await;
    tm.update_progress(tx_id, 10).await;
    let _ = ArcDaemonInterface::transaction_progress(emitter, tx_id.to_string(), 10).await;

    match provider.update(package_id).await {
        Ok(()) => {
            tm.complete(tx_id, true, "Update successful".to_string()).await;
            let _ = ArcDaemonInterface::transaction_progress(emitter, tx_id.to_string(), 100).await;
            let _ = ArcDaemonInterface::transaction_finished(
                emitter,
                tx_id.to_string(),
                true,
                "Update successful".to_string(),
            )
            .await;
            true
        }
        Err(e) => {
            warn!("Auto-update: failed to update {}: {}", package_id, e);
            tm.complete(tx_id, false, e.to_string()).await;
            let _ = ArcDaemonInterface::transaction_finished(
                emitter,
                tx_id.to_string(),
                false,
                e.to_string(),
            )
            .await;
            false
        }
    }
}

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
// making a UI click hang for that whole window - the caller can just retry
// once the background warm-up (kicked off at daemon startup) finishes.
const APP_INFO_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

pub struct ArcDaemonInterface {
    pub provider: Arc<MultiProvider>,
    pub transaction_manager: Arc<TransactionManager>,
    pub download_semaphore: Arc<Semaphore>,
    // the concurrency target last requested via set_concurrent_downloads;
    // a Mutex (not an AtomicUsize) so a shrink and a subsequent resize can't
    // race and compute their deltas against a half-applied target
    pub download_permits: Arc<Mutex<usize>>,
    // whether the frontend window is currently on screen (not minimized,
    // not closed); while it is, the Downloads/Detail pages already show
    // install/remove progress in-app, so the OS-level job notification
    // would just be a redundant popup
    pub frontend_visible: Arc<tokio::sync::RwLock<bool>>,
}

impl ArcDaemonInterface {
    // notify: caller-level opt-out. the GUI always passes true and lets
    // frontend_visible decide; the CLI passes false unless the user asked
    // for a notification with --notify.
    async fn kio_hidden(&self, notify: bool) -> bool {
        if !notify {
            return true;
        }
        let visible = *self.frontend_visible.read().await;
        info!("kio_hidden: notify={notify} frontend_visible={visible} -> hidden={visible}");
        visible
    }
}

#[interface(name = "org.blossomos.arc.daemon")]
impl ArcDaemonInterface {
    async fn install_package(
        &self,
        package_id: String,
        notify: bool,
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
        let kio_hidden = self.kio_hidden(notify).await;
        spawn(async move {
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
            spawn(async move {
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
        notify: bool,
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

        let kio_hidden = self.kio_hidden(notify).await;
        spawn(async move {
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
            spawn(async move {
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
        notify: bool,
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

        let kio_hidden = self.kio_hidden(notify).await;
        spawn(async move {
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
        notify: bool,
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

        let kio_hidden = self.kio_hidden(notify).await;
        spawn(async move {
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
                        if let Some(home) = env::var_os("HOME") {
                            let home = std::path::PathBuf::from(home);
                            let flatpak_dir = home.join(".var/app").join(&package_id);
                            if flatpak_dir.exists() {
                                let _ = fs::remove_dir_all(&flatpak_dir);
                            }
                            if let Some(appid) = package_id.strip_prefix("pwa:") {
                                let pwa_dir = home
                                    .join(".local/share/blossomos-webapps")
                                    .join(appid);
                                if pwa_dir.exists() {
                                    let _ = fs::remove_dir_all(&pwa_dir);
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
        notify: bool,
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

        let kio_hidden = self.kio_hidden(notify).await;
        spawn(async move {
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
            spawn(async move {
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

    // the frontend reports whether its window is currently on screen so job
    // notifications only appear while there's no in-app UI to show progress
    async fn set_frontend_visible(&self, visible: bool) {
        info!("SetFrontendVisible: {visible}");
        *self.frontend_visible.write().await = visible;
    }

    async fn set_concurrent_downloads(&self, count: u32) {
        let target = count.max(1) as usize;
        let mut current = self.download_permits.lock().await;
        if target == *current {
            return;
        }
        let previous = *current;
        *current = target;
        drop(current);

        if target > previous {
            self.download_semaphore.add_permits(target - previous);
        } else {
            // Semaphore::forget_permits only forgets currently-*available*
            // permits, so shrinking while every slot is checked out by an
            // in-progress download silently forgot 0 and the old (higher)
            // limit kept applying until every download finished on its own.
            // acquiring the deficit and forgetting the acquired permits
            // instead waits for slots to free up naturally and always
            // converges to the real target, no matter how busy the
            // semaphore is right now.
            let deficit = (previous - target) as u32;
            let semaphore = self.download_semaphore.clone();
            spawn(async move {
                if let Ok(permits) = semaphore.acquire_many_owned(deficit).await {
                    permits.forget();
                }
            });
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
            Ok(mut packages) => {
                proxy_icon_urls(&mut packages);
                serde_json::to_string(&packages).unwrap_or_else(|_| "[]".to_string())
            }
            Err(e) => {
                error!("Search failed: {}", e);
                format!("{{\"error\":\"{}\"}}", e)
            }
        }
    }

    async fn search_category(&self, category: String) -> String {
        info!("SearchCategory: {}", category);
        match self.provider.search_category(&category).await {
            Ok(mut packages) => {
                proxy_icon_urls(&mut packages);
                serde_json::to_string(&packages).unwrap_or_else(|_| "[]".to_string())
            }
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
            Ok(Ok(Some(mut package))) => {
                proxy_icon_url(&mut package);
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
        if let Some(mut entry) = spawn_blocking(move || crate::appstream_db::load_local_metainfo(&fast_id))
            .await
            .ok()
            .flatten()
        {
            crate::media::proxy_screenshot_urls(&mut entry.screenshots);
            if let Ok(json) = serde_json::to_string(&entry) {
                return json;
            }
        }

        // resolve_now checks the already-loaded db first then decompresses
        // and scans just the catalog files not yet folded in
        // stays well under APP_INFO_TIMEOUT instead of waiting on the full parse
        let resolve_id = package_id.clone();
        let fetch = spawn_blocking(move || {
            crate::appstream_db::resolve_now(&resolve_id)
                .or_else(|| crate::appstream_db::AppStreamDb::try_get().load_from_exported_metainfo(&resolve_id))
        });
        let entry = tokio::time::timeout(APP_INFO_TIMEOUT, fetch)
            .await
            .ok()
            .and_then(|r| r.ok())
            .flatten();
        if let Some(mut entry) = entry {
            crate::media::proxy_screenshot_urls(&mut entry.screenshots);
            if let Ok(json) = serde_json::to_string(&entry) {
                return json;
            }
        }

        // id missing from every catalog file
        // fall back to the disk-seeded package cache for a partial answer
        match self.provider.cached_package(&package_id).await {
            Some(pkg) => {
                let mut entry = crate::appstream_db::partial_entry_from_package(&pkg);
                crate::media::proxy_screenshot_urls(&mut entry.screenshots);
                serde_json::to_string(&entry).unwrap_or_else(|_| "null".to_string())
            }
            None => "null".to_string(),
        }
    }

    async fn list_installed(&self) -> String {
        info!("ListInstalled");
        match self.provider.list_installed().await {
            Ok(mut packages) => {
                proxy_icon_urls(&mut packages);
                serde_json::to_string(&packages).unwrap_or_else(|_| "[]".to_string())
            }
            Err(e) => {
                error!("ListInstalled failed: {}", e);
                format!("{{\"error\":\"{}\"}}", e)
            }
        }
    }

    async fn list_leftover_data(&self) -> String {
        info!("ListLeftoverData");

        let installed: std::collections::HashSet<String> = self
            .provider
            .list_installed()
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|p| p.id)
            .collect();

        #[derive(serde::Serialize)]
        struct LeftoverDataEntry {
            id: String,
            name: String,
        }

        let mut entries = Vec::new();

        let Some(home) = env::var_os("HOME") else {
            return "[]".to_string();
        };
        let home = std::path::PathBuf::from(home);

        if let Ok(dirs) = fs::read_dir(home.join(".var/app")) {
            for entry in dirs.flatten() {
                if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    continue;
                }
                let id = entry.file_name().to_string_lossy().to_string();
                if installed.contains(&id) {
                    continue;
                }
                let name = crate::appstream_db::AppStreamDb::try_get()
                    .find_by_id(&id)
                    .map(|e| e.name)
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| id.clone());
                entries.push(LeftoverDataEntry { id, name });
            }
        }

        if let Ok(dirs) = fs::read_dir(home.join(".local/share/blossomos-webapps")) {
            for entry in dirs.flatten() {
                if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    continue;
                }
                let appid = entry.file_name().to_string_lossy().to_string();
                let id = format!("pwa:{appid}");
                if installed.contains(&id) {
                    continue;
                }
                entries.push(LeftoverDataEntry { id, name: appid });
            }
        }

        serde_json::to_string(&entries).unwrap_or_else(|_| "[]".to_string())
    }

    async fn delete_leftover_data(&self, ids: Vec<String>) -> bool {
        info!("DeleteLeftoverData: {:?}", ids);

        let Some(home) = env::var_os("HOME") else {
            return false;
        };
        let home = std::path::PathBuf::from(home);

        let mut ok = true;
        for id in ids {
            let dir = match id.strip_prefix("pwa:") {
                Some(appid) => home.join(".local/share/blossomos-webapps").join(appid),
                None => home.join(".var/app").join(&id),
            };
            if dir.exists() && fs::remove_dir_all(&dir).is_err() {
                ok = false;
            }
        }
        ok
    }

    async fn refresh_catalog(&self, #[zbus(signal_emitter)] emitter: SignalEmitter<'_>) {
        info!("RefreshCatalog");

        let provider = self.provider.clone();
        let emitter = emitter.to_owned();
        spawn(async move {
            crate::appstream_db::refresh_remotes_now().await;
            spawn_blocking(crate::appstream_db::AppStreamDb::refresh_if_stale).await.ok();
            crate::forge_cache::refresh().await;

            crate::media::clear_icons();

            provider.pwa.refresh_installed_icons().await;
            provider.invalidate_package_cache().await;

            let count = provider.list_updates().await.map(|u| u.len() as u32).unwrap_or(0);
            info!("RefreshCatalog: done, {} update(s)", count);
            let _ = Self::updates_available(&emitter, count).await;
            let _ = Self::catalog_refreshed(&emitter).await;
        });
    }

    async fn list_updates(&self, #[zbus(signal_emitter)] emitter: SignalEmitter<'_>) -> String {
        info!("ListUpdates");
        match self.provider.list_updates().await {
            Ok(mut packages) => {
                let count = packages.len() as u32;
                // fire the signal so any notification daemon listening can
                // show a badge or popup without polling list_updates itself
                if count > 0 {
                    let _ = Self::updates_available(&emitter, count).await;
                }
                proxy_icon_urls(&mut packages);
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
        spawn_blocking(move || {
            let db = crate::appstream_db::AppStreamDb::try_get();
            let mut popular = db.get_popular_apps(popular_count as usize);
            let mut recent = db.get_recent_apps(recent_count as usize);
            for entry in popular.iter_mut().chain(recent.iter_mut()) {
                crate::media::proxy_icon_url_field(&mut entry.icon_url, &entry.id);
                crate::media::proxy_screenshot_urls(&mut entry.screenshots);
            }
            serde_json::json!({ "popular": popular, "recent": recent }).to_string()
        })
        .await
        .unwrap_or_else(|_| r#"{"popular":[],"recent":[]}"#.to_string())
    }

    async fn list_extensions(&self, app_id: String) -> String {
        info!("ListExtensions: {}", app_id);
        match self.provider.list_extensions(&app_id).await {
            Ok(mut packages) => {
                proxy_icon_urls(&mut packages);
                serde_json::to_string(&packages).unwrap_or_else(|_| "[]".to_string())
            }
            Err(e) => {
                error!("ListExtensions failed: {}", e);
                "[]".to_string()
            }
        }
    }

    async fn list_remotes(&self) -> String {
        info!("ListRemotes");
        spawn_blocking(FlatpakProvider::list_remotes)
            .await
            .ok()
            .and_then(|v| serde_json::to_string(&v).ok())
            .unwrap_or_else(|| "[]".to_string())
    }

    async fn add_remote(&self, name: String, url: String) -> bool {
        info!("AddRemote: {} {}", name, url);
        spawn_blocking(move || FlatpakProvider::add_remote_from_url(&name, &url))
            .await
            .map(|r| r.is_ok())
            .unwrap_or(false)
    }

    async fn remove_remote(&self, name: String) -> bool {
        info!("RemoveRemote: {}", name);
        spawn_blocking(move || FlatpakProvider::remove_remote(&name))
            .await
            .map(|r| r.is_ok())
            .unwrap_or(false)
    }

    async fn add_flatpakrepo(&self, content: String) -> bool {
        info!("AddFlatpakrepo");
        spawn_blocking(move || FlatpakProvider::add_remote_from_flatpakrepo(&content))
            .await
            .map(|r| r.is_ok())
            .unwrap_or(false)
    }

    async fn install_flatpak_bundle(
        &self,
        path: String,
        notify: bool,
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

        let kio_hidden = self.kio_hidden(notify).await;
        spawn(async move {
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
            spawn(async move {
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
    pub(crate) async fn transaction_started(
        signal_emitter: &SignalEmitter<'_>,
        transaction_id: String,
        package_id: String,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub(crate) async fn transaction_progress(
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
    pub(crate) async fn transaction_finished(
        signal_emitter: &SignalEmitter<'_>,
        transaction_id: String,
        success: bool,
        message: String,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub(crate) async fn updates_available(signal_emitter: &SignalEmitter<'_>, count: u32) -> zbus::Result<()>;

    #[zbus(signal)]
    pub(crate) async fn catalog_refreshed(signal_emitter: &SignalEmitter<'_>) -> zbus::Result<()>;
}
