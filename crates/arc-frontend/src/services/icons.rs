use std::path::{Path, PathBuf};


pub fn resolve(pkg_id: &str, raw_icon_url: Option<&str>) -> String {
    if let Some(url) = raw_icon_url {
        if !url.is_empty() && !url.starts_with("local:") {
            return url.to_string();
        }
    }
    find_flatpak_appstream_icon(pkg_id)
        .or_else(|| find_flatpak_export_icon(pkg_id))
        .map(|p| format!("file://{}", p.display()))
        .unwrap_or_else(|| pkg_id.to_string())
}

fn home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_default()
}

fn find_flatpak_appstream_icon(app_id: &str) -> Option<PathBuf> {
    search_appstream_root("/var/lib/flatpak/appstream", app_id)
        .or_else(|| search_appstream_root(&home_dir().join(".local/share/flatpak/appstream"), app_id))
}

fn search_appstream_root(base: impl AsRef<Path>, app_id: &str) -> Option<PathBuf> {
    let base = base.as_ref();
    let remotes = std::fs::read_dir(base).ok()?;
    for remote_dir in remotes.flatten() {
        let Ok(arches) = std::fs::read_dir(remote_dir.path()) else {
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
    for size in ["128x128", "96x96", "64x64", "48x48", "scalable"] {
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

fn find_flatpak_export_icon(app_id: &str) -> Option<PathBuf> {
    let bases = [
        home_dir().join(".local/share/flatpak"),
        PathBuf::from("/var/lib/flatpak"),
    ];
    for base in &bases {
        for size in ["128x128", "256x256", "96x96", "64x64", "48x48", "32x32", "scalable"] {
            for ext in ["png", "svg", "svgz"] {
                let p = base
                    .join("app")
                    .join(app_id)
                    .join("current/active/export/share/icons/hicolor")
                    .join(size)
                    .join("apps")
                    .join(format!("{app_id}.{ext}"));
                if p.exists() {
                    return Some(p);
                }
            }
        }
    }
    None
}
