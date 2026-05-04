use flate2::read::GzDecoder;
use image::GenericImageView;
use resvg::{tiny_skia, usvg};
use std::io::Read;
use std::path::{Path, PathBuf};

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

pub async fn load_icon(url: &str) -> Option<RawIcon> {
    let bytes = reqwest::get(url).await.ok()?.bytes().await.ok()?;
    let img = image::load_from_memory(&bytes).ok()?;
    let img = img.resize(64, 64, image::imageops::FilterType::Lanczos3);
    let (w, h) = img.dimensions();
    let pixels = img.to_rgba8().into_raw();
    Some(RawIcon {
        width: w,
        height: h,
        pixels,
    })
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

fn home_dir() -> PathBuf {
    std::env::var("HOME").map(PathBuf::from).unwrap_or_default()
}

fn xdg_data_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var("XDG_DATA_DIRS")
        .unwrap_or_else(|_| "/usr/local/share:/usr/share".to_string())
        .split(':')
        .map(PathBuf::from)
        .collect();
    let local = std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home_dir().join(".local/share"));
    dirs.insert(0, local);
    dirs
}

// kdeglobals is a plain ini file, we just hand-parse it because pulling in
// an ini crate for two lines of config isn't worth it
fn kde_icon_theme() -> Option<String> {
    let config_home = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home_dir().join(".config"));
    let content = std::fs::read_to_string(config_home.join("kdeglobals")).ok()?;
    let mut in_section = false;
    for line in content.lines() {
        let line = line.trim();
        if line == "[Icons]" {
            in_section = true;
        } else if line.starts_with('[') {
            in_section = false;
        } else if in_section {
            if let Some(val) = line.strip_prefix("Theme=") {
                return Some(val.to_string());
            }
        }
    }
    None
}

fn find_icon_path(icon_name: &str) -> Option<PathBuf> {
    let theme = kde_icon_theme().unwrap_or_else(|| "hicolor".to_string());
    let data_dirs = xdg_data_dirs();

    for data_dir in &data_dirs {
        let icons_root = data_dir.join("icons");
        for t in &[theme.as_str(), "hicolor"] {
            let theme_dir = icons_root.join(t);
            for size in &["22", "24", "32", "48"] {
                for ext in &["svgz", "svg"] {
                    let p = theme_dir
                        .join("categories")
                        .join(size)
                        .join(format!("{}.{}", icon_name, ext));
                    if p.exists() {
                        return Some(p);
                    }
                }
            }
            for size in &["48x48", "32x32"] {
                for subdir in &["categories", "legacy"] {
                    let p = theme_dir
                        .join(size)
                        .join(subdir)
                        .join(format!("{}.png", icon_name));
                    if p.exists() {
                        return Some(p);
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
    let Some(path) = find_icon_path(icon_name) else {
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

pub fn load_local_flatpak_icon(app_id: &str) -> Option<RawIcon> {
    let home = home_dir();
    let bases = [
        home.join(".local/share/flatpak"),
        std::path::PathBuf::from("/var/lib/flatpak"),
    ];

    // Try app export directories first
    for base in &bases {
        for size in &["128x128", "256x256", "96x96", "64x64", "48x48", "32x32"] {
            // Try PNG first
            for ext in &["png", "svg", "svgz"] {
                let p = base
                    .join("app")
                    .join(app_id)
                    .join("current/active/export/share/icons/hicolor")
                    .join(size)
                    .join("apps")
                    .join(format!("{}.{}", app_id, ext));
                if p.exists() {
                    if ext == &"png" {
                        if let Some(icon) = load_png_icon(&p, 48) {
                            return Some(icon);
                        }
                    } else {
                        if let Some(icon) =
                            read_svg_bytes(&p).and_then(|bytes| render_svg(&bytes, 48))
                        {
                            return Some(icon);
                        }
                    }
                }
            }
        }

        // Also try runtime directory (some flatpaks store icons here)
        for size in &["128x128", "256x256", "96x96", "64x64", "48x48"] {
            for ext in &["png", "svg", "svgz"] {
                let p = base
                    .join("app")
                    .join(app_id)
                    .join("current/active/files/share/icons/hicolor")
                    .join(size)
                    .join("apps")
                    .join(format!("{}.{}", app_id, ext));
                if p.exists() {
                    if ext == &"png" {
                        if let Some(icon) = load_png_icon(&p, 48) {
                            return Some(icon);
                        }
                    } else {
                        if let Some(icon) =
                            read_svg_bytes(&p).and_then(|bytes| render_svg(&bytes, 48))
                        {
                            return Some(icon);
                        }
                    }
                }
            }
        }
    }

    // Try user's local icon theme directories
    let local_icons = home.join(".local/share/icons");
    for size in &["128x128", "96x96", "64x64", "48x48"] {
        for ext in &["png", "svg", "svgz"] {
            let p = local_icons
                .join(size)
                .join("apps")
                .join(format!("{}.{}", app_id, ext));
            if p.exists() {
                if ext == &"png" {
                    if let Some(icon) = load_png_icon(&p, 48) {
                        return Some(icon);
                    }
                } else {
                    if let Some(icon) = read_svg_bytes(&p).and_then(|bytes| render_svg(&bytes, 48))
                    {
                        return Some(icon);
                    }
                }
            }
        }
    }

    // Try system icon themes
    let system_icon_dirs = [
        PathBuf::from("/usr/share/icons"),
        PathBuf::from("/usr/local/share/icons"),
    ];
    for icon_dir in &system_icon_dirs {
        for size in &["128x128", "96x96", "64x64", "48x48"] {
            for ext in &["png", "svg", "svgz"] {
                let p = icon_dir
                    .join("hicolor")
                    .join(size)
                    .join("apps")
                    .join(format!("{}.{}", app_id, ext));
                if p.exists() {
                    if ext == &"png" {
                        if let Some(icon) = load_png_icon(&p, 48) {
                            return Some(icon);
                        }
                    } else {
                        if let Some(icon) =
                            read_svg_bytes(&p).and_then(|bytes| render_svg(&bytes, 48))
                        {
                            return Some(icon);
                        }
                    }
                }
            }
        }
    }

    // Fallback: try to find icon from .desktop file
    if let Some(icon) = load_icon_from_desktop(app_id) {
        return Some(icon);
    }

    // Last resort: try variations of the app_id as icon name
    load_icon_by_name_variations(app_id)
}

// Load icon for native/Distrobox packages from system icon themes and desktop files
pub fn load_native_package_icon(package_name: &str) -> Option<RawIcon> {
    // First, try to find icon from .desktop file
    if let Some(icon) = load_icon_from_desktop(package_name) {
        return Some(icon);
    }

    // Generate possible icon names from package name
    let mut icon_names = Vec::new();

    // Add the package name itself
    icon_names.push(package_name.to_string());
    icon_names.push(package_name.to_lowercase());

    // Common application name mappings
    let name_mappings: &[(&str, &[&str])] = &[
        ("bazaar", &["bzr", "Bazaar"]),
        (
            "libreoffice",
            &[
                "libreoffice-writer",
                "libreoffice-calc",
                "libreoffice-impress",
                "LibreOffice",
                "org.libreoffice.LibreOffice",
            ],
        ),
        ("firefox", &["firefox", "Firefox", "mozilla-firefox"]),
        (
            "thunderbird",
            &["thunderbird", "Thunderbird", "mozilla-thunderbird"],
        ),
        ("gimp", &["gimp", "GIMP", "gimp-2.10", "gimp-2.8"]),
        ("inkscape", &["inkscape", "Inkscape"]),
        ("blender", &["blender", "Blender"]),
        ("vlc", &["vlc", "VLC", "videolan"]),
        ("code", &["code", "vscode", "visual-studio-code"]),
        ("steam", &["steam", "Steam"]),
        ("discord", &["discord", "Discord"]),
        ("spotify", &["spotify", "Spotify"]),
    ];

    for (pkg_name, mappings) in name_mappings {
        if pkg_name.eq_ignore_ascii_case(package_name) {
            for mapping in *mappings {
                icon_names.push(mapping.to_string());
            }
        }
    }

    // Add capitalized version
    if let Some(first) = package_name.chars().next() {
        let capitalized = first.to_uppercase().collect::<String>() + &package_name[1..];
        icon_names.push(capitalized);
    }

    // Remove duplicates while preserving order
    let mut seen = std::collections::HashSet::new();
    icon_names.retain(|name| seen.insert(name.clone()));

    // Try to find icon using the possible names in system icon themes
    let home = home_dir();
    let icon_dirs = [
        home.join(".local/share/icons"),
        PathBuf::from("/usr/share/icons"),
        PathBuf::from("/usr/local/share/icons"),
        PathBuf::from("/usr/share/pixmaps"),
    ];

    for icon_name in &icon_names {
        for icon_dir in &icon_dirs {
            for size in &["128x128", "96x96", "64x64", "48x48", "scalable"] {
                for ext in &["png", "svg", "svgz"] {
                    // Try in apps subdirectory
                    let p = icon_dir
                        .join(size)
                        .join("apps")
                        .join(format!("{}.{}", icon_name, ext));
                    if p.exists() {
                        if ext == &"png" {
                            if let Some(icon) = load_png_icon(&p, 48) {
                                return Some(icon);
                            }
                        } else {
                            if let Some(icon) =
                                read_svg_bytes(&p).and_then(|bytes| render_svg(&bytes, 48))
                            {
                                return Some(icon);
                            }
                        }
                    }

                    // Try without apps subdirectory
                    let p = icon_dir.join(size).join(format!("{}.{}", icon_name, ext));
                    if p.exists() {
                        if ext == &"png" {
                            if let Some(icon) = load_png_icon(&p, 48) {
                                return Some(icon);
                            }
                        } else {
                            if let Some(icon) =
                                read_svg_bytes(&p).and_then(|bytes| render_svg(&bytes, 48))
                            {
                                return Some(icon);
                            }
                        }
                    }
                }
            }
        }
    }

    // Try variations of the package name (lowercase, with dashes, etc.)
    load_icon_by_name_variations(package_name)
}

// Try to load icon from .desktop file
fn load_icon_from_desktop(package_name: &str) -> Option<RawIcon> {
    let desktop_dirs = [
        PathBuf::from("/var/lib/flatpak/exports/share/applications"),
        home_dir().join(".local/share/flatpak/exports/share/applications"),
        PathBuf::from("/usr/share/applications"),
        home_dir().join(".local/share/applications"),
    ];

    // Generate possible desktop file names from package name
    // e.g., "libreoffice" -> ["libreoffice.desktop", "libreoffice-writer.desktop", "org.libreoffice.LibreOffice.desktop"]
    let mut desktop_names = Vec::new();
    desktop_names.push(format!("{}.desktop", package_name));
    desktop_names.push(format!("{}-writer.desktop", package_name));
    desktop_names.push(format!("{}-calc.desktop", package_name));
    desktop_names.push(format!("{}-impress.desktop", package_name));
    desktop_names.push(format!("{}.desktop", package_name.to_lowercase()));

    // Try reverse-DNS style names (common for LibreOffice, etc.)
    let capitalized = package_name
        .chars()
        .next()
        .unwrap_or('A')
        .to_uppercase()
        .collect::<String>()
        + &package_name[1..];
    desktop_names.push(format!(
        "org.{}.{}.desktop",
        package_name.to_lowercase(),
        capitalized
    ));
    desktop_names.push(format!("org.libreoffice.LibreOffice.desktop"));

    // Try to find any matching .desktop file
    let desktop_file = desktop_dirs.iter().find_map(|dir| {
        for name in &desktop_names {
            let p = dir.join(name);
            if p.exists() {
                return Some(p);
            }
        }
        None
    })?;

    // Parse Icon= line from desktop file
    let content = std::fs::read_to_string(&desktop_file).ok()?;
    let icon_name = content
        .lines()
        .find(|line| line.starts_with("Icon="))
        .and_then(|line| line.strip_prefix("Icon="))?;

    if icon_name.is_empty() {
        return None;
    }

    // Handle absolute paths in Icon= field
    if icon_name.starts_with('/') {
        let icon_path = Path::new(icon_name);
        if icon_path.exists() {
            let ext = icon_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("png");
            if ext == "png" {
                return load_png_icon(icon_path, 48);
            } else if ext == "svg" || ext == "svgz" {
                return read_svg_bytes(icon_path).and_then(|bytes| render_svg(&bytes, 48));
            }
        }
        return None;
    }

    // Try to find the icon using the icon name from desktop file
    let icon_dirs = [
        home_dir().join(".local/share/icons"),
        PathBuf::from("/usr/share/icons"),
        PathBuf::from("/usr/local/share/icons"),
        home_dir().join(".local/share/flatpak/exports/share/icons"),
        PathBuf::from("/var/lib/flatpak/exports/share/icons"),
    ];

    // Also check /usr/share/pixmaps (common location for app icons)
    let pixmaps_dir = PathBuf::from("/usr/share/pixmaps");

    for icon_dir in &icon_dirs {
        for size in &["128x128", "96x96", "64x64", "48x48", "scalable"] {
            for ext in &["png", "svg", "svgz"] {
                // Try in apps subdirectory
                let p = icon_dir
                    .join(size)
                    .join("apps")
                    .join(format!("{}.{}", icon_name, ext));
                if p.exists() {
                    if ext == &"png" {
                        if let Some(icon) = load_png_icon(&p, 48) {
                            return Some(icon);
                        }
                    } else {
                        if let Some(icon) =
                            read_svg_bytes(&p).and_then(|bytes| render_svg(&bytes, 48))
                        {
                            return Some(icon);
                        }
                    }
                }

                // Try without apps subdirectory (some themes put icons directly in size folder)
                let p = icon_dir.join(size).join(format!("{}.{}", icon_name, ext));
                if p.exists() {
                    if ext == &"png" {
                        if let Some(icon) = load_png_icon(&p, 48) {
                            return Some(icon);
                        }
                    } else {
                        if let Some(icon) =
                            read_svg_bytes(&p).and_then(|bytes| render_svg(&bytes, 48))
                        {
                            return Some(icon);
                        }
                    }
                }
            }
        }
    }

    // Check /usr/share/pixmaps as a fallback
    for ext in &["png", "svg", "svgz", "xpm"] {
        let p = pixmaps_dir.join(format!("{}.{}", icon_name, ext));
        if p.exists() {
            if ext == &"png" {
                if let Some(icon) = load_png_icon(&p, 48) {
                    return Some(icon);
                }
            } else if ext == &"svg" || ext == &"svgz" {
                if let Some(icon) = read_svg_bytes(&p).and_then(|bytes| render_svg(&bytes, 48)) {
                    return Some(icon);
                }
            } else if ext == &"xpm" {
                // XPM support - try to load as PNG alternative
                if let Some(icon) = load_png_icon(&p, 48) {
                    return Some(icon);
                }
            }
        }
    }

    None
}

// Try to find icon using common name variations
fn load_icon_by_name_variations(app_id: &str) -> Option<RawIcon> {
    let home = home_dir();

    // Generate possible icon names from app_id
    // e.g., "org.libreoffice.LibreOffice" -> ["libreoffice", "LibreOffice", "org.libreoffice.LibreOffice"]
    let parts: Vec<&str> = app_id.split('.').collect();
    let mut possible_names = Vec::new();

    // Try last component (e.g., "LibreOffice")
    if let Some(last) = parts.last() {
        possible_names.push(last.to_lowercase());
        possible_names.push(last.to_string());
    }

    // Try second-to-last + last (e.g., "office.LibreOffice" -> "libreoffice")
    if parts.len() >= 2 {
        possible_names.push(parts[parts.len() - 2..].join("-").to_lowercase());
    }

    // Try full app_id lowercase
    possible_names.push(app_id.to_lowercase());

    // Remove duplicates while preserving order
    let mut seen = std::collections::HashSet::new();
    possible_names.retain(|name| seen.insert(name.clone()));

    let icon_dirs = [
        home.join(".local/share/icons"),
        PathBuf::from("/usr/share/icons"),
        PathBuf::from("/usr/local/share/icons"),
        home.join(".local/share/flatpak/exports/share/icons"),
        PathBuf::from("/var/lib/flatpak/exports/share/icons"),
    ];

    for icon_name in &possible_names {
        for icon_dir in &icon_dirs {
            for size in &["128x128", "96x96", "64x64", "48x48", "scalable"] {
                for ext in &["png", "svg", "svgz"] {
                    // Try in apps subdirectory
                    let p = icon_dir
                        .join(size)
                        .join("apps")
                        .join(format!("{}.{}", icon_name, ext));
                    if p.exists() {
                        if ext == &"png" {
                            if let Some(icon) = load_png_icon(&p, 48) {
                                return Some(icon);
                            }
                        } else {
                            if let Some(icon) =
                                read_svg_bytes(&p).and_then(|bytes| render_svg(&bytes, 48))
                            {
                                return Some(icon);
                            }
                        }
                    }

                    // Try without apps subdirectory (some themes put icons directly in size folder)
                    let p = icon_dir.join(size).join(format!("{}.{}", icon_name, ext));
                    if p.exists() {
                        if ext == &"png" {
                            if let Some(icon) = load_png_icon(&p, 48) {
                                return Some(icon);
                            }
                        } else {
                            if let Some(icon) =
                                read_svg_bytes(&p).and_then(|bytes| render_svg(&bytes, 48))
                            {
                                return Some(icon);
                            }
                        }
                    }
                }
            }
        }
    }

    None
}
