mod forge;
mod helpers;
mod icons;
mod callbacks;
mod launcher;
mod nav;
mod packages;
mod system_style;
mod transactions;
mod translations;

use anyhow::Result;
use helpers::get_proxy;
use libarc::{ArcDaemonProxy, Provider, Settings};
use nav::NavHistory;
use packages::{build_installed_cache, load_home, load_package_icons};
use std::sync::{Arc, Mutex};
use transactions::{load_running_transactions, run_signal_listener, TxStore};

slint::include_modules!();
include!(concat!(env!("OUT_DIR"), "/locale/translators.rs"));

fn main() -> Result<()> {
    use tr::tr;
    let locale = sys_locale::get_locale().unwrap_or_else(|| "en".to_string());
    translators::set_locale(&locale);
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    let proxy_result = rt.block_on(async {
        tokio::time::timeout(std::time::Duration::from_secs(2), libarc::connect())
            .await
            .unwrap_or_else(|_| Err(anyhow::anyhow!("daemon connection timed out")))
    });

    let app = AppWindow::new()?;
    translations::apply_translations(&app);
    system_style::init(rt.handle(), app.as_weak());

    if let Some(icon) = icons::load_ui_icon("go-home-symbolic") {
        app.set_icon_home(icon.to_slint_image());
    }
    if let Some(icon) = icons::load_ui_icon("edit-find-symbolic") {
        app.set_icon_search(icon.to_slint_image());
    }
    if let Some(icon) = icons::load_ui_icon("settings-configure") {
        app.set_icon_settings(icon.to_slint_image());
    }
    if let Some(icon) = icons::load_ui_icon("user-trash-symbolic") {
        app.set_icon_trash(icon.to_slint_image());
    }
    if let Some(icon) = icons::load_ui_icon_large("package-x-generic-symbolic") {
        app.set_icon_package(icon.to_slint_image());
    }
    if let Some(icon) = icons::load_ui_icon("go-previous-symbolic") {
        app.set_icon_nav_back(icon.to_slint_image());
    }
    if let Some(icon) = icons::load_ui_icon("go-next-symbolic") {
        app.set_icon_nav_forward(icon.to_slint_image());
    }

    let nav_history: Arc<Mutex<NavHistory>> = Arc::new(Mutex::new(NavHistory::new()));

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
                Provider::Pwa => "Flatpak",
            }
            .into(),
        );
        app.set_settings_ignore_native_pref(s.ignore_native_preference);
        app.set_settings_auto_updates(s.auto_updates);
        app.set_settings_concurrent_downloads(s.concurrent_downloads as i32);
        app.set_settings_show_security_warnings(s.show_security_warnings);
    }

    let tx_store: TxStore = Arc::new(Mutex::new(Vec::new()));
    let pending_runtime_ids: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let runtime_icon = icons::load_default_icon();

    type InstalledCache = Arc<Mutex<Option<Vec<packages::RawPackage>>>>;
    let installed_cache: InstalledCache = Arc::new(Mutex::new(None));

    let (cache_tx, mut cache_rx) = tokio::sync::mpsc::channel::<()>(8);

    if let Some(proxy) = get_proxy(&proxy_opt) {
        let store = tx_store.clone();
        let app_weak = app.as_weak();
        rt.handle()
            .spawn(run_signal_listener(proxy, store, app_weak, cache_tx));
    }

    // Recover transactions that were already running in the daemon, e.g.
    // when the frontend was closed and reopened during an install.
    if let Some(proxy) = get_proxy(&proxy_opt) {
        let store = tx_store.clone();
        let app_weak = app.as_weak();
        rt.handle()
            .spawn(load_running_transactions(proxy, store, app_weak));
    }

    // Populate the installed cache on startup.
    {
        let cache = installed_cache.clone();
        let proxy = get_proxy(&proxy_opt);
        let app_weak = app.as_weak();
        rt.handle().spawn(async move {
            let raw = build_installed_cache(proxy).await;
            let mut guard = cache.lock().unwrap();
            *guard = Some(raw);
            drop(guard);
            // If the user already navigated to the installed tab, update it now.
            let _ = app_weak.upgrade_in_event_loop(|app| {
                if app.get_current_view() == "installed" && app.get_is_loading() {
                    app.invoke_refresh_requested();
                }
            });
        });
    }

    // Rebuild the cache after every transaction completion and refresh the UI
    // if the user is on the installed tab.
    {
        let cache = installed_cache.clone();
        let proxy_arc = proxy_opt.clone();
        let app_weak = app.as_weak();
        let rt_handle = rt.handle().clone();
        rt_handle.clone().spawn(async move {
            while cache_rx.recv().await.is_some() {
                let proxy = get_proxy(&proxy_arc);
                let raw = build_installed_cache(proxy).await;
                {
                    let mut guard = cache.lock().unwrap();
                    *guard = Some(raw.clone());
                }
                let _ = app_weak.upgrade_in_event_loop(move |app| {
                    if app.get_current_view() == "installed" {
                        let pkgs: Vec<PackageItem> = raw.iter().map(|r| r.to_slint()).collect();
                        app.set_packages(pkgs.as_slice().into());
                        app.set_is_loading(false);
                    }
                });
            }
        });
    }

    {
        let app_weak = app.as_weak();
        let proxy = proxy_opt.lock().unwrap().clone();
        app.set_home_loading(true);
        rt.handle().spawn(async move {
            load_home(app_weak, proxy).await;
        });
    }

    if let Some(proxy) = proxy_opt.lock().unwrap().clone() {
        let app_weak = app.as_weak();
        let runtime_ids_init = pending_runtime_ids.clone();
        let runtime_icon_init = runtime_icon.clone();
        rt.handle().spawn(async move {
            let updates: Vec<libarc::Package> = proxy
                .list_updates()
                .await
                .ok()
                .and_then(|json| serde_json::from_str(&json).ok())
                .unwrap_or_default();
            if updates.is_empty() {
                return;
            }
            let (runtime_updates, app_updates): (Vec<_>, Vec<_>) =
                updates.into_iter().partition(|p| p.is_runtime);
            let raw_pkgs = load_package_icons(app_updates).await;
            let _ = app_weak.upgrade_in_event_loop(move |app| {
                let mut pkgs: Vec<PackageItem> = raw_pkgs.iter().map(|r| r.to_slint()).collect();
                if !runtime_updates.is_empty() {
                    *runtime_ids_init.lock().unwrap() =
                        runtime_updates.iter().map(|p| p.id.clone()).collect();
                    pkgs.push(PackageItem {
                        id: "arc-runtime-libraries".into(),
                        name: tr!("Arc Runtime Libraries").into(),
                        version: Default::default(),
                        description: Default::default(),
                        installed: true,
                        icon: runtime_icon_init
                            .as_ref()
                            .map(|r| r.to_slint_image())
                            .unwrap_or_default(),
                        busy: false,
                        progress: 0.0,
                        transaction_id: Default::default(),
                    });
                }
                let count = pkgs.len() as i32;
                app.set_available_updates(pkgs.as_slice().into());
                app.set_update_count(count);
                launcher::update_badge(count);
            });
        });
    }

    let ctx = callbacks::AppContext {
        app: app.as_weak(),
        rt: rt.handle().clone(),
        proxy_opt: proxy_opt.clone(),
        settings: settings.clone(),
        tx_store: tx_store.clone(),
        pending_runtime_ids: pending_runtime_ids.clone(),
        installed_cache: installed_cache.clone(),
        nav_history: nav_history.clone(),
        runtime_icon: runtime_icon.clone(),
    };
    callbacks::register_all(&app, &ctx);

    app.run()?;

    // clear the taskbar badge and progress when the app exits
    launcher::clear_blocking();

    Ok(())
}
