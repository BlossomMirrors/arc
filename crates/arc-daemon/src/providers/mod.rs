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

pub mod bottles;
pub mod distrobox;
pub mod flatpak;

pub struct MultiProvider {
    pub native: Arc<distrobox::DistroboxProvider>,
    pub flatpak: Arc<flatpak::FlatpakProvider>,
    pub bottles: Arc<bottles::BottlesProvider>,
    package_cache: RwLock<Option<(Instant, Vec<Package>)>>,
}

impl MultiProvider {
    pub fn new(
        native: distrobox::DistroboxProvider,
        flatpak: flatpak::FlatpakProvider,
        bottles: bottles::BottlesProvider,
    ) -> Self {
        Self {
            native: Arc::new(native),
            flatpak: Arc::new(flatpak),
            bottles: Arc::new(bottles),
            package_cache: RwLock::new(None),
        }
    }

    fn is_bottles_id(id: &str) -> bool {
        id.starts_with("bottles:")
    }

    // flatpak ids look like "org.gimp.GIMP" (reverse dns, dots, no semicolons).
    // distrobox ids look like "distrobox:container:name:type" or are file paths.
    fn is_flatpak_id(id: &str) -> bool {
        !id.contains('/')
            && !id.contains(';')
            && !id.starts_with("distrobox:")
            && !id.starts_with("bottles:")
            && id.matches('.').count() >= 2
    }

    async fn fetch_and_store(&self) -> Result<Vec<Package>, ArcError> {
        let (flatpak, native, bottles) = tokio::join!(
            self.flatpak.fetch_all(),
            self.native.fetch_all(),
            self.bottles.fetch_all()
        );
        let mut packages = flatpak.unwrap_or_default();
        packages.extend(native.unwrap_or_default());
        packages.extend(bottles.unwrap_or_default());
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
        let (native, flatpak, bottles) = tokio::join!(
            self.native.list_installed(),
            self.flatpak.list_installed(),
            self.bottles.list_installed()
        );
        let mut results = native.unwrap_or_default();
        results.extend(flatpak.unwrap_or_default());
        results.extend(bottles.unwrap_or_default());
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
        if Self::is_bottles_id(package_id) {
            self.bottles.install(package_id).await
        } else if Self::is_flatpak_id(package_id) {
            self.flatpak.install(package_id).await
        } else {
            self.native.install(package_id).await
        }
    }

    async fn remove(&self, package_id: &str) -> Result<(), ArcError> {
        if Self::is_bottles_id(package_id) {
            self.bottles.remove(package_id).await
        } else if Self::is_flatpak_id(package_id) {
            self.flatpak.remove(package_id).await
        } else {
            self.native.remove(package_id).await
        }
    }

    async fn update(&self, package_id: &str) -> Result<(), ArcError> {
        if Self::is_bottles_id(package_id) {
            self.bottles.update(package_id).await
        } else if Self::is_flatpak_id(package_id) {
            self.flatpak.update(package_id).await
        } else {
            self.native.update(package_id).await
        }
    }
}
