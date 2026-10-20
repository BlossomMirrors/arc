use async_trait::async_trait;
use libarc::cache::JsonCache;
use libarc::{ArcError, Package, Provider};
use std::future::Future;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tokio::spawn;
use tokio::sync::{mpsc::UnboundedSender, Mutex, RwLock};
use tokio::task::spawn_blocking;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use libflatpak::gio::prelude::CancellableExt;

// 15 minutes
// newly installed packages show up without a manual refresh
const PACKAGE_CACHE_TTL: Duration = Duration::from_secs(900);

// a single slow or unreachable provider (e.g. Lutris waiting on lutris.net)
// must not block search results from every other provider
const PROVIDER_FETCH_TIMEOUT: Duration = Duration::from_secs(30);

// flatpak's fetch depends on parsing the (potentially large, multi-remote)
// appstream catalog via the shared AppStreamDb::get_static() OnceLock, a
// one-time cost per daemon lifetime that's slow but always terminates on its
// own; it gets more headroom than network-bound providers that could hang
// indefinitely instead of just being slow
const FLATPAK_FETCH_TIMEOUT: Duration = Duration::from_secs(180);

// a timeout is presumed transient (e.g. a slow network) and worth retrying
// soon; a real error from the provider is left at the normal cache TTL so a
// persistently broken provider isn't hammered every few seconds
async fn bounded<F>(name: &str, timeout: Duration, fetch: F) -> (Vec<Package>, bool)
where
    F: Future<Output = Result<Vec<Package>, ArcError>>,
{
    match tokio::time::timeout(timeout, fetch).await {
        Ok(Ok(packages)) => (packages, true),
        Ok(Err(e)) => {
            warn!("{name} provider fetch failed: {e}");
            (Vec::new(), true)
        }
        Err(_) => {
            warn!("{name} provider fetch timed out after {timeout:?}");
            (Vec::new(), false)
        }
    }
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

// a JSON snapshot of the last successful package_cache, so a fresh daemon
// process can serve (slightly stale) search results immediately on startup
// instead of making every first search wait out the full provider fetch
static PACKAGE_STORE: OnceLock<JsonCache<Vec<Package>>> = OnceLock::new();

fn package_store() -> &'static JsonCache<Vec<Package>> {
    PACKAGE_STORE.get_or_init(|| JsonCache::new("daemon", "packages.json"))
}

fn load_disk_cache() -> Option<(Vec<Package>, Duration)> {
    let (packages, age) = package_store().load_with_age()?;
    info!("Loaded {} packages from on-disk cache ({}s old)", packages.len(), age.as_secs());
    Some((packages, age))
}

fn save_disk_cache(packages: &[Package]) {
    package_store().store(&packages.to_vec());
}

#[derive(Clone, Copy, Default)]
pub struct Progress {
    pub percent: u8,
    pub bytes_done: u64,
    pub bytes_total: u64,
}

impl Progress {
    pub fn pct(percent: u8) -> Self {
        Self { percent, ..Default::default() }
    }
}

pub struct MultiProvider {
    pub native: Arc<distrobox::DistroboxProvider>,
    pub flatpak: Arc<flatpak::FlatpakProvider>,
    pub lutris: Arc<lutris::LutrisProvider>,
    pub appimage: Arc<appimage::AppImageProvider>,
    pub pwa: Arc<pwa::PwaProvider>,
    package_cache: RwLock<Option<(Instant, Vec<Package>)>>,
    fetch_lock: Arc<Mutex<()>>,
    self_weak: std::sync::Weak<MultiProvider>,
}

impl MultiProvider {
    pub fn new(
        native: distrobox::DistroboxProvider,
        flatpak: flatpak::FlatpakProvider,
        lutris: lutris::LutrisProvider,
        appimage: appimage::AppImageProvider,
        pwa: pwa::PwaProvider,
    ) -> Arc<Self> {
        // pre-seed the cache from disk (if any) so the very first search
        // after startup doesn't have to wait for a live provider fetch; an
        // entry still within the TTL is seeded fresh, an older one is seeded
        // already-stale so a background refresh triggers on first access
        // instead of trusting possibly-days-old data for the full TTL
        let seeded = load_disk_cache().map(|(packages, age)| {
            let cached_at = if age < PACKAGE_CACHE_TTL {
                Instant::now()
            } else {
                Instant::now() - PACKAGE_CACHE_TTL
            };
            (cached_at, packages)
        });

        Arc::new_cyclic(|weak| Self {
            native: Arc::new(native),
            flatpak: Arc::new(flatpak),
            lutris: Arc::new(lutris),
            appimage: Arc::new(appimage),
            pwa: Arc::new(pwa),
            package_cache: RwLock::new(seeded),
            fetch_lock: Arc::new(Mutex::new(())),
            self_weak: weak.clone(),
        })
    }

    fn trigger_background_refresh(&self) {
        let Some(strong) = self.self_weak.upgrade() else { return };
        let Ok(guard) = Arc::clone(&self.fetch_lock).try_lock_owned() else { return };
        spawn(async move {
            let _ = strong.fetch_and_store().await;
            drop(guard);
        });
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

    async fn fetch_and_store(&self) -> Result<Vec<Package>, ArcError> {
        let (flatpak, native, lutris, appimage, pwa) = tokio::join!(
            bounded("flatpak", FLATPAK_FETCH_TIMEOUT, self.flatpak.fetch_all()),
            bounded("native", PROVIDER_FETCH_TIMEOUT, self.native.fetch_all()),
            bounded("lutris", PROVIDER_FETCH_TIMEOUT, self.lutris.fetch_all()),
            bounded("appimage", PROVIDER_FETCH_TIMEOUT, self.appimage.fetch_all()),
            bounded("pwa", PROVIDER_FETCH_TIMEOUT, self.pwa.search("")),
        );
        let complete = flatpak.1 && native.1 && lutris.1 && appimage.1 && pwa.1;
        let previous = self.package_cache.read().await.as_ref().map(|(_, p)| p.clone()).unwrap_or_default();
        let keep_previous = |provider: Provider| -> Vec<Package> {
            previous.iter().filter(|p| p.provider == provider).cloned().collect()
        };

        let mut packages = if flatpak.1 { flatpak.0 } else { keep_previous(Provider::Flatpak) };
        packages.extend(if native.1 { native.0 } else { keep_previous(Provider::Distrobox) });
        packages.extend(if lutris.1 { lutris.0 } else { keep_previous(Provider::Lutris) });
        packages.extend(if appimage.1 { appimage.0 } else { keep_previous(Provider::AppImage) });
        packages.extend(if pwa.1 { pwa.0 } else { keep_previous(Provider::Pwa) });
        {
            let mut cache = self.package_cache.write().await;
            // if a provider timed out, keep this result around only briefly so
            // the next search retries instead of being stuck on partial data
            // for the full cache TTL
            let retry_grace = Duration::from_secs(15);
            let cached_at = if complete {
                Instant::now()
            } else {
                Instant::now() - PACKAGE_CACHE_TTL.saturating_sub(retry_grace)
            };
            *cache = Some((cached_at, packages.clone()));
        }
        if complete {
            // persist off the async path; this is plain blocking file I/O
            let to_persist = packages.clone();
            spawn_blocking(move || save_disk_cache(&to_persist));
        }
        Ok(packages)
    }

    pub async fn refresh_cache(&self) -> Result<(), ArcError> {
        self.fetch_and_store().await.map(|_| ())
    }

    // true if the disk-seeded cache is still within its TTL, i.e. a live
    // provider fetch (flatpak list, distrobox enter per container, ...)
    // isn't actually needed right now
    pub async fn cache_is_fresh(&self) -> bool {
        self.package_cache
            .read()
            .await
            .as_ref()
            .is_some_and(|(cached_at, _)| cached_at.elapsed() < PACKAGE_CACHE_TTL)
    }

    pub async fn invalidate_package_cache(&self) {
        let mut cache = self.package_cache.write().await;
        if let Some((cached_at, _)) = cache.as_mut() {
            *cached_at = Instant::now() - PACKAGE_CACHE_TTL;
        }
    }

    pub async fn cached_package(&self, id: &str) -> Option<Package> {
        let cache = self.package_cache.read().await;
        cache.as_ref()?.1.iter().find(|p| p.id == id).cloned()
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

    async fn all_packages(&self) -> Result<Vec<Package>, ArcError> {
        let cached = {
            let cache = self.package_cache.read().await;
            cache
                .as_ref()
                .map(|(cached_at, packages)| (cached_at.elapsed() < PACKAGE_CACHE_TTL, packages.clone()))
        };
        if let Some((fresh, packages)) = cached {
            if !fresh {
                self.trigger_background_refresh();
            }
            return Ok(packages);
        }

        let lock = self.fetch_lock.lock().await;
        if let Some((_, packages)) = self.package_cache.read().await.as_ref() {
            return Ok(packages.clone());
        }
        let (flatpak_packages, _) = bounded("flatpak", FLATPAK_FETCH_TIMEOUT, self.flatpak.fetch_all()).await;
        {
            let mut cache = self.package_cache.write().await;
            *cache = Some((Instant::now() - PACKAGE_CACHE_TTL, flatpak_packages.clone()));
        }
        drop(lock);
        self.trigger_background_refresh();
        Ok(flatpak_packages)
    }
}

#[async_trait]
impl PackageProvider for MultiProvider {
    async fn search(&self, query: &str) -> Result<Vec<Package>, ArcError> {
        let packages = self.all_packages().await?;
        Ok(libarc::search_and_rank(packages, query))
    }

    async fn list_installed(&self) -> Result<Vec<Package>, ArcError> {
        let (native, flatpak, lutris, appimage, pwa) = tokio::join!(
            self.native.list_installed(),
            self.flatpak.list_installed(),
            self.lutris.list_installed(),
            self.appimage.list_installed(),
            self.pwa.list_installed(),
        );
        let mut results = native.unwrap_or_default();
        results.extend(flatpak.unwrap_or_default());
        results.extend(lutris.unwrap_or_default());
        results.extend(appimage.unwrap_or_default());
        results.extend(pwa.unwrap_or_default());
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
        if crate::appstream_db::AppStreamDb::fully_loaded() {
            return self.flatpak.search_category(category).await;
        }
        let cat_lower = category.to_lowercase();
        let all = self.all_packages().await?;
        Ok(all
            .into_iter()
            .filter(|p| p.categories.iter().any(|c| c.eq_ignore_ascii_case(&cat_lower)))
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
            match self.flatpak.get_app_info(package_id).await {
                Ok(Some(pkg)) => Ok(Some(pkg)),

                _ => Ok(self.cached_package(package_id).await),
            }
        } else {
            self.native.get_app_info(package_id).await
        }
    }
}
