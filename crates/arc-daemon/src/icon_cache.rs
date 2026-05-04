use appstream::enums::Icon;
use appstream::{Collection, Component};
use flate2::read::GzDecoder;
use image::GenericImageView;
use resvg::{tiny_skia, usvg};
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use tokio::sync::RwLock;
use tracing::info;

// 30 minutes cache TTL for icons
const ICON_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(1800);

/// Cached icon data with timestamp
#[derive(Clone)]
pub struct CachedIcon {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    pub cached_at: std::time::Instant,
}

/// Icon cache that pre-renders and caches icons for fast retrieval
pub struct IconCache {
    cache: RwLock<HashMap<String, CachedIcon>>,
    appstream_cache: RwLock<HashMap<String, Vec<Component>>>,
    icon_size: u32,
}

impl IconCache {
    pub fn new(icon_size: u32) -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            appstream_cache: RwLock::new(HashMap::new()),
            icon_size,
        }
    }

    /// Pre-warm the icon cache by loading and rendering icons from AppStream data
    pub async fn prewarm(&self) {
        info!("Pre-warming icon cache...");

        // Load Flatpak AppStream data
        let components = self.load_flatpak_appstream();
        info!("Loaded {} components from AppStream data", components.len());

        // Store components in appstream cache for later lookup
        // Also store the base icon path for each component
        {
            let mut appstream_cache = self.appstream_cache.write().await;
            for component in &components {
                let id = component.id.to_string();
                appstream_cache
                    .entry(id)
                    .or_insert_with(Vec::new)
                    .push(component.clone());
            }
        }

        // Pre-render icons for popular apps (first 500 components for fast home page)
        let mut icons_to_cache = Vec::new();
        let mut icons_extracted = 0;
        let mut icons_no_data = 0;
        for component in components.iter().take(500) {
            if let Some(icon_data) = self.extract_icon_data(component) {
                icons_to_cache.push((component.id.to_string(), icon_data));
                icons_extracted += 1;
            } else {
                icons_no_data += 1;
                info!("No icon data for component: {}", component.id);
            }
        }
        info!(
            "Extracted {} icons from first 500 components, {} had no icon data",
            icons_extracted, icons_no_data
        );

        // Store count before moving icons_to_cache into tasks
        let _icons_to_cache_count = icons_to_cache.len();

        // Render icons in parallel using tokio tasks for async downloads
        let icon_size = self.icon_size;
        let tasks: Vec<_> = icons_to_cache
            .into_iter()
            .map(|(id, data)| {
                let icon_size = icon_size;
                tokio::spawn(async move {
                    let rendered = render_icon_data(&data, icon_size).await;
                    (id, rendered)
                })
            })
            .collect();

        // Collect results from async tasks
        let results: Vec<_> = futures_util::future::join_all(tasks).await;

        let mut cache = self.cache.write().await;
        let mut render_failures = 0;
        for result in results {
            if let Ok((id, Some(rendered))) = result {
                info!("Cached icon for {}", id);
                cache.insert(
                    id,
                    CachedIcon {
                        width: rendered.width,
                        height: rendered.height,
                        pixels: rendered.pixels,
                        cached_at: std::time::Instant::now(),
                    },
                );
            } else {
                render_failures += 1;
            }
        }

        info!(
            "Icon cache pre-warming complete ({} icons cached, {} failed to render)",
            cache.len(),
            render_failures
        );
    }

    /// Load AppStream data from Flatpak installations
    fn load_flatpak_appstream(&self) -> Vec<Component> {
        let mut components = Vec::new();

        // System installation
        load_flatpak_root("/var/lib/flatpak/appstream", &mut components);

        // User installation
        if let Some(home) = std::env::var_os("HOME") {
            let path = PathBuf::from(home).join(".local/share/flatpak/appstream");
            load_flatpak_root(&path, &mut components);
        }

        components
    }

    /// Extract icon data from a component, resolving local paths
    fn extract_icon_data(&self, component: &Component) -> Option<IconData> {
        component.icons.first().and_then(|icon| {
            match icon {
                Icon::Remote { url, .. } => Some(IconData::Remote(url.to_string())),
                Icon::Local { path, .. } | Icon::Cached { path, .. } => {
                    // Try to find the actual icon file in Flatpak icon directories
                    if let Some(full_path) = self.find_flatpak_icon_path(&path.to_string_lossy()) {
                        Some(IconData::Local(full_path))
                    } else {
                        // Fall back to stock icon lookup
                        Some(IconData::Stock(path.to_string_lossy().to_string()))
                    }
                }
                Icon::Stock(name) => Some(IconData::Stock(name.clone())),
            }
        })
    }

    /// Find the full path to a Flatpak icon by searching icon directories
    fn find_flatpak_icon_path(&self, icon_name: &str) -> Option<PathBuf> {
        // Search system Flatpak installation
        if let Some(path) = search_flatpak_icon_dir("/var/lib/flatpak/appstream", icon_name) {
            return Some(path);
        }

        // Search user Flatpak installation
        if let Some(home) = std::env::var_os("HOME") {
            let user_path = PathBuf::from(home).join(".local/share/flatpak/appstream");
            if let Some(found) = search_flatpak_icon_dir(&user_path, icon_name) {
                return Some(found);
            }
        }

        None
    }

    /// Get a cached icon, rendering it if necessary
    pub async fn get_icon(&self, app_id: &str) -> Option<CachedIcon> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.get(app_id) {
                if cached.cached_at.elapsed() < ICON_CACHE_TTL {
                    return Some(cached.clone());
                }
            }
        }

        // Try to find and render the icon
        let appstream_cache = self.appstream_cache.read().await;
        if let Some(components) = appstream_cache.get(app_id) {
            if let Some(component) = components.first() {
                if let Some(icon_data) = self.extract_icon_data(component) {
                    if let Some(rendered) = render_icon_data(&icon_data, self.icon_size).await {
                        let cached_icon = CachedIcon {
                            width: rendered.width,
                            height: rendered.height,
                            pixels: rendered.pixels,
                            cached_at: std::time::Instant::now(),
                        };

                        // Store in cache
                        let mut cache = self.cache.write().await;
                        cache.insert(app_id.to_string(), cached_icon.clone());

                        return Some(cached_icon);
                    }
                }
            }
        }

        // Try loading from system icon theme as fallback
        if let Some(rendered) = load_system_icon(app_id, self.icon_size) {
            let cached_icon = CachedIcon {
                width: rendered.width,
                height: rendered.height,
                pixels: rendered.pixels,
                cached_at: std::time::Instant::now(),
            };

            let mut cache = self.cache.write().await;
            cache.insert(app_id.to_string(), cached_icon.clone());

            return Some(cached_icon);
        }

        None
    }

    /// Get icon for a native package
    pub async fn get_native_icon(&self, package_name: &str) -> Option<CachedIcon> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.get(package_name) {
                if cached.cached_at.elapsed() < ICON_CACHE_TTL {
                    return Some(cached.clone());
                }
            }
        }

        // Try loading from system icon theme
        if let Some(rendered) = load_system_icon(package_name, self.icon_size) {
            let cached_icon = CachedIcon {
                width: rendered.width,
                height: rendered.height,
                pixels: rendered.pixels,
                cached_at: std::time::Instant::now(),
            };

            let mut cache = self.cache.write().await;
            cache.insert(package_name.to_string(), cached_icon.clone());

            return Some(cached_icon);
        }

        // Try loading from desktop file
        if let Some(rendered) = load_icon_from_desktop(package_name, self.icon_size) {
            let cached_icon = CachedIcon {
                width: rendered.width,
                height: rendered.height,
                pixels: rendered.pixels,
                cached_at: std::time::Instant::now(),
            };

            let mut cache = self.cache.write().await;
            cache.insert(package_name.to_string(), cached_icon.clone());

            return Some(cached_icon);
        }

        None
    }

    /// Get apps by category from cached AppStream data
    pub async fn get_apps_by_category(&self, category: &str) -> Vec<String> {
        let appstream_cache = self.appstream_cache.read().await;
        let category_lower = category.to_lowercase();

        appstream_cache
            .iter()
            .filter(|(_, components)| {
                components.iter().any(|c| {
                    matches!(
                        c.kind,
                        appstream::enums::ComponentKind::DesktopApplication
                            | appstream::enums::ComponentKind::ConsoleApplication
                    )
                })
            })
            .filter(|(_, components)| {
                components.iter().any(|c| {
                    c.categories
                        .iter()
                        .any(|cat| format!("{:?}", cat).to_lowercase() == category_lower)
                })
            })
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Get popular apps (first N from cached AppStream data)
    pub async fn get_popular_apps(&self, limit: usize) -> Vec<String> {
        let appstream_cache = self.appstream_cache.read().await;

        appstream_cache
            .keys()
            .filter(|id| {
                if let Some(components) = appstream_cache.get(*id) {
                    components.iter().any(|c| {
                        matches!(
                            c.kind,
                            appstream::enums::ComponentKind::DesktopApplication
                                | appstream::enums::ComponentKind::ConsoleApplication
                        )
                    })
                } else {
                    false
                }
            })
            .take(limit)
            .cloned()
            .collect()
    }

    /// Get recent apps (last N from cached AppStream data)
    pub async fn get_recent_apps(&self, limit: usize) -> Vec<String> {
        let appstream_cache = self.appstream_cache.read().await;

        let mut apps: Vec<String> = appstream_cache
            .keys()
            .filter(|id| {
                if let Some(components) = appstream_cache.get(*id) {
                    components.iter().any(|c| {
                        matches!(
                            c.kind,
                            appstream::enums::ComponentKind::DesktopApplication
                                | appstream::enums::ComponentKind::ConsoleApplication
                        )
                    })
                } else {
                    false
                }
            })
            .cloned()
            .collect();
        apps.reverse();
        apps.into_iter().take(limit).collect()
    }

    /// Search apps in cached AppStream data
    pub async fn search_apps(&self, query: &str) -> Vec<String> {
        let q = query.to_lowercase();
        let appstream_cache = self.appstream_cache.read().await;

        appstream_cache
            .iter()
            .filter(|(_id, components)| {
                components.iter().any(|c| {
                    matches!(
                        c.kind,
                        appstream::enums::ComponentKind::DesktopApplication
                            | appstream::enums::ComponentKind::ConsoleApplication
                    )
                })
            })
            .filter(|(id, components)| {
                components.iter().any(|c| {
                    let id_lower = id.to_lowercase();
                    let name = c
                        .name
                        .get_default()
                        .map(|s| s.to_lowercase())
                        .unwrap_or_default();
                    let summary = c
                        .summary
                        .as_ref()
                        .and_then(|s| s.get_default())
                        .map(|s| s.to_lowercase())
                        .unwrap_or_default();
                    id_lower.contains(&q) || name.contains(&q) || summary.contains(&q)
                })
            })
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Get app info from cached AppStream data
    pub async fn get_app_info(&self, app_id: &str) -> Option<AppInfo> {
        let appstream_cache = self.appstream_cache.read().await;
        let components = appstream_cache.get(app_id)?;
        let component = components.first()?;

        Some(AppInfo {
            id: component.id.to_string(),
            name: component.name.get_default().cloned().unwrap_or_default(),
            summary: component
                .summary
                .as_ref()
                .and_then(|s| s.get_default())
                .cloned()
                .unwrap_or_default(),
            description: component
                .description
                .as_ref()
                .and_then(|d| d.get_default())
                .cloned()
                .unwrap_or_default(),
            icon_available: !component.icons.is_empty(),
        })
    }
}

/// App info extracted from AppStream data
#[derive(Clone, serde::Serialize)]
pub struct AppInfo {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub description: String,
    pub icon_available: bool,
}

/// Icon data extracted from AppStream component
#[derive(Clone)]
pub enum IconData {
    Remote(String),
    Local(PathBuf),
    Stock(String),
}

/// Render icon data to raw pixels (async for remote URLs)
async fn render_icon_data(icon_data: &IconData, size: u32) -> Option<RawIcon> {
    match icon_data {
        IconData::Remote(url) => {
            // Download remote icon and render it
            let resp = reqwest::get(url).await;
            if let Err(e) = &resp {
                tracing::warn!("Failed to download remote icon {}: {}", url, e);
            }
            let bytes = resp.ok()?.bytes().await.ok()?;
            let img = image::load_from_memory(&bytes);
            if let Err(e) = &img {
                tracing::warn!("Failed to decode remote icon {}: {}", url, e);
            }
            let img = img.ok()?;
            let img = img.resize(size, size, image::imageops::FilterType::Lanczos3);
            let (w, h) = img.dimensions();
            let pixels = img.to_rgba8().into_raw();
            Some(RawIcon {
                width: w,
                height: h,
                pixels,
            })
        }
        IconData::Local(path) => {
            if !path.exists() {
                tracing::warn!("Local icon path does not exist: {:?}", path);
                return None;
            }
            if path
                .extension()
                .map(|e| e == "svg" || e == "svgz")
                .unwrap_or(false)
            {
                let result = read_svg_bytes(path).and_then(|bytes| render_svg(&bytes, size));
                if result.is_none() {
                    tracing::warn!("Failed to render SVG icon: {:?}", path);
                }
                result
            } else {
                let result = load_png_icon(path, size);
                if result.is_none() {
                    tracing::warn!("Failed to load PNG icon: {:?}", path);
                }
                result
            }
        }
        IconData::Stock(name) => {
            // Try to find stock icon in system icon theme
            if let Some(path) = find_system_icon_path(name) {
                let result = if path
                    .extension()
                    .map(|e| e == "svg" || e == "svgz")
                    .unwrap_or(false)
                {
                    read_svg_bytes(&path).and_then(|bytes| render_svg(&bytes, size))
                } else {
                    load_png_icon(&path, size)
                };
                if result.is_none() {
                    tracing::warn!("Failed to render stock icon {} from {:?}", name, path);
                }
                result
            } else {
                tracing::debug!("Stock icon not found in system theme: {}", name);
                None
            }
        }
    }
}

#[derive(Clone)]
pub struct RawIcon {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

fn read_svg_bytes(path: &Path) -> Option<Vec<u8>> {
    let raw = std::fs::read(path).ok()?;
    // svgz is just gzipped svg, common in icon themes
    if path.extension().and_then(|e| e.to_str()) == Some("svgz") {
        let mut decoder = GzDecoder::new(&raw[..]);
        let mut buf = Vec::new();
        decoder.read_to_end(&mut buf).ok()?;
        Some(buf)
    } else {
        Some(raw)
    }
}

fn render_svg(svg_bytes: &[u8], size: u32) -> Option<RawIcon> {
    let opt = usvg::Options::default();
    let tree = match usvg::Tree::from_data(svg_bytes, &opt) {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!("Failed to parse SVG: {}", e);
            return None;
        }
    };
    let mut pixmap = tiny_skia::Pixmap::new(size, size)?;
    let sx = size as f32 / tree.size().width();
    let sy = size as f32 / tree.size().height();
    let transform = tiny_skia::Transform::from_scale(sx, sy);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    let mut pixels = pixmap.data().to_vec();
    for chunk in pixels.chunks_exact_mut(4) {
        let a = chunk[3];
        if a > 0 && a < 255 {
            let inv = 255.0 / a as f32;
            chunk[0] = (chunk[0] as f32 * inv).min(255.0) as u8;
            chunk[1] = (chunk[1] as f32 * inv).min(255.0) as u8;
            chunk[2] = (chunk[2] as f32 * inv).min(255.0) as u8;
        }
    }
    Some(RawIcon {
        width: size,
        height: size,
        pixels,
    })
}

fn load_png_icon(path: &Path, size: u32) -> Option<RawIcon> {
    let img = match image::open(path) {
        Ok(i) => i,
        Err(e) => {
            tracing::warn!("Failed to open PNG icon {:?}: {}", path, e);
            return None;
        }
    };
    let img = img.resize(size, size, image::imageops::FilterType::Lanczos3);
    let (w, h) = img.dimensions();
    let pixels = img.to_rgba8().into_raw();
    Some(RawIcon {
        width: w,
        height: h,
        pixels,
    })
}

fn load_flatpak_root(root: impl AsRef<Path>, out: &mut Vec<Component>) {
    let Ok(remotes) = std::fs::read_dir(root.as_ref()) else {
        return;
    };
    for remote_dir in remotes.flatten() {
        let Ok(arches) = std::fs::read_dir(remote_dir.path()) else {
            continue;
        };
        for arch in arches.flatten() {
            let base = arch.path().join("active");
            let gz = base.join("appstream.xml.gz");
            let xml = base.join("appstream.xml");
            if gz.exists() {
                if let Ok(col) = Collection::from_gzipped(gz) {
                    out.extend(col.components.into_iter());
                }
            } else if xml.exists() {
                if let Ok(col) = Collection::from_path(xml) {
                    out.extend(col.components.into_iter());
                }
            }
        }
    }
}

/// Search for an icon in Flatpak icon directories
fn search_flatpak_icon_dir(base: impl AsRef<Path>, icon_name: &str) -> Option<PathBuf> {
    let base = base.as_ref();
    let Ok(remotes) = std::fs::read_dir(base) else {
        return None;
    };

    for remote_dir in remotes.flatten() {
        let Ok(arches) = std::fs::read_dir(remote_dir.path()) else {
            continue;
        };
        for arch in arches.flatten() {
            let icons_dir = arch.path().join("active").join("icons");
            if !icons_dir.exists() {
                continue;
            }

            // Search in different icon sizes
            // Icons are stored directly in the size directory, not in an apps subdirectory
            for size in ["128x128", "96x96", "64x64", "48x48", "scalable"] {
                let size_dir = icons_dir.join(size);
                if !size_dir.exists() {
                    continue;
                }

                // Icon name may already include extension (e.g., "app.id.png")
                // Try the icon name as-is first
                let direct_path = size_dir.join(icon_name);
                if direct_path.exists() {
                    return Some(direct_path);
                }

                // If icon name doesn't have an extension, try adding common extensions
                if !icon_name.ends_with(".png")
                    && !icon_name.ends_with(".svg")
                    && !icon_name.ends_with(".svgz")
                {
                    // Try with .png extension
                    let png_path = size_dir.join(format!("{}.png", icon_name));
                    if png_path.exists() {
                        return Some(png_path);
                    }

                    // Try with .svg extension
                    let svg_path = size_dir.join(format!("{}.svg", icon_name));
                    if svg_path.exists() {
                        return Some(svg_path);
                    }

                    // Try with .svgz extension
                    let svgz_path = size_dir.join(format!("{}.svgz", icon_name));
                    if svgz_path.exists() {
                        return Some(svgz_path);
                    }
                }
            }
        }
    }
    None
}

fn find_system_icon_path(icon_name: &str) -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let icon_dirs = [
        home.join(".local/share/icons"),
        PathBuf::from("/usr/share/icons"),
        PathBuf::from("/usr/local/share/icons"),
        PathBuf::from("/usr/share/pixmaps"),
    ];

    for icon_dir in &icon_dirs {
        for size in &["128x128", "96x96", "64x64", "48x48", "scalable"] {
            for ext in &["png", "svg", "svgz"] {
                // Try in apps subdirectory
                let p = icon_dir
                    .join(size)
                    .join("apps")
                    .join(format!("{}.{}", icon_name, ext));
                if p.exists() {
                    return Some(p);
                }

                // Try without apps subdirectory
                let p = icon_dir.join(size).join(format!("{}.{}", icon_name, ext));
                if p.exists() {
                    return Some(p);
                }
            }
        }
    }
    None
}

fn load_system_icon(icon_name: &str, size: u32) -> Option<RawIcon> {
    let path = find_system_icon_path(icon_name)?;
    if path
        .extension()
        .map(|e| e == "svg" || e == "svgz")
        .unwrap_or(false)
    {
        read_svg_bytes(&path).and_then(|bytes| render_svg(&bytes, size))
    } else {
        load_png_icon(&path, size)
    }
}

fn load_icon_from_desktop(package_name: &str, size: u32) -> Option<RawIcon> {
    // Search for .desktop files
    let home_dir = std::env::var_os("HOME").map(PathBuf::from);
    let mut desktop_dirs = vec![
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/usr/local/share/applications"),
    ];
    if let Some(home) = home_dir {
        desktop_dirs.push(home.join(".local/share/applications"));
    }

    for dir in &desktop_dirs {
        if !dir.exists() {
            continue;
        }

        // Search for desktop file matching package name
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "desktop").unwrap_or(false) {
                    if let Some(content) = std::fs::read_to_string(&path).ok() {
                        // Check if this desktop file matches our package
                        if content.contains(&format!("Name={}", package_name))
                            || content.contains(&format!("Exec={}", package_name))
                            || path
                                .file_stem()
                                .map(|s| s.to_string_lossy().contains(package_name))
                                .unwrap_or(false)
                        {
                            // Extract Icon= line
                            for line in content.lines() {
                                if line.starts_with("Icon=") {
                                    let icon_name = &line[5..];
                                    if let Some(icon) = load_system_icon(icon_name, size) {
                                        return Some(icon);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}
