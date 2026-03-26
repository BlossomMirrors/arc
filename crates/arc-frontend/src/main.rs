mod icons;

use anyhow::Result;
use libarc::flathub::{fetch_popular, fetch_recently_added, FlathubApp, CATEGORIES};
use libarc::{connect, ArcDaemonProxy};
use libarc::{Package, Provider, Transaction, TransactionStatus};
use slint::{Model, SharedString};
use std::sync::{Arc, Mutex};

slint::include_modules!();

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
    loop {
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        let proxy = get_proxy(&proxy_arc);
        if let Some(p) = proxy {
            if let Ok(json) = p.get_transaction(&tx_id).await {
                if let Ok(tx) = serde_json::from_str::<Transaction>(&json) {
                    let progress = tx.progress as f32 / 100.0;
                    match &tx.status {
                        TransactionStatus::Success => {
                            let msg = success_msg.clone();
                            let pid = pkg_id.clone();
                            let _ = app_weak.upgrade_in_event_loop(move |app| {
                                update_package_installed(&app, &pid, installed_after);
                                app.set_status_text(msg.into());
                                app.set_progress(0.0);
                            });
                            break;
                        }
                        TransactionStatus::Failed(msg) => {
                            let msg = format!("Failed: {}", msg);
                            let _ = app_weak.upgrade_in_event_loop(move |app| {
                                app.set_status_text(msg.into());
                                app.set_progress(0.0);
                            });
                            break;
                        }
                        _ => {
                            let _ = app_weak.upgrade_in_event_loop(move |app| {
                                app.set_progress(progress);
                            });
                        }
                    }
                }
            }
        } else {
            break;
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
    id: String,
    name: String,
    developer: String,
    description: String,
    summary: String,
    version: String,
    icon: Option<icons::RawIcon>,
    installed: bool,
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

        app.on_search_requested(move |query| {
            let query_str = query.to_string();
            let app_weak2 = app_weak.clone();
            let proxy = get_proxy(&proxy_arc);

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
        let rt_handle = rt.handle().clone();

        app.on_detail_requested(move |app_id| {
            let id = app_id.to_string();
            let app_weak2 = app_weak.clone();

            rt_handle.spawn(async move {
                let flathub = libarc::flathub::fetch_app(&id).await.unwrap_or(None);
                let raw = if let Some(info) = flathub {
                    let icon = if let Some(url) = &info.icon {
                        icons::load_icon(url).await
                    } else {
                        None
                    };
                    RawDetailData {
                        id: info.app_id.clone(),
                        name: info.name.clone(),
                        developer: info.developer_name.clone().unwrap_or_default(),
                        description: info.description.clone().unwrap_or_default(),
                        summary: info.summary.clone(),
                        version: String::new(),
                        icon,
                        installed: false,
                    }
                } else {
                    RawDetailData {
                        id: id.clone(),
                        name: id.clone(),
                        developer: String::new(),
                        description: String::new(),
                        summary: String::new(),
                        version: String::new(),
                        icon: None,
                        installed: false,
                    }
                };

                let _ = app_weak2.upgrade_in_event_loop(move |app| {
                    app.set_detail_app(AppDetailData {
                        id: raw.id.into(),
                        name: raw.name.into(),
                        developer: raw.developer.into(),
                        description: raw.description.into(),
                        summary: raw.summary.into(),
                        version: raw.version.into(),
                        icon: raw.icon.as_ref().map(|r| r.to_slint_image()).unwrap_or_default(),
                        installed: raw.installed,
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

    app.run()?;
    Ok(())
}
