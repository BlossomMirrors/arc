use flate2::read::GzDecoder;
use icon_loader::IconLoader;
use image::GenericImageView;
use resvg::{tiny_skia, usvg};
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct RawIcon {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl RawIcon {
    pub fn to_slint_image(&self) -> slint::Image {
        let mut buf = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(self.width, self.height);
        buf.make_mut_bytes().copy_from_slice(&self.pixels);
        slint::Image::from_rgba8(buf)
    }
}

#[allow(dead_code)]
pub async fn load_icon(url: &str) -> Option<RawIcon> {
    let bytes = reqwest::get(url).await.ok()?.bytes().await.ok()?;
    let img = image::load_from_memory(&bytes).ok()?;
    let img = img.resize(64, 64, image::imageops::FilterType::Lanczos3);
    let (w, h) = img.dimensions();
    Some(RawIcon {
        width: w,
        height: h,
        pixels: img.to_rgba8().into_raw(),
    })
}

pub async fn load_screenshot(url: &str) -> Option<RawIcon> {
    let bytes = reqwest::get(url).await.ok()?.bytes().await.ok()?;
    let img = image::load_from_memory(&bytes).ok()?;
    let img = img.resize(624, 351, image::imageops::FilterType::Lanczos3);
    let (w, h) = img.dimensions();
    Some(RawIcon {
        width: w,
        height: h,
        pixels: img.to_rgba8().into_raw(),
    })
}

pub async fn load_lutris_icon(icon_url: &str) -> Option<RawIcon> {
    load_icon(icon_url).await
}

fn read_svg_bytes(path: &Path) -> Option<Vec<u8>> {
    let raw = std::fs::read(path).ok()?;
    if path.extension().and_then(|e| e.to_str()) == Some("svgz") {
        let mut decoder = GzDecoder::new(&raw[..]);
        let mut buf = Vec::new();
        decoder.read_to_end(&mut buf).ok()?;
        Some(buf)
    } else {
        Some(raw)
    }
}

// resvg outputs premultiplied rgba so we have to undo that before handing pixels
// to slint, otherwise semi-transparent areas look wrong (colors get darker)
fn render_svg(svg_bytes: &[u8], size: u32) -> Option<RawIcon> {
    let opt = usvg::Options::default();
    let tree = usvg::Tree::from_data(svg_bytes, &opt).ok()?;
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

// parse stop-color="#rrggbb" out of svg text to get gradient colors without
// pulling in a full xml parser, good enough for icon theme svgs
fn extract_stop_colors(svg_text: &str) -> Vec<(u8, u8, u8)> {
    let mut colors = Vec::new();
    let mut src = svg_text;
    let needle = "stop-color=\"#";
    while let Some(pos) = src.find(needle) {
        let start = pos + needle.len();
        if start + 6 <= src.len() {
            let hex = &src[start..start + 6];
            if let (Ok(r), Ok(g), Ok(b)) = (
                u8::from_str_radix(&hex[0..2], 16),
                u8::from_str_radix(&hex[2..4], 16),
                u8::from_str_radix(&hex[4..6], 16),
            ) {
                colors.push((r, g, b));
            }
        }
        src = &src[pos + needle.len()..];
    }
    colors
}

fn most_saturated(colors: &[(u8, u8, u8)]) -> Option<(u8, u8, u8)> {
    colors.iter().copied().max_by_key(|(r, g, b)| {
        let max = (*r as i32).max(*g as i32).max(*b as i32);
        let min = (*r as i32).min(*g as i32).min(*b as i32);
        max - min
    })
}

fn dominant_color_from_pixels(pixels: &[u8]) -> (u8, u8, u8) {
    let mut r_sum = 0u64;
    let mut g_sum = 0u64;
    let mut b_sum = 0u64;
    let mut count = 0u64;
    for chunk in pixels.chunks_exact(4) {
        if chunk[3] > 30 {
            r_sum += chunk[0] as u64;
            g_sum += chunk[1] as u64;
            b_sum += chunk[2] as u64;
            count += 1;
        }
    }
    if count == 0 {
        return (80, 80, 90);
    }
    (
        (r_sum / count) as u8,
        (g_sum / count) as u8,
        (b_sum / count) as u8,
    )
}

fn darken((r, g, b): (u8, u8, u8), factor: f32) -> (u8, u8, u8) {
    (
        (r as f32 * factor) as u8,
        (g as f32 * factor) as u8,
        (b as f32 * factor) as u8,
    )
}

fn load_png_icon(path: &Path, size: u32) -> Option<RawIcon> {
    let img = image::open(path).ok()?;
    let img = img.resize(size, size, image::imageops::FilterType::Lanczos3);
    let (w, h) = img.dimensions();
    Some(RawIcon {
        width: w,
        height: h,
        pixels: img.to_rgba8().into_raw(),
    })
}

fn load_icon_from_path(path: &Path, size: u32) -> Option<RawIcon> {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "png" => load_png_icon(path, size),
        "svg" | "svgz" => read_svg_bytes(path).and_then(|bytes| render_svg(&bytes, size)),
        _ => None,
    }
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
}

fn system_theme_name() -> Option<String> {
    let config_home = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home_dir().join(".config"));
    for cfg in ["gtk-4.0/settings.ini", "gtk-3.0/settings.ini"] {
        if let Ok(content) = std::fs::read_to_string(config_home.join(cfg)) {
            for line in content.lines() {
                if let Some(val) = line.trim().strip_prefix("gtk-icon-theme-name=") {
                    return Some(val.to_string());
                }
            }
        }
    }
    if let Ok(content) = std::fs::read_to_string(config_home.join("kdeglobals")) {
        let mut in_icons = false;
        for line in content.lines() {
            let line = line.trim();
            if line == "[Icons]" {
                in_icons = true;
            } else if line.starts_with('[') {
                in_icons = false;
            } else if in_icons {
                if let Some(val) = line.strip_prefix("Theme=") {
                    return Some(val.to_string());
                }
            }
        }
    }
    None
}

// Try the icon in each theme in order, stopping at the first hit.
// IconLoader::new() uses SearchPaths::System (XDG base dirs) by default.
// The default ThemeNameProvider is User("hicolor") which is useless for
// category icons, so we detect the system theme ourselves and try a
// fallback chain.
fn find_system_icon(icon_name: &str, size: u16) -> Option<PathBuf> {
    let mut themes: Vec<String> = Vec::new();
    if let Some(t) = system_theme_name() {
        themes.push(t);
    }
    for t in ["BlossomUI", "breeze", "Adwaita", "hicolor"] {
        if !themes.iter().any(|x| x == t) {
            themes.push(t.to_string());
        }
    }

    let flatpak_extra = [
        home_dir().join(".local/share/flatpak/exports/share/icons"),
        PathBuf::from("/var/lib/flatpak/exports/share/icons"),
    ];

    for theme in &themes {
        let mut loader = IconLoader::new();
        let mut paths = loader.search_paths().to_vec();
        for p in &flatpak_extra {
            if !paths.contains(p) {
                paths.push(p.clone());
            }
        }
        loader.set_search_paths(paths);
        loader.set_theme_name_provider(theme.as_str());
        if loader.update_theme_name().is_err() {
            continue;
        }
        if let Some(icon) = loader.load_icon(icon_name) {
            let path = icon.file_for_size(size).path().to_path_buf();
            if path.exists() {
                return Some(path);
            }
        }
    }
    None
}

fn find_flatpak_icon_path(icon_name: &str) -> Option<PathBuf> {
    if let Some(path) = search_flatpak_icon_dir("/var/lib/flatpak/appstream", icon_name) {
        return Some(path);
    }
    if let Some(home) = std::env::var_os("HOME") {
        let user_path = PathBuf::from(home).join(".local/share/flatpak/appstream");
        if let Some(found) = search_flatpak_icon_dir(&user_path, icon_name) {
            return Some(found);
        }
    }
    None
}

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
            for size in ["128x128", "96x96", "64x64", "48x48", "scalable"] {
                let size_dir = icons_dir.join(size);
                if !size_dir.exists() {
                    continue;
                }
                let direct_path = size_dir.join(icon_name);
                if direct_path.exists() {
                    return Some(direct_path);
                }
                if !icon_name.ends_with(".png")
                    && !icon_name.ends_with(".svg")
                    && !icon_name.ends_with(".svgz")
                {
                    for ext in ["png", "svg", "svgz"] {
                        let p = size_dir.join(format!("{}.{}", icon_name, ext));
                        if p.exists() {
                            return Some(p);
                        }
                    }
                }
            }
        }
    }
    None
}

pub struct CategoryIconData {
    pub icon: Option<RawIcon>,
    pub color: (u8, u8, u8),
}

pub fn load_category_icon(icon_name: &str) -> CategoryIconData {
    let Some(path) = find_system_icon(icon_name, 48) else {
        return CategoryIconData {
            icon: None,
            color: (80, 80, 90),
        };
    };

    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if ext == "png" {
        let icon = load_png_icon(&path, 48);
        let color = icon
            .as_ref()
            .map(|r| dominant_color_from_pixels(&r.pixels))
            .unwrap_or((80, 80, 90));
        return CategoryIconData { icon, color };
    }

    let Some(svg_bytes) = read_svg_bytes(&path) else {
        return CategoryIconData {
            icon: None,
            color: (80, 80, 90),
        };
    };

    let svg_text = String::from_utf8_lossy(&svg_bytes);
    let stops = extract_stop_colors(&svg_text);
    let color = most_saturated(&stops).unwrap_or((80, 80, 90));
    let color = darken(color, 0.72);
    let icon = render_svg(&svg_bytes, 48);

    CategoryIconData { icon, color }
}

pub fn load_local_flatpak_icon(app_id: &str) -> Option<RawIcon> {
    if let Some(path) = find_flatpak_icon_path(app_id) {
        return load_icon_from_path(&path, 64);
    }

    let home = home_dir();
    let bases = [
        home.join(".local/share/flatpak"),
        PathBuf::from("/var/lib/flatpak"),
    ];
    for base in &bases {
        for size in &["128x128", "256x256", "96x96", "64x64", "48x48", "32x32"] {
            for ext in &["png", "svg", "svgz"] {
                let p = base
                    .join("app")
                    .join(app_id)
                    .join("current/active/export/share/icons/hicolor")
                    .join(size)
                    .join("apps")
                    .join(format!("{}.{}", app_id, ext));
                if p.exists() {
                    if let Some(icon) = load_icon_from_path(&p, 64) {
                        return Some(icon);
                    }
                }
            }
        }
    }

    load_icon_by_name_variations(app_id)
}

fn load_icon_by_name_variations(app_id: &str) -> Option<RawIcon> {
    let parts: Vec<&str> = app_id.split('.').collect();
    let mut names = Vec::new();
    if let Some(last) = parts.last() {
        names.push(last.to_lowercase());
        names.push(last.to_string());
    }
    if parts.len() >= 2 {
        names.push(parts[parts.len() - 2..].join("-").to_lowercase());
    }
    names.push(app_id.to_lowercase());

    let mut seen = std::collections::HashSet::new();
    names.retain(|n| seen.insert(n.clone()));

    for name in &names {
        if let Some(path) = find_system_icon(name, 48) {
            if let Some(icon) = load_icon_from_path(&path, 48) {
                return Some(icon);
            }
        }
    }
    None
}

pub fn load_appimage_icon(icon_url: Option<&str>, stem: &str) -> Option<RawIcon> {
    // Try the stored path first (set by the daemon on install, may be PNG or SVG)
    if let Some(path_str) = icon_url {
        let p = Path::new(path_str);
        if p.exists() {
            if let Some(icon) = load_icon_from_path(p, 64) {
                return Some(icon);
            }
        }
    }
    // Fall back to stem-based scan of the icons directory (covers legacy installs)
    let icons_dir = home_dir().join(".local/share/arc/appimages/icons");
    for ext in ["png", "svg", "svgz"] {
        let p = icons_dir.join(format!("{}.{}", stem, ext));
        if p.exists() {
            if let Some(icon) = load_icon_from_path(&p, 64) {
                return Some(icon);
            }
        }
    }
    None
}

pub fn load_ui_icon(icon_name: &str) -> Option<RawIcon> {
    find_system_icon(icon_name, 16).and_then(|path| load_icon_from_path(&path, 16))
}

pub fn load_native_package_icon(package_name: &str) -> Option<RawIcon> {
    if let Some(icon) = load_icon_from_desktop(package_name) {
        return Some(icon);
    }

    let mut names = vec![package_name.to_string(), package_name.to_lowercase()];
    if let Some(c) = package_name.chars().next() {
        names.push(c.to_uppercase().collect::<String>() + &package_name[1..]);
    }
    let mut seen = std::collections::HashSet::new();
    names.retain(|n| seen.insert(n.clone()));

    for name in &names {
        if let Some(path) = find_system_icon(name, 48) {
            if let Some(icon) = load_icon_from_path(&path, 48) {
                return Some(icon);
            }
        }
    }

    load_icon_by_name_variations(package_name)
}

fn load_icon_from_desktop(package_name: &str) -> Option<RawIcon> {
    let desktop_dirs = [
        PathBuf::from("/var/lib/flatpak/exports/share/applications"),
        home_dir().join(".local/share/flatpak/exports/share/applications"),
        PathBuf::from("/usr/share/applications"),
        home_dir().join(".local/share/applications"),
    ];

    let capitalized = package_name
        .chars()
        .next()
        .unwrap_or('A')
        .to_uppercase()
        .collect::<String>()
        + &package_name[1..];

    let desktop_names = [
        format!("{}.desktop", package_name),
        format!("{}.desktop", package_name.to_lowercase()),
        format!("org.{}.{}.desktop", package_name.to_lowercase(), capitalized),
    ];

    let desktop_file = desktop_dirs.iter().find_map(|dir| {
        desktop_names
            .iter()
            .map(|name| dir.join(name))
            .find(|p| p.exists())
    })?;

    let content = std::fs::read_to_string(&desktop_file).ok()?;
    let icon_name = content
        .lines()
        .find(|line| line.starts_with("Icon="))
        .and_then(|line| line.strip_prefix("Icon="))?;

    if icon_name.is_empty() {
        return None;
    }

    if icon_name.starts_with('/') {
        let path = Path::new(icon_name);
        return if path.exists() {
            load_icon_from_path(path, 48)
        } else {
            None
        };
    }

    // Try /usr/share/pixmaps first (not covered by icon_loader)
    let pixmaps = Path::new("/usr/share/pixmaps");
    for ext in ["png", "svg", "svgz", "xpm"] {
        let p = pixmaps.join(format!("{}.{}", icon_name, ext));
        if p.exists() {
            if let Some(icon) = load_icon_from_path(&p, 48) {
                return Some(icon);
            }
        }
    }

    find_system_icon(icon_name, 48).and_then(|path| load_icon_from_path(&path, 48))
}
