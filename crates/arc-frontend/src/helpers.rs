use libarc::{connect, ArcDaemonProxy, Package, Provider, Settings};
use slint::Model;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub fn parse_flatpakref(content: &str) -> (String, String, String) {
    let mut name = String::new();
    let mut title = String::new();
    let mut url = String::new();
    for line in content.lines() {
        let line = line.trim();
        if let Some(v) = line.strip_prefix("Name=") {
            name = v.to_string();
        } else if let Some(v) = line.strip_prefix("Title=") {
            title = v.to_string();
        } else if let Some(v) = line.strip_prefix("Url=") {
            url = v.to_string();
        }
    }
    if title.is_empty() {
        title = name.clone();
    }
    (title, name, url)
}

pub fn is_pkg_file(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.ends_with(".deb")
        || lower.ends_with(".rpm")
        || lower.ends_with(".pkg.tar.xz")
        || lower.ends_with(".pkg.tar.zst")
        || lower.ends_with(".appimage")
        || lower.ends_with(".flatpak")
}

pub fn is_flatpak_bundle(path: &str) -> bool {
    path.to_lowercase().ends_with(".flatpak")
}

pub fn is_flatpakrepo(path: &str) -> bool {
    path.to_lowercase().ends_with(".flatpakrepo")
}

pub fn parse_flatpakrepo(content: &str) -> (String, String) {
    let mut title = String::new();
    let mut url = String::new();
    for line in content.lines() {
        let line = line.trim();
        if let Some(v) = line.strip_prefix("Title=") {
            title = v.trim().to_string();
        } else if let Some(v) = line.strip_prefix("Url=") {
            url = v.trim().to_string();
        }
    }
    (title, url)
}

pub fn is_appimage(path: &str) -> bool {
    path.to_lowercase().ends_with(".appimage")
}

pub fn pkg_name_from_filename(filename: &str) -> String {
    if filename.ends_with(".deb") {
        return filename.split('_').next().unwrap_or(filename).to_string();
    }
    if filename.ends_with(".rpm") {
        let no_ext = filename.strip_suffix(".rpm").unwrap_or(filename);
        if let Some(i) = no_ext.find('-') {
            if no_ext[i + 1..].starts_with(|c: char| c.is_ascii_digit()) {
                return no_ext[..i].to_string();
            }
        }
        return no_ext.to_string();
    }
    let no_ext = filename
        .strip_suffix(".pkg.tar.zst")
        .or_else(|| filename.strip_suffix(".pkg.tar.xz"))
        .unwrap_or(filename);
    let parts: Vec<&str> = no_ext.split('-').collect();
    let end = parts
        .iter()
        .position(|p| p.starts_with(|c: char| c.is_ascii_digit()))
        .unwrap_or(parts.len());
    parts[..end].join("-")
}

pub fn dedup_by_preference(pkgs: Vec<Package>, settings: &Settings) -> Vec<Package> {
    use std::collections::HashSet;

    let flatpak_id_by_name: HashMap<String, String> = pkgs
        .iter()
        .filter(|p| p.provider == Provider::Flatpak)
        .map(|p| (p.name.to_lowercase(), p.id.clone()))
        .collect();
    let native_names: HashSet<String> = pkgs
        .iter()
        .filter(|p| p.provider == Provider::Distrobox)
        .map(|p| p.name.to_lowercase())
        .collect();

    pkgs.into_iter()
        .filter(|p| {
            let name = p.name.to_lowercase();
            match p.provider {
                Provider::Flatpak => {
                    !native_names.contains(&name)
                        || settings.preferred_for(&p.id) == Provider::Flatpak
                }
                Provider::Distrobox => {
                    let Some(flatpak_id) = flatpak_id_by_name.get(&name) else {
                        return true;
                    };
                    settings.preferred_for(flatpak_id) == Provider::Distrobox
                }
                Provider::Lutris => {
                    !native_names.contains(&name)
                        || settings.preferred_for(&p.id) == Provider::Lutris
                }
                Provider::AppImage => true,
            }
        })
        .collect()
}

pub fn get_proxy(
    proxy_arc: &Arc<Mutex<Option<ArcDaemonProxy<'static>>>>,
) -> Option<ArcDaemonProxy<'static>> {
    proxy_arc.lock().unwrap().clone()
}

pub async fn check_flathub_verification(app_id: &str) -> bool {
    let url = format!("https://flathub.org/api/v2/appstream/{}", app_id);
    let Ok(resp) = reqwest::get(&url).await else {
        return false;
    };
    let Ok(json) = resp.json::<serde_json::Value>().await else {
        return false;
    };
    json.get("metadata")
        .and_then(|m| m.get("flathub::verification::verified"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

pub async fn get_or_connect(
    proxy_arc: &Arc<Mutex<Option<ArcDaemonProxy<'static>>>>,
) -> Option<ArcDaemonProxy<'static>> {
    if let Some(p) = get_proxy(proxy_arc) {
        return Some(p);
    }
    if let Ok(proxy) = connect().await {
        *proxy_arc.lock().unwrap() = Some(proxy.clone());
        Some(proxy)
    } else {
        None
    }
}

pub fn get_display_name(app: &crate::AppWindow, pkg_id: &str) -> String {
    let detail = app.get_detail_app();
    if detail.flatpak_id.as_str() == pkg_id
        || detail.native_id.as_str() == pkg_id
        || detail.lutris_id.as_str() == pkg_id
    {
        return detail.name.to_string();
    }
    let model = app.get_packages();
    for i in 0..model.row_count() {
        if let Some(p) = model.row_data(i) {
            if p.id.as_str() == pkg_id {
                return p.name.to_string();
            }
        }
    }
    let upd = app.get_available_updates();
    for i in 0..upd.row_count() {
        if let Some(p) = upd.row_data(i) {
            if p.id.as_str() == pkg_id {
                return p.name.to_string();
            }
        }
    }
    pkg_id.to_string()
}
