use super::PackageProvider;
use async_trait::async_trait;
use libarc::{ArcError, Package, Provider};
use serde::{Deserialize, Deserializer};
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::fs;
use tokio::process::Command;
use tokio::sync::RwLock;
use tracing::{info, warn};

fn null_as_empty<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Option::<String>::deserialize(d).map(|o| o.unwrap_or_default())
}

const WHITELIST_URL: &str = "https://repo.blossomos.org/lutris.txt";
const LUTRIS_GAMES_API: &str = "https://lutris.net/api/games";
const CATALOG_CACHE_TTL: Duration = Duration::from_secs(3600);

#[derive(Debug, Clone, Deserialize)]
struct LutrisGame {
    #[serde(default, deserialize_with = "null_as_empty")]
    name: String,
    #[serde(default, deserialize_with = "null_as_empty")]
    description: String,
    #[serde(default, deserialize_with = "null_as_empty")]
    coverart: String,
    #[serde(default, deserialize_with = "null_as_empty")]
    #[allow(dead_code)]
    banner: String,
    #[serde(default, deserialize_with = "null_as_empty")]
    icon_url: String,
}

// installer_slug, game metadata, scraped screenshot URLs
type CatalogEntry = (String, LutrisGame, Vec<String>);

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

    // Each non-comment line is either "game_slug:installer_slug" or just "installer_slug"
    // (where game_slug == installer_slug).
    async fn fetch_whitelist(&self) -> Result<Vec<(String, String)>, ArcError> {
        let text = self
            .http_client
            .get(WHITELIST_URL)
            .send()
            .await
            .map_err(|e| ArcError::ProviderError(format!("Whitelist fetch: {}", e)))?
            .text()
            .await
            .map_err(|e| ArcError::ProviderError(format!("Whitelist read: {}", e)))?;

        let entries = text
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    return None;
                }
                if let Some((game_slug, installer_slug)) = trimmed.split_once(':') {
                    Some((installer_slug.to_string(), game_slug.to_string()))
                } else {
                    Some((trimmed.to_string(), trimmed.to_string()))
                }
            })
            .collect();

        Ok(entries)
    }

    async fn fetch_game(&self, game_slug: &str) -> Result<LutrisGame, ArcError> {
        let url = format!("{}/{}", LUTRIS_GAMES_API, game_slug);
        let resp = self
            .http_client
            .get(&url)
            .send()
            .await
            .map_err(|e| ArcError::ProviderError(format!("Game fetch {}: {}", game_slug, e)))?;

        if !resp.status().is_success() {
            return Err(ArcError::ProviderError(format!(
                "Games API {} for {}",
                resp.status(),
                game_slug
            )));
        }

        resp.json::<LutrisGame>()
            .await
            .map_err(|e| ArcError::ProviderError(format!("Game parse {}: {}", game_slug, e)))
    }

    async fn scrape_screenshots(&self, game_slug: &str) -> Vec<String> {
        let url = format!("https://lutris.net/games/{}/", game_slug);
        let html = match self.http_client.get(&url).send().await {
            Ok(r) => match r.text().await {
                Ok(t) => t,
                Err(_) => return vec![],
            },
            Err(_) => return vec![],
        };

        let document = scraper::Html::parse_document(&html);
        // Screenshots live in a hidden #screenshots div as <a href="//lutris.net/..."> tags.
        // JavaScript reads these and feeds them into the blueimp carousel at runtime.
        let selector = match scraper::Selector::parse("#screenshots a") {
            Ok(s) => s,
            Err(_) => return vec![],
        };

        document
            .select(&selector)
            .filter_map(|el| {
                el.value().attr("href").map(|href| {
                    // protocol-relative URLs — make them absolute
                    if href.starts_with("//") {
                        format!("https:{}", href)
                    } else {
                        href.to_string()
                    }
                })
            })
            .take(5)
            .collect()
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

        info!("Fetching Lutris catalog");
        let whitelist = self.fetch_whitelist().await?;

        let mut entries = Vec::new();
        for (installer_slug, game_slug) in whitelist {
            match self.fetch_game(&game_slug).await {
                Ok(game) => {
                    let screenshots = self.scrape_screenshots(&game_slug).await;
                    entries.push((installer_slug, game, screenshots));
                }
                Err(e) => {
                    warn!("Failed to fetch game info for {}: {}", game_slug, e);
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

        info!("Lutris not found — installing via Flathub");
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

    fn game_to_package(
        &self,
        installer_slug: &str,
        game: &LutrisGame,
        screenshots: &[String],
        installed: bool,
    ) -> Package {
        let name = if !game.name.is_empty() {
            game.name.clone()
        } else {
            installer_slug.replace('-', " ")
        };

        let icon_url = if !game.icon_url.is_empty() {
            Some(game.icon_url.clone())
        } else if !game.coverart.is_empty() {
            Some(game.coverart.clone())
        } else {
            None
        };

        Package {
            id: Self::pkg_id(installer_slug),
            name,
            version: String::new(),
            description: game.description.clone(),
            provider: Provider::Lutris,
            installed,
            icon_url,
            remote: None,
            screenshots: screenshots.to_vec(),
        }
    }

    pub async fn fetch_all(&self) -> Result<Vec<Package>, ArcError> {
        let entries = self.fetch_catalog().await?;
        let installed = self.installed_slugs().await;

        Ok(entries
            .iter()
            .map(|(slug, game, screenshots)| {
                self.game_to_package(slug, game, screenshots, installed.contains(slug))
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

        info!("Installing Lutris game: {}", slug);
        let out = self.lutris_cmd(&[&format!("lutris:{}", slug)]).await?;

        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            warn!("lutris install exited non-zero: {}", stderr);
        }

        fs::create_dir_all(&self.installs_dir)
            .await
            .map_err(|e| ArcError::ProviderError(e.to_string()))?;

        let entries = self.fetch_catalog().await?;
        let name = entries
            .iter()
            .find(|(s, _, _)| s == slug)
            .map(|(_, g, _)| g.name.clone())
            .unwrap_or_else(|| slug.to_string());

        fs::write(
            self.info_path(slug),
            serde_json::json!({ "name": name, "slug": slug }).to_string(),
        )
        .await
        .map_err(|e| ArcError::ProviderError(e.to_string()))?;

        Ok(())
    }

    async fn remove(&self, package_id: &str) -> Result<(), ArcError> {
        let slug = package_id.strip_prefix("lutris:").ok_or_else(|| {
            ArcError::ProviderError(format!("Invalid lutris package id: {}", package_id))
        })?;

        info!("Removing Lutris game: {}", slug);

        let out = self
            .lutris_cmd(&["--uninstall-game", slug])
            .await
            .map_err(|e| ArcError::ProviderError(format!("lutris uninstall: {}", e)))?;

        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            warn!("lutris --uninstall-game exited non-zero: {}", stderr);
        }

        let _ = fs::remove_file(self.info_path(slug)).await;

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

        if !self.info_path(slug).exists() {
            return Err(ArcError::PackageNotFound(format!(
                "{} is not installed",
                package_id
            )));
        }

        info!("Running Lutris game: {}", slug);
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
        Ok(Vec::new())
    }

    async fn get_app_info(&self, package_id: &str) -> Result<Option<Package>, ArcError> {
        let slug = package_id.strip_prefix("lutris:").ok_or_else(|| {
            ArcError::ProviderError(format!("Invalid lutris package id: {}", package_id))
        })?;

        let entries = self.fetch_catalog().await?;
        if let Some((_, game, screenshots)) = entries.iter().find(|(s, _, _)| s == slug) {
            let installed = self.info_path(slug).exists();
            Ok(Some(self.game_to_package(slug, game, screenshots, installed)))
        } else {
            Ok(None)
        }
    }
}
