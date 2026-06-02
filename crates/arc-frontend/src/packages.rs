use crate::helpers::check_flathub_verification;

fn push_decoded(text: &str, out: &mut String) {
    let mut rest = text;
    while !rest.is_empty() {
        match rest.find('&') {
            None => {
                out.push_str(rest);
                break;
            }
            Some(pos) => {
                out.push_str(&rest[..pos]);
                rest = &rest[pos..];
                if rest.starts_with("&amp;") {
                    out.push('&');
                    rest = &rest[5..];
                } else if rest.starts_with("&lt;") {
                    out.push('<');
                    rest = &rest[4..];
                } else if rest.starts_with("&gt;") {
                    out.push('>');
                    rest = &rest[4..];
                } else if rest.starts_with("&quot;") {
                    out.push('"');
                    rest = &rest[6..];
                } else if rest.starts_with("&apos;") {
                    out.push('\'');
                    rest = &rest[6..];
                } else {
                    out.push('&');
                    rest = &rest[1..];
                }
            }
        }
    }
}

pub(crate) struct DescBlock {
    text: String,
    is_list_item: bool,
    is_heading: bool,
    is_bold: bool,
}

// Split a block's text on `**...**` markers into bold and normal segments.
// Headings are already bold so we skip splitting them.
fn split_bold(text: String, is_list_item: bool, is_heading: bool) -> Vec<DescBlock> {
    if is_heading || !text.contains("**") {
        return vec![DescBlock { text, is_list_item, is_heading, is_bold: false }];
    }
    let mut result = Vec::new();
    let mut remaining = text.as_str();
    while !remaining.is_empty() {
        match remaining.find("**") {
            None => {
                result.push(DescBlock { text: remaining.to_string(), is_list_item, is_heading, is_bold: false });
                break;
            }
            Some(start) => {
                if start > 0 {
                    result.push(DescBlock { text: remaining[..start].to_string(), is_list_item, is_heading, is_bold: false });
                }
                remaining = &remaining[start + 2..];
                match remaining.find("**") {
                    None => {
                        result.push(DescBlock { text: remaining.to_string(), is_list_item, is_heading, is_bold: false });
                        break;
                    }
                    Some(end) => {
                        let bold = &remaining[..end];
                        if !bold.is_empty() {
                            result.push(DescBlock { text: bold.to_string(), is_list_item, is_heading, is_bold: true });
                        }
                        remaining = &remaining[end + 2..];
                    }
                }
            }
        }
    }
    result
}

fn parse_heading(s: &str) -> Option<String> {
    let s = s.trim();
    if s.starts_with("===") && s.ends_with("===") && s.len() > 6 {
        Some(s.trim_matches('=').trim().to_string())
    } else {
        None
    }
}

fn html_to_blocks(html: &str) -> Vec<DescBlock> {
    let mut blocks: Vec<DescBlock> = Vec::new();
    let mut current = String::new();
    let mut rest = html;
    while !rest.is_empty() {
        match rest.find('<') {
            None => { push_decoded(rest, &mut current); break; }
            Some(tag_start) => {
                push_decoded(&rest[..tag_start], &mut current);
                rest = &rest[tag_start + 1..];
                let tag_end = match rest.find('>') {
                    Some(e) => e,
                    None => { current.push('<'); continue; }
                };
                let raw_tag = rest[..tag_end].trim();
                let closing = raw_tag.starts_with('/');
                let name = raw_tag.trim_start_matches('/')
                    .split_ascii_whitespace().next().unwrap_or("")
                    .to_ascii_lowercase();
                match (closing, name.as_str()) {
                    (false, "li") => {
                        let t = current.trim().to_string();
                        if !t.is_empty() { blocks.extend(split_bold(t, false, false)); }
                        current.clear();
                    }
                    (true, "li") => {
                        let t = current.trim().to_string();
                        if !t.is_empty() { blocks.extend(split_bold(t, true, false)); }
                        current.clear();
                    }
                    (true, "p") => {
                        let t = current.trim().to_string();
                        if !t.is_empty() {
                            if let Some(heading) = parse_heading(&t) {
                                blocks.push(DescBlock { text: heading, is_list_item: false, is_heading: true, is_bold: false });
                            } else {
                                blocks.extend(split_bold(t, false, false));
                            }
                        }
                        current.clear();
                    }
                    (true, "h1") | (true, "h2") | (true, "h3") => {
                        let t = current.trim().to_string();
                        if !t.is_empty() { blocks.push(DescBlock { text: t, is_list_item: false, is_heading: true, is_bold: false }); }
                        current.clear();
                    }
                    _ => {}
                }
                rest = &rest[tag_end + 1..];
            }
        }
    }
    let t = current.trim().to_string();
    if !t.is_empty() { blocks.extend(split_bold(t, false, false)); }
    blocks
}

use crate::icons::{self, RawIcon};
use crate::transactions::{has_ongoing_transaction_for_package, TxStore};
use futures_util::future::join_all;
use libarc::{ArcDaemonProxy, Package, Provider};
use slint::{Model, SharedString};

pub struct RawCard {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub icon: Option<RawIcon>,
    pub installed: bool,
}

#[derive(serde::Deserialize, Clone)]
#[allow(dead_code)]
pub struct CachedAppInfo {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub description: String,
    pub icon_available: bool,
}

pub struct RawCategoryData {
    pub id: String,
    pub label: String,
    pub icon: Option<RawIcon>,
    pub color: (u8, u8, u8),
}

pub struct RawDetailData {
    pub name: String,
    pub developer: String,
    pub description_blocks: Vec<DescBlock>,
    pub summary: String,
    pub version: String,
    pub icon: Option<RawIcon>,
    pub flatpak_id: String,
    pub native_id: String,
    pub lutris_id: String,
    pub appimage_id: String,
    pub flatpak_installed: bool,
    pub native_installed: bool,
    pub lutris_installed: bool,
    pub appimage_installed: bool,
    pub verified: bool,
    pub license: String,
    pub eula_url: String,
    pub homepage_url: String,
    pub content_rating: String,
}

#[derive(Clone)]
pub struct RawPackage {
    pub pkg: libarc::Package,
    pub icon: Option<RawIcon>,
}

impl RawPackage {
    pub fn to_slint(&self) -> crate::PackageItem {
        crate::PackageItem {
            id: SharedString::from(self.pkg.id.as_str()),
            name: SharedString::from(self.pkg.name.as_str()),
            version: SharedString::from(self.pkg.version.as_str()),
            description: SharedString::from(self.pkg.description.as_str()),
            installed: self.pkg.installed,
            icon: self
                .icon
                .as_ref()
                .map(|r| r.to_slint_image())
                .unwrap_or_default(),
            busy: false,
            progress: 0.0,
            transaction_id: Default::default(),
        }
    }
}

impl RawCard {
    pub fn to_slint(&self) -> crate::AppCardData {
        crate::AppCardData {
            id: SharedString::from(self.id.as_str()),
            name: SharedString::from(self.name.as_str()),
            summary: SharedString::from(self.summary.as_str()),
            icon: self
                .icon
                .as_ref()
                .map(|r| r.to_slint_image())
                .unwrap_or_default(),
            installed: self.installed,
        }
    }
}

#[derive(serde::Deserialize)]
struct AppMetadata {
    summary: String,
    description: String,
    license: Option<String>,
    eula_url: Option<String>,
    homepage_url: Option<String>,
    content_rating: String,
    developer_name: Option<String>,
}

#[derive(serde::Deserialize)]
struct DaemonHomeEntry {
    id: String,
    name: String,
    summary: String,
    icon_url: Option<String>,
}

#[derive(serde::Deserialize)]
struct DaemonHomeData {
    popular: Vec<DaemonHomeEntry>,
    recent: Vec<DaemonHomeEntry>,
}

pub async fn load_home(
    app_weak: slint::Weak<crate::AppWindow>,
    proxy: Option<ArcDaemonProxy<'static>>,
) {
    let (popular_apps, recent_apps): (Vec<DaemonHomeEntry>, Vec<DaemonHomeEntry>) =
        if let Some(ref p) = proxy {
            p.get_home_apps(10, 20)
                .await
                .ok()
                .and_then(|json| serde_json::from_str::<DaemonHomeData>(&json).ok())
                .map(|d| (d.popular, d.recent))
                .unwrap_or_default()
        } else {
            (vec![], vec![])
        };

    let installed_ids: std::collections::HashSet<String> = if let Some(ref p) = proxy {
        p.list_installed()
            .await
            .ok()
            .and_then(|json| serde_json::from_str::<Vec<libarc::Package>>(&json).ok())
            .unwrap_or_default()
            .into_iter()
            .map(|pkg| pkg.id)
            .collect()
    } else {
        std::collections::HashSet::new()
    };

    let popular_tasks: Vec<_> = popular_apps
        .into_iter()
        .map(|app| {
            tokio::task::spawn_blocking(move || {
                let icon = app.icon_url.as_ref().and_then(|url| {
                    if url.starts_with("local:") {
                        icons::load_local_flatpak_icon(&app.id)
                    } else {
                        let rt = tokio::runtime::Handle::current();
                        rt.block_on(icons::load_icon(url))
                    }
                });
                (app, icon)
            })
        })
        .collect();

    let popular_results: Vec<_> = join_all(popular_tasks).await;
    let mut popular_cards: Vec<RawCard> = Vec::new();
    for result in popular_results {
        if let Ok((app, icon)) = result {
            let installed = installed_ids.contains(&app.id);
            popular_cards.push(RawCard {
                id: app.id.clone(),
                name: app.name.clone(),
                summary: app.summary.clone(),
                icon,
                installed,
            });
        }
    }

    let recent_tasks: Vec<_> = recent_apps
        .into_iter()
        .map(|app| {
            tokio::task::spawn_blocking(move || {
                let icon = app.icon_url.as_ref().and_then(|url| {
                    if url.starts_with("local:") {
                        icons::load_local_flatpak_icon(&app.id)
                    } else {
                        let rt = tokio::runtime::Handle::current();
                        rt.block_on(icons::load_icon(url))
                    }
                });
                (app, icon)
            })
        })
        .collect();

    let recent_results: Vec<_> = join_all(recent_tasks).await;
    let mut recent_cards: Vec<RawCard> = Vec::new();
    for result in recent_results {
        if let Ok((app, icon)) = result {
            let installed = installed_ids.contains(&app.id);
            recent_cards.push(RawCard {
                id: app.id.clone(),
                name: app.name.clone(),
                summary: app.summary.clone(),
                icon,
                installed,
            });
        }
    }

    let categories = [
        ("AudioVideo", "Multimedia", "applications-multimedia"),
        ("Development", "Developer Tools", "applications-development"),
        ("Education", "Education", "applications-education"),
        ("Graphics", "Graphics", "applications-graphics"),
        ("Network", "Internet", "applications-internet"),
        ("Office", "Office", "applications-office"),
        ("Science", "Science", "applications-science"),
        ("System", "System", "applications-system"),
        ("Utility", "Utilities", "applications-utilities"),
    ];

    let mut raw_cats: Vec<RawCategoryData> = Vec::new();
    for (id, label, icon_name) in &categories {
        let name = icon_name.to_string();
        let data = tokio::task::spawn_blocking(move || icons::load_category_icon(&name))
            .await
            .unwrap_or(icons::CategoryIconData {
                icon: None,
                color: (80, 80, 90),
            });
        raw_cats.push(RawCategoryData {
            id: id.to_string(),
            label: label.to_string(),
            icon: data.icon,
            color: data.color,
        });
    }

    let _ = app_weak.upgrade_in_event_loop(move |app| {
        let pop: Vec<crate::AppCardData> = popular_cards.iter().map(|c| c.to_slint()).collect();
        let rec: Vec<crate::AppCardData> = recent_cards.iter().map(|c| c.to_slint()).collect();
        let cats: Vec<crate::CategoryItem> = raw_cats
            .iter()
            .map(|c| crate::CategoryItem {
                id: SharedString::from(c.id.as_str()),
                label: SharedString::from(c.label.as_str()),
                icon: c
                    .icon
                    .as_ref()
                    .map(|r| r.to_slint_image())
                    .unwrap_or_default(),
                bg_color: slint::Color::from_rgb_u8(c.color.0, c.color.1, c.color.2),
            })
            .collect();
        app.set_popular_apps(pop.as_slice().into());
        app.set_recent_apps(rec.as_slice().into());
        app.set_categories(cats.as_slice().into());
        app.set_home_loading(false);
    });
}

pub async fn refresh_home_installed(
    app_weak: slint::Weak<crate::AppWindow>,
    proxy: Option<ArcDaemonProxy<'static>>,
) {
    let installed_ids: std::collections::HashSet<String> = if let Some(ref p) = proxy {
        p.list_installed()
            .await
            .ok()
            .and_then(|json| serde_json::from_str::<Vec<libarc::Package>>(&json).ok())
            .unwrap_or_default()
            .into_iter()
            .map(|pkg| pkg.id)
            .collect()
    } else {
        std::collections::HashSet::new()
    };

    let _ = app_weak.upgrade_in_event_loop(move |app| {
        let popular_model = app.get_popular_apps();
        let popular_count = popular_model.row_count();
        let mut popular_items: Vec<crate::AppCardData> = (0..popular_count)
            .filter_map(|i| popular_model.row_data(i))
            .collect();
        for item in &mut popular_items {
            item.installed = installed_ids.contains(item.id.as_str());
        }
        app.set_popular_apps(popular_items.as_slice().into());

        let recent_model = app.get_recent_apps();
        let recent_count = recent_model.row_count();
        let mut recent_items: Vec<crate::AppCardData> = (0..recent_count)
            .filter_map(|i| recent_model.row_data(i))
            .collect();
        for item in &mut recent_items {
            item.installed = installed_ids.contains(item.id.as_str());
        }
        app.set_recent_apps(recent_items.as_slice().into());
    });
}

pub async fn build_installed_cache(proxy: Option<ArcDaemonProxy<'static>>) -> Vec<RawPackage> {
    let pkgs: Vec<libarc::Package> = if let Some(p) = &proxy {
        p.list_installed()
            .await
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default()
    } else {
        vec![]
    };
    load_package_icons(pkgs).await
}

pub async fn load_package_icons(pkgs: Vec<libarc::Package>) -> Vec<RawPackage> {
    let icon_futures: Vec<_> = pkgs
        .iter()
        .map(|pkg| async {
            match pkg.provider {
                Provider::Flatpak => {
                    let id = pkg.id.clone();
                    let icon_url = pkg.icon_url.clone();
                    let local = tokio::task::spawn_blocking(move || icons::load_local_flatpak_icon(&id))
                        .await
                        .unwrap_or(None);
                    if local.is_some() {
                        local
                    } else {
                        match icon_url.as_deref() {
                            Some(url) if !url.starts_with("local:") => icons::load_icon(url).await,
                            _ => None,
                        }
                    }
                }
                Provider::Lutris => {
                    if let Some(url) = &pkg.icon_url {
                        let url = url.clone();
                        tokio::spawn(async move { icons::load_lutris_icon(&url).await })
                            .await
                            .ok()
                            .flatten()
                    } else {
                        None
                    }
                }
                Provider::AppImage => {
                    let stem = pkg
                        .id
                        .strip_prefix("appimage:")
                        .unwrap_or(&pkg.id)
                        .to_string();
                    let icon_url = pkg.icon_url.clone();
                    tokio::task::spawn_blocking(move || {
                        icons::load_appimage_icon(icon_url.as_deref(), &stem)
                    })
                    .await
                    .unwrap_or(None)
                }
                Provider::Distrobox => {
                    let name = pkg.name.clone();
                    let icon_url = pkg.icon_url.clone();
                    tokio::task::spawn_blocking(move || {
                        icons::load_distrobox_icon(icon_url.as_deref(), &name)
                    })
                    .await
                    .unwrap_or(None)
                }
            }
        })
        .collect();
    let icons_result: Vec<_> = join_all(icon_futures).await;
    pkgs.into_iter()
        .zip(icons_result)
        .map(|(pkg, icon)| RawPackage { pkg, icon })
        .collect()
}

pub async fn load_detail(
    id: slint::SharedString,
    proxy: Option<ArcDaemonProxy<'static>>,
    store: TxStore,
    app_weak: slint::Weak<crate::AppWindow>,
) {
    let app_id = id.to_string();

    let app_name = app_id.split(';').next().unwrap_or(&app_id).to_string();

    let (search_pkgs, installed_pkgs, metadata): (Vec<Package>, Vec<Package>, Option<AppMetadata>) =
        if let Some(ref p) = proxy {
            tokio::join!(
                async {
                    p.search(&app_name)
                        .await
                        .ok()
                        .and_then(|j| serde_json::from_str(&j).ok())
                        .unwrap_or_default()
                },
                async {
                    p.list_installed()
                        .await
                        .ok()
                        .and_then(|j| serde_json::from_str(&j).ok())
                        .unwrap_or_default()
                },
                async {
                    p.get_app_metadata(&app_id)
                        .await
                        .ok()
                        .and_then(|j| serde_json::from_str(&j).ok())
                }
            )
        } else {
            (vec![], vec![], None)
        };

    let name_lower = app_name.to_lowercase();
    let all_pkgs: Vec<&Package> = search_pkgs.iter().chain(installed_pkgs.iter()).collect();

    let flatpak_pkg = all_pkgs.iter().copied().find(|p| {
        p.provider == Provider::Flatpak
            && (p.id == app_id.as_str()
                || p.id.to_lowercase() == app_id.to_string().to_lowercase()
                || p.name.to_lowercase() == name_lower)
    });

    let native_pkg = all_pkgs.iter().copied().find(|p| {
        p.provider == Provider::Distrobox
            && (p.id == app_id.as_str()
                || p.id.split(';').next().map(|n| n.to_lowercase()).as_deref()
                    == Some(name_lower.as_str())
                || p.name.to_lowercase() == name_lower)
    });

    let lutris_pkg = all_pkgs.iter().copied().find(|p| {
        p.provider == Provider::Lutris
            && (p.id == app_id.as_str() || p.name.to_lowercase() == name_lower)
    });

    let appimage_pkg = all_pkgs.iter().copied().find(|p| {
        p.provider == Provider::AppImage
            && (p.id == app_id.as_str() || p.name.to_lowercase() == name_lower)
    });

    let (developer, verified) = if appimage_pkg.is_some() {
        (String::new(), false)
    } else if let Some(fp_pkg) = &flatpak_pkg {
        let remote = fp_pkg.remote.as_deref().unwrap_or("");
        let dev_name = metadata
            .as_ref()
            .and_then(|m| m.developer_name.clone())
            .or_else(|| fp_pkg.id.split('.').last().map(|s| s.to_string()))
            .unwrap_or_else(|| fp_pkg.name.clone());

        if remote == "blossomos" {
            (dev_name, true)
        } else if remote == "flathub" {
            let app_id_for_api = fp_pkg.id.clone();
            let api_verified = check_flathub_verification(&app_id_for_api).await;
            (dev_name, api_verified)
        } else {
            (dev_name, false)
        }
    } else {
        let dev_name = metadata
            .as_ref()
            .and_then(|m| m.developer_name.clone())
            .unwrap_or_else(|| app_name.clone());
        (dev_name, false)
    };

    let flatpak_id = flatpak_pkg.map(|p| p.id.clone()).unwrap_or_else(|| {
        // Only infer a Flatpak ID from the raw app_id if it looks like a reverse-DNS
        // Flatpak identifier. Exclude AppImage/Lutris IDs which also contain dots.
        if app_id.contains('.')
            && !app_id.contains(';')
            && !app_id.starts_with("appimage:")
            && !app_id.starts_with("lutris:")
            && !app_id.starts_with("distrobox:")
        {
            app_id.to_string()
        } else {
            String::new()
        }
    });
    let native_id = native_pkg.map(|p| p.id.clone()).unwrap_or_default();
    let lutris_id = lutris_pkg.map(|p| p.id.clone()).unwrap_or_default();

    let flatpak_installed = flatpak_pkg.map(|p| p.installed).unwrap_or(false)
        || installed_pkgs
            .iter()
            .any(|p| p.provider == Provider::Flatpak && p.id == flatpak_id);
    let native_installed = native_pkg.map(|p| p.installed).unwrap_or(false);
    let lutris_installed = lutris_pkg.map(|p| p.installed).unwrap_or(false);
    let appimage_id = appimage_pkg.map(|p| p.id.clone()).unwrap_or_default();
    let appimage_installed = appimage_pkg.map(|p| p.installed).unwrap_or(false);

    let icon = if !flatpak_id.is_empty() {
        let fid = flatpak_id.clone();
        let remote_icon_url = flatpak_pkg.and_then(|p| p.icon_url.clone())
            .filter(|u| !u.starts_with("local:"));
        let local = tokio::task::spawn_blocking(move || icons::load_local_flatpak_icon(&fid))
            .await
            .unwrap_or(None);
        if local.is_some() {
            local
        } else {
            match remote_icon_url.as_deref() {
                Some(url) => icons::load_icon(url).await,
                None => None,
            }
        }
    } else if !lutris_id.is_empty() {
        if let Some(lutris) = &lutris_pkg {
            if let Some(icon_url) = &lutris.icon_url {
                let url = icon_url.clone();
                tokio::spawn(async move { icons::load_lutris_icon(&url).await })
                    .await
                    .ok()
                    .flatten()
            } else {
                None
            }
        } else {
            None
        }
    } else if !native_id.is_empty() {
        let name = native_pkg.map(|p| p.name.clone()).unwrap_or_default();
        let icon_url = native_pkg.and_then(|p| p.icon_url.clone());
        tokio::task::spawn_blocking(move || icons::load_distrobox_icon(icon_url.as_deref(), &name))
            .await
            .unwrap_or(None)
    } else if !appimage_id.is_empty() {
        let stem = appimage_id
            .strip_prefix("appimage:")
            .unwrap_or(&appimage_id)
            .to_string();
        let icon_url = appimage_pkg.and_then(|p| p.icon_url.clone());
        tokio::task::spawn_blocking(move || icons::load_appimage_icon(icon_url.as_deref(), &stem))
            .await
            .unwrap_or(None)
    } else {
        None
    };

    let name = flatpak_pkg
        .or(native_pkg)
        .or(lutris_pkg)
        .or(appimage_pkg)
        .map(|p| p.name.clone())
        .unwrap_or_else(|| app_name.clone());

    let plain_description: Option<String> = if flatpak_pkg.is_none() {
        native_pkg
            .or(lutris_pkg)
            .or(appimage_pkg)
            .map(|p| p.description.clone())
            .filter(|s| !s.is_empty())
    } else {
        None
    };

    let version = flatpak_pkg
        .or(native_pkg)
        .or(lutris_pkg)
        .or(appimage_pkg)
        .map(|p| p.version.clone())
        .unwrap_or_default();

    let screenshot_urls: Vec<String> = flatpak_pkg
        .or(lutris_pkg)
        .or(native_pkg)
        .map(|p| p.screenshots.clone())
        .unwrap_or_default();

    let raw = RawDetailData {
        name,
        developer,
        description_blocks: metadata
            .as_ref()
            .map(|m| html_to_blocks(&m.description))
            .or_else(|| plain_description.map(|t| split_bold(t, false, false)))
            .unwrap_or_default(),
        summary: metadata
            .as_ref()
            .map(|m| m.summary.clone())
            .unwrap_or_default(),
        version,
        icon,
        flatpak_id,
        native_id,
        lutris_id,
        appimage_id,
        flatpak_installed,
        native_installed,
        lutris_installed,
        appimage_installed,
        verified,
        license: metadata
            .as_ref()
            .and_then(|m| m.license.clone())
            .unwrap_or_default(),
        eula_url: metadata
            .as_ref()
            .and_then(|m| m.eula_url.clone())
            .unwrap_or_default(),
        homepage_url: metadata
            .as_ref()
            .and_then(|m| m.homepage_url.clone())
            .unwrap_or_default(),
        content_rating: metadata
            .as_ref()
            .map(|m| m.content_rating.clone())
            .unwrap_or_default(),
    };

    let pkg_id_for_busy_check = if !raw.flatpak_id.is_empty() {
        raw.flatpak_id.clone()
    } else {
        raw.appimage_id.clone()
    };
    let flatpak_id_for_extensions = raw.flatpak_id.clone();
    let description_blocks: Vec<crate::DescriptionBlock> = raw
        .description_blocks
        .into_iter()
        .map(|b| crate::DescriptionBlock {
            text: b.text.into(),
            is_list_item: b.is_list_item,
            is_heading: b.is_heading,
            is_bold: b.is_bold,
        })
        .collect();
    let _ = app_weak.upgrade_in_event_loop(move |app| {
        app.set_detail_screenshots([].as_slice().into());
        app.set_detail_extensions([].as_slice().into());
        app.set_detail_description_blocks(description_blocks.as_slice().into());
        app.set_detail_app(crate::AppDetailData {
            id: Default::default(),
            name: raw.name.into(),
            developer: raw.developer.into(),
            summary: raw.summary.into(),
            version: raw.version.into(),
            icon: raw
                .icon
                .as_ref()
                .map(|r| r.to_slint_image())
                .unwrap_or_default(),
            flatpak_id: raw.flatpak_id.into(),
            native_id: raw.native_id.into(),
            lutris_id: raw.lutris_id.into(),
            appimage_id: raw.appimage_id.into(),
            flatpak_installed: raw.flatpak_installed,
            native_installed: raw.native_installed,
            lutris_installed: raw.lutris_installed,
            appimage_installed: raw.appimage_installed,
            installed: raw.flatpak_installed
                || raw.native_installed
                || raw.lutris_installed
                || raw.appimage_installed,
            verified: raw.verified,
            license: raw.license.into(),
            eula_url: raw.eula_url.into(),
            homepage_url: raw.homepage_url.into(),
            content_rating: raw.content_rating.into(),
        });
        app.set_detail_loading(false);
        let is_busy = has_ongoing_transaction_for_package(&store, &pkg_id_for_busy_check);
        app.set_detail_busy(is_busy);
    });

    if !flatpak_id_for_extensions.is_empty() {
        let flatpak_id = flatpak_id_for_extensions;
        let proxy_clone = proxy.clone();
        let app_weak3 = app_weak.clone();
        tokio::spawn(async move {
            let extensions: Vec<libarc::Package> = if let Some(p) = &proxy_clone {
                p.list_extensions(&flatpak_id)
                    .await
                    .ok()
                    .and_then(|j| serde_json::from_str(&j).ok())
                    .unwrap_or_default()
            } else {
                vec![]
            };
            if !extensions.is_empty() {
                let _ = app_weak3.upgrade_in_event_loop(move |app| {
                    let items: Vec<crate::ExtensionItem> = extensions
                        .iter()
                        .map(|e| crate::ExtensionItem {
                            id: SharedString::from(e.id.as_str()),
                            name: SharedString::from(e.name.as_str()),
                            installed: e.installed,
                        })
                        .collect();
                    app.set_detail_extensions(items.as_slice().into());
                });
            }
        });
    }

    if !screenshot_urls.is_empty() {
        let app_weak2 = app_weak.clone();
        tokio::spawn(async move {
            let futs: Vec<_> = screenshot_urls
                .iter()
                .map(|url| {
                    let url = url.clone();
                    tokio::spawn(async move { icons::load_screenshot(&url).await })
                })
                .collect();
            let raw_shots: Vec<RawIcon> = join_all(futs)
                .await
                .into_iter()
                .filter_map(|r| r.ok().flatten())
                .collect();
            if !raw_shots.is_empty() {
                let _ = app_weak2.upgrade_in_event_loop(move |app| {
                    let shots: Vec<slint::Image> =
                        raw_shots.iter().map(|r| r.to_slint_image()).collect();
                    app.set_detail_screenshots(shots.as_slice().into());
                });
            }
        });
    }
}
