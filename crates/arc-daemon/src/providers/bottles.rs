use super::PackageProvider;
use async_trait::async_trait;
use libarc::{ArcError, Package, Provider};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::fs;
use tokio::process::Command;
use tokio::sync::RwLock;
use tracing::info;

const INDEX_URL: &str =
    "https://raw.githubusercontent.com/bottlesdevs/programs/main/index.yml";
const CATALOG_CACHE_TTL: Duration = Duration::from_secs(3600);
const DEFAULT_BOTTLE: &str = "arc-programs";

// Index keys of apps already available as native Linux Flatpaks.
// Only entries that actually appear in the index need to be listed here.
const BLACKLIST: &[&str] = &["steam"];

#[derive(Debug, Clone, Deserialize)]
struct BottlesEntry {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Description", default)]
    description: String,
    #[serde(rename = "Category", default)]
    category: String,
}

// The index is a YAML map: { slug_key: { Name, Description, Category, ... } }
type BottlesIndex = HashMap<String, BottlesEntry>;

// What we store in the cache: (key, entry) pairs in stable sorted order.
type CatalogEntry = (String, BottlesEntry);

pub struct BottlesProvider {
    catalog_cache: RwLock<Option<(Instant, Vec<CatalogEntry>)>>,
    installs_dir: PathBuf,
    http_client: reqwest::Client,
}

impl BottlesProvider {
    pub fn new() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
        Self {
            catalog_cache: RwLock::new(None),
            installs_dir: PathBuf::from(home).join(".local/share/arc/bottles-packages"),
            http_client: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("reqwest client"),
        }
    }

    pub fn pkg_id(key: &str) -> String {
        format!("bottles:{}", key)
    }

    async fn fetch_catalog(&self) -> Result<Vec<CatalogEntry>, ArcError> {
        {
            let cache = self.catalog_cache.read().await;
            if let Some((t, entries)) = cache.as_ref() {
                if t.elapsed() < CATALOG_CACHE_TTL {
                    return Ok(entries.clone());
                }
            }
        }

        info!("Fetching Bottles programs index");
        let text = self
            .http_client
            .get(INDEX_URL)
            .send()
            .await
            .map_err(|e| ArcError::ProviderError(format!("Bottles index fetch: {}", e)))?
            .text()
            .await
            .map_err(|e| ArcError::ProviderError(format!("Bottles index read: {}", e)))?;

        let index: BottlesIndex = serde_yaml::from_str(&text)
            .map_err(|e| ArcError::ProviderError(format!("Bottles index parse: {}", e)))?;

        let mut entries: Vec<CatalogEntry> = index
            .into_iter()
            .filter(|(key, _)| !BLACKLIST.contains(&key.as_str()))
            .collect();

        // Stable ordering so the UI list doesn't jump around between refreshes.
        entries.sort_by(|(a, _), (b, _)| a.cmp(b));

        {
            let mut cache = self.catalog_cache.write().await;
            *cache = Some((Instant::now(), entries.clone()));
        }

        Ok(entries)
    }

    fn info_path(&self, key: &str) -> PathBuf {
        self.installs_dir.join(format!("{}.json", key))
    }

    async fn installed_keys(&self) -> HashSet<String> {
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

    // Try bottles-cli on PATH first; fall back to the Flatpak-bundled binary.
    async fn bottles_cmd(&self, args: &[&str]) -> Result<std::process::Output, ArcError> {
        if let Ok(out) = Command::new("bottles-cli").args(args).output().await {
            return Ok(out);
        }
        Command::new("flatpak")
            .args(["run", "--command=bottles-cli", "com.usebottles.bottles"])
            .args(args)
            .output()
            .await
            .map_err(|e| {
                ArcError::ProviderError(format!(
                    "bottles-cli not found (install com.usebottles.bottles): {}",
                    e
                ))
            })
    }

    pub async fn fetch_all(&self) -> Result<Vec<Package>, ArcError> {
        let entries = self.fetch_catalog().await?;
        let installed = self.installed_keys().await;
        Ok(entries
            .iter()
            .map(|(key, e)| {
                let desc = if !e.description.is_empty() {
                    e.description.clone()
                } else if !e.category.is_empty() {
                    format!("Windows application — {}", e.category)
                } else {
                    "Windows application via Bottles".to_string()
                };
                Package {
                    id: Self::pkg_id(key),
                    name: e.name.clone(),
                    version: String::new(),
                    description: desc,
                    provider: Provider::Bottles,
                    installed: installed.contains(key),
                }
            })
            .collect())
    }
}

#[async_trait]
impl PackageProvider for BottlesProvider {
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
        let key = package_id.strip_prefix("bottles:").ok_or_else(|| {
            ArcError::ProviderError(format!("Invalid bottles package id: {}", package_id))
        })?;

        let entries = self.fetch_catalog().await?;
        let (_, entry) = entries
            .iter()
            .find(|(k, _)| k == key)
            .ok_or_else(|| ArcError::PackageNotFound(key.to_string()))?;

        // Ensure the default bottle exists; ignore failure (may already exist).
        let _ = self
            .bottles_cmd(&[
                "new",
                "--bottle-name",
                DEFAULT_BOTTLE,
                "--environment",
                "application",
            ])
            .await;

        info!("Installing '{}' via Bottles (bottle: {})", entry.name, DEFAULT_BOTTLE);
        let out = self
            .bottles_cmd(&["install", "-b", DEFAULT_BOTTLE, "-l", key])
            .await?;

        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            return Err(ArcError::ProviderError(format!(
                "Bottles installation of '{}' failed: {}",
                entry.name, stderr
            )));
        }

        fs::create_dir_all(&self.installs_dir)
            .await
            .map_err(|e| ArcError::ProviderError(e.to_string()))?;
        fs::write(
            self.info_path(key),
            serde_json::json!({ "name": entry.name, "bottle": DEFAULT_BOTTLE }).to_string(),
        )
        .await
        .map_err(|e| ArcError::ProviderError(e.to_string()))?;

        Ok(())
    }

    async fn remove(&self, package_id: &str) -> Result<(), ArcError> {
        let key = package_id.strip_prefix("bottles:").ok_or_else(|| {
            ArcError::ProviderError(format!("Invalid bottles package id: {}", package_id))
        })?;

        let entries = self.fetch_catalog().await?;
        let (_, entry) = entries
            .iter()
            .find(|(k, _)| k == key)
            .ok_or_else(|| ArcError::PackageNotFound(key.to_string()))?;

        info!("Removing '{}' via Bottles", entry.name);
        let out = self
            .bottles_cmd(&["uninstall", "-b", DEFAULT_BOTTLE, "-p", key])
            .await?;

        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            return Err(ArcError::ProviderError(format!(
                "Bottles removal of '{}' failed: {}",
                entry.name, stderr
            )));
        }

        let _ = fs::remove_file(self.info_path(key)).await;
        Ok(())
    }

    async fn update(&self, _package_id: &str) -> Result<(), ArcError> {
        Err(ArcError::ProviderError(
            "Updates are not supported for Bottles packages".to_string(),
        ))
    }
}
