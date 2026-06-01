use async_trait::async_trait;
use libarc::{ArcError, Package};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc::UnboundedSender, RwLock};
use tokio_util::sync::CancellationToken;

use libflatpak::gio::prelude::CancellableExt;

// 15 minutes
// newly installed packages show up without a manual refresh
const PACKAGE_CACHE_TTL: Duration = Duration::from_secs(900);

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

pub struct MultiProvider {
    pub native: Arc<distrobox::DistroboxProvider>,
    pub flatpak: Arc<flatpak::FlatpakProvider>,
    pub lutris: Arc<lutris::LutrisProvider>,
    pub appimage: Arc<appimage::AppImageProvider>,
    package_cache: RwLock<Option<(Instant, Vec<Package>)>>,
}

impl MultiProvider {
    pub fn new(
        native: distrobox::DistroboxProvider,
        flatpak: flatpak::FlatpakProvider,
        lutris: lutris::LutrisProvider,
        appimage: appimage::AppImageProvider,
    ) -> Self {
        Self {
            native: Arc::new(native),
            flatpak: Arc::new(flatpak),
            lutris: Arc::new(lutris),
            appimage: Arc::new(appimage),
            package_cache: RwLock::new(None),
        }
    }

    fn is_lutris_id(id: &str) -> bool {
        id.starts_with("lutris:")
    }

    fn is_appimage_id(id: &str) -> bool {
        id.starts_with("appimage:") || id.to_lowercase().ends_with(".appimage")
    }

    // flatpak ids look like "org.gimp.GIMP" (reverse dns, dots, no semicolons).
    // distrobox ids look like "distrobox:container:name:type" or are file paths.
    fn is_flatpak_id(id: &str) -> bool {
        !id.contains('/')
            && !id.contains(';')
            && !id.starts_with("distrobox:")
            && !id.starts_with("lutris:")
            && !id.starts_with("appimage:")
            && id.matches('.').count() >= 2
    }

    async fn fetch_and_store(&self) -> Result<Vec<Package>, ArcError> {
        let (flatpak, native, lutris, appimage) = tokio::join!(
            self.flatpak.fetch_all(),
            self.native.fetch_all(),
            self.lutris.fetch_all(),
            self.appimage.fetch_all(),
        );
        let mut packages = flatpak.unwrap_or_default();
        packages.extend(native.unwrap_or_default());
        packages.extend(lutris.unwrap_or_default());
        packages.extend(appimage.unwrap_or_default());
        {
            let mut cache = self.package_cache.write().await;
            *cache = Some((Instant::now(), packages.clone()));
        }
        Ok(packages)
    }

    pub async fn refresh_cache(&self) -> Result<(), ArcError> {
        self.fetch_and_store().await.map(|_| ())
    }

    pub async fn invalidate_package_cache(&self) {
        let mut cache = self.package_cache.write().await;
        *cache = None;
    }

    pub async fn list_extensions(&self, app_id: &str) -> Result<Vec<Package>, ArcError> {
        self.flatpak.list_extensions(app_id).await
    }

    pub async fn install_with_progress(
        &self,
        package_id: &str,
        progress_tx: UnboundedSender<u8>,
        cancel_token: CancellationToken,
    ) -> Result<(), ArcError> {
        if Self::is_appimage_id(package_id) {
            let _ = progress_tx.send(10);
            let result = self.appimage.install(package_id).await;
            let _ = progress_tx.send(100);
            result
        } else if Self::is_flatpak_id(package_id) {
            let gio_cancel = libflatpak::gio::Cancellable::new();
            let gio_cancel_bridge = gio_cancel.clone();
            let bridge = tokio::spawn(async move {
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
        progress_tx: UnboundedSender<u8>,
        cancel_token: CancellationToken,
    ) -> Result<(), ArcError> {
        let gio_cancel = libflatpak::gio::Cancellable::new();
        let gio_cancel_bridge = gio_cancel.clone();
        let bridge = tokio::spawn(async move {
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
        progress_tx: UnboundedSender<u8>,
        cancel_token: CancellationToken,
    ) -> Result<(), ArcError> {
        let gio_cancel = libflatpak::gio::Cancellable::new();
        let gio_cancel_bridge = gio_cancel.clone();
        let bridge = tokio::spawn(async move {
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
        progress_tx: UnboundedSender<u8>,
        cancel_token: CancellationToken,
    ) -> Result<(), ArcError> {
        if Self::is_appimage_id(package_id) {
            let _ = progress_tx.send(10);
            let result = self.appimage.update(package_id).await;
            let _ = progress_tx.send(100);
            result
        } else if Self::is_flatpak_id(package_id) {
            let gio_cancel = libflatpak::gio::Cancellable::new();
            let gio_cancel_bridge = gio_cancel.clone();
            let bridge = tokio::spawn(async move {
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
            self.native.update(package_id).await
        }
    }

    async fn all_packages(&self) -> Result<Vec<Package>, ArcError> {
        {
            let cache = self.package_cache.read().await;
            if let Some((cached_at, packages)) = cache.as_ref() {
                if cached_at.elapsed() < PACKAGE_CACHE_TTL {
                    return Ok(packages.clone());
                }
            }
        }
        self.fetch_and_store().await
    }
}

#[async_trait]
impl PackageProvider for MultiProvider {
    async fn search(&self, query: &str) -> Result<Vec<Package>, ArcError> {
        let packages = self.all_packages().await?;
        Ok(libarc::search_and_rank(packages, query))
    }

    async fn list_installed(&self) -> Result<Vec<Package>, ArcError> {
        let (native, flatpak, lutris, appimage) = tokio::join!(
            self.native.list_installed(),
            self.flatpak.list_installed(),
            self.lutris.list_installed(),
            self.appimage.list_installed(),
        );
        let mut results = native.unwrap_or_default();
        results.extend(flatpak.unwrap_or_default());
        results.extend(lutris.unwrap_or_default());
        results.extend(appimage.unwrap_or_default());
        Ok(results)
    }

    async fn list_updates(&self) -> Result<Vec<Package>, ArcError> {
        let (native, flatpak, appimage) = tokio::join!(
            self.native.list_updates(),
            self.flatpak.list_updates(),
            self.appimage.list_updates(),
        );
        let mut results = native.unwrap_or_default();
        results.extend(flatpak.unwrap_or_default());
        results.extend(appimage.unwrap_or_default());
        Ok(results)
    }

    async fn install(&self, package_id: &str) -> Result<(), ArcError> {
        if Self::is_appimage_id(package_id) {
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
        if Self::is_appimage_id(package_id) {
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
        if Self::is_appimage_id(package_id) {
            self.appimage.update(package_id).await
        } else if Self::is_lutris_id(package_id) {
            self.lutris.update(package_id).await
        } else if Self::is_flatpak_id(package_id) {
            self.flatpak.update(package_id).await
        } else {
            self.native.update(package_id).await
        }
    }

    async fn run(&self, package_id: &str) -> Result<(), ArcError> {
        if Self::is_appimage_id(package_id) {
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
        self.flatpak.search_category(category).await
    }

    async fn get_app_info(&self, package_id: &str) -> Result<Option<Package>, ArcError> {
        if Self::is_appimage_id(package_id) {
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
