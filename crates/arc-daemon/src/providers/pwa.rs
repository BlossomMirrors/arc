use super::PackageProvider;
use async_trait::async_trait;
use libarc::desktop_entry::{quote_exec_arg, DesktopEntry};
use libarc::{ArcError, Package, Provider};
use reqwest::Client;
use std::env;
use std::fs;
use std::path::PathBuf;
use tracing::{info, warn};

fn detect_lang() -> String {
    for var in ["LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(val) = env::var(var) {
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
    #[serde(default)]
    url_filter: String,
}

pub struct PwaProvider {
    desktop_dir: PathBuf,
    hicolor_dir: PathBuf,
    http: Client,
}

impl PwaProvider {
    pub fn new() -> Self {
        let home = PathBuf::from(env::var("HOME").unwrap_or_else(|_| "/root".to_string()));
        Self {
            desktop_dir: home.join(".local/share/applications"),
            hicolor_dir: home.join(".local/share/icons/hicolor"),
            http: Client::new(),
        }
    }

    fn icon_theme_path(&self, appid: &str, ext: &str) -> PathBuf {
        let size_dir = if ext == "svg" || ext == "svgz" { "scalable" } else { "256x256" };
        self.hicolor_dir.join(size_dir).join("apps").join(format!("arc-pwa-{}.{}", appid, ext))
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
        let url = format!("{}/api/pwas?lang={}", libarc::FORGE_BASE_URL, detect_lang());
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
            is_runtime: false,
            categories: vec![],
        }
    }

    async fn download_icon(&self, appid: &str, icon_url: &str) -> String {
        let is_svg = icon_url.contains(".svg");
        let icon_name = format!("arc-pwa-{}", appid);

        match crate::media::fetch_raw(icon_url).await {
            Some((bytes, _content_type)) => {
                if libarc::icons::install_icon(&icon_name, &bytes, is_svg).is_none() {
                    warn!("Could not write icon for {}", appid);
                }
            }
            None => warn!("Could not download icon for {}", appid),
        }

        icon_name
    }

    /// Return a file:// URI for the locally cached icon, if one has already
    /// been downloaded for this appid.
    fn local_icon_uri(&self, appid: &str) -> Option<String> {
        for ext in ["png", "svg", "webp"] {
            let path = self.icon_theme_path(appid, ext);
            if path.exists() {
                return Some(format!("file://{}", path.to_string_lossy()));
            }
        }
        None
    }

    fn build_exec(&self, pwa: &ForgePwa) -> String {
        let mut exec = format!(
            "blossomos-webapps -- --url={url} --name={name} --appid={appid}",
            url = quote_exec_arg(&pwa.url),
            name = quote_exec_arg(&pwa.name),
            appid = quote_exec_arg(&pwa.appid),
        );
        if !pwa.color.is_empty() && pwa.color != "#000000" {
            exec.push_str(&format!(" --color={}", quote_exec_arg(&pwa.color)));
        }
        if let Some(icon_uri) = self.local_icon_uri(&pwa.appid) {
            exec.push_str(&format!(" --icon={}", quote_exec_arg(&icon_uri)));
        } else if let Some(ref icon_url) = pwa.icon_url {
            exec.push_str(&format!(" --icon={}", quote_exec_arg(icon_url)));
        }
        if !pwa.css.is_empty() {
            exec.push_str(&format!(" --css={}", quote_exec_arg(&pwa.css)));
        }
        if !pwa.js.is_empty() {
            exec.push_str(&format!(" --js={}", quote_exec_arg(&pwa.js)));
        }
        if !pwa.useragent.is_empty() {
            exec.push_str(&format!(" --useragent={}", quote_exec_arg(&pwa.useragent)));
        }
        if pwa.widevine {
            exec.push_str(" --widevine");
        }
        if pwa.tray {
            exec.push_str(" --tray");
        }
        if !pwa.url_filter.is_empty() {
            exec.push_str(&format!(
                " --url-filter={}",
                quote_exec_arg(&pwa.url_filter)
            ));
        }
        exec
    }

    fn write_desktop(&self, pwa: &ForgePwa, icon_name: &str) -> Result<(), ArcError> {
        fs::create_dir_all(&self.desktop_dir)?;

        let entry = DesktopEntry {
            name: pwa.name.clone(),
            comment: pwa.summary.clone(),
            exec: self.build_exec(pwa),
            icon: icon_name.to_string(),
            categories: "Network;WebApplication;".to_string(),
            start_notify: true,
            startup_wm_class: Some(pwa.appid.clone()),
            ..Default::default()
        };
        entry.write(&self.desktop_path(&pwa.appid))?;

        Ok(())
    }

    pub async fn get_metadata_json(&self, package_id: &str) -> String {
        let appid = Self::strip_prefix(package_id);
        let pwas = self.fetch_pwas().await;
        let Some(pwa) = pwas.iter().find(|p| p.appid == appid) else {
            return "null".to_string();
        };
        let screenshots: Vec<String> = pwa.screenshots.iter().map(|url| crate::media::image_proxy_url(url)).collect();
        let mut icon_url = pwa.icon_url.clone();
        crate::media::proxy_icon_url_field(&mut icon_url, package_id);
        serde_json::json!({
            "summary": pwa.summary,
            "description": pwa.description,
            "license": null,
            "eula_url": null,
            "homepage_url": pwa.homepage_url,
            "content_rating": pwa.content_rating.as_deref().unwrap_or("All ages"),
            "developer_name": pwa.developer_name,
            "screenshots": screenshots,
            "icon_url": icon_url,
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
        let entries = match fs::read_dir(&self.desktop_dir) {
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
                        is_runtime: false,
                        categories: vec![],
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
            self.download_icon(appid, url).await
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
            fs::remove_file(&desktop)?;
        }

        for ext in ["png", "svg", "webp"] {
            let icon = self.icon_theme_path(appid, ext);
            if icon.exists() {
                let _ = fs::remove_file(&icon);
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
        if !self.is_installed(appid) {
            return Err(ArcError::PackageNotFound(appid.to_string()));
        }

        let desktop_id = format!("arc-pwa-{}", appid);
        info!("Launching PWA {} via {}", appid, desktop_id);
        tokio::process::Command::new("gtk-launch")
            .arg(&desktop_id)
            .spawn()
            .map_err(|e| ArcError::ProviderError(format!("gtk-launch: {}", e)))?;

        Ok(())
    }
}
