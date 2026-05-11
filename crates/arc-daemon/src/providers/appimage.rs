use super::PackageProvider;
use async_trait::async_trait;
use libarc::{ArcError, Package, Provider};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::process::Command;
use tracing::{info, warn};
use uuid::Uuid;

pub struct AppImageProvider {
    pub appimages_dir: PathBuf,
    desktop_dir: PathBuf,
    data_dir: PathBuf,
}

struct AppImageMeta {
    name: String,
    version: String,
    description: String,
    categories: String,
    update_info: String,
    icon_path: Option<PathBuf>,
}

impl AppImageProvider {
    pub fn new() -> Self {
        let home = PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| "/root".to_string()));
        Self {
            appimages_dir: home.join(".appimages"),
            desktop_dir: home.join(".local/share/applications"),
            data_dir: home.join(".local/share/arc/appimages"),
        }
    }

    fn info_file(&self, stem: &str) -> PathBuf {
        self.data_dir.join(format!("{}.info", stem))
    }

    fn desktop_file(&self, stem: &str) -> PathBuf {
        self.desktop_dir
            .join(format!("arc-appimage-{}.desktop", stem))
    }

    fn icon_file(&self, stem: &str) -> PathBuf {
        self.data_dir.join("icons").join(format!("{}.png", stem))
    }

    fn appimage_path(&self, stem: &str) -> PathBuf {
        self.appimages_dir.join(format!("{}.AppImage", stem))
    }

    fn stem_from_path(path: &Path) -> Option<String> {
        let name = path.file_name()?.to_str()?;
        name.strip_suffix(".AppImage").map(|s| s.to_string())
    }

    async fn extract_metadata(&self, path: &Path) -> AppImageMeta {
        let extract_dir = std::env::temp_dir()
            .join(format!("arc-appimage-{}", Uuid::new_v4().simple()));

        if fs::create_dir_all(&extract_dir).await.is_err() {
            return AppImageMeta::default_from_path(path);
        }

        // set executable before running
        let _ = Command::new("chmod")
            .args(["+x", path.to_str().unwrap_or("")])
            .status()
            .await;

        let extract_ok = Command::new(path)
            .current_dir(&extract_dir)
            .arg("--appimage-extract")
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !extract_ok {
            let _ = fs::remove_dir_all(&extract_dir).await;
            return AppImageMeta::default_from_path(path);
        }

        let squashfs = extract_dir.join("squashfs-root");
        let meta = Self::read_squashfs_meta(&squashfs, path).await;
        let _ = fs::remove_dir_all(&extract_dir).await;
        meta
    }

    async fn read_squashfs_meta(squashfs: &Path, appimage_path: &Path) -> AppImageMeta {
        // find the .desktop file: first at squashfs root, then in usr/share/applications/
        let desktop_content = Self::find_desktop_content(squashfs).await;
        let mut meta = if let Some(ref content) = desktop_content {
            parse_desktop(content)
        } else {
            AppImageMeta::default_from_path(appimage_path)
        };

        // look for appstream metadata for better description
        if let Some(appstream) = Self::find_appstream(squashfs).await {
            if meta.description.is_empty() {
                meta.description = appstream;
            }
        }

        // extract icon: prefer root-level PNG (AppImage convention), then hicolor
        meta.icon_path = Self::find_icon(squashfs).await;

        meta
    }

    async fn find_desktop_content(squashfs: &Path) -> Option<String> {
        // check root level first (AppImage convention)
        if let Ok(mut entries) = fs::read_dir(squashfs).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let p = entry.path();
                if p.extension().and_then(|e| e.to_str()) == Some("desktop") {
                    return fs::read_to_string(&p).await.ok();
                }
            }
        }
        // fallback: usr/share/applications/
        let apps_dir = squashfs.join("usr/share/applications");
        if let Ok(mut entries) = fs::read_dir(&apps_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let p = entry.path();
                if p.extension().and_then(|e| e.to_str()) == Some("desktop") {
                    return fs::read_to_string(&p).await.ok();
                }
            }
        }
        None
    }

    async fn find_appstream(squashfs: &Path) -> Option<String> {
        for dir in &["usr/share/metainfo", "usr/share/appdata"] {
            let d = squashfs.join(dir);
            if let Ok(mut entries) = fs::read_dir(&d).await {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    let p = entry.path();
                    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
                    if ext == "xml" || ext == "appdata.xml" || ext == "metainfo.xml" {
                        if let Ok(content) = fs::read_to_string(&p).await {
                            return extract_appstream_summary(&content);
                        }
                    }
                }
            }
        }
        None
    }

    async fn find_icon(squashfs: &Path) -> Option<PathBuf> {
        // root-level PNG (AppImage convention)
        if let Ok(mut entries) = fs::read_dir(squashfs).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let p = entry.path();
                if p.extension().and_then(|e| e.to_str()) == Some("png") {
                    return Some(p);
                }
            }
        }
        // hicolor 256x256 or 128x128
        for size in &["256x256", "128x128", "64x64"] {
            let icon_dir = squashfs
                .join("usr/share/icons/hicolor")
                .join(size)
                .join("apps");
            if let Ok(mut entries) = fs::read_dir(&icon_dir).await {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    let p = entry.path();
                    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
                    if ext == "png" || ext == "svg" {
                        return Some(p);
                    }
                }
            }
        }
        None
    }

    async fn process_appimage(&self, path: &Path, stem: &str) -> Result<(), ArcError> {
        fs::create_dir_all(&self.appimages_dir)
            .await
            .map_err(|e| ArcError::ProviderError(e.to_string()))?;
        fs::create_dir_all(&self.data_dir)
            .await
            .map_err(|e| ArcError::ProviderError(e.to_string()))?;
        fs::create_dir_all(&self.desktop_dir)
            .await
            .map_err(|e| ArcError::ProviderError(e.to_string()))?;
        fs::create_dir_all(self.data_dir.join("icons"))
            .await
            .map_err(|e| ArcError::ProviderError(e.to_string()))?;

        // make executable
        let _ = Command::new("chmod")
            .args(["+x", path.to_str().unwrap_or("")])
            .status()
            .await;

        let meta = self.extract_metadata(path).await;

        // copy icon to stable location
        let icon_dest = self.icon_file(stem);
        if let Some(ref icon_src) = meta.icon_path {
            let _ = fs::copy(icon_src, &icon_dest).await;
        }

        // write .desktop file
        let icon_str = if icon_dest.exists() {
            icon_dest.to_string_lossy().to_string()
        } else {
            "application-x-executable".to_string()
        };
        let desktop = format!(
            "[Desktop Entry]\n\
             Type=Application\n\
             Name={}\n\
             Comment={}\n\
             Exec={}\n\
             Icon={}\n\
             Categories={}\n\
             Version={}\n\
             X-AppImage-Path={}\n\
             X-AppImage-Update-Information={}\n",
            meta.name,
            meta.description,
            path.display(),
            icon_str,
            if meta.categories.is_empty() { "Utility;" } else { &meta.categories },
            meta.version,
            path.display(),
            meta.update_info,
        );
        fs::write(self.desktop_file(stem), &desktop)
            .await
            .map_err(|e| ArcError::ProviderError(e.to_string()))?;

        // write .info file
        let info = format!(
            "STEM={}\nNAME={}\nVERSION={}\nDESCRIPTION={}\nPATH={}\nUPDATE_INFO={}\n",
            stem,
            meta.name,
            meta.version,
            meta.description,
            path.display(),
            meta.update_info,
        );
        fs::write(self.info_file(stem), &info)
            .await
            .map_err(|e| ArcError::ProviderError(e.to_string()))?;

        // refresh desktop db
        let _ = Command::new("update-desktop-database")
            .arg(self.desktop_dir.to_str().unwrap_or(""))
            .status()
            .await;

        info!("AppImage processed: {} ({})", meta.name, stem);
        Ok(())
    }

    /// Called on daemon start to ensure all AppImages in ~/.appimages have desktop entries,
    /// and all orphaned .info files (whose AppImage was removed) are cleaned up.
    pub async fn scan_and_sync(&self) {
        // create appimages dir if missing
        let _ = fs::create_dir_all(&self.appimages_dir).await;

        // find all .AppImage files
        let mut found_stems: Vec<String> = Vec::new();
        if let Ok(mut entries) = fs::read_dir(&self.appimages_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if let Some(stem) = Self::stem_from_path(&path) {
                    found_stems.push(stem.clone());
                    // if no .info → process it
                    if !self.info_file(&stem).exists() {
                        info!("New AppImage discovered: {}", stem);
                        if let Err(e) = self.process_appimage(&path, &stem).await {
                            warn!("Failed to process AppImage {}: {}", stem, e);
                        }
                    }
                }
            }
        }

        // cleanup orphaned .info files
        if let Ok(mut entries) = fs::read_dir(&self.data_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("info") {
                    continue;
                }
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if !found_stems.contains(&stem.to_string()) {
                        info!("Orphaned AppImage info, cleaning up: {}", stem);
                        self.cleanup_stem(stem).await;
                    }
                }
            }
        }
    }

    /// Spawn inotify watcher on ~/.appimages. Returns the watcher (must be kept alive).
    pub fn start_watcher(provider: std::sync::Arc<Self>) -> anyhow::Result<RecommendedWatcher> {
        let appimages_dir = provider.appimages_dir.clone();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<notify::Result<Event>>();

        let mut watcher = notify::recommended_watcher(move |res| {
            let _ = tx.send(res);
        })?;
        watcher.watch(&appimages_dir, RecursiveMode::NonRecursive)?;

        tokio::spawn(async move {
            while let Some(res) = rx.recv().await {
                match res {
                    Ok(event) => {
                        handle_watch_event(&provider, event).await;
                    }
                    Err(e) => warn!("AppImage watch error: {}", e),
                }
            }
        });

        info!("Watching {:?} for AppImages", appimages_dir);
        Ok(watcher)
    }

    async fn cleanup_stem(&self, stem: &str) {
        let _ = fs::remove_file(self.desktop_file(stem)).await;
        let _ = fs::remove_file(self.icon_file(stem)).await;
        let _ = fs::remove_file(self.info_file(stem)).await;
        let _ = Command::new("update-desktop-database")
            .arg(self.desktop_dir.to_str().unwrap_or(""))
            .status()
            .await;
    }

    async fn read_installed(&self) -> Result<Vec<Package>, ArcError> {
        let mut entries = match fs::read_dir(&self.data_dir).await {
            Ok(e) => e,
            Err(_) => return Ok(Vec::new()),
        };
        let mut packages = Vec::new();
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("info") {
                continue;
            }
            if let Ok(content) = fs::read_to_string(&path).await {
                if let Some(pkg) = parse_info(&content) {
                    packages.push(pkg);
                }
            }
        }
        Ok(packages)
    }

    pub async fn fetch_all(&self) -> Result<Vec<Package>, ArcError> {
        self.read_installed().await
    }
}

async fn handle_watch_event(provider: &AppImageProvider, event: Event) {
    match event.kind {
        EventKind::Create(_) => {
            for path in event.paths {
                if path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.ends_with(".AppImage"))
                    .unwrap_or(false)
                {
                    // small delay: file may still be written
                    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                    if let Some(stem) = AppImageProvider::stem_from_path(&path) {
                        if path.exists() && !provider.info_file(&stem).exists() {
                            info!("AppImage dropped into watch dir: {}", stem);
                            if let Err(e) = provider.process_appimage(&path, &stem).await {
                                warn!("Failed to process {}: {}", stem, e);
                            }
                        }
                    }
                }
            }
        }
        EventKind::Remove(_) => {
            for path in event.paths {
                if let Some(stem) = AppImageProvider::stem_from_path(&path) {
                    info!("AppImage removed from watch dir: {}", stem);
                    provider.cleanup_stem(&stem).await;
                }
            }
        }
        _ => {}
    }
}

fn parse_desktop(content: &str) -> AppImageMeta {
    let mut meta = AppImageMeta {
        name: String::new(),
        version: String::new(),
        description: String::new(),
        categories: String::new(),
        update_info: String::new(),
        icon_path: None,
    };
    for line in content.lines() {
        if let Some(v) = line.strip_prefix("Name=") {
            meta.name = v.to_string();
        } else if let Some(v) = line.strip_prefix("Version=") {
            meta.version = v.to_string();
        } else if let Some(v) = line.strip_prefix("Comment=") {
            meta.description = v.to_string();
        } else if let Some(v) = line.strip_prefix("Categories=") {
            meta.categories = v.to_string();
        } else if let Some(v) = line.strip_prefix("X-AppImage-Update-Information=") {
            meta.update_info = v.to_string();
        }
    }
    meta
}

fn extract_appstream_summary(xml: &str) -> Option<String> {
    // minimal XML scan: find first <summary> text
    let tag = "<summary>";
    let end_tag = "</summary>";
    let start = xml.find(tag)? + tag.len();
    let end = xml[start..].find(end_tag)?;
    let summary = xml[start..start + end].trim().to_string();
    if summary.is_empty() { None } else { Some(summary) }
}

impl AppImageMeta {
    fn default_from_path(path: &Path) -> Self {
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Unknown App")
            .to_string();
        // strip common version suffixes like "-1.0.0-x86_64" for display name
        let name = stem
            .split(|c: char| c == '-' || c == '_')
            .next()
            .unwrap_or(&stem)
            .to_string();
        Self {
            name,
            version: String::new(),
            description: String::new(),
            categories: String::new(),
            update_info: String::new(),
            icon_path: None,
        }
    }
}

fn parse_info(content: &str) -> Option<Package> {
    let mut stem = String::new();
    let mut name = String::new();
    let mut version = String::new();
    let mut description = String::new();
    let mut path = String::new();

    for line in content.lines() {
        if let Some(v) = line.strip_prefix("STEM=") {
            stem = v.to_string();
        } else if let Some(v) = line.strip_prefix("NAME=") {
            name = v.to_string();
        } else if let Some(v) = line.strip_prefix("VERSION=") {
            version = v.to_string();
        } else if let Some(v) = line.strip_prefix("DESCRIPTION=") {
            description = v.to_string();
        } else if let Some(v) = line.strip_prefix("PATH=") {
            path = v.to_string();
        }
    }

    if stem.is_empty() {
        return None;
    }

    let display_name = if name.is_empty() { stem.clone() } else { name };
    let desc = if description.is_empty() {
        "AppImage application".to_string()
    } else {
        description
    };

    Some(Package {
        id: format!("appimage:{}", stem),
        name: display_name,
        version,
        description: desc,
        provider: Provider::AppImage,
        installed: std::path::Path::new(&path).exists(),
        icon_url: None,
        remote: None,
        screenshots: vec![],
    })
}

#[async_trait]
impl PackageProvider for AppImageProvider {
    async fn search(&self, query: &str) -> Result<Vec<Package>, ArcError> {
        let q = query.to_lowercase();
        let installed = self.read_installed().await?;
        Ok(installed
            .into_iter()
            .filter(|p| p.name.to_lowercase().contains(&q) || p.id.to_lowercase().contains(&q))
            .collect())
    }

    async fn search_category(&self, _category: &str) -> Result<Vec<Package>, ArcError> {
        Ok(Vec::new())
    }

    async fn get_app_info(&self, package_id: &str) -> Result<Option<Package>, ArcError> {
        let installed = self.read_installed().await?;
        Ok(installed.into_iter().find(|p| p.id == package_id))
    }

    async fn list_installed(&self) -> Result<Vec<Package>, ArcError> {
        self.read_installed().await
    }

    async fn install(&self, package_id: &str) -> Result<(), ArcError> {
        let src = PathBuf::from(package_id);
        let stem = Self::stem_from_path(&src).ok_or_else(|| {
            ArcError::ProviderError(format!("Not an AppImage path: {}", package_id))
        })?;

        fs::create_dir_all(&self.appimages_dir)
            .await
            .map_err(|e| ArcError::ProviderError(e.to_string()))?;

        let dest = self.appimage_path(&stem);

        // if file is already in ~/.appimages, just process it
        if src == dest {
            return self.process_appimage(&dest, &stem).await;
        }

        fs::copy(&src, &dest)
            .await
            .map_err(|e| ArcError::ProviderError(format!("copy AppImage: {}", e)))?;

        self.process_appimage(&dest, &stem).await
    }

    async fn remove(&self, package_id: &str) -> Result<(), ArcError> {
        let stem = package_id
            .strip_prefix("appimage:")
            .ok_or_else(|| ArcError::ProviderError(format!("Invalid AppImage id: {}", package_id)))?;

        let appimage = self.appimage_path(stem);
        let _ = fs::remove_file(&appimage).await;
        self.cleanup_stem(stem).await;
        Ok(())
    }

    async fn list_updates(&self) -> Result<Vec<Package>, ArcError> {
        let installed = self.read_installed().await?;
        let mut updates = Vec::new();

        for pkg in installed {
            let stem = pkg.id.strip_prefix("appimage:").unwrap_or("").to_string();
            let info_content = match fs::read_to_string(self.info_file(&stem)).await {
                Ok(c) => c,
                Err(_) => continue,
            };

            let update_info: String = info_content
                .lines()
                .find_map(|l| l.strip_prefix("UPDATE_INFO=").map(|v| v.to_string()))
                .unwrap_or_default();

            if update_info.is_empty() {
                continue;
            }

            let appimage_path = self.appimage_path(&stem);
            if !appimage_path.exists() {
                continue;
            }

            // use AppImageUpdate if available to check
            let has_update = check_update_available(&appimage_path).await;
            if has_update {
                updates.push(pkg);
            }
        }

        Ok(updates)
    }

    async fn update(&self, package_id: &str) -> Result<(), ArcError> {
        let stem = package_id
            .strip_prefix("appimage:")
            .ok_or_else(|| ArcError::ProviderError(format!("Invalid AppImage id: {}", package_id)))?;

        let appimage_path = self.appimage_path(stem);
        if !appimage_path.exists() {
            return Err(ArcError::PackageNotFound(package_id.to_string()));
        }

        // try AppImageUpdate tool
        let status = Command::new("AppImageUpdate")
            .arg(&appimage_path)
            .status()
            .await
            .map_err(|_| {
                ArcError::ProviderError(
                    "AppImageUpdate not found. Install it to enable AppImage updates.".to_string(),
                )
            })?;

        if !status.success() {
            return Err(ArcError::ProviderError(
                "AppImageUpdate failed".to_string(),
            ));
        }

        // re-process metadata (version may have changed)
        self.process_appimage(&appimage_path, stem).await
    }

    async fn run(&self, package_id: &str) -> Result<(), ArcError> {
        let stem = package_id
            .strip_prefix("appimage:")
            .ok_or_else(|| ArcError::ProviderError(format!("Invalid AppImage id: {}", package_id)))?;

        let appimage_path = self.appimage_path(stem);
        if !appimage_path.exists() {
            return Err(ArcError::PackageNotFound(package_id.to_string()));
        }

        tokio::process::Command::new(&appimage_path)
            .spawn()
            .map_err(|e| ArcError::ProviderError(format!("Failed to launch AppImage: {}", e)))?;

        Ok(())
    }
}

async fn check_update_available(appimage_path: &Path) -> bool {
    let Ok(output) = Command::new("AppImageUpdate")
        .arg("--check-for-update")
        .arg(appimage_path)
        .output()
        .await
    else {
        return false; // tool not installed
    };
    // exit code 1 = update available, 0 = up to date
    output.status.code() == Some(1)
}
