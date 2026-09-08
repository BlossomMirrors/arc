use libarc::icons::{find_flatpak_appstream_icon, find_flatpak_export_icon, find_icon_theme_icon};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

static CACHE_GENERATION: AtomicU32 = AtomicU32::new(0);

pub fn resolve(pkg_id: &str, raw_icon_url: Option<&str>) -> String {
    if let Some(p) = find_icon_theme_icon(pkg_id) {
        return padded_file_url(pkg_id, &p);
    }
    if let Some(url) = raw_icon_url {
        if !url.is_empty() && !matches!(libarc::media::classify(url), libarc::media::MediaRef::Local) {
            return url.to_string();
        }
    }
    find_flatpak_appstream_icon(pkg_id)
        .or_else(|| find_flatpak_export_icon(pkg_id))
        .map(|p| padded_file_url(pkg_id, &p))
        .unwrap_or_else(|| pkg_id.to_string())
}

fn padded_file_url(pkg_id: &str, source: &Path) -> String {
    let path = normalized_icon_path(pkg_id, source).unwrap_or_else(|| source.to_path_buf());
    format!("file://{}", path.display())
}

pub fn icon_cache_dir() -> Option<PathBuf> {
    libarc::cache::namespace("icon-pad")
}

pub fn clear_icon_cache() -> std::io::Result<()> {
    let Some(dir) = icon_cache_dir() else { return Ok(()) };
    let result = match fs::remove_dir_all(&dir) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    };
    CACHE_GENERATION.fetch_add(1, Ordering::Relaxed);
    result
}

fn normalized_icon_path(pkg_id: &str, source: &Path) -> Option<PathBuf> {
    let ext = source.extension().and_then(|e| e.to_str()).unwrap_or("");
    let is_svg = ext.eq_ignore_ascii_case("svg") || ext.eq_ignore_ascii_case("svgz");

    let generation = CACHE_GENERATION.load(Ordering::Relaxed);
    let cache_dir = icon_cache_dir()?;
    // source (not just pkg_id) is part of the key so a theme switch, which
    // changes which file resolve() finds for the same pkg_id, lands on a
    // fresh cache entry instead of matching the old theme's stale one
    let source_hash = hash_path(source);
    let cache_path = cache_dir.join(format!("{}-{source_hash}-{generation}.png", sanitize_filename(pkg_id)));

    let source_mtime = fs::metadata(source).ok()?.modified().ok()?;
    if let Ok(cache_mtime) = fs::metadata(&cache_path).and_then(|m| m.modified()) {
        if cache_mtime >= source_mtime {
            return Some(cache_path);
        }
    }

    let bytes = fs::read(source).ok()?;
    let padded = if is_svg {
        libarc::icons::normalize_padding_svg(&bytes)?
    } else {
        libarc::icons::normalize_padding(&bytes)?
    };
    fs::create_dir_all(&cache_dir).ok()?;
    fs::write(&cache_path, &padded).ok()?;
    Some(cache_path)
}

fn sanitize_filename(pkg_id: &str) -> String {
    pkg_id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') { c } else { '_' })
        .collect()
}

fn hash_path(path: &Path) -> String {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut hash = FNV_OFFSET;
    for byte in path.to_string_lossy().as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("{hash:016x}")
}
