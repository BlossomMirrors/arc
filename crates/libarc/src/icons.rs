use image::{imageops::FilterType, DynamicImage, GenericImageView, Rgba, RgbaImage};
use ini::Ini;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const OUTPUT_SIZE: u32 = 256;
const PADDING_PX: u32 = 22;
const CONTENT_RATIO: f32 = 1.0 - (2.0 * PADDING_PX as f32) / OUTPUT_SIZE as f32;
const ALPHA_THRESHOLD: u8 = 10;
const MIN_SOURCE_SIZE: u32 = 48;
const DEFAULT_SIZE_DIRS: &[&str; 7] = &["scalable", "256x256", "128x128", "96x96", "64x64", "48x48", "32x32"];
//
// padding / normalization
//
pub fn normalize_padding(bytes: &[u8]) -> Option<Vec<u8>> {
    let img = image::load_from_memory(bytes).ok()?;
    let (w, h) = img.dimensions();
    if w.max(h) < MIN_SOURCE_SIZE {
        return None;
    }
    normalize_padding_image(img)
}

pub fn normalize_padding_svg(bytes: &[u8]) -> Option<Vec<u8>> {
    let png = rasterize_svg(bytes, OUTPUT_SIZE)?;
    let img = image::load_from_memory(&png).ok()?;
    normalize_padding_image(img).or(Some(png))
}

fn rasterize_svg(bytes: &[u8], size: u32) -> Option<Vec<u8>> {
    let opt = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_data(bytes, &opt).ok()?;

    let tree_size = tree.size();
    let scale = (size as f32 / tree_size.width()).min(size as f32 / tree_size.height());
    let dx = (size as f32 - tree_size.width() * scale) / 2.0;
    let dy = (size as f32 - tree_size.height() * scale) / 2.0;
    let transform = resvg::tiny_skia::Transform::from_scale(scale, scale).post_translate(dx, dy);

    let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size)?;
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    pixmap.encode_png().ok()
}

fn normalize_padding_image(img: DynamicImage) -> Option<Vec<u8>> {
    let (native_w, native_h) = img.dimensions();
    let native_rgba = img.into_rgba8();

    let mut min_x = native_w;
    let mut min_y = native_h;
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    let mut has_content = false;
    for (x, y, px) in native_rgba.enumerate_pixels() {
        if px[3] > ALPHA_THRESHOLD {
            has_content = true;
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
    }
    if !has_content {
        return None;
    }

    let content_w = (max_x - min_x + 1) as f32;
    let content_h = (max_y - min_y + 1) as f32;
    let current_ratio = (content_w / native_w as f32).max(content_h / native_h as f32);

    if current_ratio < CONTENT_RATIO && content_w.max(content_h) < MIN_SOURCE_SIZE as f32 {
        return None;
    }

    let scale = CONTENT_RATIO / current_ratio;
    let base = DynamicImage::ImageRgba8(native_rgba)
        .resize_exact(OUTPUT_SIZE, OUTPUT_SIZE, FilterType::Lanczos3)
        .into_rgba8();

    let scaled_w = ((OUTPUT_SIZE as f32) * scale).round().max(1.0) as u32;
    let scaled_h = ((OUTPUT_SIZE as f32) * scale).round().max(1.0) as u32;
    let scaled = image::imageops::resize(&base, scaled_w, scaled_h, FilterType::Lanczos3);

    let mut canvas = RgbaImage::from_pixel(OUTPUT_SIZE, OUTPUT_SIZE, Rgba([0, 0, 0, 0]));
    let off_x = (OUTPUT_SIZE as i64 - scaled_w as i64) / 2;
    let off_y = (OUTPUT_SIZE as i64 - scaled_h as i64) / 2;
    image::imageops::overlay(&mut canvas, &scaled, off_x, off_y);

    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(canvas)
        .write_to(&mut out, image::ImageFormat::Png)
        .ok()?;
    Some(out.into_inner())
}
//
// theme / appstream
//
pub fn icon_content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "svg" | "svgz" => "image/svg+xml",
        _ => "image/png",
    }
}

pub fn system_theme_name() -> Option<String> {
    let config_dir = dirs::config_dir()?;
    read_ini_value(&config_dir.join("kdeglobals"), "Icons", "Theme")
        .or_else(|| read_ini_value(&config_dir.join("gtk-4.0/settings.ini"), "Settings", "gtk-icon-theme-name"))
        .or_else(|| read_ini_value(&config_dir.join("gtk-3.0/settings.ini"), "Settings", "gtk-icon-theme-name"))
}

fn read_ini_value(path: &Path, section: &str, key: &str) -> Option<String> {
    let ini = Ini::load_from_file(path).ok()?;
    let value = ini.section(Some(section))?.get(key)?;
    (!value.is_empty()).then(|| value.to_string())
}

pub fn home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_default()
}

pub fn find_icon_theme_icon(app_id: &str) -> Option<PathBuf> {
    let mut bases = vec![home_dir().join(".local/share/icons"), home_dir().join(".icons")];
    if let Ok(xdg_data_dirs) = env::var("XDG_DATA_DIRS") {
        bases.extend(env::split_paths(&xdg_data_dirs).map(|p| p.join("icons")));
    } else {
        bases.push(PathBuf::from("/usr/local/share/icons"));
        bases.push(PathBuf::from("/usr/share/icons"));
    }

    let mut themes = system_theme_name().into_iter().collect::<Vec<_>>();
    themes.push("hicolor".to_string());

    for base in &bases {
        for theme in &themes {
            if let Some(p) = find_in_theme_dir(&base.join(theme), app_id) {
                return Some(p);
            }
        }
    }
    None
}

pub fn find_in_theme_dir(theme_dir: &Path, app_id: &str) -> Option<PathBuf> {
    for dir in icon_dirs(theme_dir, "apps") {
        for ext in ["png", "svg", "svgz"] {
            let p = theme_dir.join(&dir).join(format!("{app_id}.{ext}"));
            if p.exists() {
                return Some(p);
            }
        }
    }
    None
}

pub fn icon_dirs(theme_dir: &Path, context: &str) -> Vec<String> {
    let configured = Ini::load_from_file(theme_dir.join("index.theme"))
        .ok()
        .and_then(|ini| ini.section(Some("Icon Theme"))?.get("Directories").map(|d| d.to_string()));

    if let Some(list) = configured {
        let mut dirs: Vec<String> = list
            .split(',')
            .map(|d| d.trim().to_string())
            .filter(|d| d.ends_with(&format!("/{context}")))
            .collect();
        if !dirs.is_empty() {
            dirs.sort_by_key(|d| std::cmp::Reverse(dir_size_rank(d)));
            return dirs;
        }
    }

    DEFAULT_SIZE_DIRS.iter().map(|size| format!("{size}/{context}")).collect()
}

fn dir_size_rank(dir: &str) -> u32 {
    let size = dir.split('/').next().unwrap_or("");
    if size.starts_with("scalable") {
        return u32::MAX;
    }
    size.split(['x', '@']).next().and_then(|s| s.parse().ok()).unwrap_or(0)
}

pub fn find_flatpak_appstream_icon(app_id: &str) -> Option<PathBuf> {
    search_appstream_root("/var/lib/flatpak/appstream", app_id)
        .or_else(|| search_appstream_root(&home_dir().join(".local/share/flatpak/appstream"), app_id))
}

fn search_appstream_root(base: impl AsRef<Path>, app_id: &str) -> Option<PathBuf> {
    let base = base.as_ref();
    let remotes = fs::read_dir(base).ok()?;
    for remote_dir in remotes.flatten() {
        let Ok(arches) = fs::read_dir(remote_dir.path()) else {
            continue;
        };
        for arch in arches.flatten() {
            let icons_dir = arch.path().join("active").join("icons");
            if !icons_dir.exists() {
                continue;
            }
            let search_roots = [icons_dir.clone(), icons_dir.join("flatpak")];
            for root in &search_roots {
                if let Some(p) = search_size_dirs(root, app_id) {
                    return Some(p);
                }
            }
        }
    }
    None
}

fn search_size_dirs(root: &Path, icon_name: &str) -> Option<PathBuf> {
    for size in DEFAULT_SIZE_DIRS {
        let size_dir = root.join(size);
        if !size_dir.exists() {
            continue;
        }
        for ext in ["png", "svg", "svgz"] {
            let p = size_dir.join(format!("{icon_name}.{ext}"));
            if p.exists() {
                return Some(p);
            }
            let p2 = size_dir.join(format!("{icon_name}.desktop.{ext}"));
            if p2.exists() {
                return Some(p2);
            }
        }
    }
    None
}

pub fn find_flatpak_export_icon(app_id: &str) -> Option<PathBuf> {
    let bases = [
        home_dir().join(".local/share/flatpak"),
        PathBuf::from("/var/lib/flatpak"),
    ];
    for base in &bases {
        let hicolor_dir = base
            .join("app")
            .join(app_id)
            .join("current/active/export/share/icons/hicolor");
        if let Some(p) = find_in_theme_dir(&hicolor_dir, app_id) {
            return Some(p);
        }
    }
    None
}
//
// icon install
//
pub fn install_icon(icon_name: &str, bytes: &[u8], is_svg: bool) -> Option<PathBuf> {
    let size_dir = if is_svg { "scalable" } else { "256x256" };
    let ext = if is_svg { "svg" } else { "png" };
    let hicolor_root = home_dir().join(".local/share/icons/hicolor");
    let dest = hicolor_root.join(size_dir).join("apps").join(format!("{icon_name}.{ext}"));

    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).ok()?;
    }
    fs::write(&dest, bytes).ok()?;

    let _ = Command::new("gtk-update-icon-cache")
        .arg("-f")
        .arg("-t")
        .arg(&hicolor_root)
        .status();

    Some(dest)
}
