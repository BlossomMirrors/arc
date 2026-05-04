use super::PackageProvider;
use async_trait::async_trait;
use libarc::{ArcError, Package, Provider};
use serde::Deserialize;
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::fs;
use tokio::process::Command;
use tokio::sync::RwLock;
use tracing::{info, warn};

const WHITELIST_URL: &str = "https://repo.blossomos.org/lutris.txt";
const LUTRIS_API_BASE: &str = "https://lutris.net/api/installers";
const CATALOG_CACHE_TTL: Duration = Duration::from_secs(3600);

#[derive(Debug, Clone, Deserialize)]
struct LutrisInstallerResponse {
    count: u32,
    results: Vec<LutrisInstaller>,
}

#[derive(Debug, Clone, Deserialize)]
struct ScriptMetadata {
    #[serde(default)]
    icon_url: String,
    #[serde(default)]
    banner_url: String,
    #[serde(default)]
    cover_url: String,
}

#[derive(Debug, Clone, Deserialize)]
struct LutrisScript {
    #[serde(default)]
    metadata: Option<ScriptMetadata>,
}

#[derive(Debug, Clone, Deserialize)]
struct LutrisInstaller {
    #[serde(default)]
    slug: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    game_slug: String,
    #[serde(default)]
    runner: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    notes: String,
    #[serde(default)]
    script: Option<LutrisScript>,
}

type CatalogEntry = (String, LutrisInstaller);

pub struct LutrisProvider {
    catalog_cache: RwLock<Option<(Instant, Vec<CatalogEntry>)>>,
    installs_dir: PathBuf,
    http_client: reqwest::Client,
}

impl LutrisProvider {
    pub fn new() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
        Self {
            catalog_cache: RwLock::new(None),
            installs_dir: PathBuf::from(&home).join(".local/share/arc/lutris-installs"),
            http_client: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(30))
                .build()
                .expect("reqwest client"),
        }
    }

    pub fn pkg_id(slug: &str) -> String {
        format!("lutris:{}", slug)
    }

    fn info_path(&self, slug: &str) -> PathBuf {
        self.installs_dir.join(format!("{}.json", slug))
    }

    async fn fetch_whitelist(&self) -> Result<Vec<String>, ArcError> {
        let text = self
            .http_client
            .get(WHITELIST_URL)
            .send()
            .await
            .map_err(|e| ArcError::ProviderError(format!("Whitelist fetch: {}", e)))?
            .text()
            .await
            .map_err(|e| ArcError::ProviderError(format!("Whitelist read: {}", e)))?;

        let slugs = text
            .lines()
            .filter(|line| {
                let trimmed = line.trim();
                !trimmed.is_empty() && !trimmed.starts_with('#')
            })
            .map(|s| s.trim().to_string())
            .collect();

        Ok(slugs)
    }

    async fn fetch_installer(&self, slug: &str) -> Result<LutrisInstaller, ArcError> {
        let url = format!("{}/{}", LUTRIS_API_BASE, slug);
        let response =
            self.http_client.get(&url).send().await.map_err(|e| {
                ArcError::ProviderError(format!("Installer fetch for {}: {}", slug, e))
            })?;

        if !response.status().is_success() {
            return Err(ArcError::ProviderError(format!(
                "Installer API returned {} for {}",
                response.status(),
                slug
            )));
        }

        let wrapper: LutrisInstallerResponse = response
            .json()
            .await
            .map_err(|e| ArcError::ProviderError(format!("Installer parse for {}: {}", slug, e)))?;

        wrapper
            .results
            .into_iter()
            .next()
            .ok_or_else(|| ArcError::ProviderError(format!("No installer found for {}", slug)))
    }

    async fn fetch_catalog(&self) -> Result<Vec<CatalogEntry>, ArcError> {
        {
            let cache = self.catalog_cache.read().await;
            if let Some((t, entries)) = cache.as_ref() {
                if t.elapsed() < CATALOG_CACHE_TTL {
                    return Ok(entries.to_vec());
                }
            }
        }

        info!("Fetching Lutris whitelist");
        let slugs = self.fetch_whitelist().await?;

        let mut entries = Vec::new();
        for slug in slugs {
            match self.fetch_installer(&slug).await {
                Ok(installer) => {
                    entries.push((slug.clone(), installer));
                }
                Err(e) => {
                    warn!("Failed to fetch installer for {}: {}", slug, e);
                }
            }
        }

        {
            let mut cache = self.catalog_cache.write().await;
            *cache = Some((Instant::now(), entries.to_vec()));
        }

        Ok(entries)
    }

    async fn installed_slugs(&self) -> HashSet<String> {
        let mut out = HashSet::new();
        let Ok(mut dir) = fs::read_dir(&self.installs_dir).await else {
            return out;
        };
        while let Ok(Some(entry)) = dir.next_entry().await {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    out.insert(stem.to_string());
                }
            }
        }
        out
    }

    async fn ensure_lutris_installed(&self) -> Result<(), ArcError> {
        let already = Command::new("which")
            .arg("lutris")
            .output()
            .await
            .map_err(|e| ArcError::ProviderError(e.to_string()))?;

        if already.status.success() {
            return Ok(());
        }

        info!("Lutris not found — installing lutris");
        let status = Command::new("flatpak")
            .args([
                "install",
                "-y",
                "--noninteractive",
                "flathub",
                "net.lutris.Lutris",
            ])
            .output()
            .await
            .map_err(|e| ArcError::ProviderError(e.to_string()))?;

        if !status.status.success() {
            return Err(ArcError::ProviderError(
                "Failed to install net.lutris.Lutris from Flathub".to_string(),
            ));
        }

        Ok(())
    }

    async fn lutris_cmd(&self, args: &[&str]) -> Result<std::process::Output, ArcError> {
        if let Ok(out) = Command::new("lutris").args(args).output().await {
            return Ok(out);
        }
        Command::new("flatpak")
            .args(["run", "--command=lutris", "net.lutris.Lutris"])
            .args(args)
            .output()
            .await
            .map_err(|e| {
                ArcError::ProviderError(format!(
                    "lutris unavailable (install net.lutris.Lutris): {}",
                    e
                ))
            })
    }

    pub async fn fetch_all(&self) -> Result<Vec<Package>, ArcError> {
        let entries = self.fetch_catalog().await?;
        let installed = self.installed_slugs().await;

        Ok(entries
            .iter()
            .map(|(slug, installer)| {
                let name = if !installer.name.is_empty() {
                    installer.name.clone()
                } else {
                    slug.replace('-', " ").to_uppercase()
                };

                let description = if !installer.description.is_empty() {
                    installer.description.clone()
                } else if !installer.notes.is_empty() {
                    installer.notes.split('\n').next().unwrap_or("").to_string()
                } else if !installer.runner.is_empty() {
                    format!("Windows application via Lutris ({})", installer.runner)
                } else {
                    "Windows application via Lutris".to_string()
                };

                let icon_url = installer
                    .script
                    .as_ref()
                    .and_then(|s| s.metadata.as_ref())
                    .and_then(|m| {
                        if !m.icon_url.is_empty() {
                            Some(m.icon_url.clone())
                        } else if !m.cover_url.is_empty() {
                            Some(m.cover_url.clone())
                        } else if !m.banner_url.is_empty() {
                            Some(m.banner_url.clone())
                        } else {
                            None
                        }
                    });

                Package {
                    id: Self::pkg_id(slug),
                    name,
                    version: String::new(),
                    description,
                    provider: Provider::Lutris,
                    installed: installed.contains(slug),
                    icon_url,
                    remote: None,
                }
            })
            .collect())
    }
}

#[async_trait]
impl PackageProvider for LutrisProvider {
    async fn search(&self, query: &str) -> Result<Vec<Package>, ArcError> {
        let q = query.to_lowercase();
        let all = self.fetch_all().await?;
        Ok(all
            .into_iter()
            .filter(|p| {
                p.name.to_lowercase().contains(&q) || p.description.to_lowercase().contains(&q)
            })
            .collect())
    }

    async fn list_installed(&self) -> Result<Vec<Package>, ArcError> {
        let all = self.fetch_all().await?;
        Ok(all.into_iter().filter(|p| p.installed).collect())
    }

    async fn list_updates(&self) -> Result<Vec<Package>, ArcError> {
        Ok(Vec::new())
    }

    async fn install(&self, package_id: &str) -> Result<(), ArcError> {
        let slug = package_id.strip_prefix("lutris:").ok_or_else(|| {
            ArcError::ProviderError(format!("Invalid lutris package id: {}", package_id))
        })?;

        self.ensure_lutris_installed().await?;

        info!("Installing Lutris installer: {}", slug);

        // Use lutris to install the game/application
        // lutris -i <installer_url> or lutris lutris:<slug>
        let out = self.lutris_cmd(&[&format!("lutris:{}", slug)]).await?;

        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            warn!("lutris install exited non-zero: {}", stderr);
        }

        // Write arc metadata to track installation
        fs::create_dir_all(&self.installs_dir)
            .await
            .map_err(|e| ArcError::ProviderError(e.to_string()))?;

        let entries = self.fetch_catalog().await?;
        let installer_name = entries
            .iter()
            .find(|(s, _)| s == slug)
            .map(|(_, i)| i.name.clone())
            .unwrap_or_else(|| slug.to_string());

        fs::write(
            self.info_path(slug),
            serde_json::json!({ "name": installer_name, "slug": slug }).to_string(),
        )
        .await
        .map_err(|e| ArcError::ProviderError(e.to_string()))?;

        Ok(())
    }

    async fn remove(&self, package_id: &str) -> Result<(), ArcError> {
        let slug = package_id.strip_prefix("lutris:").ok_or_else(|| {
            ArcError::ProviderError(format!("Invalid lutris package id: {}", package_id))
        })?;

        info!("Removing Lutris application: {}", slug);

        // Remove metadata file
        let _ = fs::remove_file(self.info_path(slug)).await;

        // Note: Lutris doesn't have a clean uninstall command, so we just remove
        // our tracking. The user would need to manually remove the game from Lutris.
        Ok(())
    }

    async fn update(&self, _package_id: &str) -> Result<(), ArcError> {
        Err(ArcError::ProviderError(
            "Updates are managed through Lutris directly".to_string(),
        ))
    }

    async fn run(&self, package_id: &str) -> Result<(), ArcError> {
        let slug = package_id.strip_prefix("lutris:").ok_or_else(|| {
            ArcError::ProviderError(format!("Invalid lutris package id: {}", package_id))
        })?;

        // Read metadata to verify installation
        let info_path = self.info_path(slug);
        if !fs::metadata(&info_path).await.is_ok() {
            return Err(ArcError::PackageNotFound(format!(
                "{} is not installed",
                package_id
            )));
        }

        info!("Running Lutris application: {}", slug);

        // Launch via lutris:<slug> URI scheme
        let out = self.lutris_cmd(&[&format!("lutris:{}", slug)]).await?;

        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            if !stderr.is_empty() {
                warn!("lutris run exited non-zero: {}", stderr);
            }
        }

        Ok(())
    }

    async fn search_category(&self, _category: &str) -> Result<Vec<Package>, ArcError> {
        // Lutris doesn't have standard categories, return empty
        Ok(Vec::new())
    }

    async fn get_app_info(&self, package_id: &str) -> Result<Option<Package>, ArcError> {
        let slug = package_id.strip_prefix("lutris:").ok_or_else(|| {
            ArcError::ProviderError(format!("Invalid lutris package id: {}", package_id))
        })?;

        // Check if we have metadata for this slug
        let info_path = self.info_path(slug);
        if !std::path::Path::new(&info_path).exists() {
            return Ok(None);
        }

        // Try to get info from catalog
        let entries = self.fetch_catalog().await?;
        if let Some((_, installer)) = entries.iter().find(|(s, _)| s == slug) {
            let installed = std::path::Path::new(&self.info_path(slug)).exists();
            let icon_url = installer
                .script
                .as_ref()
                .and_then(|s| s.metadata.as_ref())
                .and_then(|m| {
                    if !m.icon_url.is_empty() {
                        Some(m.icon_url.clone())
                    } else if !m.cover_url.is_empty() {
                        Some(m.cover_url.clone())
                    } else if !m.banner_url.is_empty() {
                        Some(m.banner_url.clone())
                    } else {
                        None
                    }
                });
            Ok(Some(Package {
                id: Self::pkg_id(slug),
                name: installer.name.clone(),
                version: String::new(),
                description: installer.description.clone(),
                provider: Provider::Lutris,
                installed,
                icon_url,
                remote: None,
            }))
        } else {
            Ok(None)
        }
    }
}
