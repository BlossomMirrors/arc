use crate::appstream_db::AppStreamDb;
use crate::helpers::check_flathub_verification;
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
    pub description: String,
    pub summary: String,
    pub version: String,
    pub icon: Option<RawIcon>,
    pub flatpak_id: String,
    pub native_id: String,
    pub lutris_id: String,
    pub flatpak_installed: bool,
    pub native_installed: bool,
    pub lutris_installed: bool,
    pub verified: bool,
    pub license: String,
    pub homepage_url: String,
    pub content_rating: String,
}

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

pub async fn load_home(
    app_weak: slint::Weak<crate::AppWindow>,
    proxy: Option<ArcDaemonProxy<'static>>,
) {
    let appstream_db = AppStreamDb::get_static();
    let popular_apps: Vec<_> = appstream_db.get_popular_apps(10);
    let recent_apps: Vec<_> = appstream_db.get_recent_apps(20);

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

pub async fn load_package_icons(pkgs: Vec<libarc::Package>) -> Vec<RawPackage> {
    let icon_futures: Vec<_> = pkgs
        .iter()
        .map(|pkg| async {
            match pkg.provider {
                Provider::Flatpak => {
                    let id = pkg.id.clone();
                    tokio::task::spawn_blocking(move || icons::load_local_flatpak_icon(&id))
                        .await
                        .unwrap_or(None)
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
                Provider::Distrobox | Provider::AppImage => {
                    let name = pkg.name.clone();
                    tokio::task::spawn_blocking(move || icons::load_native_package_icon(&name))
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
    let appstream_db = AppStreamDb::get_static();

    let app_name = appstream_db
        .find_by_id(&id)
        .map(|a| a.name.clone())
        .unwrap_or_else(|| id.split(';').next().unwrap_or(&id).to_string());

    let (search_pkgs, installed_pkgs): (Vec<Package>, Vec<Package>) =
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
                }
            )
        } else {
            (vec![], vec![])
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
            && (p.id.split(';').next().map(|n| n.to_lowercase()).as_deref()
                == Some(name_lower.as_str())
                || p.name.to_lowercase() == name_lower)
    });

    let lutris_pkg = all_pkgs.iter().copied().find(|p| {
        p.provider == Provider::Lutris
            && (p.id == app_id.as_str() || p.name.to_lowercase() == name_lower)
    });

    let appstream_info = appstream_db.find_by_id(&app_id);

    let (developer, verified) = if let Some(fp_pkg) = &flatpak_pkg {
        let remote = fp_pkg.remote.as_deref().unwrap_or("");
        let dev_name = appstream_info
            .as_ref()
            .and_then(|a| a.id.split('.').last().map(|s| s.to_string()))
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
        let dev_name = appstream_info
            .as_ref()
            .and_then(|a| a.id.split('.').last().map(|s| s.to_string()))
            .unwrap_or_else(|| app_name.clone());
        (dev_name, false)
    };

    let flatpak_id = flatpak_pkg.map(|p| p.id.clone()).unwrap_or_else(|| {
        if app_id.contains('.') && !app_id.contains(';') {
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

    let icon = if !flatpak_id.is_empty() {
        let fid = flatpak_id.clone();
        tokio::task::spawn_blocking(move || icons::load_local_flatpak_icon(&fid))
            .await
            .unwrap_or(None)
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
        tokio::task::spawn_blocking(move || icons::load_native_package_icon(&name))
            .await
            .unwrap_or(None)
    } else {
        None
    };

    let icon = icon.or_else(|| {
        appstream_info.as_ref().and_then(|info| {
            info.icon_url.as_ref().and_then(|url| {
                if url.starts_with("local:") {
                    icons::load_local_flatpak_icon(&info.id)
                } else {
                    let url = url.clone();
                    let rt = tokio::runtime::Handle::current();
                    rt.block_on(icons::load_icon(&url))
                }
            })
        })
    });

    let name = flatpak_pkg
        .or(native_pkg)
        .or(lutris_pkg)
        .map(|p| p.name.clone())
        .or_else(|| appstream_info.as_ref().map(|a| a.name.clone()))
        .unwrap_or_else(|| app_name.clone());

    let description = if flatpak_pkg.is_some() {
        String::new()
    } else {
        native_pkg
            .or(lutris_pkg)
            .map(|p| p.description.clone())
            .or_else(|| appstream_info.as_ref().map(|a| a.summary.clone()))
            .unwrap_or_default()
    };

    let version = flatpak_pkg
        .or(native_pkg)
        .or(lutris_pkg)
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
        description,
        summary: appstream_info
            .as_ref()
            .map(|a| a.summary.clone())
            .unwrap_or_default(),
        version,
        icon,
        flatpak_id,
        native_id,
        lutris_id,
        flatpak_installed,
        native_installed,
        lutris_installed,
        verified,
        license: appstream_info
            .as_ref()
            .and_then(|a| a.license.clone())
            .unwrap_or_default(),
        homepage_url: appstream_info
            .as_ref()
            .and_then(|a| a.homepage_url.clone())
            .unwrap_or_default(),
        content_rating: appstream_info
            .as_ref()
            .map(|a| a.content_rating_age.clone())
            .unwrap_or_default(),
    };

    let pkg_id_for_busy_check = raw.flatpak_id.clone();
    let _ = app_weak.upgrade_in_event_loop(move |app| {
        app.set_detail_screenshots([].as_slice().into());
        app.set_detail_app(crate::AppDetailData {
            id: Default::default(),
            name: raw.name.into(),
            developer: raw.developer.into(),
            description: raw.description.into(),
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
            flatpak_installed: raw.flatpak_installed,
            native_installed: raw.native_installed,
            lutris_installed: raw.lutris_installed,
            installed: raw.flatpak_installed || raw.native_installed || raw.lutris_installed,
            verified: raw.verified,
            license: raw.license.into(),
            homepage_url: raw.homepage_url.into(),
            content_rating: raw.content_rating.into(),
        });
        app.set_detail_loading(false);
        let is_busy = has_ongoing_transaction_for_package(&store, &pkg_id_for_busy_check);
        app.set_detail_busy(is_busy);
    });

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
