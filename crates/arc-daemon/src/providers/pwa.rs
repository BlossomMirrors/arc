use super::PackageProvider;
use async_trait::async_trait;
use libarc::{ArcError, Package, Provider};
use reqwest::Client;
use std::path::PathBuf;
use tracing::{info, warn};

const FORGE_PWAS_BASE: &str = "https://forge.blossomos.org/api/pwas";

fn detect_lang() -> String {
    for var in ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(val) = std::env::var(var) {
            let lang = val.split(['.', '_', '-']).next().unwrap_or("").to_string();
            if !lang.is_empty() && lang != "C" && lang != "POSIX" {
                return lang;
            }
        }
    }
    "en".to_string()
}

#[derive(serde::Deserialize, Clone)]
#[allow(dead_code)]
struct ForgePwa {
    appid: String,
    name: String,
    summary: String,
    #[serde(default)]
    description: String,
    icon_url: Option<String>,
    #[serde(default)]
    screenshots: Vec<String>,
    url: String,
    homepage_url: Option<String>,
    developer_name: Option<String>,
    content_rating: Option<String>,
    #[serde(default)]
    verified: bool,
    #[serde(default)]
    color: String,
    #[serde(default)]
    css: String,
    #[serde(default)]
    js: String,
    #[serde(default)]
    useragent: String,
    #[serde(default)]
    widevine: bool,
    #[serde(default)]
    tray: bool,
}

pub struct PwaProvider {
    desktop_dir: PathBuf,
    icons_dir: PathBuf,
    http: Client,
}

impl PwaProvider {
    pub fn new() -> Self {
        let home = PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/root".to_string()));
        Self {
            desktop_dir: home.join(".local/share/applications"),
            icons_dir: home.join(".local/share/icons/hicolor/256x256/apps"),
            http: Client::new(),
        }
    }

    fn strip_prefix(id: &str) -> &str {
        id.strip_prefix("pwa:").unwrap_or(id)
    }

    fn desktop_path(&self, appid: &str) -> PathBuf {
        self.desktop_dir.join(format!("arc-pwa-{}.desktop", appid))
    }

    fn is_installed(&self, appid: &str) -> bool {
        self.desktop_path(appid).exists()
    }

    async fn fetch_pwas(&self) -> Vec<ForgePwa> {
        let url = format!("{}?lang={}", FORGE_PWAS_BASE, detect_lang());
        match self
            .http
            .get(&url)
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await
        {
            Ok(r) => r.json::<Vec<ForgePwa>>().await.unwrap_or_default(),
            Err(e) => {
                warn!("Failed to fetch PWA list: {}", e);
                vec![]
            }
        }
    }

    fn to_package(&self, pwa: &ForgePwa) -> Package {
        Package {
            id: format!("pwa:{}", pwa.appid),
            name: pwa.name.clone(),
            version: String::new(),
            description: pwa.summary.clone(),
            provider: Provider::Pwa,
            installed: self.is_installed(&pwa.appid),
            icon_url: pwa.icon_url.clone(),
            remote: None,
            screenshots: pwa.screenshots.clone(),
            developer_name: pwa.developer_name.clone(),
            homepage_url: pwa.homepage_url.clone(),
            content_rating: pwa.content_rating.clone(),
        }
    }

    async fn download_icon(&self, appid: &str, icon_url: &str) -> (String, String) {
        let ext = if icon_url.contains(".svg") {
            "svg"
        } else {
            "png"
        };
        let icon_name = format!("arc-pwa-{}", appid);
        let icon_path = self.icons_dir.join(format!("{}.{}", icon_name, ext));
        let icon_path_str = icon_path.to_string_lossy().to_string();

        if let Err(e) = std::fs::create_dir_all(&self.icons_dir) {
            warn!("Could not create icons dir: {}", e);
            return (icon_name, icon_path_str);
        }

        match self
            .http
            .get(icon_url)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
        {
            Ok(r) => match r.bytes().await {
                Ok(bytes) => {
                    if let Err(e) = std::fs::write(&icon_path, &bytes) {
                        warn!("Could not write icon: {}", e);
                    }
                }
                Err(e) => warn!("Could not read icon bytes: {}", e),
            },
            Err(e) => warn!("Could not download icon: {}", e),
        }

        (icon_name, icon_path_str)
    }

    fn build_exec(&self, pwa: &ForgePwa) -> String {
        let mut exec = format!(
            "blossomos-webapps -- --url={url} --name={name} --appid={appid}",
            url = pwa.url,
            name = pwa.name,
            appid = pwa.appid,
        );
        if !pwa.color.is_empty() && pwa.color != "#000000" {
            exec.push_str(&format!(" --color={}", pwa.color));
        }
        if let Some(ref icon_url) = pwa.icon_url {
            exec.push_str(&format!(" --icon={}", icon_url));
        }
        if !pwa.css.is_empty() {
            exec.push_str(&format!(" --css={}", pwa.css));
        }
        if !pwa.js.is_empty() {
            exec.push_str(&format!(" --js={}", pwa.js));
        }
        if !pwa.useragent.is_empty() {
            exec.push_str(&format!(" --useragent=\"{}\"", pwa.useragent));
        }
        if pwa.widevine {
            exec.push_str(" --widevine");
        }
        if pwa.tray {
            exec.push_str(" --tray");
        }
        exec
    }

    fn write_desktop(&self, pwa: &ForgePwa, icon_name: &str) -> Result<(), ArcError> {
        std::fs::create_dir_all(&self.desktop_dir)?;

        let comment = pwa.summary.replace('\n', " ");
        let exec = self.build_exec(pwa);
        let content = format!(
            "[Desktop Entry]\nVersion=1.0\nType=Application\nName={name}\nComment={comment}\nExec={exec}\nIcon={icon}\nCategories=Network;WebApplication;\nStartupNotify=true\n",
            name = pwa.name,
            comment = comment,
            exec = exec,
            icon = icon_name,
        );

        let path = self.desktop_path(&pwa.appid);
        std::fs::write(&path, &content)?;

        // Mark executable so desktop environments treat it as trusted.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = std::fs::metadata(&path) {
                let mut perms = meta.permissions();
                perms.set_mode(0o755);
                let _ = std::fs::set_permissions(&path, perms);
            }
        }

        Ok(())
    }

    pub async fn get_metadata_json(&self, package_id: &str) -> String {
        let appid = Self::strip_prefix(package_id);
        let pwas = self.fetch_pwas().await;
        let Some(pwa) = pwas.iter().find(|p| p.appid == appid) else {
            return "null".to_string();
        };
        serde_json::json!({
            "summary": pwa.summary,
            "description": pwa.description,
            "license": null,
            "eula_url": null,
            "homepage_url": pwa.homepage_url,
            "content_rating": pwa.content_rating.as_deref().unwrap_or("All ages"),
            "developer_name": pwa.developer_name,
        })
        .to_string()
    }
}

#[async_trait]
impl PackageProvider for PwaProvider {
    async fn search(&self, query: &str) -> Result<Vec<Package>, ArcError> {
        let pwas = self.fetch_pwas().await;
        let packages: Vec<Package> = pwas.iter().map(|p| self.to_package(p)).collect();
        Ok(libarc::search_and_rank(packages, query))
    }

    async fn search_category(&self, _category: &str) -> Result<Vec<Package>, ArcError> {
        Ok(vec![])
    }

    async fn get_app_info(&self, package_id: &str) -> Result<Option<Package>, ArcError> {
        let appid = Self::strip_prefix(package_id);
        let pwas = self.fetch_pwas().await;
        Ok(pwas
            .iter()
            .find(|p| p.appid == appid)
            .map(|p| self.to_package(p)))
    }

    async fn list_installed(&self) -> Result<Vec<Package>, ArcError> {
        let entries = match std::fs::read_dir(&self.desktop_dir) {
            Ok(e) => e,
            Err(_) => return Ok(vec![]),
        };

        let installed_appids: Vec<String> = entries
            .flatten()
            .filter_map(|e| {
                let name = e.file_name();
                let s = name.to_string_lossy();
                if s.starts_with("arc-pwa-") && s.ends_with(".desktop") {
                    let appid = s
                        .strip_prefix("arc-pwa-")?
                        .strip_suffix(".desktop")?
                        .to_string();
                    if !appid.is_empty() {
                        Some(appid)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        if installed_appids.is_empty() {
            return Ok(vec![]);
        }

        let pwas = self.fetch_pwas().await;
        let packages = installed_appids
            .iter()
            .map(|appid| {
                pwas.iter()
                    .find(|p| &p.appid == appid)
                    .map(|p| self.to_package(p))
                    .unwrap_or_else(|| Package {
                        id: format!("pwa:{}", appid),
                        name: appid.clone(),
                        version: String::new(),
                        description: String::new(),
                        provider: Provider::Pwa,
                        installed: true,
                        icon_url: None,
                        remote: None,
                        screenshots: vec![],
                        developer_name: None,
                        homepage_url: None,
                        content_rating: None,
                    })
            })
            .collect();

        Ok(packages)
    }

    async fn install(&self, package_id: &str) -> Result<(), ArcError> {
        let appid = Self::strip_prefix(package_id);
        info!("Installing PWA: {}", appid);

        let pwas = self.fetch_pwas().await;
        let pwa = pwas
            .iter()
            .find(|p| p.appid == appid)
            .ok_or_else(|| ArcError::PackageNotFound(appid.to_string()))?;

        let icon_name = if let Some(ref url) = pwa.icon_url {
            let (name, _) = self.download_icon(appid, url).await;
            name
        } else {
            format!("arc-pwa-{}", appid)
        };

        self.write_desktop(pwa, &icon_name)
    }

    async fn remove(&self, package_id: &str) -> Result<(), ArcError> {
        let appid = Self::strip_prefix(package_id);
        info!("Removing PWA: {}", appid);

        let desktop = self.desktop_path(appid);
        if desktop.exists() {
            std::fs::remove_file(&desktop)?;
        }

        for ext in ["png", "svg", "webp"] {
            let icon = self.icons_dir.join(format!("arc-pwa-{}.{}", appid, ext));
            if icon.exists() {
                let _ = std::fs::remove_file(&icon);
            }
        }

        Ok(())
    }

    async fn list_updates(&self) -> Result<Vec<Package>, ArcError> {
        Ok(vec![])
    }

    async fn update(&self, _package_id: &str) -> Result<(), ArcError> {
        Ok(())
    }

    async fn run(&self, package_id: &str) -> Result<(), ArcError> {
        let appid = Self::strip_prefix(package_id);
        let pwas = self.fetch_pwas().await;

        let Some(pwa) = pwas.iter().find(|p| p.appid == appid) else {
            return Err(ArcError::PackageNotFound(appid.to_string()));
        };

        let exec = self.build_exec(pwa);

        info!("Running PWA {}: {}", appid, exec);
        let mut parts = exec.split_whitespace();
        let bin = parts.next().unwrap_or("blossomos-webapps");
        tokio::process::Command::new(bin)
            .args(parts)
            .spawn()
            .map_err(|e| ArcError::ProviderError(e.to_string()))?;

        Ok(())
    }
}
