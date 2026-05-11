use super::PackageProvider;
use anvil_appimage::{find_icons_in_dir, move_appimage, select_best_icon, set_executable_permissions};
use async_trait::async_trait;
use futures_util::StreamExt;
use libarc::{ArcError, Package, Provider};
use notify::{
    event::{AccessKind, AccessMode},
    Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher,
};
use reqwest::Client;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tracing::{debug, info, warn};
use uuid::Uuid;

pub struct AppImageProvider {
    pub appimages_dir: PathBuf,
    desktop_dir: PathBuf,
    data_dir: PathBuf,
    home: PathBuf,
    http: Client,
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
            home: home.clone(),
            http: Client::new(),
        }
    }

    fn hicolor_icon_path(&self, stem: &str, ext: &str) -> PathBuf {
        let size_dir = if ext == "svg" || ext == "svgz" { "scalable" } else { "256x256" };
        self.home
            .join(".local/share/icons/hicolor")
            .join(size_dir)
            .join("apps")
            .join(format!("arc-appimage-{}.{}", stem, ext))
    }

    async fn install_icon_to_hicolor(&self, stem: &str, icon_src: &Path) -> Option<String> {
        let ext = icon_src.extension().and_then(|e| e.to_str()).unwrap_or("png").to_string();
        let dest = self.hicolor_icon_path(stem, &ext);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).await.ok()?;
        }
        fs::copy(icon_src, &dest).await.ok()?;
        let hicolor_root = self.home.join(".local/share/icons/hicolor");
        let _ = Command::new("gtk-update-icon-cache")
            .arg("-f")
            .arg("-t")
            .arg(&hicolor_root)
            .status()
            .await;
        Some(format!("arc-appimage-{}", stem))
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

    /// Returns the actual on-disk path for an installed AppImage.
    /// Reads the stored PATH= from the .info file so the extension case (.AppImage vs
    /// .appimage) matches whatever was originally placed in the watch directory.
    async fn resolve_appimage_path(&self, stem: &str) -> PathBuf {
        if let Ok(content) = fs::read_to_string(self.info_file(stem)).await {
            if let Some(p) = content
                .lines()
                .find_map(|l| l.strip_prefix("PATH=").map(|v| v.to_string()))
                .filter(|s| !s.is_empty())
                .map(PathBuf::from)
            {
                if p.exists() {
                    return p;
                }
            }
        }
        self.appimage_path(stem)
    }

    fn stem_from_path(path: &Path) -> Option<String> {
        let name = path.file_name()?.to_str()?;
        // case-insensitive: handles .AppImage, .appimage, .APPIMAGE, etc.
        if name.to_lowercase().ends_with(".appimage") {
            Some(name[..name.len() - ".appimage".len()].to_string())
        } else {
            None
        }
    }

    async fn extract_metadata(&self, path: &Path) -> AppImageMeta {
        let extract_dir = std::env::temp_dir()
            .join(format!("arc-appimage-{}", Uuid::new_v4().simple()));

        if fs::create_dir_all(&extract_dir).await.is_err() {
            return AppImageMeta::default_from_path(path);
        }

        // set executable before running --appimage-extract
        let path_for_extract = path.to_path_buf();
        tokio::task::spawn_blocking(move || {
            set_executable_permissions(&path_for_extract, false);
        })
        .await
        .ok();

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
        let mut meta = Self::read_squashfs_meta(&squashfs, path).await;

        // The icon path points inside extract_dir which is about to be deleted.
        // Copy it out to a standalone temp file before cleanup.
        if let Some(ref icon_src) = meta.icon_path.clone() {
            if icon_src.exists() {
                let ext = icon_src.extension().and_then(|e| e.to_str()).unwrap_or("png");
                let icon_tmp = std::env::temp_dir()
                    .join(format!("arc-icon-{}.{}", Uuid::new_v4().simple(), ext));
                if fs::copy(icon_src, &icon_tmp).await.is_ok() {
                    meta.icon_path = Some(icon_tmp);
                } else {
                    meta.icon_path = None;
                }
            } else {
                meta.icon_path = None;
            }
        }

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

        // use anvil-appimage to find and select the best icon from the extracted contents
        let sq = squashfs.to_path_buf();
        meta.icon_path = tokio::task::spawn_blocking(move || {
            let icons = find_icons_in_dir(&sq);
            select_best_icon(icons)
        })
        .await
        .unwrap_or(None);

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

        // make executable using anvil-appimage
        let path_for_perms = path.to_path_buf();
        tokio::task::spawn_blocking(move || {
            set_executable_permissions(&path_for_perms, false);
        })
        .await
        .ok();

        let meta = self.extract_metadata(path).await;

        // copy icon to stable data location, preserving the source extension
        let icon_dest = if let Some(ref icon_src) = meta.icon_path {
            let ext = icon_src
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("png");
            let dest = self.data_dir.join("icons").join(format!("{}.{}", stem, ext));
            let ok = fs::copy(icon_src, &dest).await.is_ok();
            let _ = fs::remove_file(icon_src).await; // remove the temp copy
            if ok { Some(dest) } else { None }
        } else {
            None
        };

        // install into hicolor so GNOME Shell / app launchers pick it up
        let hicolor_name = if let Some(ref src) = icon_dest {
            self.install_icon_to_hicolor(stem, src).await
        } else {
            None
        };

        // prefer hicolor theme name for .desktop Icon= (better integration); fall back to full path
        let icon_str = hicolor_name
            .clone()
            .or_else(|| icon_dest.as_ref().map(|p| p.to_string_lossy().to_string()))
            .unwrap_or_else(|| "application-x-executable".to_string());

        // write .desktop file
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

        // write .info file — ICON_PATH lets the frontend and cleanup find the icon
        let info = format!(
            "STEM={}\nNAME={}\nVERSION={}\nDESCRIPTION={}\nPATH={}\nUPDATE_INFO={}\nICON_PATH={}\n",
            stem,
            meta.name,
            meta.version,
            meta.description,
            path.display(),
            meta.update_info,
            icon_dest
                .as_ref()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default(),
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
        // read icon path before deleting the .info file
        let icon_path = match fs::read_to_string(self.info_file(stem)).await {
            Ok(content) => content
                .lines()
                .find_map(|l| l.strip_prefix("ICON_PATH=").map(|v| v.to_string()))
                .filter(|s| !s.is_empty()),
            Err(_) => None,
        };

        let _ = fs::remove_file(self.desktop_file(stem)).await;
        if let Some(ref path) = icon_path {
            let _ = fs::remove_file(path).await;
        } else {
            // legacy fallback for info files written before ICON_PATH was added
            let _ = fs::remove_file(self.icon_file(stem)).await;
        }

        // remove hicolor icons (try all known extensions)
        for ext in ["svg", "svgz", "png"] {
            let p = self.hicolor_icon_path(stem, ext);
            if p.exists() {
                let _ = fs::remove_file(&p).await;
            }
        }

        let _ = fs::remove_file(self.info_file(stem)).await;
        let _ = Command::new("update-desktop-database")
            .arg(self.desktop_dir.to_str().unwrap_or(""))
            .status()
            .await;
        let hicolor_root = self.home.join(".local/share/icons/hicolor");
        let _ = Command::new("gtk-update-icon-cache")
            .arg("-f")
            .arg("-t")
            .arg(&hicolor_root)
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

fn is_appimage_path(path: &std::path::Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.to_lowercase().ends_with(".appimage"))
        .unwrap_or(false)
}

async fn process_if_new(provider: &AppImageProvider, path: &std::path::Path) {
    if let Some(stem) = AppImageProvider::stem_from_path(path) {
        if path.exists() && !provider.info_file(&stem).exists() {
            info!("AppImage detected in watch dir: {}", stem);
            if let Err(e) = provider.process_appimage(path, &stem).await {
                warn!("Failed to process {}: {}", stem, e);
            }
        }
    }
}

async fn handle_watch_event(provider: &AppImageProvider, event: Event) {
    match event.kind {
        // IN_MOVED_TO (mv same-fs) arrives as Create — file is complete immediately
        EventKind::Create(_) => {
            for path in event.paths {
                if is_appimage_path(&path) {
                    process_if_new(provider, &path).await;
                }
            }
        }
        // IN_CLOSE_WRITE fires when a cp/download finishes writing — file is complete
        EventKind::Access(AccessKind::Close(AccessMode::Write)) => {
            for path in event.paths {
                if is_appimage_path(&path) {
                    process_if_new(provider, &path).await;
                }
            }
        }
        EventKind::Remove(_) => {
            for path in event.paths {
                debug!("Remove event: {:?}", path);
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
    let mut icon_path = String::new();

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
        } else if let Some(v) = line.strip_prefix("ICON_PATH=") {
            icon_path = v.to_string();
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
        icon_url: if icon_path.is_empty() { None } else { Some(icon_path) },
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

        // use anvil-appimage for a cross-device-safe move (rename, fallback to copy+delete)
        // destination is the full target file path, not a directory
        let src_str = src.to_str().unwrap_or("").to_string();
        let dest_path = dest.clone();
        let file_name = dest.file_name().unwrap_or_default().to_os_string();
        let moved = tokio::task::spawn_blocking(move || {
            move_appimage(&src_str, &dest_path, &file_name, false)
        })
        .await
        .unwrap_or(false);

        if !moved {
            return Err(ArcError::ProviderError("Failed to move AppImage to install directory".to_string()));
        }

        self.process_appimage(&dest, &stem).await
    }

    async fn remove(&self, package_id: &str) -> Result<(), ArcError> {
        let stem = package_id
            .strip_prefix("appimage:")
            .ok_or_else(|| ArcError::ProviderError(format!("Invalid AppImage id: {}", package_id)))?;

        let appimage = self.resolve_appimage_path(stem).await;
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

            let appimage_path = self.resolve_appimage_path(&stem).await;
            if !appimage_path.exists() {
                continue;
            }

            let has_update = if let Some(gh) = parse_github_info(&update_info) {
                match github_latest_release(&self.http, &gh.owner, &gh.repo, &gh.asset_glob).await {
                    Some((remote_tag, _)) => versions_differ(&pkg.version, &remote_tag),
                    None => false,
                }
            } else {
                check_update_available(&appimage_path).await
            };

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

        let appimage_path = self.resolve_appimage_path(stem).await;
        if !appimage_path.exists() {
            return Err(ArcError::PackageNotFound(package_id.to_string()));
        }

        let update_info = match fs::read_to_string(self.info_file(stem)).await {
            Ok(c) => c
                .lines()
                .find_map(|l| l.strip_prefix("UPDATE_INFO=").map(|v| v.to_string()))
                .unwrap_or_default(),
            Err(_) => String::new(),
        };

        let updated_via_github = if let Some(gh) = parse_github_info(&update_info) {
            match github_latest_release(&self.http, &gh.owner, &gh.repo, &gh.asset_glob).await {
                Some((tag, download_url)) => {
                    info!("Downloading {} update from GitHub ({}) — {}", stem, tag, download_url);
                    download_github_update(&self.http, &download_url, &appimage_path).await?;
                    true
                }
                None => {
                    warn!("GitHub release lookup failed for {}", stem);
                    false
                }
            }
        } else {
            false
        };

        if !updated_via_github {
            // fall back to AppImageUpdate tool
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
                return Err(ArcError::ProviderError("AppImageUpdate failed".to_string()));
            }
        }

        // re-process metadata (version may have changed)
        self.process_appimage(&appimage_path, stem).await
    }

    async fn run(&self, package_id: &str) -> Result<(), ArcError> {
        let stem = package_id
            .strip_prefix("appimage:")
            .ok_or_else(|| ArcError::ProviderError(format!("Invalid AppImage id: {}", package_id)))?;

        let appimage_path = self.resolve_appimage_path(stem).await;
        if !appimage_path.exists() {
            return Err(ArcError::PackageNotFound(package_id.to_string()));
        }

        tokio::process::Command::new(&appimage_path)
            .spawn()
            .map_err(|e| ArcError::ProviderError(format!("Failed to launch AppImage: {}", e)))?;

        Ok(())
    }
}

struct GitHubInfo {
    owner: String,
    repo: String,
    asset_glob: String,
}

fn parse_github_info(update_info: &str) -> Option<GitHubInfo> {
    let parts: Vec<&str> = update_info.splitn(5, '|').collect();
    if parts.len() >= 5 && parts[0] == "gh-releases-zsync" {
        let glob = parts[4].strip_suffix(".zsync").unwrap_or(parts[4]).to_string();
        return Some(GitHubInfo {
            owner: parts[1].to_string(),
            repo: parts[2].to_string(),
            asset_glob: glob,
        });
    }
    if parts.len() >= 2 && parts[0] == "zsync" {
        let url = parts[1];
        if url.contains("github.com") {
            // https://github.com/{owner}/{repo}/releases/...
            let stripped = url.trim_start_matches("https://github.com/").trim_start_matches("http://github.com/");
            let url_parts: Vec<&str> = stripped.splitn(3, '/').collect();
            if url_parts.len() >= 2 {
                let filename = url.rsplit('/').next().unwrap_or("*.AppImage");
                let glob = filename.strip_suffix(".zsync").unwrap_or(filename).to_string();
                return Some(GitHubInfo {
                    owner: url_parts[0].to_string(),
                    repo: url_parts[1].to_string(),
                    asset_glob: glob,
                });
            }
        }
    }
    None
}

fn glob_matches(pattern: &str, name: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == name;
    }
    let mut pos = 0usize;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if i == 0 {
            if !name.starts_with(part) {
                return false;
            }
            pos = part.len();
        } else if i == parts.len() - 1 {
            return name[pos..].ends_with(part);
        } else if let Some(found) = name[pos..].find(part) {
            pos += found + part.len();
        } else {
            return false;
        }
    }
    true
}

fn versions_differ(local: &str, remote_tag: &str) -> bool {
    let norm = |s: &str| s.trim_start_matches('v').to_lowercase();
    norm(local) != norm(remote_tag)
}

async fn github_latest_release(
    http: &Client,
    owner: &str,
    repo: &str,
    asset_glob: &str,
) -> Option<(String, String)> {
    let url = format!("https://api.github.com/repos/{}/{}/releases/latest", owner, repo);
    let resp = http
        .get(&url)
        .header("User-Agent", "arc-daemon/1.0")
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let json: serde_json::Value = resp.json().await.ok()?;
    let tag = json.get("tag_name")?.as_str()?.to_string();
    let assets = json.get("assets")?.as_array()?;
    let glob_lower = asset_glob.to_lowercase();
    for asset in assets {
        let name = asset.get("name")?.as_str()?;
        if glob_matches(&glob_lower, &name.to_lowercase()) {
            let dl = asset.get("browser_download_url")?.as_str()?.to_string();
            return Some((tag, dl));
        }
    }
    None
}

async fn download_github_update(http: &Client, url: &str, dest: &Path) -> Result<(), ArcError> {
    let resp = http
        .get(url)
        .header("User-Agent", "arc-daemon/1.0")
        .send()
        .await
        .map_err(|e| ArcError::ProviderError(format!("Download request failed: {}", e)))?;
    if !resp.status().is_success() {
        return Err(ArcError::ProviderError(format!("Download HTTP {}", resp.status())));
    }
    let tmp = dest.with_extension("download.tmp");
    let mut file = tokio::fs::File::create(&tmp)
        .await
        .map_err(|e| ArcError::ProviderError(format!("Failed to create temp file: {}", e)))?;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| ArcError::ProviderError(format!("Download stream error: {}", e)))?;
        file.write_all(&chunk)
            .await
            .map_err(|e| ArcError::ProviderError(format!("Write error: {}", e)))?;
    }
    file.flush().await.ok();
    drop(file);
    tokio::fs::rename(&tmp, dest)
        .await
        .map_err(|e| ArcError::ProviderError(format!("Failed to place downloaded file: {}", e)))?;
    Ok(())
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
