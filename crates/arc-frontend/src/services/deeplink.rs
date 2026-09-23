use ini::Ini;
use std::sync::{Mutex, OnceLock};

pub fn parse_flatpakref(content: &str) -> (String, String, String) {
    let ini = Ini::load_from_str(content).unwrap_or_default();
    let section = ini.section(Some("Flatpak Ref"));
    let get = |key: &str| section.and_then(|s| s.get(key)).unwrap_or("").to_string();
    let name = get("Name");
    let mut title = get("Title");
    if title.is_empty() {
        title = name.clone();
    }
    (title, name, get("Url"))
}

pub fn parse_flatpakrepo(content: &str) -> (String, String) {
    let ini = Ini::load_from_str(content).unwrap_or_default();
    let section = ini.section(Some("Flatpak Repo"));
    let get = |key: &str| section.and_then(|s| s.get(key)).unwrap_or("").to_string();
    (get("Title"), get("Url"))
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

pub fn is_flatpakref(path: &str) -> bool {
    path.to_lowercase().ends_with(".flatpakref")
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
        .or_else(|| filename.strip_suffix(".appimage"))
        .or_else(|| filename.strip_suffix(".AppImage"))
        .or_else(|| filename.strip_suffix(".flatpak"))
        .unwrap_or(filename);
    no_ext.split('-').next().unwrap_or(no_ext).to_string()
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
            if let Ok(b) = u8::from_str_radix(hex, 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn local_path_from_arg(arg: &str) -> String {
    let path = match arg.strip_prefix("file://") {
        Some(rest) => percent_decode(rest.strip_prefix("localhost").unwrap_or(rest)),
        None => arg.to_string(),
    };
    std::path::absolute(&path)
        .ok()
        .and_then(|p| p.to_str().map(str::to_string))
        .unwrap_or(path)
}

pub enum LaunchIntent {
    Detail { pkg_id: String },
    List { slug: String },
    InstallFlatpakref { source: String, is_local_file: bool },
    AddRepo { content: String },
    InstallFile { path: String, file_name: String, pkg_name: String, is_appimage: bool, is_bundle: bool },
}

pub fn parse_args() -> Option<LaunchIntent> {
    let args: Vec<String> = std::env::args().collect();

    if let Some(a) = args.iter().find(|a| a.starts_with("appstream://") || a.starts_with("appstream:")) {
        let id = a
            .trim_start_matches("appstream://")
            .trim_start_matches("appstream:")
            .trim_start_matches("//")
            .trim_start_matches('/');
        let id = id.strip_suffix(".desktop").unwrap_or(id);
        if !id.is_empty() {
            return Some(LaunchIntent::Detail { pkg_id: id.to_string() });
        }
    }

    if let Some(a) = args.iter().find(|a| a.starts_with("arc://") || a.starts_with("arc:")) {
        let rest = a
            .trim_start_matches("arc://")
            .trim_start_matches("arc:")
            .trim_start_matches("//");
        if let Some(slug) = rest.strip_prefix("list/").or_else(|| rest.strip_prefix("list")) {
            let slug = slug.trim_matches('/').trim();
            if !slug.is_empty() {
                return Some(LaunchIntent::List { slug: slug.to_string() });
            }
        }
    }

    if let Some(a) = args.iter().find(|a| a.starts_with("flatpak+https://") || a.starts_with("flatpak+http://")) {
        let url = a.trim_start_matches("flatpak+").to_string();
        let flathub_app_id = url
            .strip_prefix("https://dl.flathub.org/repo/appstream/")
            .or_else(|| url.strip_prefix("http://dl.flathub.org/repo/appstream/"))
            .and_then(|s| s.strip_suffix(".flatpakref"));
        return Some(match flathub_app_id {
            Some(id) => LaunchIntent::Detail { pkg_id: id.to_string() },
            None => LaunchIntent::InstallFlatpakref { source: url, is_local_file: false },
        });
    }

    let file_args: Vec<String> = args
        .iter()
        .skip(1)
        .map(|a| local_path_from_arg(a))
        .collect();

    if let Some(path) = file_args.iter().find(|a| is_flatpakref(a)) {
        return Some(LaunchIntent::InstallFlatpakref { source: path.clone(), is_local_file: true });
    }

    if let Some(path) = file_args.iter().find(|a| is_flatpakrepo(a)) {
        let content = std::fs::read_to_string(path).unwrap_or_default();
        return Some(LaunchIntent::AddRepo { content });
    }

    if let Some(path) = file_args.iter().find(|a| is_pkg_file(a)) {
        let file_name = std::path::Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(path)
            .to_string();
        let pkg_name = pkg_name_from_filename(&file_name);
        return Some(LaunchIntent::InstallFile {
            path: path.clone(),
            file_name,
            pkg_name,
            is_appimage: is_appimage(path),
            is_bundle: is_flatpak_bundle(path),
        });
    }

    None
}

static INTENT: OnceLock<Mutex<Option<LaunchIntent>>> = OnceLock::new();

pub fn init() {
    let _ = INTENT.set(Mutex::new(parse_args()));
}

pub fn take_intent() -> Option<LaunchIntent> {
    INTENT.get().and_then(|m| m.lock().unwrap().take())
}
