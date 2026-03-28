use async_trait::async_trait;
use libarc::{ArcError, Package};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

// 15 minutes
// newly installed packages show up without a manual refresh
const PACKAGE_CACHE_TTL: Duration = Duration::from_secs(900);

#[async_trait]
pub trait PackageProvider: Send + Sync {
    async fn search(&self, query: &str) -> Result<Vec<Package>, ArcError>;
    async fn list_installed(&self) -> Result<Vec<Package>, ArcError>;
    async fn install(&self, package_id: &str) -> Result<(), ArcError>;
    async fn remove(&self, package_id: &str) -> Result<(), ArcError>;
    async fn list_updates(&self) -> Result<Vec<Package>, ArcError>;
    async fn update(&self, package_id: &str) -> Result<(), ArcError>;
}

pub mod flatpak;
pub mod packagekit;

pub struct MultiProvider {
    pub native: Arc<packagekit::PackageKitProvider>,
    pub flatpak: Arc<flatpak::FlatpakProvider>,
    package_cache: RwLock<Option<(Instant, Vec<Package>)>>,
}

impl MultiProvider {
    pub fn new(native: packagekit::PackageKitProvider, flatpak: flatpak::FlatpakProvider) -> Self {
        Self {
            native: Arc::new(native),
            flatpak: Arc::new(flatpak),
            package_cache: RwLock::new(None),
        }
    }

    // flatpak ids look like "org.gimp.GIMP" (reverse dns, dots, no semicolons... also why i hate this with flatpaks).
    // packagekit ids look like "gimp;2.10;x86_64;fedora" (semicolons everywhere)
    fn is_flatpak_id(id: &str) -> bool {
        !id.contains(';') && id.matches('.').count() >= 2
    }

    async fn fetch_and_store(&self) -> Result<Vec<Package>, ArcError> {
        // fetch from both providers at the same time instead of one after the other
        let (flatpak, native) = tokio::join!(self.flatpak.fetch_all(), self.native.fetch_all());
        let mut packages = flatpak.unwrap_or_default();
        packages.extend(native.unwrap_or_default());
        {
            let mut cache = self.package_cache.write().await;
            *cache = Some((Instant::now(), packages.clone()));
        }
        Ok(packages)
    }

    pub async fn refresh_cache(&self) -> Result<(), ArcError> {
        self.fetch_and_store().await.map(|_| ())
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
        let q = query.to_lowercase();
        let packages = self.all_packages().await?;
        Ok(packages
            .into_iter()
            .filter(|p| {
                p.name.to_lowercase().contains(&q)
                    || p.id.to_lowercase().contains(&q)
                    || p.description.to_lowercase().contains(&q)
            })
            .collect())
    }

    async fn list_installed(&self) -> Result<Vec<Package>, ArcError> {
        let (native, flatpak) =
            tokio::join!(self.native.list_installed(), self.flatpak.list_installed());
        let mut results = native.unwrap_or_default();
        results.extend(flatpak.unwrap_or_default());
        Ok(results)
    }

    async fn list_updates(&self) -> Result<Vec<Package>, ArcError> {
        let (native, flatpak) =
            tokio::join!(self.native.list_updates(), self.flatpak.list_updates());
        let mut results = native.unwrap_or_default();
        results.extend(flatpak.unwrap_or_default());
        Ok(results)
    }

    async fn install(&self, package_id: &str) -> Result<(), ArcError> {
        if Self::is_flatpak_id(package_id) {
            self.flatpak.install(package_id).await
        } else {
            self.native.install(package_id).await
        }
    }

    async fn remove(&self, package_id: &str) -> Result<(), ArcError> {
        if Self::is_flatpak_id(package_id) {
            self.flatpak.remove(package_id).await
        } else {
            self.native.remove(package_id).await
        }
    }

    async fn update(&self, package_id: &str) -> Result<(), ArcError> {
        if Self::is_flatpak_id(package_id) {
            self.flatpak.update(package_id).await
        } else {
            self.native.update(package_id).await
        }
    }
}
