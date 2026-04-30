mod icons;

use anyhow::Result;
use futures_util::StreamExt;
use libarc::flathub::{fetch_popular, fetch_recently_added, FlathubApp, CATEGORIES};
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

fn packages_to_slint(pkgs: &[Package]) -> Vec<PackageItem> {
    pkgs.iter()
        .map(|p| PackageItem {
            id: SharedString::from(p.id.as_str()),
            name: SharedString::from(p.name.as_str()),
            version: SharedString::from(p.version.as_str()),
            description: SharedString::from(p.description.as_str()),
            installed: p.installed,
            icon: Default::default(),
        })
        .collect()
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
            }
        })
        .collect()
}

fn get_proxy(
    proxy_arc: &Arc<Mutex<Option<ArcDaemonProxy<'static>>>>,
) -> Option<ArcDaemonProxy<'static>> {
    proxy_arc.lock().unwrap().clone()
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
) {
    let Some(p) = get_proxy(&proxy_arc) else { return };

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
                                    });
                                } else {
                                    let msg = format!("Failed: {}", args.message());
                                    let _ = app_weak.upgrade_in_event_loop(move |app| {
                                        app.set_status_text(msg.into());
                                        app.set_progress(0.0);
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
    icon: Option<icons::RawIcon>,
}

struct RawCategoryData {
    id: String,
    label: String,
    icon: Option<icons::RawIcon>,
    color: (u8, u8, u8),
}

struct RawDetailData {
    name: String,
    developer: String,
    description: String,
    summary: String,
    version: String,
    icon: Option<icons::RawIcon>,
    flatpak_id: String,
    native_id: String,
    flatpak_installed: bool,
    native_installed: bool,
}

struct RawPackage {
    pkg: libarc::Package,
    icon: Option<icons::RawIcon>,
}

impl RawPackage {
    fn to_slint(&self) -> PackageItem {
        PackageItem {
            id: slint::SharedString::from(self.pkg.id.as_str()),
            name: slint::SharedString::from(self.pkg.name.as_str()),
            version: slint::SharedString::from(self.pkg.version.as_str()),
            description: slint::SharedString::from(self.pkg.description.as_str()),
            installed: self.pkg.installed,
            icon: self.icon.as_ref().map(|r| r.to_slint_image()).unwrap_or_default(),
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

async fn load_home(app_weak: slint::Weak<AppWindow>) {
    let (popular_raw, recent_raw) = tokio::join!(fetch_popular(), fetch_recently_added());

    let popular: Vec<FlathubApp> = popular_raw.unwrap_or_default().into_iter().take(10).collect();
    let recent: Vec<FlathubApp> = recent_raw.unwrap_or_default().into_iter().take(4).collect();

    let mut popular_cards: Vec<RawCard> = Vec::new();
    for app in &popular {
        let icon = if let Some(url) = &app.icon {
            icons::load_icon(url).await
        } else {
            None
        };
        popular_cards.push(RawCard {
            id: app.app_id.clone(),
            name: app.name.clone(),
            summary: app.summary.clone(),
            icon,
        });
    }

    let mut recent_cards: Vec<RawCard> = Vec::new();
    for app in &recent {
        let icon = if let Some(url) = &app.icon {
            icons::load_icon(url).await
        } else {
            None
        };
        recent_cards.push(RawCard {
            id: app.app_id.clone(),
            name: app.name.clone(),
            summary: app.summary.clone(),
            icon,
        });
    }

    let mut raw_cats: Vec<RawCategoryData> = Vec::new();
    for (id, label, icon_name) in CATEGORIES {
        let name = icon_name.to_string();
        let data = tokio::task::spawn_blocking(move || icons::load_category_icon(&name))
            .await
            .unwrap_or(icons::CategoryIconData { icon: None, color: (80, 80, 90) });
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
                icon: c.icon.as_ref().map(|r| r.to_slint_image()).unwrap_or_default(),
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
        app.set_settings_preferred(match s.preferred_provider {
            Provider::Distrobox => "Native",
            Provider::Flatpak => "Flatpak",
        }.into());
        app.set_settings_ignore_native_pref(s.ignore_native_preference);
    }

    {
        let app_weak = app.as_weak();
        app.set_home_loading(true);
        rt.handle().spawn(async move {
            load_home(app_weak).await;
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
                let daemon_future = async {
                    if let Some(p) = &proxy {
                        p.search(&query_str).await.ok()
                            .and_then(|json| serde_json::from_str::<Vec<libarc::Package>>(&json).ok())
                            .unwrap_or_default()
                    } else {
                        vec![]
                    }
                };
                let flathub_future = libarc::flathub::search(&query_str);
                let (daemon_pkgs, flathub_result) = tokio::join!(daemon_future, flathub_future);
                let flathub_apps = flathub_result.unwrap_or_default();
                let icon_map: std::collections::HashMap<String, String> = flathub_apps
                    .iter()
                    .filter_map(|a| a.icon.as_ref().map(|url| (a.app_id.clone(), url.clone())))
                    .collect();

                let daemon_pkgs = dedup_by_preference(daemon_pkgs, &s);

                let mut raw_pkgs: Vec<RawPackage> = Vec::new();
                for pkg in daemon_pkgs {
                    let icon = if let Some(url) = icon_map.get(&pkg.id) {
                        icons::load_icon(url).await
                    } else if pkg.provider == Provider::Flatpak {
                        let id = pkg.id.clone();
                        tokio::task::spawn_blocking(move || icons::load_local_flatpak_icon(&id))
                            .await
                            .unwrap_or(None)
                    } else {
                        None
                    };
                    raw_pkgs.push(RawPackage { pkg, icon });
                }

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
                let packages: Vec<libarc::Package> = if let Some(p) = proxy {
                    p.list_installed().await.ok()
                        .and_then(|json| serde_json::from_str(&json).ok())
                        .unwrap_or_default()
                } else {
                    vec![]
                };

                let mut raw_pkgs: Vec<RawPackage> = Vec::new();
                for pkg in packages {
                    let icon = if pkg.provider == Provider::Flatpak {
                        let id = pkg.id.clone();
                        tokio::task::spawn_blocking(move || icons::load_local_flatpak_icon(&id))
                            .await
                            .unwrap_or(None)
                    } else {
                        None
                    };
                    raw_pkgs.push(RawPackage { pkg, icon });
                }

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
            let proxy = get_proxy(&proxy_arc);
            let proxy_arc2 = proxy_arc.clone();

            rt_handle.spawn(async move {
                let result = if let Some(p) = proxy {
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
                        )
                        .await;
                    }
                    None => {
                        let _ = app_weak2.upgrade_in_event_loop(move |app| {
                            app.set_status_text(
                                format!("Failed to start install for {}", pkg_id_str).into(),
                            );
                        });
                    }
                }
            });

            if let Some(app_ref) = app_weak.upgrade() {
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
            let proxy = get_proxy(&proxy_arc);
            let proxy_arc2 = proxy_arc.clone();

            rt_handle.spawn(async move {
                let result = if let Some(p) = proxy {
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
                        )
                        .await;
                    }
                    None => {
                        let _ = app_weak2.upgrade_in_event_loop(move |app| {
                            app.set_status_text(
                                format!("Failed to start removal of {}", pkg_id_str).into(),
                            );
                        });
                    }
                }
            });

            if let Some(app_ref) = app_weak.upgrade() {
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
            let _proxy = get_proxy(&proxy_arc);

            rt_handle.spawn(async move {
                let result = libarc::flathub::fetch_category(&cat).await.unwrap_or_default();
                let packages: Vec<Package> = result
                    .iter()
                    .map(|a| Package {
                        id: a.app_id.clone(),
                        name: a.name.clone(),
                        version: String::new(),
                        description: a.summary.clone(),
                        provider: libarc::Provider::Flatpak,
                        installed: false,
                    })
                    .collect();
                let status = format!("Category: {} ({} apps)", cat, packages.len());
                let _ = app_weak2.upgrade_in_event_loop(move |app| {
                    let slint_pkgs = packages_to_slint(&packages);
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
        let settings = settings.clone();
        let rt_handle = rt.handle().clone();

        app.on_detail_requested(move |app_id| {
            let id = app_id.to_string();
            let app_weak2 = app_weak.clone();
            let proxy = get_proxy(&proxy_arc);
            let s = settings.lock().unwrap().clone();

            rt_handle.spawn(async move {
                let flathub = libarc::flathub::fetch_app(&id).await.unwrap_or(None);

                let app_name = flathub.as_ref()
                    .map(|f| f.name.clone())
                    .unwrap_or_else(|| id.split(';').next().unwrap_or(&id).to_string());

                // search + installed from daemon so we can find both providers
                let (search_pkgs, installed_pkgs): (Vec<Package>, Vec<Package>) = if let Some(ref p) = proxy {
                    tokio::join!(
                        async {
                            p.search(&app_name).await.ok()
                                .and_then(|j| serde_json::from_str(&j).ok())
                                .unwrap_or_default()
                        },
                        async {
                            p.list_installed().await.ok()
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
                        && (p.id == id
                            || p.id.to_lowercase() == id.to_lowercase()
                            || p.name.to_lowercase() == name_lower)
                });

                let native_pkg = all_pkgs.iter().copied().find(|p| {
                    p.provider == Provider::Distrobox
                        && (p.id.split(';').next().map(|n| n.to_lowercase()).as_deref() == Some(name_lower.as_str())
                            || p.name.to_lowercase() == name_lower)
                });

                let flatpak_id = flatpak_pkg
                    .map(|p| p.id.clone())
                    .unwrap_or_else(|| if id.contains('.') && !id.contains(';') { id.clone() } else { String::new() });
                let native_id = native_pkg.map(|p| p.id.clone()).unwrap_or_default();

                let flatpak_installed = flatpak_pkg.map(|p| p.installed).unwrap_or(false)
                    || installed_pkgs.iter().any(|p| p.provider == Provider::Flatpak && p.id == flatpak_id);
                let native_installed = native_pkg.map(|p| p.installed).unwrap_or(false);

                let preferred = s.preferred_for(&id);
                let selected_provider = if preferred == Provider::Distrobox && !native_id.is_empty() {
                    "native"
                } else {
                    "flatpak"
                }.to_string();

                let icon = if let Some(ref info) = flathub {
                    if let Some(url) = &info.icon { icons::load_icon(url).await } else { None }
                } else if !flatpak_id.is_empty() {
                    let fid = flatpak_id.clone();
                    tokio::task::spawn_blocking(move || icons::load_local_flatpak_icon(&fid))
                        .await.unwrap_or(None)
                } else {
                    None
                };

                let version = flatpak_pkg.or(native_pkg).map(|p| p.version.clone()).unwrap_or_default();

                let raw = RawDetailData {
                    name: flathub.as_ref().map(|f| f.name.clone()).unwrap_or_else(|| app_name.clone()),
                    developer: flathub.as_ref().and_then(|f| f.developer_name.clone()).unwrap_or_default(),
                    description: flathub.as_ref().and_then(|f| f.description.clone()).unwrap_or_default(),
                    summary: flathub.as_ref().map(|f| f.summary.clone()).unwrap_or_default(),
                    version,
                    icon,
                    flatpak_id,
                    native_id,
                    flatpak_installed,
                    native_installed,
                };

                let _ = app_weak2.upgrade_in_event_loop(move |app| {
                    app.set_detail_selected_provider(selected_provider.into());
                    app.set_detail_app(AppDetailData {
                        id: Default::default(),
                        name: raw.name.into(),
                        developer: raw.developer.into(),
                        description: raw.description.into(),
                        summary: raw.summary.into(),
                        version: raw.version.into(),
                        icon: raw.icon.as_ref().map(|r| r.to_slint_image()).unwrap_or_default(),
                        installed: raw.flatpak_installed || raw.native_installed,
                        flatpak_id: raw.flatpak_id.into(),
                        native_id: raw.native_id.into(),
                        flatpak_installed: raw.flatpak_installed,
                        native_installed: raw.native_installed,
                    });
                    app.set_detail_loading(false);
                });
            });

            if let Some(app_ref) = app_weak.upgrade() {
                app_ref.set_detail_loading(true);
                app_ref.set_current_view("detail".into());
            }
        });
    }

    {
        let settings = settings.clone();
        app.on_save_settings(move |preferred, ignore_native_pref| {
            let mut s = settings.lock().unwrap();
            s.preferred_provider = if preferred == "Native" { Provider::Distrobox } else { Provider::Flatpak };
            s.ignore_native_preference = ignore_native_pref;
            let _ = s.save();
        });
    }

    {
        let app_weak = app.as_weak();
        let initial_app = std::env::args()
            .find(|a| a.starts_with("appstream://") || a.starts_with("appstream:"))
            .map(|a| {
                a.trim_start_matches("appstream://")
                    .trim_start_matches("appstream:")
                    .trim_start_matches("//")
                    .to_string()
            });
        if let Some(app_id) = initial_app {
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

            // search Flathub in background for a matching app
            let app_weak2 = app_weak.clone();
            rt_handle.spawn(async move {
                if let Ok(results) = libarc::flathub::search(&pkg_name).await {
                    if let Some(first) = results.into_iter().next() {
                        let id = first.app_id.clone();
                        let name = first.name.clone();
                        let _ = app_weak2.upgrade_in_event_loop(move |app| {
                            app.set_install_file_flatpak_id(id.into());
                            app.set_install_file_flatpak_name(name.into());
                            app.set_install_file_has_flatpak(true);
                        });
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
                                )
                                .await;
                                let _ = app_weak4.upgrade_in_event_loop(|app| {
                                    app.set_current_view("home".into());
                                });
                            }
                            None => {
                                let _ = app_weak4.upgrade_in_event_loop(|app| {
                                    app.set_status_text(
                                        "Failed to connect to Arc daemon.".into(),
                                    );
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
