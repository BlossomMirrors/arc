mod appstream_db;
mod icons;

use crate::icons::RawIcon;
use anyhow::Result;
use appstream_db::AppStreamDb;
use futures_util::{future::join_all, StreamExt};
use libarc::{connect, ArcDaemonProxy, Package, Provider, Settings};
use slint::{Model, SharedString};
use std::sync::{Arc, Mutex};

slint::include_modules!();

fn is_pkg_file(path: &str) -> bool {
    path.ends_with(".deb")
        || path.ends_with(".rpm")
        || path.ends_with(".pkg.tar.xz")
        || path.ends_with(".pkg.tar.zst")
}

fn pkg_name_from_filename(filename: &str) -> String {
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

// When Flatpak is preferred, both a Flatpak and a native package can match the same
// app name. This collapses those duplicates by keeping only the preferred provider's
// entry. Apps that only exist in one provider always pass through unchanged.
fn dedup_by_preference(pkgs: Vec<libarc::Package>, settings: &Settings) -> Vec<libarc::Package> {
    use std::collections::{HashMap, HashSet};

    // build name → flatpak id so we can check the preference list with the right id
    let flatpak_id_by_name: HashMap<String, String> = pkgs
        .iter()
        .filter(|p| p.provider == Provider::Flatpak)
        .map(|p| (p.name.to_lowercase(), p.id.clone()))
        .collect();
    // let flatpak_names: HashSet<String> = flatpak_id_by_name.keys().cloned().collect();
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
                    // no native counterpart → always show
                    !native_names.contains(&name)
                        || settings.preferred_for(&p.id) == Provider::Flatpak
                }
                Provider::Distrobox => {
                    // no flatpak counterpart (native-only app) → always show
                    let Some(flatpak_id) = flatpak_id_by_name.get(&name) else {
                        return true;
                    };
                    // use the flatpak id for the preference lookup since the list uses those ids
                    settings.preferred_for(flatpak_id) == Provider::Distrobox
                }
                Provider::Lutris => {
                    // no native counterpart → always show
                    !native_names.contains(&name)
                        || settings.preferred_for(&p.id) == Provider::Lutris
                }
            }
        })
        .collect()
}

fn get_proxy(
    proxy_arc: &Arc<Mutex<Option<ArcDaemonProxy<'static>>>>,
) -> Option<ArcDaemonProxy<'static>> {
    proxy_arc.lock().unwrap().clone()
}

async fn check_flathub_verification(app_id: &str) -> bool {
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

async fn get_or_connect(
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

fn update_package_installed(app: &AppWindow, pkg_id: &str, installed: bool) {
    let model = app.get_packages();
    let count = model.row_count();
    let mut items: Vec<PackageItem> = (0..count).filter_map(|i| model.row_data(i)).collect();
    for item in &mut items {
        if item.id == pkg_id {
            item.installed = installed;
        }
    }
    app.set_packages(items.as_slice().into());
}

async fn wait_for_transaction(
    proxy_arc: Arc<Mutex<Option<ArcDaemonProxy<'static>>>>,
    tx_id: String,
    app_weak: slint::Weak<AppWindow>,
    success_msg: String,
    pkg_id: String,
    installed_after: bool,
    refresh_detail: bool,
) {
    let Some(p) = get_proxy(&proxy_arc) else {
        return;
    };

    let (mut progress_stream, mut finished_stream) = match tokio::join!(
        p.receive_transaction_progress(),
        p.receive_transaction_finished(),
    ) {
        (Ok(ps), Ok(fs)) => (ps, fs),
        _ => return,
    };

    loop {
        tokio::select! {
            sig = progress_stream.next() => {
                if let Some(sig) = sig {
                    if let Ok(args) = sig.args() {
                        if *args.transaction_id() == tx_id {
                            let progress = *args.progress() as f32 / 100.0;
                            let _ = app_weak.upgrade_in_event_loop(move |app| {
                                app.set_progress(progress);
                            });
                        }
                    }
                }
            }
            sig = finished_stream.next() => {
                match sig {
                    Some(sig) => {
                        if let Ok(args) = sig.args() {
                            if *args.transaction_id() == tx_id {
                                if *args.success() {
                                    let msg = success_msg.clone();
                                    let pid = pkg_id.clone();
                                    let _ = app_weak.upgrade_in_event_loop(move |app| {
                                        update_package_installed(&app, &pid, installed_after);
                                        app.set_status_text(msg.into());
                                        app.set_progress(0.0);
                                        app.set_detail_busy(false);
                                        if refresh_detail {
                                            app.invoke_detail_requested(pid.into());
                                        }
                                    });
                                } else {
                                    let msg = format!("Failed: {}", args.message());
                                    let _ = app_weak.upgrade_in_event_loop(move |app| {
                                        app.set_status_text(msg.into());
                                        app.set_progress(0.0);
                                        app.set_detail_busy(false);
                                    });
                                }
                                break;
                            }
                        }
                    }
                    None => break,
                }
            }
        }
    }
}

struct RawCard {
    id: String,
    name: String,
    summary: String,
    icon: Option<RawIcon>,
}

// CachedAppInfo matches the daemon's icon_cache::AppInfo struct
#[derive(serde::Deserialize, Clone)]
#[allow(dead_code)]
struct CachedAppInfo {
    id: String,
    name: String,
    summary: String,
    description: String,
    icon_available: bool,
}

struct RawCategoryData {
    id: String,
    label: String,
    icon: Option<RawIcon>,
    color: (u8, u8, u8),
}

struct RawDetailData {
    name: String,
    developer: String,
    description: String,
    summary: String,
    version: String,
    icon: Option<RawIcon>,
    flatpak_id: String,
    native_id: String,
    lutris_id: String,
    flatpak_installed: bool,
    native_installed: bool,
    lutris_installed: bool,
    verified: bool,
}

struct RawPackage {
    pkg: libarc::Package,
    icon: Option<RawIcon>,
}

impl RawPackage {
    fn to_slint(&self) -> PackageItem {
        PackageItem {
            id: slint::SharedString::from(self.pkg.id.as_str()),
            name: slint::SharedString::from(self.pkg.name.as_str()),
            version: slint::SharedString::from(self.pkg.version.as_str()),
            description: slint::SharedString::from(self.pkg.description.as_str()),
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
    fn to_slint(&self) -> AppCardData {
        AppCardData {
            id: SharedString::from(self.id.as_str()),
            name: SharedString::from(self.name.as_str()),
            summary: SharedString::from(self.summary.as_str()),
            icon: self
                .icon
                .as_ref()
                .map(|r| r.to_slint_image())
                .unwrap_or_default(),
        }
    }
}

async fn load_home(app_weak: slint::Weak<AppWindow>, _proxy: Option<ArcDaemonProxy<'static>>) {
    let appstream_db = AppStreamDb::get_static();
    // Load popular and recent apps from cached AppStream data
    let popular_apps: Vec<_> = appstream_db.get_popular_apps(10);
    let recent_apps: Vec<_> = appstream_db.get_recent_apps(4);

    // Load icons in parallel for popular apps
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
            popular_cards.push(RawCard {
                id: app.id.clone(),
                name: app.name.clone(),
                summary: app.summary.clone(),
                icon,
            });
        }
    }

    // Load icons in parallel for recent apps
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
            recent_cards.push(RawCard {
                id: app.id.clone(),
                name: app.name.clone(),
                summary: app.summary.clone(),
                icon,
            });
        }
    }

    // Use standard AppStream categories
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
        let pop: Vec<AppCardData> = popular_cards.iter().map(|c| c.to_slint()).collect();
        let rec: Vec<AppCardData> = recent_cards.iter().map(|c| c.to_slint()).collect();
        let cats: Vec<CategoryItem> = raw_cats
            .iter()
            .map(|c| CategoryItem {
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

fn main() -> Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    let proxy_result = rt.block_on(async {
        tokio::time::timeout(std::time::Duration::from_secs(2), connect())
            .await
            .unwrap_or_else(|_| Err(anyhow::anyhow!("daemon connection timed out")))
    });

    let app = AppWindow::new()?;

    let proxy_opt: Arc<Mutex<Option<ArcDaemonProxy<'static>>>> =
        Arc::new(Mutex::new(proxy_result.ok()));

    {
        let guard = proxy_opt.lock().unwrap();
        if guard.is_none() {
            app.set_status_text("Warning: Arc daemon not running. Start arc-daemon first.".into());
        } else {
            app.set_status_text("Connected to Arc daemon.".into());
        }
    }

    let settings: Arc<Mutex<Settings>> = Arc::new(Mutex::new(Settings::load()));

    {
        let s = settings.lock().unwrap();
        app.set_settings_preferred(
            match s.preferred_provider {
                Provider::Distrobox => "Native",
                Provider::Flatpak => "Flatpak",
                Provider::Lutris => "Lutris",
            }
            .into(),
        );
        app.set_settings_ignore_native_pref(s.ignore_native_preference);
    }

    // Load AppStream DB in background while showing home page
    {
        let app_weak = app.as_weak();
        let proxy = proxy_opt.lock().unwrap().clone();
        app.set_home_loading(true);
        rt.handle().spawn(async move {
            // Warm the static AppStream DB cache, then load the home page using it
            tokio::task::spawn_blocking(AppStreamDb::get_static)
                .await
                .unwrap();
            load_home(app_weak, proxy).await;
        });
    }

    {
        let app_weak = app.as_weak();
        let proxy_arc = proxy_opt.clone();
        let rt_handle = rt.handle().clone();
        let settings = settings.clone();

        app.on_search_requested(move |query| {
            let query_str = query.to_string();
            let app_weak2 = app_weak.clone();
            let proxy = get_proxy(&proxy_arc);
            let s = settings.lock().unwrap().clone();

            rt_handle.spawn(async move {
                let daemon_pkgs = if let Some(p) = &proxy {
                    p.search(&query_str)
                        .await
                        .ok()
                        .and_then(|json| serde_json::from_str::<Vec<libarc::Package>>(&json).ok())
                        .unwrap_or_default()
                } else {
                    vec![]
                };

                // Use only daemon results which include AppStream data from Flatpak remotes
                let all_pkgs = daemon_pkgs;

                let all_pkgs = dedup_by_preference(all_pkgs, &s);

                let icon_futures: Vec<_> = all_pkgs
                    .iter()
                    .map(|pkg| async {
                        match pkg.provider {
                            Provider::Flatpak => {
                                let id = pkg.id.clone();
                                tokio::task::spawn_blocking(move || {
                                    icons::load_local_flatpak_icon(&id)
                                })
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
                            Provider::Distrobox => {
                                let name = pkg.name.clone();
                                tokio::task::spawn_blocking(move || {
                                    icons::load_native_package_icon(&name)
                                })
                                .await
                                .unwrap_or(None)
                            }
                        }
                    })
                    .collect();
                let icons_result: Vec<_> = join_all(icon_futures).await;
                let raw_pkgs: Vec<RawPackage> = all_pkgs
                    .into_iter()
                    .zip(icons_result)
                    .map(|(pkg, icon)| RawPackage { pkg, icon })
                    .collect();

                let status = format!("Found {} result(s) for '{}'", raw_pkgs.len(), query_str);
                let _ = app_weak2.upgrade_in_event_loop(move |app| {
                    let pkgs: Vec<PackageItem> = raw_pkgs.iter().map(|r| r.to_slint()).collect();
                    app.set_packages(pkgs.as_slice().into());
                    app.set_status_text(status.into());
                    app.set_is_loading(false);
                });
            });

            if let Some(app_ref) = app_weak.upgrade() {
                app_ref.set_is_loading(true);
                app_ref.set_status_text("Searching...".into());
            }
        });
    }

    {
        let app_weak = app.as_weak();
        let proxy_arc = proxy_opt.clone();
        let rt_handle = rt.handle().clone();

        app.on_refresh_requested(move || {
            let app_weak2 = app_weak.clone();
            let proxy = get_proxy(&proxy_arc);

            rt_handle.spawn(async move {
                // Query daemon for installed packages
                let search_pkgs: Vec<libarc::Package> = if let Some(p) = &proxy {
                    p.list_installed()
                        .await
                        .ok()
                        .and_then(|json| serde_json::from_str(&json).ok())
                        .unwrap_or_default()
                } else {
                    vec![]
                };

                // Use cached AppStream DB (loaded once at startup)

                // Load icons for each package in parallel
                let icon_futures: Vec<_> = search_pkgs
                    .iter()
                    .map(|pkg| async {
                        match pkg.provider {
                            Provider::Flatpak => {
                                // Use local AppStream icon
                                let id = pkg.id.clone();
                                tokio::task::spawn_blocking(move || {
                                    icons::load_local_flatpak_icon(&id)
                                })
                                .await
                                .unwrap_or(None)
                            }
                            Provider::Distrobox => {
                                // Use load_native_package_icon for native/Distrobox packages
                                let name = pkg.name.clone();
                                tokio::task::spawn_blocking(move || {
                                    icons::load_native_package_icon(&name)
                                })
                                .await
                                .unwrap_or(None)
                            }
                            Provider::Lutris => {
                                // Load Lutris icon from remote URL provided by Lutris API
                                if let Some(icon_url) = &pkg.icon_url {
                                    let url = icon_url.clone();
                                    tokio::spawn(async move { icons::load_lutris_icon(&url).await })
                                        .await
                                        .ok()
                                        .flatten()
                                } else {
                                    None
                                }
                            }
                        }
                    })
                    .collect();
                let icons_result: Vec<_> = join_all(icon_futures).await;
                let raw_pkgs: Vec<RawPackage> = search_pkgs
                    .into_iter()
                    .zip(icons_result)
                    .map(|(pkg, icon)| RawPackage { pkg, icon })
                    .collect();

                let status = format!("{} application(s) installed", raw_pkgs.len());
                let _ = app_weak2.upgrade_in_event_loop(move |app| {
                    let pkgs: Vec<PackageItem> = raw_pkgs.iter().map(|r| r.to_slint()).collect();
                    app.set_packages(pkgs.as_slice().into());
                    app.set_status_text(status.into());
                    app.set_is_loading(false);
                });
            });

            if let Some(app_ref) = app_weak.upgrade() {
                app_ref.set_is_loading(true);
                app_ref.set_status_text("Loading installed apps...".into());
            }
        });
    }

    {
        let app_weak = app.as_weak();
        let proxy_arc = proxy_opt.clone();
        let rt_handle = rt.handle().clone();

        app.on_install_requested(move |pkg_id| {
            let pkg_id_str = pkg_id.to_string();
            let app_weak2 = app_weak.clone();
            let proxy_arc2 = proxy_arc.clone();

            let in_detail = app_weak
                .upgrade()
                .map(|a| a.get_current_view() == "detail")
                .unwrap_or(false);

            rt_handle.spawn(async move {
                let result = if let Some(p) = get_or_connect(&proxy_arc2).await {
                    p.install_package(&pkg_id_str).await.ok()
                } else {
                    None
                };

                match result {
                    Some(tx_id) => {
                        wait_for_transaction(
                            proxy_arc2,
                            tx_id,
                            app_weak2,
                            format!("Installed {}", pkg_id_str),
                            pkg_id_str.clone(),
                            true,
                            in_detail,
                        )
                        .await;
                    }
                    None => {
                        let _ = app_weak2.upgrade_in_event_loop(move |app| {
                            app.set_detail_busy(false);
                            app.set_status_text(
                                format!("Failed to start install for {}", pkg_id_str).into(),
                            );
                        });
                    }
                }
            });

            if let Some(app_ref) = app_weak.upgrade() {
                if in_detail {
                    app_ref.set_detail_busy(true);
                }
                app_ref.set_status_text(format!("Installing {}...", pkg_id).into());
            }
        });
    }

    {
        let app_weak = app.as_weak();
        let proxy_arc = proxy_opt.clone();
        let rt_handle = rt.handle().clone();

        app.on_remove_requested(move |pkg_id| {
            let pkg_id_str = pkg_id.to_string();
            let app_weak2 = app_weak.clone();
            let proxy_arc2 = proxy_arc.clone();

            let in_detail = app_weak
                .upgrade()
                .map(|a| a.get_current_view() == "detail")
                .unwrap_or(false);

            rt_handle.spawn(async move {
                let result = if let Some(p) = get_or_connect(&proxy_arc2).await {
                    p.remove_package(&pkg_id_str).await.ok()
                } else {
                    None
                };

                match result {
                    Some(tx_id) => {
                        wait_for_transaction(
                            proxy_arc2,
                            tx_id,
                            app_weak2,
                            format!("Removed {}", pkg_id_str),
                            pkg_id_str.clone(),
                            false,
                            in_detail,
                        )
                        .await;
                    }
                    None => {
                        let _ = app_weak2.upgrade_in_event_loop(move |app| {
                            app.set_detail_busy(false);
                            app.set_status_text(
                                format!("Failed to start removal of {}", pkg_id_str).into(),
                            );
                        });
                    }
                }
            });

            if let Some(app_ref) = app_weak.upgrade() {
                if in_detail {
                    app_ref.set_detail_busy(true);
                }
                app_ref.set_status_text(format!("Removing {}...", pkg_id).into());
            }
        });
    }

    {
        let app_weak = app.as_weak();
        let proxy_arc = proxy_opt.clone();
        let rt_handle = rt.handle().clone();

        app.on_category_selected(move |category_id| {
            let cat = category_id.to_string();
            let app_weak2 = app_weak.clone();
            let proxy = get_proxy(&proxy_arc);

            rt_handle.spawn(async move {
                // Query daemon for category apps from AppStream data
                let packages: Vec<libarc::Package> = if let Some(p) = &proxy {
                    p.search_category(&cat)
                        .await
                        .ok()
                        .and_then(|json| serde_json::from_str(&json).ok())
                        .unwrap_or_default()
                } else {
                    vec![]
                };

                // Load icons for each package in parallel
                let icon_futures: Vec<_> = packages
                    .iter()
                    .map(|pkg| async {
                        match pkg.provider {
                            Provider::Flatpak => {
                                let id = pkg.id.clone();
                                tokio::task::spawn_blocking(move || {
                                    icons::load_local_flatpak_icon(&id)
                                })
                                .await
                                .unwrap_or(None)
                            }
                            Provider::Lutris => {
                                // Load Lutris icon from remote URL
                                if let Some(icon_url) = &pkg.icon_url {
                                    let url = icon_url.clone();
                                    tokio::spawn(async move { icons::load_lutris_icon(&url).await })
                                        .await
                                        .ok()
                                        .flatten()
                                } else {
                                    None
                                }
                            }
                            Provider::Distrobox => {
                                let name = pkg.name.clone();
                                tokio::task::spawn_blocking(move || {
                                    icons::load_native_package_icon(&name)
                                })
                                .await
                                .unwrap_or(None)
                            }
                        }
                    })
                    .collect();
                let icons_result: Vec<_> = join_all(icon_futures).await;
                let raw_pkgs: Vec<RawPackage> = packages
                    .into_iter()
                    .zip(icons_result)
                    .map(|(pkg, icon)| RawPackage { pkg, icon })
                    .collect();

                let status = format!("Category: {} ({} apps)", cat, raw_pkgs.len());
                let _ = app_weak2.upgrade_in_event_loop(move |app| {
                    let slint_pkgs: Vec<PackageItem> =
                        raw_pkgs.iter().map(|r| r.to_slint()).collect();
                    app.set_current_view("search".into());
                    app.set_packages(slint_pkgs.as_slice().into());
                    app.set_status_text(status.into());
                    app.set_is_loading(false);
                });
            });

            if let Some(app_ref) = app_weak.upgrade() {
                app_ref.set_current_view("search".into());
                app_ref.set_is_loading(true);
                app_ref.set_status_text(format!("Loading {}...", category_id).into());
            }
        });
    }

    {
        let app_weak = app.as_weak();
        let proxy_arc = proxy_opt.clone();
        let rt_handle = rt.handle().clone();

        app.on_refresh_updates_requested(move || {
            let app_weak2 = app_weak.clone();
            let proxy = get_proxy(&proxy_arc);

            rt_handle.spawn(async move {
                let updates: Vec<libarc::Package> = if let Some(p) = &proxy {
                    p.list_updates()
                        .await
                        .ok()
                        .and_then(|json| serde_json::from_str(&json).ok())
                        .unwrap_or_default()
                } else {
                    vec![]
                };

                let count = updates.len();

                let icon_futures: Vec<_> = updates
                    .iter()
                    .map(|pkg| async {
                        match pkg.provider {
                            Provider::Flatpak => {
                                let id = pkg.id.clone();
                                tokio::task::spawn_blocking(move || {
                                    icons::load_local_flatpak_icon(&id)
                                })
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
                            Provider::Distrobox => {
                                let name = pkg.name.clone();
                                tokio::task::spawn_blocking(move || {
                                    icons::load_native_package_icon(&name)
                                })
                                .await
                                .unwrap_or(None)
                            }
                        }
                    })
                    .collect();
                let icons_result: Vec<_> = join_all(icon_futures).await;
                let raw_pkgs: Vec<RawPackage> = updates
                    .into_iter()
                    .zip(icons_result)
                    .map(|(pkg, icon)| RawPackage { pkg, icon })
                    .collect();

                let status = if count > 0 {
                    format!("{} update(s) available", count)
                } else {
                    "Everything is up to date.".to_string()
                };

                let _ = app_weak2.upgrade_in_event_loop(move |app| {
                    let pkgs: Vec<PackageItem> = raw_pkgs.iter().map(|r| r.to_slint()).collect();
                    app.set_packages(pkgs.as_slice().into());
                    app.set_update_count(count as i32);
                    app.set_status_text(status.into());
                    app.set_is_loading(false);
                });
            });

            if let Some(app_ref) = app_weak.upgrade() {
                app_ref.set_is_loading(true);
                app_ref.set_status_text("Checking for updates...".into());
            }
        });
    }

    {
        let app_weak = app.as_weak();
        let proxy_arc = proxy_opt.clone();
        let rt_handle = rt.handle().clone();

        app.on_update_requested(move |pkg_id| {
            let pkg_id_str = pkg_id.to_string();
            let app_weak2 = app_weak.clone();
            let proxy_arc2 = proxy_arc.clone();

            let in_detail = app_weak
                .upgrade()
                .map(|a| a.get_current_view() == "detail")
                .unwrap_or(false);

            rt_handle.spawn(async move {
                let result = if let Some(p) = get_or_connect(&proxy_arc2).await {
                    p.update_package(&pkg_id_str).await.ok()
                } else {
                    None
                };

                match result {
                    Some(tx_id) => {
                        wait_for_transaction(
                            proxy_arc2,
                            tx_id,
                            app_weak2,
                            format!("Updated {}", pkg_id_str),
                            pkg_id_str.clone(),
                            true,
                            in_detail,
                        )
                        .await;
                    }
                    None => {
                        let _ = app_weak2.upgrade_in_event_loop(move |app| {
                            app.set_detail_busy(false);
                            app.set_status_text(
                                format!("Failed to start update for {}", pkg_id_str).into(),
                            );
                        });
                    }
                }
            });

            if let Some(app_ref) = app_weak.upgrade() {
                if in_detail {
                    app_ref.set_detail_busy(true);
                }
                app_ref.set_status_text(format!("Updating {}...", pkg_id).into());
            }
        });
    }

    {
        let app_weak = app.as_weak();
        let proxy_arc = proxy_opt.clone();
        let settings = settings.clone();
        let rt_handle = rt.handle().clone();

        app.on_detail_requested(move |id| {
            let app_id = id.to_string();
            let app_weak2 = app_weak.clone();
            let proxy = get_proxy(&proxy_arc);
            let _s = settings.lock().unwrap().clone();

            rt_handle.spawn(async move {
                let appstream_db = AppStreamDb::get_static();

                // Get app name from AppStream immediately (fast local lookup)
                let app_name = appstream_db
                    .find_by_id(&id)
                    .map(|a| a.name.clone())
                    .unwrap_or_else(|| id.split(';').next().unwrap_or(&id).to_string());

                // search + installed from daemon so we can find both providers
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
                let all_pkgs: Vec<&Package> =
                    search_pkgs.iter().chain(installed_pkgs.iter()).collect();

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

                // Use locally loaded AppStream DB for fast name/description/icon lookup
                let appstream_info = appstream_db.find_by_id(&app_id);

                // Determine verification status and developer name
                let (developer, verified) = if let Some(fp_pkg) = &flatpak_pkg {
                    let remote = fp_pkg.remote.as_deref().unwrap_or("");
                    let dev_name = appstream_info
                        .as_ref()
                        .and_then(|a| a.id.split('.').last().map(|s| s.to_string()))
                        .unwrap_or_else(|| fp_pkg.name.clone());

                    if remote == "blossomos" {
                        // Always verified for blossomos remote
                        (dev_name, true)
                    } else if remote == "flathub" {
                        // Check Flathub API for verification status
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

                // Load icon in parallel with UI display
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

                // Load icon from AppStream data if no package icon found
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

                // Get name from packages or AppStream
                let name = flatpak_pkg
                    .or(native_pkg)
                    .or(lutris_pkg)
                    .map(|p| p.name.clone())
                    .or_else(|| appstream_info.as_ref().map(|a| a.name.clone()))
                    .unwrap_or_else(|| app_name.clone());

                // Get description from packages or AppStream
                // Skip description for Flatpaks since it duplicates the summary
                let description = native_pkg
                    .or(lutris_pkg)
                    .map(|p| p.description.clone())
                    .or_else(|| appstream_info.as_ref().map(|a| a.summary.clone()))
                    .unwrap_or_default();

                let version = flatpak_pkg
                    .or(native_pkg)
                    .or(lutris_pkg)
                    .map(|p| p.version.clone())
                    .unwrap_or_default();

                // Collect screenshot URLs from the best matching package
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
                };

                let _ = app_weak2.upgrade_in_event_loop(move |app| {
                    app.set_detail_screenshots([].as_slice().into());
                    app.set_detail_app(AppDetailData {
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
                        installed: raw.flatpak_installed
                            || raw.native_installed
                            || raw.lutris_installed,
                        verified: raw.verified,
                    });
                    app.set_detail_loading(false);
                });

                // Load screenshots in background after the detail view is shown
                if !screenshot_urls.is_empty() {
                    let app_weak3 = app_weak2.clone();
                    tokio::spawn(async move {
                        let futs: Vec<_> = screenshot_urls
                            .iter()
                            .map(|url| {
                                let url = url.clone();
                                tokio::spawn(async move { icons::load_screenshot(&url).await })
                            })
                            .collect();
                        // Collect as RawIcon (Send), convert to slint::Image on the UI thread
                        let raw_shots: Vec<RawIcon> = join_all(futs)
                            .await
                            .into_iter()
                            .filter_map(|r| r.ok().flatten())
                            .collect();
                        if !raw_shots.is_empty() {
                            let _ = app_weak3.upgrade_in_event_loop(move |app| {
                                let shots: Vec<slint::Image> =
                                    raw_shots.iter().map(|r| r.to_slint_image()).collect();
                                app.set_detail_screenshots(shots.as_slice().into());
                            });
                        }
                    });
                }
            });

            if let Some(app_ref) = app_weak.upgrade() {
                app_ref.set_detail_screenshots([].as_slice().into());
                app_ref.set_detail_loading(true);
                app_ref.set_current_view("detail".into());
            }
        });
    }

    {
        let settings = settings.clone();
        app.on_save_settings(move |preferred, ignore_native_pref| {
            let mut s = settings.lock().unwrap();
            s.preferred_provider = if preferred == "Native" {
                Provider::Distrobox
            } else {
                Provider::Flatpak
            };
            s.ignore_native_preference = ignore_native_pref;
            let _ = s.save();
        });
    }

    {
        let app_weak = app.as_weak();
        let manage_extensions = std::env::args().any(|a| a == "--manage-extensions");
        let initial_app = std::env::args()
            .find(|a| a.starts_with("appstream://") || a.starts_with("appstream:"))
            .map(|a| {
                a.trim_start_matches("appstream://")
                    .trim_start_matches("appstream:")
                    .trim_start_matches("//")
                    .to_string()
            });

        if manage_extensions || initial_app.as_deref() == Some("") {
            if let Some(app_ref) = app_weak.upgrade() {
                app_ref.set_current_view("installed".into());
                app_ref.invoke_refresh_requested();
            }
        } else if let Some(app_id) = initial_app.filter(|s| !s.is_empty()) {
            if let Some(app_ref) = app_weak.upgrade() {
                app_ref.invoke_detail_requested(app_id.into());
            }
        }
    }

    // package file opened via file manager / MIME association
    {
        let app_weak = app.as_weak();
        let rt_handle = rt.handle().clone();
        let proxy_arc = proxy_opt.clone();

        let pkg_file = std::env::args().skip(1).find(|a| is_pkg_file(a));

        if let Some(file_path) = pkg_file {
            let file_name = std::path::Path::new(&file_path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&file_path)
                .to_string();
            let pkg_name = pkg_name_from_filename(&file_name);

            if let Some(app_ref) = app_weak.upgrade() {
                app_ref.set_install_file_path(file_path.clone().into());
                app_ref.set_install_file_name(file_name.clone().into());
                app_ref.set_install_file_has_flatpak(false);
                app_ref.set_current_view("install-file".into());
            }

            // search daemon in background for a matching Flatpak app
            let app_weak2 = app_weak.clone();
            let proxy_search = get_proxy(&proxy_arc);
            rt_handle.spawn(async move {
                if let Some(p) = proxy_search {
                    if let Ok(results) = p.search(&pkg_name).await {
                        if let Ok(pkgs) = serde_json::from_str::<Vec<libarc::Package>>(&results) {
                            if let Some(first) =
                                pkgs.iter().find(|p| p.provider == Provider::Flatpak)
                            {
                                let id = first.id.clone();
                                let name = first.name.clone();
                                let _ = app_weak2.upgrade_in_event_loop(move |app| {
                                    app.set_install_file_flatpak_id(id.into());
                                    app.set_install_file_flatpak_name(name.into());
                                    app.set_install_file_has_flatpak(true);
                                });
                            }
                        }
                    }
                }
            });

            // "Install via Distrobox" — pass the file path as the package_id
            {
                let app_weak3 = app_weak.clone();
                let proxy_arc2 = proxy_arc.clone();
                let rt_handle2 = rt_handle.clone();
                let fp = file_path.clone();
                app.on_install_file_distrobox_requested(move || {
                    let fp2 = fp.clone();
                    let app_weak4 = app_weak3.clone();
                    let proxy_arc3 = proxy_arc2.clone();
                    rt_handle2.spawn(async move {
                        let result = if let Some(p) = get_proxy(&proxy_arc3) {
                            p.install_package(&fp2).await.ok()
                        } else {
                            None
                        };
                        match result {
                            Some(tx_id) => {
                                wait_for_transaction(
                                    proxy_arc3,
                                    tx_id,
                                    app_weak4.clone(),
                                    format!("Installed {}", fp2),
                                    fp2.clone(),
                                    true,
                                    false,
                                )
                                .await;
                                let _ = app_weak4.upgrade_in_event_loop(|app| {
                                    app.set_current_view("home".into());
                                });
                            }
                            None => {
                                let _ = app_weak4.upgrade_in_event_loop(|app| {
                                    app.set_status_text("Failed to connect to Arc daemon.".into());
                                });
                            }
                        }
                    });
                    if let Some(app_ref) = app_weak3.upgrade() {
                        app_ref.set_current_view("home".into());
                        app_ref.set_status_text("Installing package...".into());
                    }
                });
            }

            // "Install from Flathub instead" — navigate to detail view
            {
                let app_weak3 = app_weak.clone();
                app.on_install_file_flatpak_requested(move || {
                    if let Some(app_ref) = app_weak3.upgrade() {
                        let flatpak_id = app_ref.get_install_file_flatpak_id().to_string();
                        app_ref.invoke_detail_requested(flatpak_id.into());
                    }
                });
            }

            // "Cancel"
            {
                let app_weak3 = app_weak.clone();
                app.on_install_file_cancelled(move || {
                    if let Some(app_ref) = app_weak3.upgrade() {
                        app_ref.set_current_view("home".into());
                    }
                });
            }
        }
    }

    app.run()?;
    Ok(())
}
