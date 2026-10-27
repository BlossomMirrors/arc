use async_trait::async_trait;
use libarc::{ArcError, Package, Provider};
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::spawn;
use tokio::sync::{mpsc::UnboundedSender, Mutex};
use tokio::task::spawn_blocking;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::catalog;
use libflatpak::gio::prelude::CancellableExt;

// a single slow or unreachable provider (e.g. Lutris waiting on lutris.net)
// must not block search results from every other provider
const PROVIDER_FETCH_TIMEOUT: Duration = Duration::from_secs(30);

// listing updates checks the providers again in the background at most this often
const UPDATES_RECHECK: Duration = Duration::from_secs(300);

// becomes -1 when bound and sqlite reads LIMIT -1 as no limit
pub const NO_LIMIT: u64 = u64::MAX;

// a timeout or error keeps that provider's previous rows so one broken provider never empties the results of the others
async fn fetch_with_timeout<F>(name: &str, fetch: F) -> Option<Vec<Package>>
where
    F: Future<Output = Result<Vec<Package>, ArcError>>,
{
    match tokio::time::timeout(PROVIDER_FETCH_TIMEOUT, fetch).await {
        Ok(Ok(packages)) => Some(packages),
        Ok(Err(e)) => {
            warn!("{name} provider fetch failed: {e}");
            None
        }
        Err(_) => {
            warn!("{name} provider fetch timed out after {PROVIDER_FETCH_TIMEOUT:?}");
            None
        }
    }
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    spawn_blocking(f).await.ok()
}

#[async_trait]
pub trait PackageProvider: Send + Sync {
    async fn search(&self, query: &str) -> Result<Vec<Package>, ArcError>;
    async fn search_category(&self, category: &str) -> Result<Vec<Package>, ArcError>;
    async fn get_app_info(&self, package_id: &str) -> Result<Option<Package>, ArcError>;
    async fn list_installed(&self) -> Result<Vec<Package>, ArcError>;
    async fn install(&self, package_id: &str) -> Result<(), ArcError>;
    async fn remove(&self, package_id: &str) -> Result<(), ArcError>;
    async fn list_updates(&self) -> Result<Vec<Package>, ArcError>;
    async fn update(&self, package_id: &str) -> Result<(), ArcError>;
    async fn run(&self, package_id: &str) -> Result<(), ArcError>;
}

pub mod appimage;
pub mod distrobox;
pub mod flatpak;
pub mod lutris;
pub mod pwa;

#[derive(Clone, Copy, Default)]
pub struct Progress {
    pub percent: u8,
    pub bytes_done: u64,
    pub bytes_total: u64,
}

impl Progress {
    pub fn pct(percent: u8) -> Self {
        Self {
            percent,
            ..Default::default()
        }
    }
}

pub struct MultiProvider {
    pub native: Arc<distrobox::DistroboxProvider>,
    pub flatpak: Arc<flatpak::FlatpakProvider>,
    pub lutris: Arc<lutris::LutrisProvider>,
    pub appimage: Arc<appimage::AppImageProvider>,
    pub pwa: Arc<pwa::PwaProvider>,
    refresh_lock: Mutex<()>,
    updates_lock: Mutex<()>,
    installed_ready: AtomicBool,
}

impl MultiProvider {
    pub fn new(
        native: distrobox::DistroboxProvider,
        flatpak: flatpak::FlatpakProvider,
        lutris: lutris::LutrisProvider,
        appimage: appimage::AppImageProvider,
        pwa: pwa::PwaProvider,
    ) -> Arc<Self> {
        Arc::new(Self {
            native: Arc::new(native),
            flatpak: Arc::new(flatpak),
            lutris: Arc::new(lutris),
            appimage: Arc::new(appimage),
            pwa: Arc::new(pwa),
            refresh_lock: Mutex::new(()),
            updates_lock: Mutex::new(()),
            installed_ready: AtomicBool::new(false),
        })
    }

    fn is_lutris_id(id: &str) -> bool {
        id.starts_with("lutris:")
    }

    fn is_appimage_id(id: &str) -> bool {
        id.starts_with("appimage:") || id.to_lowercase().ends_with(".appimage")
    }

    fn is_pwa_id(id: &str) -> bool {
        id.starts_with("pwa:")
    }

    // flatpak ids look like "org.gimp.GIMP" (reverse dns, dots, no semicolons).
    // distrobox ids look like "distrobox:container:name:type" or are file paths.
    fn is_flatpak_id(id: &str) -> bool {
        !id.contains('/')
            && !id.contains(';')
            && !id.starts_with("distrobox:")
            && !id.starts_with("lutris:")
            && !id.starts_with("appimage:")
            && !id.starts_with("pwa:")
            && id.matches('.').count() >= 2
    }

    fn provider_of(id: &str) -> Provider {
        if Self::is_pwa_id(id) {
            Provider::Pwa
        } else if Self::is_appimage_id(id) {
            Provider::AppImage
        } else if Self::is_lutris_id(id) {
            Provider::Lutris
        } else if Self::is_flatpak_id(id) {
            Provider::Flatpak
        } else {
            Provider::Distrobox
        }
    }

    async fn refresh_provider(&self, provider: Provider) {
        let (name, rows, installed) = match provider {
            Provider::Flatpak => {
                let (installed, runtimes) = tokio::join!(
                    fetch_with_timeout("flatpak", self.flatpak.list_installed()),
                    tokio::time::timeout(
                        PROVIDER_FETCH_TIMEOUT,
                        self.flatpak.list_installed_runtimes()
                    ),
                );
                if let Ok(Ok(runtimes)) = runtimes {
                    blocking(move || catalog::replace_installed_runtimes(&runtimes)).await;
                }
                ("flatpak", installed.clone(), installed)
            }
            Provider::Distrobox => {
                let (rows, installed) = tokio::join!(
                    fetch_with_timeout("native", self.native.fetch_all()),
                    fetch_with_timeout("native", self.native.list_installed()),
                );
                ("native", rows, installed)
            }
            Provider::Lutris => {
                let (rows, installed) = tokio::join!(
                    fetch_with_timeout("lutris", self.lutris.fetch_all()),
                    fetch_with_timeout("lutris", self.lutris.list_installed()),
                );
                ("lutris", rows, installed)
            }
            Provider::AppImage => {
                let (rows, installed) = tokio::join!(
                    fetch_with_timeout("appimage", self.appimage.fetch_all()),
                    fetch_with_timeout("appimage", self.appimage.list_installed()),
                );
                ("appimage", rows, installed)
            }
            Provider::Pwa => {
                let (rows, installed) = tokio::join!(
                    fetch_with_timeout("pwa", self.pwa.search("")),
                    fetch_with_timeout("pwa", self.pwa.list_installed()),
                );
                ("pwa", rows, installed)
            }
        };
        let count = rows.as_ref().map(|r| r.len());
        blocking(move || {
            if let Some(rows) = rows {
                catalog::replace_provider(&provider, &rows);
            }
            if let Some(installed) = installed {
                catalog::replace_installed(&provider, &installed);
            }
        })
        .await;
        if let Some(n) = count {
            info!("catalogue: {n} {name} packages");
        }
    }

    async fn refresh_installed_and_rows(&self) {
        tokio::join!(
            self.refresh_provider(Provider::Flatpak),
            self.refresh_provider(Provider::Distrobox),
            self.refresh_provider(Provider::Lutris),
            self.refresh_provider(Provider::AppImage),
            self.refresh_provider(Provider::Pwa),
        );
        self.installed_ready.store(true, Ordering::Relaxed);
    }

    pub async fn refresh_cache(&self) -> Result<(), ArcError> {
        let _guard = self.refresh_lock.lock().await;
        self.refresh_installed_and_rows().await;
        if let Some(refs) = bounded_refs(self.flatpak.list_remote_runtime_refs()).await {
            blocking(move || catalog::replace_remote_refs(&refs)).await;
        }
        let icons = blocking(catalog::installed_remote_icons)
            .await
            .unwrap_or_default();
        spawn(async move {
            use futures_util::StreamExt;
            futures_util::stream::iter(icons)
                .for_each_concurrent(4, |url| async move {
                    crate::media::fetch(crate::media::MediaKind::Icon, &url, None).await;
                })
                .await;
        });
        Ok(())
    }

    pub async fn refresh_appimages(&self) {
        self.refresh_provider(Provider::AppImage).await;
    }

    pub async fn after_transaction(&self, package_id: Option<&str>) {
        let provider = package_id
            .map(Self::provider_of)
            .unwrap_or(Provider::Flatpak);
        self.refresh_provider(provider).await;
        if let Some(id) = package_id.map(str::to_string) {
            blocking(move || catalog::remove_update(&id)).await;
        }
    }

    pub async fn on_flatpak_changed(self: &Arc<Self>) {
        self.refresh_provider(Provider::Flatpak).await;
        let this = Arc::clone(self);
        spawn(async move {
            let _ = this.refresh_updates().await;
        });
    }

    pub async fn refresh_updates(&self) -> Result<Vec<Package>, ArcError> {
        let _guard = self.updates_lock.lock().await;
        let (native, flatpak, appimage) = tokio::join!(
            fetch_with_timeout("native", self.native.list_updates()),
            fetch_with_timeout("flatpak", self.flatpak.list_updates()),
            fetch_with_timeout("appimage", self.appimage.list_updates()),
        );
        blocking(move || {
            for (provider, packages) in [
                (Provider::Distrobox, native),
                (Provider::Flatpak, flatpak),
                (Provider::AppImage, appimage),
            ] {
                if let Some(packages) = packages {
                    catalog::replace_updates(&provider, &packages);
                }
            }
            catalog::mark_updates_checked();
            catalog::list_updates()
        })
        .await
        .ok_or_else(|| ArcError::ProviderError("update check was cancelled".into()))
    }

    pub async fn cached_updates(&self) -> Result<Vec<Package>, ArcError> {
        if blocking(catalog::updates_checked_age)
            .await
            .flatten()
            .is_some()
        {
            return Ok(blocking(catalog::list_updates).await.unwrap_or_default());
        }
        self.refresh_updates().await
    }

    pub async fn updates_stale(&self) -> bool {
        blocking(catalog::updates_checked_age)
            .await
            .flatten()
            .is_none_or(|age| age >= UPDATES_RECHECK)
    }

    pub async fn search_page(
        &self,
        query: &str,
        locales: Vec<String>,
        filter: catalog::SearchFilter,
        offset: u64,
        limit: u64,
    ) -> catalog::SearchPage {
        let query = query.to_string();
        blocking(move || catalog::search(&query, &locales, true, &filter, offset, limit))
            .await
            .unwrap_or_default()
    }

    pub async fn category_page(
        &self,
        category: &str,
        locales: Vec<String>,
        priority: Vec<String>,
        offset: u64,
        limit: u64,
    ) -> catalog::SearchPage {
        let category = category.to_string();
        blocking(move || catalog::by_category(&category, &locales, &priority, offset, limit))
            .await
            .unwrap_or_default()
    }

    pub async fn list_extensions(&self, app_id: &str) -> Result<Vec<Package>, ArcError> {
        self.flatpak.list_extensions(app_id).await
    }

    pub async fn install_with_progress(
        &self,
        package_id: &str,
        progress_tx: UnboundedSender<Progress>,
        cancel_token: CancellationToken,
    ) -> Result<(), ArcError> {
        if Self::is_pwa_id(package_id) {
            let _ = progress_tx.send(Progress::pct(10));
            let result = self.pwa.install(package_id).await;
            let _ = progress_tx.send(Progress::pct(100));
            result
        } else if Self::is_appimage_id(package_id) {
            let _ = progress_tx.send(Progress::pct(10));
            let result = self.appimage.install(package_id).await;
            let _ = progress_tx.send(Progress::pct(100));
            result
        } else if Self::is_flatpak_id(package_id) {
            let gio_cancel = libflatpak::gio::Cancellable::new();
            let gio_cancel_bridge = gio_cancel.clone();
            let bridge = spawn(async move {
                cancel_token.cancelled().await;
                gio_cancel_bridge.cancel();
            });
            let result = self
                .flatpak
                .install_with_progress(package_id, progress_tx, gio_cancel)
                .await;
            bridge.abort();
            result
        } else if Self::is_lutris_id(package_id) {
            self.lutris.install(package_id).await
        } else {
            self.native
                .install_with_progress(package_id, progress_tx, cancel_token)
                .await
        }
    }

    pub async fn install_flatpakref_with_progress(
        &self,
        url: &str,
        progress_tx: UnboundedSender<Progress>,
        cancel_token: CancellationToken,
    ) -> Result<(), ArcError> {
        let gio_cancel = libflatpak::gio::Cancellable::new();
        let gio_cancel_bridge = gio_cancel.clone();
        let bridge = spawn(async move {
            cancel_token.cancelled().await;
            gio_cancel_bridge.cancel();
        });
        let result = self
            .flatpak
            .install_flatpakref_with_progress(url, progress_tx, gio_cancel)
            .await;
        bridge.abort();
        result
    }

    pub async fn install_bundle_with_progress(
        &self,
        path: &str,
        progress_tx: UnboundedSender<Progress>,
        cancel_token: CancellationToken,
    ) -> Result<(), ArcError> {
        let gio_cancel = libflatpak::gio::Cancellable::new();
        let gio_cancel_bridge = gio_cancel.clone();
        let bridge = spawn(async move {
            cancel_token.cancelled().await;
            gio_cancel_bridge.cancel();
        });
        let result = self
            .flatpak
            .install_bundle_with_progress(path, progress_tx, gio_cancel)
            .await;
        bridge.abort();
        result
    }

    pub async fn update_with_progress(
        &self,
        package_id: &str,
        progress_tx: UnboundedSender<Progress>,
        cancel_token: CancellationToken,
    ) -> Result<(), ArcError> {
        if Self::is_appimage_id(package_id) {
            let _ = progress_tx.send(Progress::pct(10));
            let result = self.appimage.update(package_id).await;
            let _ = progress_tx.send(Progress::pct(100));
            result
        } else if Self::is_flatpak_id(package_id) {
            let gio_cancel = libflatpak::gio::Cancellable::new();
            let gio_cancel_bridge = gio_cancel.clone();
            let bridge = spawn(async move {
                cancel_token.cancelled().await;
                gio_cancel_bridge.cancel();
            });
            let result = self
                .flatpak
                .update_with_progress(package_id, progress_tx, gio_cancel)
                .await;
            bridge.abort();
            result
        } else {
            self.native
                .update_with_progress(package_id, progress_tx, cancel_token)
                .await
        }
    }
}

async fn bounded_refs<F>(fetch: F) -> Option<Vec<(String, String, String)>>
where
    F: Future<Output = Result<Vec<(String, String, String)>, ArcError>>,
{
    match tokio::time::timeout(PROVIDER_FETCH_TIMEOUT, fetch).await {
        Ok(Ok(refs)) => Some(refs),
        Ok(Err(e)) => {
            warn!("flatpak remote refs failed: {e}");
            None
        }
        Err(_) => {
            warn!("flatpak remote refs timed out");
            None
        }
    }
}

#[async_trait]
impl PackageProvider for MultiProvider {
    async fn search(&self, query: &str) -> Result<Vec<Package>, ArcError> {
        let page = self
            .search_page(
                query,
                crate::appstream_db::system_locales(),
                Default::default(),
                0,
                NO_LIMIT,
            )
            .await;
        Ok(page
            .items
            .into_iter()
            .map(catalog::CatalogApp::into_package)
            .collect())
    }

    async fn list_installed(&self) -> Result<Vec<Package>, ArcError> {
        let stored = blocking(catalog::list_installed).await.unwrap_or_default();
        if !stored.is_empty() || self.installed_ready.load(Ordering::Relaxed) {
            return Ok(stored);
        }
        self.refresh_installed_and_rows().await;
        Ok(blocking(catalog::list_installed).await.unwrap_or_default())
    }

    async fn list_updates(&self) -> Result<Vec<Package>, ArcError> {
        self.refresh_updates().await
    }

    async fn install(&self, package_id: &str) -> Result<(), ArcError> {
        if Self::is_pwa_id(package_id) {
            self.pwa.install(package_id).await
        } else if Self::is_appimage_id(package_id) {
            self.appimage.install(package_id).await
        } else if Self::is_lutris_id(package_id) {
            self.lutris.install(package_id).await
        } else if Self::is_flatpak_id(package_id) {
            self.flatpak.install(package_id).await
        } else {
            self.native.install(package_id).await
        }
    }

    async fn remove(&self, package_id: &str) -> Result<(), ArcError> {
        if Self::is_pwa_id(package_id) {
            self.pwa.remove(package_id).await
        } else if Self::is_appimage_id(package_id) {
            self.appimage.remove(package_id).await
        } else if Self::is_lutris_id(package_id) {
            self.lutris.remove(package_id).await
        } else if Self::is_flatpak_id(package_id) {
            self.flatpak.remove(package_id).await
        } else {
            self.native.remove(package_id).await
        }
    }

    async fn update(&self, package_id: &str) -> Result<(), ArcError> {
        if Self::is_pwa_id(package_id) {
            self.pwa.update(package_id).await
        } else if Self::is_appimage_id(package_id) {
            self.appimage.update(package_id).await
        } else if Self::is_flatpak_id(package_id) {
            self.flatpak.update(package_id).await
        } else {
            self.native.update(package_id).await
        }
    }

    async fn run(&self, package_id: &str) -> Result<(), ArcError> {
        if Self::is_pwa_id(package_id) {
            self.pwa.run(package_id).await
        } else if Self::is_appimage_id(package_id) {
            self.appimage.run(package_id).await
        } else if Self::is_lutris_id(package_id) {
            self.lutris.run(package_id).await
        } else if Self::is_flatpak_id(package_id) {
            self.flatpak.run(package_id).await
        } else {
            self.native.run(package_id).await
        }
    }

    async fn search_category(&self, category: &str) -> Result<Vec<Package>, ArcError> {
        let page = self
            .category_page(
                category,
                crate::appstream_db::system_locales(),
                Vec::new(),
                0,
                NO_LIMIT,
            )
            .await;
        Ok(page
            .items
            .into_iter()
            .map(catalog::CatalogApp::into_package)
            .collect())
    }

    async fn get_app_info(&self, package_id: &str) -> Result<Option<Package>, ArcError> {
        if Self::is_pwa_id(package_id) {
            self.pwa.get_app_info(package_id).await
        } else if Self::is_appimage_id(package_id) {
            self.appimage.get_app_info(package_id).await
        } else if Self::is_lutris_id(package_id) {
            self.lutris.get_app_info(package_id).await
        } else if Self::is_flatpak_id(package_id) {
            self.flatpak.get_app_info(package_id).await
        } else {
            self.native.get_app_info(package_id).await
        }
    }
}
