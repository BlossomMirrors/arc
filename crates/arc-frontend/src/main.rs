mod appstream_db;
mod helpers;
mod icons;
mod packages;
mod transactions;

use anyhow::Result;
use helpers::{
    get_display_name, get_proxy, is_appimage, is_flatpak_bundle, is_flatpakrepo, is_pkg_file,
    parse_flatpakref, parse_flatpakrepo, pkg_name_from_filename,
};
use libarc::{ArcDaemonProxy, Provider, Settings};
use packages::{load_detail, load_home, load_package_icons, refresh_home_installed};
use slint::Model;
use std::sync::{Arc, Mutex};
use transactions::{
    add_to_available_updates, begin_transaction, push_transactions_to_ui, remove_from_available_updates, run_signal_listener,
    SavedPkgData, TxStatus, TxStore,
};

slint::include_modules!();

fn main() -> Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    let proxy_result = rt.block_on(async {
        tokio::time::timeout(std::time::Duration::from_secs(2), libarc::connect())
            .await
            .unwrap_or_else(|_| Err(anyhow::anyhow!("daemon connection timed out")))
    });

    let app = AppWindow::new()?;

    if let Some(icon) = icons::load_ui_icon("go-home-symbolic") {
        app.set_icon_home(icon.to_slint_image());
    }
    if let Some(icon) = icons::load_ui_icon("edit-find-symbolic") {
        app.set_icon_search(icon.to_slint_image());
    }
    if let Some(icon) = icons::load_ui_icon("settings-configure") {
        app.set_icon_settings(icon.to_slint_image());
    }

    let proxy_opt: Arc<Mutex<Option<ArcDaemonProxy<'static>>>> =
        Arc::new(Mutex::new(proxy_result.ok()));

    let settings: Arc<Mutex<Settings>> = Arc::new(Mutex::new(Settings::load()));

    {
        let s = settings.lock().unwrap();
        app.set_settings_preferred(
            match s.preferred_provider {
                Provider::Distrobox => "Native",
                Provider::Flatpak => "Flatpak",
                Provider::Lutris => "Lutris",
                Provider::AppImage => "AppImage",
            }
            .into(),
        );
        app.set_settings_ignore_native_pref(s.ignore_native_preference);
        app.set_settings_auto_updates(s.auto_updates);
        app.set_settings_concurrent_downloads(s.concurrent_downloads as i32);
    }

    let tx_store: TxStore = Arc::new(Mutex::new(Vec::new()));

    {
        let app_weak = app.as_weak();
        let proxy = proxy_opt.lock().unwrap().clone();
        let rt_handle = rt.handle().clone();

        app.on_home_clicked(move || {
            let app_weak = app_weak.clone();
            let proxy = proxy.clone();
            let rt_handle = rt_handle.clone();
            rt_handle.spawn(async move {
                refresh_home_installed(app_weak, proxy).await;
            });
        });
    }

    {
        let app_weak = app.as_weak();
        let proxy = proxy_opt.lock().unwrap().clone();
        let rt_handle = rt.handle().clone();

        app.on_back_to_home(move || {
            let app_weak = app_weak.clone();
            let proxy = proxy.clone();
            let rt_handle = rt_handle.clone();
            rt_handle.spawn(async move {
                refresh_home_installed(app_weak, proxy).await;
            });
        });
    }

    if let Some(proxy) = get_proxy(&proxy_opt) {
        let store = tx_store.clone();
        let app_weak = app.as_weak();
        rt.handle()
            .spawn(run_signal_listener(proxy, store, app_weak));
    }

    {
        let app_weak = app.as_weak();
        let proxy = proxy_opt.lock().unwrap().clone();
        app.set_home_loading(true);
        rt.handle().spawn(async move {
            tokio::task::spawn_blocking(appstream_db::AppStreamDb::get_static)
                .await
                .unwrap();
            load_home(app_weak, proxy).await;
        });
    }

    if let Some(proxy) = proxy_opt.lock().unwrap().clone() {
        let app_weak = app.as_weak();
        rt.handle().spawn(async move {
            let updates: Vec<libarc::Package> = proxy
                .list_updates()
                .await
                .ok()
                .and_then(|json| serde_json::from_str(&json).ok())
                .unwrap_or_default();
            let count = updates.len();
            if count == 0 {
                return;
            }
            let raw_pkgs = load_package_icons(updates).await;
            let _ = app_weak.upgrade_in_event_loop(move |app| {
                let pkgs: Vec<PackageItem> = raw_pkgs.iter().map(|r| r.to_slint()).collect();
                app.set_available_updates(pkgs.as_slice().into());
                app.set_update_count(count as i32);
            });
        });
    }

    {
        let app_weak = app.as_weak();
        let proxy_arc = proxy_opt.clone();
        let rt_handle = rt.handle().clone();
        let settings = settings.clone();

        app.on_search_requested(move |query| {
            let query_str = query.to_string();
            if query_str.trim().is_empty() {
                return;
            }

            if let Some(app_ref) = app_weak.upgrade() {
                app_ref.set_packages([].as_slice().into());
                app_ref.set_is_loading(true);
            }

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

                // Fall back to local AppStream DB when daemon is unavailable or returns nothing
                let all_pkgs = if daemon_pkgs.is_empty() {
                    let q = query_str.clone();
                    tokio::task::spawn_blocking(move || {
                        appstream_db::AppStreamDb::get_static()
                            .search_apps(&q)
                            .into_iter()
                            .map(|e| libarc::Package {
                                id: e.id,
                                name: e.name,
                                version: String::new(),
                                description: e.summary,
                                provider: libarc::Provider::Flatpak,
                                installed: false,
                                icon_url: e.icon_url,
                                remote: e.remote,
                                screenshots: vec![],
                            })
                            .collect::<Vec<_>>()
                    })
                    .await
                    .unwrap_or_default()
                } else {
                    daemon_pkgs
                };

                let all_pkgs = helpers::dedup_by_preference(all_pkgs, &s);
                let raw_pkgs = load_package_icons(all_pkgs).await;
                let _ = app_weak2.upgrade_in_event_loop(move |app| {
                    let pkgs: Vec<PackageItem> = raw_pkgs.iter().map(|r| r.to_slint()).collect();
                    app.set_packages(pkgs.as_slice().into());

                    app.set_is_loading(false);
                });
            });
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
                let search_pkgs: Vec<libarc::Package> = if let Some(p) = &proxy {
                    p.list_installed()
                        .await
                        .ok()
                        .and_then(|json| serde_json::from_str(&json).ok())
                        .unwrap_or_default()
                } else {
                    vec![]
                };

                let raw_pkgs = load_package_icons(search_pkgs).await;
                let _ = app_weak2.upgrade_in_event_loop(move |app| {
                    let pkgs: Vec<PackageItem> = raw_pkgs.iter().map(|r| r.to_slint()).collect();
                    app.set_packages(pkgs.as_slice().into());

                    app.set_is_loading(false);
                });
            });

            if let Some(app_ref) = app_weak.upgrade() {
                app_ref.set_is_loading(true);
            }
        });
    }

    {
        let app_weak = app.as_weak();
        let proxy_arc = proxy_opt.clone();
        let rt_handle = rt.handle().clone();
        let store = tx_store.clone();

        app.on_install_requested(move |pkg_id| {
            let pkg_id_str = pkg_id.to_string();
            let in_detail = app_weak
                .upgrade()
                .map(|a| a.get_current_view() == "detail")
                .unwrap_or(false);
            let display_name = app_weak
                .upgrade()
                .map(|a| get_display_name(&a, &pkg_id_str))
                .unwrap_or_else(|| pkg_id_str.clone());

            begin_transaction(
                pkg_id_str,
                display_name,
                "install".to_string(),
                true,
                in_detail,
                None,
                store.clone(),
                proxy_arc.clone(),
                app_weak.clone(),
                rt_handle.clone(),
            );
        });
    }

    {
        let app_weak = app.as_weak();
        let proxy_arc = proxy_opt.clone();
        let rt_handle = rt.handle().clone();
        let store = tx_store.clone();

        app.on_remove_requested(move |pkg_id| {
            let pkg_id_str = pkg_id.to_string();
            let in_detail = app_weak
                .upgrade()
                .map(|a| a.get_current_view() == "detail")
                .unwrap_or(false);
            let display_name = app_weak
                .upgrade()
                .map(|a| get_display_name(&a, &pkg_id_str))
                .unwrap_or_else(|| pkg_id_str.clone());

            begin_transaction(
                pkg_id_str,
                display_name,
                "remove".to_string(),
                false,
                in_detail,
                None,
                store.clone(),
                proxy_arc.clone(),
                app_weak.clone(),
                rt_handle.clone(),
            );
        });
    }

    {
        let app_weak = app.as_weak();
        let proxy_arc = proxy_opt.clone();
        let rt_handle = rt.handle().clone();
        let store = tx_store.clone();

        app.on_update_requested(move |pkg_id| {
            let pkg_id_str = pkg_id.to_string();
            let in_detail = app_weak
                .upgrade()
                .map(|a| a.get_current_view() == "detail")
                .unwrap_or(false);
            let display_name = app_weak
                .upgrade()
                .map(|a| get_display_name(&a, &pkg_id_str))
                .unwrap_or_else(|| pkg_id_str.clone());
            let saved_pkg = app_weak.upgrade().and_then(|a| {
                let model = a.get_available_updates();
                (0..model.row_count())
                    .filter_map(|i| model.row_data(i))
                    .find(|p| p.id.as_str() == pkg_id_str.as_str())
                    .map(|p| SavedPkgData {
                        id: p.id.to_string(),
                        name: p.name.to_string(),
                        version: p.version.to_string(),
                        description: p.description.to_string(),
                        installed: p.installed,
                    })
            });

            begin_transaction(
                pkg_id_str.clone(),
                display_name,
                "update".to_string(),
                true,
                in_detail,
                saved_pkg,
                store.clone(),
                proxy_arc.clone(),
                app_weak.clone(),
                rt_handle.clone(),
            );
            if let Some(a) = app_weak.upgrade() {
                remove_from_available_updates(&a, &pkg_id_str);
            }
        });
    }

    {
        let app_weak = app.as_weak();
        let proxy_arc = proxy_opt.clone();
        let rt_handle = rt.handle().clone();
        let store = tx_store.clone();

        app.on_update_all_requested(move || {
            let updates: Vec<SavedPkgData> = app_weak
                .upgrade()
                .map(|a| {
                    let model = a.get_available_updates();
                    (0..model.row_count())
                        .filter_map(|i| model.row_data(i))
                        .map(|p| SavedPkgData {
                            id: p.id.to_string(),
                            name: p.name.to_string(),
                            version: p.version.to_string(),
                            description: p.description.to_string(),
                            installed: p.installed,
                        })
                        .collect()
                })
                .unwrap_or_default();

            if !updates.is_empty() {
                if let Some(a) = app_weak.upgrade() {
                    a.set_updates_all_queued(true);
                }
            }

            for pkg in updates {
                let pkg_id = pkg.id.clone();
                let name = pkg.name.clone();
                begin_transaction(
                    pkg_id,
                    name,
                    "update".to_string(),
                    true,
                    false,
                    Some(pkg),
                    store.clone(),
                    proxy_arc.clone(),
                    app_weak.clone(),
                    rt_handle.clone(),
                );
            }
            if let Some(a) = app_weak.upgrade() {
                a.set_available_updates(Default::default());
                a.set_update_count(0);
            }
        });
    }

    {
        let store = tx_store.clone();
        let app_weak = app.as_weak();

        app.on_clear_completed_requested(move || {
            {
                let mut s = store.lock().unwrap();
                s.retain(|tx| tx.status != TxStatus::Completed && tx.status != TxStatus::Failed);
            }
            push_transactions_to_ui(store.clone(), &app_weak);
        });
    }

    {
        let store = tx_store.clone();
        let app_weak = app.as_weak();
        let proxy_arc = proxy_opt.clone();
        let rt_handle = rt.handle().clone();

        app.on_cancel_requested(move |tx_id| {
            let tx_id = tx_id.to_string();
            let proxy = get_proxy(&proxy_arc);
            let store = store.clone();
            let app_weak = app_weak.clone();
            let rt_handle = rt_handle.clone();

            rt_handle.spawn(async move {
                if let Some(p) = proxy {
                    let cancelled = p.cancel_transaction(&tx_id).await.unwrap_or(false);
                    if cancelled {
                        let (saved_pkg, saved_icon) = {
                            let mut s = store.lock().unwrap();
                            let mut saved = None;
                            let mut icon = None;
                            if let Some(tx) = s.iter_mut().find(|t| t.id == tx_id) {
                                tx.status = TxStatus::Failed;
                                tx.error = "Cancelled".to_string();
                                if tx.tx_type == "update" {
                                    saved = tx.saved_pkg.clone();
                                    icon = tx.icon.clone();
                                }
                            }
                            (saved, icon)
                        };
                        push_transactions_to_ui(store.clone(), &app_weak);
                        let _ = app_weak.upgrade_in_event_loop(move |app| {
                            app.set_updates_all_queued(false);
                            if let Some(pkg) = saved_pkg {
                                add_to_available_updates(&app, pkg, saved_icon.as_ref());
                            }
                        });
                    }
                }
            });
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
                let packages: Vec<libarc::Package> = if let Some(p) = &proxy {
                    p.search_category(&cat)
                        .await
                        .ok()
                        .and_then(|json| serde_json::from_str(&json).ok())
                        .unwrap_or_default()
                } else {
                    vec![]
                };

                let raw_pkgs = load_package_icons(packages).await;
                let _ = app_weak2.upgrade_in_event_loop(move |app| {
                    let slint_pkgs: Vec<PackageItem> =
                        raw_pkgs.iter().map(|r| r.to_slint()).collect();
                    app.set_current_view("search".into());
                    app.set_packages(slint_pkgs.as_slice().into());

                    app.set_is_loading(false);
                });
            });

            if let Some(app_ref) = app_weak.upgrade() {
                app_ref.set_current_view("search".into());
                app_ref.set_is_loading(true);
            }
        });
    }

    {
        let app_weak = app.as_weak();
        let proxy_arc = proxy_opt.clone();
        let rt_handle = rt.handle().clone();
        let store = tx_store.clone();

        app.on_refresh_updates_requested(move || {
            let app_weak2 = app_weak.clone();
            let proxy = get_proxy(&proxy_arc);
            let store2 = store.clone();

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

                let raw_pkgs = load_package_icons(updates).await;

                let _ = app_weak2.upgrade_in_event_loop(move |app| {
                    let mut pkgs: Vec<PackageItem> =
                        raw_pkgs.iter().map(|r| r.to_slint()).collect();

                    // Re-add packages whose update transactions are still in flight
                    // but were not returned by the daemon in this refresh.
                    {
                        let s = store2.lock().unwrap();
                        for tx in s.iter() {
                            if tx.tx_type == "update"
                                && (tx.status == TxStatus::Pending
                                    || tx.status == TxStatus::Running)
                            {
                                if let Some(saved) = &tx.saved_pkg {
                                    if !pkgs.iter().any(|p| p.id.as_str() == saved.id.as_str()) {
                                        pkgs.push(PackageItem {
                                            id: saved.id.clone().into(),
                                            name: saved.name.clone().into(),
                                            version: saved.version.clone().into(),
                                            description: saved.description.clone().into(),
                                            installed: saved.installed,
                                            icon: tx
                                                .icon
                                                .as_ref()
                                                .map(|r| r.to_slint_image())
                                                .unwrap_or_default(),
                                        });
                                    }
                                }
                            }
                        }
                    }

                    let count = pkgs.len() as i32;
                    app.set_available_updates(pkgs.as_slice().into());
                    app.set_update_count(count);
                    app.set_updates_loading(false);
                });
            });

            if let Some(app_ref) = app_weak.upgrade() {
                app_ref.set_updates_loading(true);
            }
        });
    }

    {
        let app_weak = app.as_weak();
        let proxy_arc = proxy_opt.clone();
        let rt_handle = rt.handle().clone();
        let store = tx_store.clone();

        app.on_detail_requested(move |id| {
            let app_weak2 = app_weak.clone();
            let proxy = get_proxy(&proxy_arc);
            let store = store.clone();

            rt_handle.spawn(async move {
                load_detail(id, proxy, store, app_weak2).await;
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
        app.on_save_settings(
            move |preferred, ignore_native_pref, auto_updates, concurrent_downloads| {
                let mut s = settings.lock().unwrap();
                s.preferred_provider = if preferred == "Native" {
                    Provider::Distrobox
                } else {
                    Provider::Flatpak
                };
                s.ignore_native_preference = ignore_native_pref;
                s.auto_updates = auto_updates;
                s.concurrent_downloads = (concurrent_downloads as u32).max(1);
                let _ = s.save();
            },
        );
    }

    {
        let app_weak = app.as_weak();
        let proxy_arc = proxy_opt.clone();
        let rt_handle = rt.handle().clone();

        app.on_load_remotes(move || {
            if let Some(app_ref) = app_weak.upgrade() {
                app_ref.set_settings_remotes_loading(true);
            }
            let app_weak2 = app_weak.clone();
            let proxy = get_proxy(&proxy_arc);
            rt_handle.spawn(async move {
                let remotes: Vec<libarc::RemoteInfo> = if let Some(p) = proxy {
                    p.list_remotes()
                        .await
                        .ok()
                        .and_then(|j| serde_json::from_str(&j).ok())
                        .unwrap_or_default()
                } else {
                    vec![]
                };
                let _ = app_weak2.upgrade_in_event_loop(move |app| {
                    let items: Vec<RemoteItem> = remotes
                        .iter()
                        .map(|r| RemoteItem {
                            name: r.name.as_str().into(),
                            url: r.url.as_str().into(),
                            protected: r.protected,
                        })
                        .collect();
                    app.set_settings_remotes(items.as_slice().into());
                    app.set_settings_remotes_loading(false);
                });
            });
        });
    }

    {
        let app_weak = app.as_weak();
        let proxy_arc = proxy_opt.clone();
        let rt_handle = rt.handle().clone();

        app.on_add_remote(move |name, url| {
            let name = name.to_string();
            let url = url.to_string();
            let proxy = get_proxy(&proxy_arc);
            let app_weak2 = app_weak.clone();
            rt_handle.spawn(async move {
                let ok = if let Some(p) = proxy {
                    p.add_remote(&name, &url).await.unwrap_or(false)
                } else {
                    false
                };
                let _ = app_weak2.upgrade_in_event_loop(move |app| {
                    if ok {
                        app.invoke_load_remotes();
                    } else {
                    }
                });
            });
        });
    }

    {
        let app_weak = app.as_weak();
        let proxy_arc = proxy_opt.clone();
        let rt_handle = rt.handle().clone();

        app.on_remove_remote(move |name| {
            let name = name.to_string();
            let proxy = get_proxy(&proxy_arc);
            let app_weak2 = app_weak.clone();
            rt_handle.spawn(async move {
                let ok = if let Some(p) = proxy {
                    p.remove_remote(&name).await.unwrap_or(false)
                } else {
                    false
                };
                let _ = app_weak2.upgrade_in_event_loop(move |app| {
                    if ok {
                        app.invoke_load_remotes();
                    } else {
                    }
                });
            });
        });
    }

    {
        let app_weak = app.as_weak();
        let proxy_arc = proxy_opt.clone();
        let rt_handle = rt.handle().clone();
        let store = tx_store.clone();
        let manage_extensions = std::env::args().any(|a| a == "--manage-extensions");
        let initial_app = std::env::args()
            .find(|a| a.starts_with("appstream://") || a.starts_with("appstream:"))
            .map(|a| {
                a.trim_start_matches("appstream://")
                    .trim_start_matches("appstream:")
                    .trim_start_matches("//")
                    .trim_start_matches('/')
                    .to_string()
            });
        let flatpakref_url = std::env::args()
            .find(|a| a.starts_with("flatpak+https://") || a.starts_with("flatpak+http://"))
            .map(|a| a.trim_start_matches("flatpak+").to_string());
        let flatpakrepo_file = std::env::args().skip(1).find(|a| is_flatpakrepo(a));

        if manage_extensions || initial_app.as_deref() == Some("") {
            if let Some(app_ref) = app_weak.upgrade() {
                app_ref.set_current_view("installed".into());
                app_ref.invoke_refresh_requested();
            }
        } else if let Some(app_id) = initial_app.filter(|s| !s.is_empty()) {
            if let Some(app_ref) = app_weak.upgrade() {
                app_ref.invoke_detail_requested(app_id.into());
            }
        } else if let Some(url) = flatpakref_url {
            let app_weak2 = app_weak.clone();
            let url2 = url.clone();
            rt_handle.spawn(async move {
                let content = reqwest::get(&url2)
                    .await
                    .ok()
                    .and_then(|r| {
                        let rt = tokio::runtime::Handle::current();
                        rt.block_on(r.text()).ok()
                    })
                    .unwrap_or_default();
                let (title, app_id, repo_url) = parse_flatpakref(&content);
                let _ = app_weak2.upgrade_in_event_loop(move |app| {
                    app.set_install_flatpakref_name(title.into());
                    app.set_install_flatpakref_app_id(app_id.into());
                    app.set_install_flatpakref_repo_url(repo_url.into());
                    app.set_current_view("install-flatpakref".into());
                });
            });

            {
                let app_weak3 = app_weak.clone();
                let proxy_arc2 = proxy_arc.clone();
                let rt_handle2 = rt_handle.clone();
                let store2 = store.clone();
                app.on_install_flatpakref_confirmed(move || {
                    let url3 = url.clone();
                    let title = app_weak3
                        .upgrade()
                        .map(|a| a.get_install_flatpakref_name().to_string())
                        .unwrap_or_else(|| url.clone());

                    begin_transaction(
                        url3,
                        title,
                        "flatpakref".to_string(),
                        true,
                        false,
                        None,
                        store2.clone(),
                        proxy_arc2.clone(),
                        app_weak3.clone(),
                        rt_handle2.clone(),
                    );

                    if let Some(app_ref) = app_weak3.upgrade() {
                        app_ref.set_current_view("downloads".into());
                    }
                });
            }

            {
                let app_weak3 = app_weak.clone();
                app.on_install_flatpakref_cancelled(move || {
                    if let Some(app_ref) = app_weak3.upgrade() {
                        app_ref.set_current_view("home".into());
                    }
                });
            }
        } else if let Some(repo_path) = flatpakrepo_file {
            let content = std::fs::read_to_string(&repo_path).unwrap_or_default();
            let (title, url) = parse_flatpakrepo(&content);
            if let Some(app_ref) = app_weak.upgrade() {
                app_ref.set_add_repo_title(title.into());
                app_ref.set_add_repo_url(url.into());
                app_ref.set_current_view("add-repo".into());
            }

            {
                let app_weak3 = app_weak.clone();
                let proxy_arc2 = proxy_arc.clone();
                let rt_handle2 = rt_handle.clone();
                let content2 = content.clone();
                app.on_add_repo_confirmed(move || {
                    let proxy = get_proxy(&proxy_arc2);
                    let app_weak4 = app_weak3.clone();
                    let c = content2.clone();
                    rt_handle2.spawn(async move {
                        let ok = if let Some(p) = proxy {
                            p.add_flatpakrepo(&c).await.unwrap_or(false)
                        } else {
                            false
                        };
                        let _ = app_weak4.upgrade_in_event_loop(move |app| {
                            if ok {
                            } else {
                            }
                            app.set_current_view("home".into());
                        });
                    });
                });
            }

            {
                let app_weak3 = app_weak.clone();
                app.on_add_repo_cancelled(move || {
                    if let Some(app_ref) = app_weak3.upgrade() {
                        app_ref.set_current_view("home".into());
                    }
                });
            }
        }
    }

    {
        let app_weak = app.as_weak();
        let rt_handle = rt.handle().clone();
        let proxy_arc = proxy_opt.clone();
        let store = tx_store.clone();

        let pkg_file = std::env::args().skip(1).find(|a| is_pkg_file(a));

        if let Some(file_path) = pkg_file {
            let file_name = std::path::Path::new(&file_path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&file_path)
                .to_string();
            let pkg_name = pkg_name_from_filename(&file_name);
            let file_is_appimage = is_appimage(&file_path);
            let file_is_bundle = is_flatpak_bundle(&file_path);

            if let Some(app_ref) = app_weak.upgrade() {
                app_ref.set_install_file_path(file_path.clone().into());
                app_ref.set_install_file_name(file_name.clone().into());
                app_ref.set_install_file_has_flatpak(false);
                // bundles and appimages skip the Flatpak alternative search
                app_ref.set_install_file_flatpak_searched(file_is_appimage || file_is_bundle);
                app_ref.set_install_file_is_appimage(file_is_appimage);
                app_ref.set_install_file_is_flatpak_bundle(file_is_bundle);
                app_ref.set_current_view("install-file".into());
            }

            // search for a Flatpak alternative (for non-AppImage, non-bundle files)
            if !file_is_appimage && !file_is_bundle {
                let app_weak2 = app_weak.clone();
                let proxy_search = get_proxy(&proxy_arc);
                rt_handle.spawn(async move {
                    let flatpak_found = if let Some(p) = proxy_search {
                        if let Ok(results) = p.search(&pkg_name).await {
                            if let Ok(pkgs) = serde_json::from_str::<Vec<libarc::Package>>(&results)
                            {
                                pkgs.iter()
                                    .find(|p| p.provider == Provider::Flatpak)
                                    .map(|first| (first.id.clone(), first.name.clone()))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    let _ = app_weak2.upgrade_in_event_loop(move |app| {
                        if let Some((id, name)) = flatpak_found {
                            app.set_install_file_flatpak_id(id.into());
                            app.set_install_file_flatpak_name(name.into());
                            app.set_install_file_has_flatpak(true);
                        }
                        app.set_install_file_flatpak_searched(true);
                    });
                });
            }

            {
                let app_weak3 = app_weak.clone();
                let proxy_arc2 = proxy_arc.clone();
                let rt_handle2 = rt_handle.clone();
                let store2 = store.clone();
                let fp = file_path.clone();
                let fn_ = file_name.clone();
                app.on_install_file_distrobox_requested(move || {
                    begin_transaction(
                        fp.clone(),
                        fn_.clone(),
                        "install".to_string(),
                        true,
                        false,
                        None,
                        store2.clone(),
                        proxy_arc2.clone(),
                        app_weak3.clone(),
                        rt_handle2.clone(),
                    );
                    if let Some(app_ref) = app_weak3.upgrade() {
                        app_ref.set_current_view("downloads".into());
                    }
                });
            }

            {
                let app_weak3 = app_weak.clone();
                let proxy_arc2 = proxy_arc.clone();
                let rt_handle2 = rt_handle.clone();
                let store2 = store.clone();
                let fp = file_path.clone();
                let fn_ = file_name.clone();
                app.on_install_file_appimage_requested(move || {
                    begin_transaction(
                        fp.clone(),
                        fn_.clone(),
                        "install".to_string(),
                        true,
                        false,
                        None,
                        store2.clone(),
                        proxy_arc2.clone(),
                        app_weak3.clone(),
                        rt_handle2.clone(),
                    );
                    if let Some(app_ref) = app_weak3.upgrade() {
                        app_ref.set_current_view("downloads".into());
                    }
                });
            }

            {
                let app_weak3 = app_weak.clone();
                let proxy_arc2 = proxy_arc.clone();
                let rt_handle2 = rt_handle.clone();
                let store2 = store.clone();
                let fp = file_path.clone();
                let fn_ = file_name.clone();
                app.on_install_file_bundle_requested(move || {
                    begin_transaction(
                        fp.clone(),
                        fn_.clone(),
                        "bundle".to_string(),
                        true,
                        false,
                        None,
                        store2.clone(),
                        proxy_arc2.clone(),
                        app_weak3.clone(),
                        rt_handle2.clone(),
                    );
                    if let Some(app_ref) = app_weak3.upgrade() {
                        app_ref.set_current_view("downloads".into());
                    }
                });
            }

            {
                let app_weak3 = app_weak.clone();
                app.on_install_file_flatpak_requested(move || {
                    if let Some(app_ref) = app_weak3.upgrade() {
                        let flatpak_id = app_ref.get_install_file_flatpak_id().to_string();
                        app_ref.invoke_detail_requested(flatpak_id.into());
                    }
                });
            }

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

    app.on_open_url_requested(|url| {
        let _ = std::process::Command::new("xdg-open")
            .arg(url.as_str())
            .spawn();
    });

    app.run()?;
    Ok(())
}
