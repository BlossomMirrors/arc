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
    Some(RawIcon { width: w, height: h, pixels })
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
    Some(RawIcon { width: size, height: size, pixels })
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
    Some(RawIcon { width: w, height: h, pixels: img.to_rgba8().into_raw() })
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
                    let p = theme_dir.join("categories").join(size).join(format!("{}.{}", icon_name, ext));
                    if p.exists() { return Some(p); }
                }
            }
            for size in &["48x48", "32x32"] {
                for subdir in &["categories", "legacy"] {
                    let p = theme_dir.join(size).join(subdir).join(format!("{}.png", icon_name));
                    if p.exists() { return Some(p); }
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
        return CategoryIconData { icon: None, color: (80, 80, 90) };
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
        return CategoryIconData { icon: None, color: (80, 80, 90) };
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
    ((r_sum / count) as u8, (g_sum / count) as u8, (b_sum / count) as u8)
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
    for base in &bases {
        for size in &["128x128", "256x256", "64x64"] {
            let p = base
                .join("app")
                .join(app_id)
                .join("current/active/export/share/icons/hicolor")
                .join(size)
                .join("apps")
                .join(format!("{}.png", app_id));
            if p.exists() {
                if let Some(icon) = load_png_icon(&p, 48) {
                    return Some(icon);
                }
            }
        }
    }
    None
}
