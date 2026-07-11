use tr::tr;

use slint::Model;
use libarc::Provider;

use crate::helpers::{
    self, get_display_name, get_proxy, is_appimage, is_flatpak_bundle, is_flatpakref,
    is_flatpakrepo, is_pkg_file, parse_flatpakref, parse_flatpakrepo, pkg_name_from_filename,
};
use crate::nav::NavEntry;
use crate::packages::{load_detail, load_package_icons, refresh_home_installed};
use crate::transactions::{
    add_to_available_updates, begin_transaction, push_transactions_to_ui,
    remove_from_available_updates, SavedPkgData, TxStatus,
};
use crate::{launcher, AppWindow, PackageItem, RemoteItem};

pub struct AppContext {
    pub app: slint::Weak<AppWindow>,
    pub rt: tokio::runtime::Handle,
    pub proxy_opt: std::sync::Arc<std::sync::Mutex<Option<libarc::ArcDaemonProxy<'static>>>>,
    pub settings: std::sync::Arc<std::sync::Mutex<libarc::Settings>>,
    pub tx_store: crate::transactions::TxStore,
    pub pending_runtime_ids: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    pub installed_cache: std::sync::Arc<std::sync::Mutex<Option<Vec<crate::packages::RawPackage>>>>,
    pub nav_history: std::sync::Arc<std::sync::Mutex<crate::nav::NavHistory>>,
    pub runtime_icon: Option<crate::icons::RawIcon>,
}

pub fn register_all(app: &AppWindow, ctx: &AppContext) {
    {
        let app_weak = ctx.app.clone();
        let proxy = ctx.proxy_opt.lock().unwrap().clone();
        let rt_handle = ctx.rt.clone();
        let history = ctx.nav_history.clone();

        app.on_home_clicked(move || {
            {
                let mut h = history.lock().unwrap();
                h.reset();
            }
            if let Some(app_ref) = app_weak.upgrade() {
                app_ref.set_can_go_back(false);
                app_ref.set_can_go_forward(false);
                app_ref.set_current_view("home".into());
            }
            let app_weak2 = app_weak.clone();
            let proxy2 = proxy.clone();
            let rt_handle2 = rt_handle.clone();
            rt_handle2.spawn(async move {
                refresh_home_installed(app_weak2, proxy2).await;
            });
        });
    }

    {
        let app_weak = ctx.app.clone();
        let proxy = ctx.proxy_opt.lock().unwrap().clone();
        let rt_handle = ctx.rt.clone();

        app.on_back_to_home(move || {
            let app_weak = app_weak.clone();
            let proxy = proxy.clone();
            let rt_handle = rt_handle.clone();
            rt_handle.spawn(async move {
                refresh_home_installed(app_weak, proxy).await;
            });
        });
    }

    {
        let app_weak = ctx.app.clone();
        let proxy_arc = ctx.proxy_opt.clone();
        let rt_handle = ctx.rt.clone();
        let settings = ctx.settings.clone();
        let history = ctx.nav_history.clone();

        app.on_search_requested(move |query| {
            let query_str = query.to_string();
            if query_str.trim().is_empty() {
                return;
            }

            let (can_back, can_fwd) = {
                let mut h = history.lock().unwrap();
                h.push(NavEntry {
                    view: "search".to_string(),
                    search_text: query_str.clone(),
                    ..Default::default()
                });
                (h.can_go_back(), h.can_go_forward())
            };

            if let Some(app_ref) = app_weak.upgrade() {
                app_ref.set_can_go_back(can_back);
                app_ref.set_can_go_forward(can_fwd);
                app_ref.set_current_view("search".into());
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

                let all_pkgs = daemon_pkgs;

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
        let app_weak = ctx.app.clone();

        let installed_cache = ctx.installed_cache.clone();

        app.on_refresh_requested(move || {
            let cached = {
                let guard = installed_cache.lock().unwrap();
                guard.clone()
            };
            if let Some(raw) = cached {
                let pkgs: Vec<PackageItem> = raw.iter().map(|r| r.to_slint()).collect();
                if let Some(app_ref) = app_weak.upgrade() {
                    app_ref.set_packages(pkgs.as_slice().into());
                    app_ref.set_is_loading(false);
                }
            } else {
                // Cache is still being built. Show loading screen and wait.
                if let Some(app_ref) = app_weak.upgrade() {
                    app_ref.set_is_loading(true);
                }
            }
        });
    }

    {
        let app_weak = ctx.app.clone();
        let proxy_arc = ctx.proxy_opt.clone();
        let rt_handle = ctx.rt.clone();
        let store = ctx.tx_store.clone();

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

            // Detail screen already checks EULA before emitting install-requested,
            // so skip the async EULA fetch when coming from there.
            if in_detail {
                begin_transaction(
                    pkg_id_str,
                    String::new(),
                    display_name,
                    None,
                    "install".to_string(),
                    true,
                    true,
                    false,
                    None,
                    store.clone(),
                    proxy_arc.clone(),
                    app_weak.clone(),
                    rt_handle.clone(),
                );
                return;
            }

            // For package-list installs: fetch metadata to check for EULA first.
            let proxy = get_proxy(&proxy_arc);
            let store_c = store.clone();
            let proxy_arc_c = proxy_arc.clone();
            let rt_handle_c = rt_handle.clone();
            let app_weak_c = app_weak.clone();
            rt_handle.spawn(async move {
                let eula_url: String = if let Some(ref p) = proxy {
                    p.get_app_metadata(&pkg_id_str)
                        .await
                        .ok()
                        .and_then(|j| serde_json::from_str::<serde_json::Value>(&j).ok())
                        .and_then(|v| {
                            v.get("eula_url")
                                .and_then(|u| u.as_str())
                                .map(|s| s.to_string())
                        })
                        .filter(|s| !s.is_empty())
                        .unwrap_or_default()
                } else {
                    String::new()
                };

                if !eula_url.is_empty() {
                    let name = display_name.clone();
                    let id = pkg_id_str.clone();
                    let _ = app_weak_c.upgrade_in_event_loop(move |app| {
                        // Pick up the icon already loaded during the search.
                        let pkg_model = app.get_packages();
                        let pkg_icon = (0..pkg_model.row_count())
                            .filter_map(|i| pkg_model.row_data(i))
                            .find(|p| p.id.as_str() == id.as_str())
                            .map(|p| p.icon);

                        let mut detail = app.get_detail_app();
                        detail.flatpak_id = id.into();
                        detail.name = name.into();
                        detail.eula_url = eula_url.into();
                        if let Some(icon) = pkg_icon {
                            detail.icon = icon;
                        }
                        app.set_detail_app(detail);
                        app.set_eula_source_view(app.get_current_view());
                        app.set_current_view("eula".into());
                    });
                } else {
                    let icon_url = if pkg_id_str.starts_with("pwa:") {
                        if let Some(ref p) = proxy {
                            p.get_app_info(&pkg_id_str)
                                .await
                                .ok()
                                .and_then(|j| serde_json::from_str::<libarc::Package>(&j).ok())
                                .and_then(|pkg| pkg.icon_url)
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    begin_transaction(
                        pkg_id_str,
                        String::new(),
                        display_name,
                        icon_url,
                        "install".to_string(),
                        true,
                        false,
                        false,
                        None,
                        store_c,
                        proxy_arc_c,
                        app_weak_c,
                        rt_handle_c,
                    );
                }
            });
        });
    }

    {
        let app_weak = ctx.app.clone();
        let proxy_arc = ctx.proxy_opt.clone();
        let rt_handle = ctx.rt.clone();
        let store = ctx.tx_store.clone();

        app.on_install_extension_requested(move |pkg_id| {
            let pkg_id_str = pkg_id.to_string();
            let parent_id = app_weak
                .upgrade()
                .map(|a| a.get_detail_app().flatpak_id.to_string())
                .unwrap_or_default();
            let display_name = app_weak
                .upgrade()
                .map(|a| get_display_name(&a, &pkg_id_str))
                .unwrap_or_else(|| pkg_id_str.clone());

            begin_transaction(
                pkg_id_str,
                parent_id,
                display_name,
                None,
                "install".to_string(),
                true,
                false, // don't reload the detail page
                true,  // re-fetch extensions
                None,
                store.clone(),
                proxy_arc.clone(),
                app_weak.clone(),
                rt_handle.clone(),
            );
        });
    }

    {
        let app_weak = ctx.app.clone();
        let proxy_arc = ctx.proxy_opt.clone();
        let rt_handle = ctx.rt.clone();
        let store = ctx.tx_store.clone();

        app.on_remove_extension_requested(move |pkg_id| {
            let pkg_id_str = pkg_id.to_string();
            let parent_id = app_weak
                .upgrade()
                .map(|a| a.get_detail_app().flatpak_id.to_string())
                .unwrap_or_default();
            let display_name = app_weak
                .upgrade()
                .map(|a| get_display_name(&a, &pkg_id_str))
                .unwrap_or_else(|| pkg_id_str.clone());

            begin_transaction(
                pkg_id_str,
                parent_id,
                display_name,
                None,
                "remove".to_string(),
                false,
                false, // don't reload the detail page
                true,  // re-fetch extensions
                None,
                store.clone(),
                proxy_arc.clone(),
                app_weak.clone(),
                rt_handle.clone(),
            );
        });
    }

    {
        let app_weak = ctx.app.clone();

        app.on_remove_requested(move |pkg_id| {
            let pkg_id_str = pkg_id.to_string();
            let Some(app) = app_weak.upgrade() else {
                return;
            };
            let display_name = get_display_name(&app, &pkg_id_str);
            let icon = {
                let pkgs = app.get_packages();
                let count = pkgs.row_count();
                (0..count)
                    .filter_map(|i| pkgs.row_data(i))
                    .find(|p| p.id.as_str() == pkg_id_str)
                    .map(|p| p.icon)
                    .unwrap_or_else(|| app.get_detail_app().icon)
            };
            app.set_remove_dialog_pkg_id(pkg_id.clone());
            app.set_remove_dialog_name(display_name.into());
            app.set_remove_dialog_icon(icon);
            app.set_remove_dialog_delete_data(true);
            app.set_show_remove_dialog(true);
        });
    }

    {
        let app_weak = ctx.app.clone();
        let proxy_arc = ctx.proxy_opt.clone();
        let rt_handle = ctx.rt.clone();
        let store = ctx.tx_store.clone();

        app.on_remove_confirmed(move |pkg_id, delete_data| {
            let pkg_id_str = pkg_id.to_string();
            let in_detail = app_weak
                .upgrade()
                .map(|a| a.get_current_view() == "detail")
                .unwrap_or(false);
            let display_name = app_weak
                .upgrade()
                .map(|a| get_display_name(&a, &pkg_id_str))
                .unwrap_or_else(|| pkg_id_str.clone());
            let tx_type = if delete_data {
                "remove_with_data"
            } else {
                "remove"
            }
            .to_string();

            begin_transaction(
                pkg_id_str,
                String::new(),
                display_name,
                None,
                tx_type,
                false,
                in_detail,
                false,
                None,
                store.clone(),
                proxy_arc.clone(),
                app_weak.clone(),
                rt_handle.clone(),
            );
        });
    }

    {
        let app_weak = ctx.app.clone();
        let proxy_arc = ctx.proxy_opt.clone();
        let rt_handle = ctx.rt.clone();
        let store = ctx.tx_store.clone();
        let runtime_ids_upd = ctx.pending_runtime_ids.clone();

        app.on_update_requested(move |pkg_id| {
            let pkg_id_str = pkg_id.to_string();

            if pkg_id_str == "arc-runtime-libraries" {
                let ids: Vec<String> = runtime_ids_upd.lock().unwrap().drain(..).collect();
                let label = tr!("Arc Runtime Libraries").to_string();
                for runtime_id in ids {
                    begin_transaction(
                        runtime_id,
                        "arc-runtime-libraries".to_string(),
                        label.clone(),
                        None,
                        "update".to_string(),
                        true,
                        false,
                        false,
                        None,
                        store.clone(),
                        proxy_arc.clone(),
                        app_weak.clone(),
                        rt_handle.clone(),
                    );
                }
                if let Some(a) = app_weak.upgrade() {
                    remove_from_available_updates(&a, "arc-runtime-libraries");
                }
                return;
            }

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
                String::new(),
                display_name,
                None,
                "update".to_string(),
                true,
                in_detail,
                false,
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
        let app_weak = ctx.app.clone();
        let proxy_arc = ctx.proxy_opt.clone();
        let rt_handle = ctx.rt.clone();
        let store = ctx.tx_store.clone();
        let runtime_ids_all = ctx.pending_runtime_ids.clone();

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

            let label = tr!("Arc Runtime Libraries").to_string();
            for pkg in updates {
                if pkg.id == "arc-runtime-libraries" {
                    let ids: Vec<String> = runtime_ids_all.lock().unwrap().drain(..).collect();
                    for runtime_id in ids {
                        begin_transaction(
                            runtime_id,
                            "arc-runtime-libraries".to_string(),
                            label.clone(),
                            None,
                            "update".to_string(),
                            true,
                            false,
                            false,
                            None,
                            store.clone(),
                            proxy_arc.clone(),
                            app_weak.clone(),
                            rt_handle.clone(),
                        );
                    }
                } else {
                    let pkg_id = pkg.id.clone();
                    let name = pkg.name.clone();
                    begin_transaction(
                        pkg_id,
                        String::new(),
                        name,
                        None,
                        "update".to_string(),
                        true,
                        false,
                        false,
                        Some(pkg),
                        store.clone(),
                        proxy_arc.clone(),
                        app_weak.clone(),
                        rt_handle.clone(),
                    );
                }
            }
            if let Some(a) = app_weak.upgrade() {
                a.set_available_updates(Default::default());
                a.set_update_count(0);
                launcher::update_badge(0);
            }
        });
    }

    {
        let store = ctx.tx_store.clone();
        let app_weak = ctx.app.clone();

        app.on_clear_completed_requested(move || {
            {
                let mut s = store.lock().unwrap();
                s.retain(|tx| tx.status != TxStatus::Completed && tx.status != TxStatus::Failed);
            }
            push_transactions_to_ui(store.clone(), &app_weak);
        });
    }

    {
        let store = ctx.tx_store.clone();
        let app_weak = ctx.app.clone();
        let proxy_arc = ctx.proxy_opt.clone();
        let rt_handle = ctx.rt.clone();

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
        let app_weak = ctx.app.clone();
        let proxy_arc = ctx.proxy_opt.clone();
        let rt_handle = ctx.rt.clone();

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
        let app_weak = ctx.app.clone();
        let proxy_arc = ctx.proxy_opt.clone();
        let rt_handle = ctx.rt.clone();
        let store = ctx.tx_store.clone();
        let runtime_ids_refresh = ctx.pending_runtime_ids.clone();
        let runtime_icon_refresh = ctx.runtime_icon.clone();

        app.on_refresh_updates_requested(move || {
            let app_weak2 = app_weak.clone();
            let proxy = get_proxy(&proxy_arc);
            let store2 = store.clone();
            let runtime_ids2 = runtime_ids_refresh.clone();
            let runtime_icon2 = runtime_icon_refresh.clone();

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

                let (runtime_updates, app_updates): (Vec<_>, Vec<_>) =
                    updates.into_iter().partition(|p| p.is_runtime);
                let raw_pkgs = load_package_icons(app_updates).await;

                let _ = app_weak2.upgrade_in_event_loop(move |app| {
                    let mut pkgs: Vec<PackageItem> =
                        raw_pkgs.iter().map(|r| r.to_slint()).collect();

                    if !runtime_updates.is_empty() {
                        let has_inflight_runtime = {
                            let s = store2.lock().unwrap();
                            s.iter().any(|tx| {
                                tx.tx_type == "update"
                                    && (tx.status == TxStatus::Pending
                                        || tx.status == TxStatus::Running)
                                    && tx.parent_id == "arc-runtime-libraries"
                            })
                        };
                        if !has_inflight_runtime {
                            *runtime_ids2.lock().unwrap() =
                                runtime_updates.iter().map(|p| p.id.clone()).collect();
                            pkgs.push(PackageItem {
                                id: "arc-runtime-libraries".into(),
                                name: tr!("Arc Runtime Libraries").into(),
                                version: Default::default(),
                                description: Default::default(),
                                installed: true,
                                icon: runtime_icon2
                                    .as_ref()
                                    .map(|r| r.to_slint_image())
                                    .unwrap_or_default(),
                                busy: false,
                                progress: 0.0,
                                transaction_id: Default::default(),
                            });
                        }
                    } else {
                        runtime_ids2.lock().unwrap().clear();
                    }

                    // Re-add app packages whose update transactions are still in flight
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
                                            busy: false,
                                            progress: 0.0,
                                            transaction_id: Default::default(),
                                        });
                                    }
                                }
                            }
                        }
                    }

                    let count = pkgs.len() as i32;
                    app.set_available_updates(pkgs.as_slice().into());
                    app.set_update_count(count);
                    launcher::update_badge(count);
                    app.set_updates_loading(false);
                });
            });

            if let Some(app_ref) = app_weak.upgrade() {
                app_ref.set_updates_loading(true);
            }
        });
    }

    {
        let app_weak = ctx.app.clone();
        let proxy_arc = ctx.proxy_opt.clone();
        let rt_handle = ctx.rt.clone();
        let store = ctx.tx_store.clone();
        let history = ctx.nav_history.clone();

        app.on_detail_requested(move |id| {
            let id_str = id.to_string();
            let app_weak2 = app_weak.clone();
            let proxy = get_proxy(&proxy_arc);
            let store = store.clone();

            rt_handle.spawn(async move {
                load_detail(id, proxy, store, app_weak2).await;
            });

            let (can_back, can_fwd) = {
                let mut h = history.lock().unwrap();
                h.push(NavEntry {
                    view: "detail".to_string(),
                    detail_id: id_str,
                    ..Default::default()
                });
                (h.can_go_back(), h.can_go_forward())
            };

            if let Some(app_ref) = app_weak.upgrade() {
                app_ref.set_can_go_back(can_back);
                app_ref.set_can_go_forward(can_fwd);
                app_ref.set_detail_screenshots([].as_slice().into());
                app_ref.set_detail_screenshots_loading(false);
                app_ref.set_detail_description_blocks([].as_slice().into());
                app_ref.set_detail_loading(true);
                app_ref.set_current_view("detail".into());
            }
        });
    }

    {
        let app_weak = ctx.app.clone();
        let proxy_arc = ctx.proxy_opt.clone();
        let rt_handle = ctx.rt.clone();

        app.on_refresh_extensions_requested(move || {
            let flatpak_id = app_weak
                .upgrade()
                .map(|a| a.get_detail_app().flatpak_id.to_string())
                .unwrap_or_default();
            if flatpak_id.is_empty() {
                return;
            }
            let proxy = get_proxy(&proxy_arc);
            let app_weak2 = app_weak.clone();
            rt_handle.spawn(async move {
                let extensions: Vec<libarc::Package> = if let Some(p) = &proxy {
                    p.list_extensions(&flatpak_id)
                        .await
                        .ok()
                        .and_then(|j| serde_json::from_str(&j).ok())
                        .unwrap_or_default()
                } else {
                    vec![]
                };
                let _ = app_weak2.upgrade_in_event_loop(move |app| {
                    let items: Vec<crate::ExtensionItem> = extensions
                        .iter()
                        .map(|e| crate::ExtensionItem {
                            id: slint::SharedString::from(e.id.as_str()),
                            name: slint::SharedString::from(e.name.as_str()),
                            installed: e.installed,
                        })
                        .collect();
                    app.set_detail_extensions(items.as_slice().into());
                });
            });
        });
    }

    {
        let app_weak = ctx.app.clone();
        let proxy_arc = ctx.proxy_opt.clone();
        let rt_handle = ctx.rt.clone();
        let store = ctx.tx_store.clone();

        app.on_eula_confirmed(move || {
            let (pkg_id, display_name, source_view) = app_weak
                .upgrade()
                .map(|a| {
                    let d = a.get_detail_app();
                    let sv = a.get_eula_source_view().to_string();
                    (d.flatpak_id.to_string(), d.name.to_string(), sv)
                })
                .unwrap_or_default();
            if pkg_id.is_empty() {
                return;
            }
            let in_detail = source_view == "detail";
            if let Some(app_ref) = app_weak.upgrade() {
                app_ref.set_current_view(source_view.into());
            }
            begin_transaction(
                pkg_id,
                String::new(),
                display_name,
                None,
                "install".to_string(),
                true,
                in_detail,
                false,
                None,
                store.clone(),
                proxy_arc.clone(),
                app_weak.clone(),
                rt_handle.clone(),
            );
        });
    }

    {
        let app_weak = ctx.app.clone();

        app.on_eula_cancelled(move || {
            if let Some(app_ref) = app_weak.upgrade() {
                let source = app_ref.get_eula_source_view().to_string();
                app_ref.set_current_view(source.into());
            }
        });
    }

    {
        let settings = ctx.settings.clone();
        app.on_save_settings(
            move |preferred,
                  ignore_native_pref,
                  auto_updates,
                  concurrent_downloads,
                  show_security_warnings| {
                let mut s = settings.lock().unwrap();
                s.preferred_provider = if preferred == "Native" {
                    Provider::Distrobox
                } else {
                    Provider::Flatpak
                };
                s.ignore_native_preference = ignore_native_pref;
                s.auto_updates = auto_updates;
                s.concurrent_downloads = (concurrent_downloads as u32).max(1);
                s.show_security_warnings = show_security_warnings;
                let _ = s.save();
            },
        );
    }

    {
        let app_weak = ctx.app.clone();
        let proxy_arc = ctx.proxy_opt.clone();
        let rt_handle = ctx.rt.clone();

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
        let app_weak = ctx.app.clone();
        let proxy_arc = ctx.proxy_opt.clone();
        let rt_handle = ctx.rt.clone();

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
        let app_weak = ctx.app.clone();
        let proxy_arc = ctx.proxy_opt.clone();
        let rt_handle = ctx.rt.clone();

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
        let rt_handle = ctx.rt.clone();
        app.on_force_update_requested(move || {
            rt_handle.spawn(async move {
                let _ = tokio::process::Command::new("flatpak")
                    .args(["update", "-y"])
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn();
            });
        });
    }

    {
        let rt_handle = ctx.rt.clone();
        app.on_restart_daemon_requested(move || {
            rt_handle.spawn(async move {
                let _ = tokio::process::Command::new("pkill")
                    .args(["-x", "arc-daemon"])
                    .status()
                    .await;

                tokio::time::sleep(std::time::Duration::from_millis(800)).await;

                // Spawn detached via setsid so it is NOT a child of this process.
                // setsid creates a new session; --fork ensures the original setsid
                // process exits immediately, leaving the daemon under init/systemd.
                let _ = std::process::Command::new("setsid")
                    .args(["--fork", "/usr/bin/arc-daemon"])
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn();
            });
        });
    }

    {
        let app_weak = ctx.app.clone();
        let proxy_arc = ctx.proxy_opt.clone();
        let rt_handle = ctx.rt.clone();
        let store = ctx.tx_store.clone();
        let manage_extensions = std::env::args().any(|a| a == "--manage-extensions");
        let initial_app = std::env::args()
            .find(|a| a.starts_with("appstream://") || a.starts_with("appstream:"))
            .map(|a| {
                let id = a
                    .trim_start_matches("appstream://")
                    .trim_start_matches("appstream:")
                    .trim_start_matches("//")
                    .trim_start_matches('/');
                id.strip_suffix(".desktop").unwrap_or(id).to_string()
            });
        let flatpakref_url = std::env::args()
            .find(|a| a.starts_with("flatpak+https://") || a.starts_with("flatpak+http://"))
            .map(|a| a.trim_start_matches("flatpak+").to_string());
        let flatpakref_file = std::env::args()
            .skip(1)
            .map(|a| a.strip_prefix("file://").map(str::to_string).unwrap_or(a))
            .find(|a| is_flatpakref(a));
        let flatpakrepo_file = std::env::args()
            .skip(1)
            .map(|a| a.strip_prefix("file://").map(str::to_string).unwrap_or(a))
            .find(|a| is_flatpakrepo(a));

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
            let flathub_app_id = url
                .strip_prefix("https://dl.flathub.org/repo/appstream/")
                .or_else(|| url.strip_prefix("http://dl.flathub.org/repo/appstream/"))
                .and_then(|s| s.strip_suffix(".flatpakref"))
                .map(str::to_string);

            if let Some(app_id) = flathub_app_id {
                if let Some(app_ref) = app_weak.upgrade() {
                    app_ref.invoke_detail_requested(app_id.into());
                }
            } else {
                let app_weak2 = app_weak.clone();
                let url2 = url.clone();
                rt_handle.spawn(async move {
                    let content = match reqwest::get(&url2).await {
                        Ok(r) => r.text().await.unwrap_or_default(),
                        Err(_) => String::new(),
                    };
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
                            String::new(),
                            title,
                            None,
                            "flatpakref".to_string(),
                            true,
                            false,
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
            } // end non-flathub flatpakref branch
        } else if let Some(file_path) = flatpakref_file {
            let content = std::fs::read_to_string(&file_path).unwrap_or_default();
            let (title, app_id, repo_url) = parse_flatpakref(&content);
            let is_flathub =
                repo_url.contains("dl.flathub.org") || repo_url.contains("flathub.org");

            if is_flathub {
                if let Some(app_ref) = app_weak.upgrade() {
                    app_ref.invoke_detail_requested(app_id.into());
                }
            } else {
                if let Some(app_ref) = app_weak.upgrade() {
                    app_ref.set_install_flatpakref_name(title.into());
                    app_ref.set_install_flatpakref_app_id(app_id.into());
                    app_ref.set_install_flatpakref_repo_url(repo_url.into());
                    app_ref.set_current_view("install-flatpakref".into());
                }

                {
                    let app_weak3 = app_weak.clone();
                    let proxy_arc2 = proxy_arc.clone();
                    let rt_handle2 = rt_handle.clone();
                    let store2 = store.clone();
                    let file_url = format!("file://{}", file_path);
                    app.on_install_flatpakref_confirmed(move || {
                        let url3 = file_url.clone();
                        let title = app_weak3
                            .upgrade()
                            .map(|a| a.get_install_flatpakref_name().to_string())
                            .unwrap_or_default();

                        begin_transaction(
                            url3,
                            String::new(),
                            title,
                            None,
                            "flatpakref".to_string(),
                            true,
                            false,
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
            } // end non-flathub flatpakref file branch
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
                        if let Some(p) = proxy {
                            let _ = p.add_flatpakrepo(&c).await;
                        };
                        let _ = app_weak4.upgrade_in_event_loop(move |app| {
                            app.set_current_view("settings".into());
                        });
                    });
                });
            }

            {
                let app_weak3 = app_weak.clone();
                app.on_add_repo_cancelled(move || {
                    if let Some(app_ref) = app_weak3.upgrade() {
                        app_ref.set_current_view("settings".into());
                    }
                });
            }
        }
    }

    {
        let app_weak = ctx.app.clone();
        let rt_handle = ctx.rt.clone();
        let proxy_arc = ctx.proxy_opt.clone();
        let store = ctx.tx_store.clone();

        let pkg_file = std::env::args()
            .skip(1)
            .map(|a| a.strip_prefix("file://").map(str::to_string).unwrap_or(a))
            .find(|a| is_pkg_file(a));

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
                let pkg_name_for_search = pkg_name.clone();
                rt_handle.spawn(async move {
                    let flatpak_found = if let Some(p) = proxy_search {
                        if let Ok(results) = p.search(&pkg_name_for_search).await {
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
                let pn_distrobox = pkg_name.clone();
                app.on_install_file_distrobox_requested(move || {
                    begin_transaction(
                        fp.clone(),
                        String::new(),
                        pn_distrobox.clone(),
                        None,
                        "install".to_string(),
                        true,
                        false,
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
                let pn = pkg_name.clone();
                app.on_install_file_appimage_requested(move || {
                    begin_transaction(
                        fp.clone(),
                        String::new(),
                        pn.clone(),
                        None,
                        "install".to_string(),
                        true,
                        false,
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
                        String::new(),
                        fn_.clone(),
                        None,
                        "bundle".to_string(),
                        true,
                        false,
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

    // Nav: story
    {
        let app_weak = ctx.app.clone();
        let history = ctx.nav_history.clone();
        app.on_nav_story(move |story| {
            let (can_back, can_fwd) = {
                let mut h = history.lock().unwrap();
                h.push(NavEntry {
                    view: "story".to_string(),
                    ..Default::default()
                });
                (h.can_go_back(), h.can_go_forward())
            };
            if let Some(app_ref) = app_weak.upgrade() {
                app_ref.set_can_go_back(can_back);
                app_ref.set_can_go_forward(can_fwd);
                app_ref.set_current_story(story);
                app_ref.set_current_view("story".into());
            }
        });
    }

    // Nav: settings
    {
        let app_weak = ctx.app.clone();
        let history = ctx.nav_history.clone();
        app.on_nav_settings(move || {
            let (can_back, can_fwd) = {
                let mut h = history.lock().unwrap();
                h.push(NavEntry {
                    view: "settings".to_string(),
                    ..Default::default()
                });
                (h.can_go_back(), h.can_go_forward())
            };
            if let Some(app_ref) = app_weak.upgrade() {
                app_ref.set_can_go_back(can_back);
                app_ref.set_can_go_forward(can_fwd);
                app_ref.set_current_view("settings".into());
            }
        });
    }

    // Nav: installed tab
    {
        let app_weak = ctx.app.clone();
        let history = ctx.nav_history.clone();
        app.on_nav_installed(move || {
            let (can_back, can_fwd) = {
                let mut h = history.lock().unwrap();
                h.push(NavEntry {
                    view: "installed".to_string(),
                    ..Default::default()
                });
                (h.can_go_back(), h.can_go_forward())
            };
            if let Some(app_ref) = app_weak.upgrade() {
                app_ref.set_can_go_back(can_back);
                app_ref.set_can_go_forward(can_fwd);
                app_ref.set_current_view("installed".into());
                app_ref.invoke_refresh_requested();
            }
        });
    }

    // Nav: downloads tab
    {
        let app_weak = ctx.app.clone();
        let history = ctx.nav_history.clone();
        app.on_nav_downloads(move || {
            let (can_back, can_fwd) = {
                let mut h = history.lock().unwrap();
                h.push(NavEntry {
                    view: "downloads".to_string(),
                    ..Default::default()
                });
                (h.can_go_back(), h.can_go_forward())
            };
            if let Some(app_ref) = app_weak.upgrade() {
                app_ref.set_can_go_back(can_back);
                app_ref.set_can_go_forward(can_fwd);
                app_ref.set_current_view("downloads".into());
                app_ref.invoke_refresh_updates_requested();
            }
        });
    }

    // Nav: back
    {
        let app_weak = ctx.app.clone();
        let proxy_arc = ctx.proxy_opt.clone();
        let rt_handle = ctx.rt.clone();
        let store = ctx.tx_store.clone();
        let history = ctx.nav_history.clone();
        app.on_nav_back(move || {
            let entry = {
                let mut h = history.lock().unwrap();
                h.back()
            };
            let Some(entry) = entry else { return };
            let (can_back, can_fwd) = {
                let h = history.lock().unwrap();
                (h.can_go_back(), h.can_go_forward())
            };
            let Some(app_ref) = app_weak.upgrade() else {
                return;
            };
            app_ref.set_can_go_back(can_back);
            app_ref.set_can_go_forward(can_fwd);
            match entry.view.as_str() {
                "home" => {
                    app_ref.set_current_view("home".into());
                    let app_weak2 = app_weak.clone();
                    let proxy2 = proxy_arc.lock().unwrap().clone();
                    let rt2 = rt_handle.clone();
                    rt2.spawn(async move {
                        refresh_home_installed(app_weak2, proxy2).await;
                    });
                }
                "detail" => {
                    let id: slint::SharedString = entry.detail_id.into();
                    let app_weak2 = app_weak.clone();
                    let proxy2 = get_proxy(&proxy_arc);
                    let store2 = store.clone();
                    rt_handle.spawn(async move {
                        load_detail(id, proxy2, store2, app_weak2).await;
                    });
                    app_ref.set_detail_screenshots([].as_slice().into());
                    app_ref.set_detail_screenshots_loading(false);
                    app_ref.set_detail_description_blocks([].as_slice().into());
                    app_ref.set_detail_loading(true);
                    app_ref.set_current_view("detail".into());
                }
                "story" => {
                    app_ref.set_current_view("story".into());
                }
                "settings" => {
                    app_ref.set_current_view("settings".into());
                }
                "installed" => {
                    app_ref.set_current_view("installed".into());
                    app_ref.invoke_refresh_requested();
                }
                "downloads" => {
                    app_ref.set_current_view("downloads".into());
                    app_ref.invoke_refresh_updates_requested();
                }
                "search" => {
                    app_ref.set_current_view("search".into());
                }
                _ => {}
            }
        });
    }

    // Nav: forward
    {
        let app_weak = ctx.app.clone();
        let proxy_arc = ctx.proxy_opt.clone();
        let rt_handle = ctx.rt.clone();
        let store = ctx.tx_store.clone();
        let history = ctx.nav_history.clone();
        app.on_nav_forward(move || {
            let entry = {
                let mut h = history.lock().unwrap();
                h.forward()
            };
            let Some(entry) = entry else { return };
            let (can_back, can_fwd) = {
                let h = history.lock().unwrap();
                (h.can_go_back(), h.can_go_forward())
            };
            let Some(app_ref) = app_weak.upgrade() else {
                return;
            };
            app_ref.set_can_go_back(can_back);
            app_ref.set_can_go_forward(can_fwd);
            match entry.view.as_str() {
                "home" => {
                    app_ref.set_current_view("home".into());
                    let app_weak2 = app_weak.clone();
                    let proxy2 = proxy_arc.lock().unwrap().clone();
                    let rt2 = rt_handle.clone();
                    rt2.spawn(async move {
                        refresh_home_installed(app_weak2, proxy2).await;
                    });
                }
                "detail" => {
                    let id: slint::SharedString = entry.detail_id.into();
                    let app_weak2 = app_weak.clone();
                    let proxy2 = get_proxy(&proxy_arc);
                    let store2 = store.clone();
                    rt_handle.spawn(async move {
                        load_detail(id, proxy2, store2, app_weak2).await;
                    });
                    app_ref.set_detail_screenshots([].as_slice().into());
                    app_ref.set_detail_screenshots_loading(false);
                    app_ref.set_detail_description_blocks([].as_slice().into());
                    app_ref.set_detail_loading(true);
                    app_ref.set_current_view("detail".into());
                }
                "story" => {
                    app_ref.set_current_view("story".into());
                }
                "settings" => {
                    app_ref.set_current_view("settings".into());
                }
                "installed" => {
                    app_ref.set_current_view("installed".into());
                    app_ref.invoke_refresh_requested();
                }
                "downloads" => {
                    app_ref.set_current_view("downloads".into());
                    app_ref.invoke_refresh_updates_requested();
                }
                "search" => {
                    app_ref.set_current_view("search".into());
                }
                _ => {}
            }
        });
    }

    app.on_open_url_requested(|url| {
        let _ = std::process::Command::new("xdg-open")
            .arg(url.as_str())
            .spawn();
    });

    {
        let proxy_arc = ctx.proxy_opt.clone();
        let rt_handle = ctx.rt.clone();
        app.on_launch_requested(move |pkg_id| {
            let pkg_id = pkg_id.to_string();
            let proxy_arc = proxy_arc.clone();
            rt_handle.spawn(async move {
                if let Some(proxy) = helpers::get_or_connect(&proxy_arc).await {
                    let _ = proxy.run_package(&pkg_id).await;
                }
            });
        });
    }
}
