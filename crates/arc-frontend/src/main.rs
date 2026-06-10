mod forge;
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
use packages::{
    build_installed_cache, load_detail, load_home, load_package_icons, refresh_home_installed,
};
use slint::Model;
use std::sync::{Arc, Mutex};
use transactions::{
    add_to_available_updates, begin_transaction, push_transactions_to_ui,
    remove_from_available_updates, run_signal_listener, SavedPkgData, TxStatus, TxStore,
};

slint::include_modules!();
include!(concat!(env!("OUT_DIR"), "/locale/translators.rs"));

#[derive(Clone, Debug, Default)]
struct NavEntry {
    view: String,
    detail_id: String,
    search_text: String,
}

struct NavHistory {
    entries: Vec<NavEntry>,
    cursor: usize,
}

impl NavHistory {
    fn new() -> Self {
        Self {
            entries: vec![NavEntry { view: "home".to_string(), ..Default::default() }],
            cursor: 0,
        }
    }

    fn push(&mut self, entry: NavEntry) {
        self.entries.truncate(self.cursor + 1);
        self.entries.push(entry);
        self.cursor = self.entries.len() - 1;
    }

    fn can_go_back(&self) -> bool {
        self.cursor > 0
    }

    fn can_go_forward(&self) -> bool {
        self.cursor < self.entries.len() - 1
    }

    fn back(&mut self) -> Option<NavEntry> {
        if self.cursor > 0 {
            self.cursor -= 1;
            Some(self.entries[self.cursor].clone())
        } else {
            None
        }
    }

    fn forward(&mut self) -> Option<NavEntry> {
        if self.cursor < self.entries.len() - 1 {
            self.cursor += 1;
            Some(self.entries[self.cursor].clone())
        } else {
            None
        }
    }

    fn reset(&mut self) {
        *self = Self::new();
    }
}

fn apply_translations(app: &AppWindow) {
    use tr::tr;
    app.set_tr_install(tr!("Install").into());
    app.set_tr_remove(tr!("Remove").into());
    app.set_tr_update(tr!("Update").into());
    app.set_tr_cancel(tr!("Cancel").into());
    app.set_tr_loading(tr!("Loading...").into());
    app.set_tr_installed(tr!("Installed").into());
    app.set_tr_back(tr!("Back").into());
    app.set_tr_run(tr!("Run").into());
    app.set_tr_working(tr!("Working...").into());
    app.set_tr_verified(tr!("Verified developer").into());
    app.set_tr_website(tr!("Website").into());
    app.set_tr_extensions(tr!("Extensions").into());
    app.set_tr_restart(tr!("Restart").into());
    app.set_tr_name(tr!("Name").into());
    app.set_tr_url(tr!("URL").into());
    app.set_tr_categories(tr!("Categories").into());
    app.set_tr_recommended_apps(tr!("Recommended Apps").into());
    app.set_tr_no_packages(tr!("No packages found.\nSearch for an application to get started.").into());
    app.set_tr_up_to_date(tr!("Everything is up to date.").into());
    app.set_tr_no_installed(tr!("No applications installed.").into());
    app.set_tr_downloads(tr!("Downloads & Updates").into());
    app.set_tr_checking(tr!("Checking...").into());
    app.set_tr_check_updates(tr!("Check for Updates").into());
    app.set_tr_active_prefix(tr!("Active (").into());
    app.set_tr_pending_prefix(tr!("Pending (").into());
    app.set_tr_completed_prefix(tr!("Completed (").into());
    app.set_tr_clear_all(tr!("Clear All").into());
    app.set_tr_updates_available_prefix(tr!("Updates Available (").into());
    app.set_tr_update_all(tr!("Update All").into());
    app.set_tr_no_downloads(tr!("No active downloads").into());
    app.set_tr_downloads_empty(tr!("Installs and updates will appear here").into());
    app.set_tr_removed(tr!("Removed").into());
    app.set_tr_failed(tr!("Failed").into());
    app.set_tr_queued(tr!("Queued").into());
    app.set_tr_removing(tr!("Removing").into());
    app.set_tr_updating(tr!("Updating").into());
    app.set_tr_installing(tr!("Installing").into());
    app.set_tr_license(tr!("License Agreement").into());
    app.set_tr_eula_requires(tr!(" requires you to accept its End User License Agreement before installing.").into());
    app.set_tr_read_license(tr!("Read License Agreement →").into());
    app.set_tr_accept_install(tr!("Accept & Install").into());
    app.set_tr_install_flatpak_bundle(tr!("Install Flatpak Bundle").into());
    app.set_tr_install_flatpak_bundle_desc(tr!("This will install the bundled Flatpak application on your system.").into());
    app.set_tr_install_appimage(tr!("Install AppImage").into());
    app.set_tr_install_appimage_desc(tr!("This will register the AppImage so it appears in your app launcher.").into());
    app.set_tr_install_package(tr!("Install Package").into());
    app.set_tr_install_package_desc(tr!("Choose how you'd like to install this package file.").into());
    app.set_tr_install_bundle(tr!("Install Bundle").into());
    app.set_tr_install_as_appimage(tr!("Install as AppImage").into());
    app.set_tr_install_distrobox(tr!("Install via Distrobox").into());
    app.set_tr_searching_flathub(tr!("Searching Flathub for an alternative...").into());
    app.set_tr_file_security(tr!("Third-party software has broad access to system resources and may pose a security risk. Only install files from sources you trust.").into());
    app.set_tr_add_repo(tr!("Add Repository").into());
    app.set_tr_unknown_repo(tr!("Unknown repository").into());
    app.set_tr_repo_add_desc(tr!("This will add the repository to your Flatpak user installation and make its applications available to install.").into());
    app.set_tr_repo_warning(tr!("Adding a third-party repository gives it the ability to distribute software to your system. Only add repositories from sources you trust.").into());
    app.set_tr_contains_repo(tr!("Contains repository").into());
    app.set_tr_install_application(tr!("Install Application").into());
    app.set_tr_settings(tr!("Settings").into());
    app.set_tr_general(tr!("GENERAL").into());
    app.set_tr_auto_updates(tr!("Auto-updates").into());
    app.set_tr_auto_updates_desc(tr!("Automatically update installed apps in the background").into());
    app.set_tr_security_warnings(tr!("Security warnings").into());
    app.set_tr_security_warnings_desc(tr!("Show warnings when installing third-party software or adding repositories").into());
    app.set_tr_downloads_section(tr!("DOWNLOADS").into());
    app.set_tr_concurrent(tr!("Concurrent downloads").into());
    app.set_tr_concurrent_desc(tr!("Maximum number of simultaneous downloads (1-16)").into());
    app.set_tr_repositories(tr!("REPOSITORIES").into());
    app.set_tr_loading_ellipsis(tr!("Loading…").into());
    app.set_tr_protected(tr!("Protected").into());
    app.set_tr_add_repo_section(tr!("ADD REPOSITORY").into());
    app.set_tr_danger_zone(tr!("DANGER ZONE").into());
    app.set_tr_force_updates(tr!("Force updates").into());
    app.set_tr_force_updates_desc(tr!("Runs flatpak update -y directly, bypassing the daemon").into());
    app.set_tr_force_update(tr!("Force Update").into());
    app.set_tr_restart_daemon(tr!("Restart daemon").into());
    app.set_tr_restart_daemon_desc(tr!("Kills the running arc-daemon and starts a fresh one in the background").into());
    app.set_tr_search_placeholder(tr!("Search for applications...").into());
    app.set_tr_proprietary(tr!("Proprietary").into());
    app.set_tr_all_ages(tr!("All ages").into());
    app.set_tr_pwa(tr!("PWA").into());
    app.set_tr_uninstall(tr!("Uninstall").into());
    app.set_tr_install_from_suffix(tr!(" from Arc Software instead").into());
    // Verb-final languages (German, etc.) translate this sentinel to a non-empty value.
    // When detected, clear the verb prefix so the name leads and the suffix carries the verb.
    if tr!("verb_final_word_order") != "verb_final_word_order" {
        app.set_tr_install_from_prefix("".into());
        app.set_tr_uninstall_title_prefix("".into());
        app.set_tr_uninstall_title_suffix(tr!("uninstall_suffix").into());
    }
    app.set_tr_delete_data(tr!("Also delete app data").into());
    app.set_tr_delete_data_desc(tr!("Removes settings and saved data").into());
}

fn main() -> Result<()> {
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
    apply_translations(&app);

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

    let rt_handle_global = rt.handle().clone();

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

    type InstalledCache = Arc<Mutex<Option<Vec<packages::RawPackage>>>>;
    let installed_cache: InstalledCache = Arc::new(Mutex::new(None));

    {
        let app_weak = app.as_weak();
        let proxy = proxy_opt.lock().unwrap().clone();
        let rt_handle = rt_handle_global.clone();
        let history = nav_history.clone();

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
        let app_weak = app.as_weak();
        let proxy = proxy_opt.lock().unwrap().clone();
        let rt_handle = rt_handle_global.clone();

        app.on_back_to_home(move || {
            let app_weak = app_weak.clone();
            let proxy = proxy.clone();
            let rt_handle = rt_handle.clone();
            rt_handle.spawn(async move {
                refresh_home_installed(app_weak, proxy).await;
            });
        });
    }

    let (cache_tx, mut cache_rx) = tokio::sync::mpsc::channel::<()>(8);

    if let Some(proxy) = get_proxy(&proxy_opt) {
        let store = tx_store.clone();
        let app_weak = app.as_weak();
        rt.handle()
            .spawn(run_signal_listener(proxy, store, app_weak, cache_tx));
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
        let history = nav_history.clone();

        app.on_search_requested(move |query| {
            let query_str = query.to_string();
            if query_str.trim().is_empty() {
                return;
            }

            let (can_back, can_fwd) = {
                let mut h = history.lock().unwrap();
                h.push(NavEntry { view: "search".to_string(), search_text: query_str.clone(), ..Default::default() });
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
        let app_weak = app.as_weak();

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

            // Detail screen already checks EULA before emitting install-requested,
            // so skip the async EULA fetch when coming from there.
            if in_detail {
                begin_transaction(
                    pkg_id_str,
                    String::new(),
                    display_name,
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
                    begin_transaction(
                        pkg_id_str,
                        String::new(),
                        display_name,
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
        let app_weak = app.as_weak();
        let proxy_arc = proxy_opt.clone();
        let rt_handle = rt.handle().clone();
        let store = tx_store.clone();

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
        let app_weak = app.as_weak();
        let proxy_arc = proxy_opt.clone();
        let rt_handle = rt.handle().clone();
        let store = tx_store.clone();

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
        let app_weak = app.as_weak();

        app.on_remove_requested(move |pkg_id| {
            let pkg_id_str = pkg_id.to_string();
            let Some(app) = app_weak.upgrade() else { return };
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
        let app_weak = app.as_weak();
        let proxy_arc = proxy_opt.clone();
        let rt_handle = rt.handle().clone();
        let store = tx_store.clone();

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
            let tx_type = if delete_data { "remove_with_data" } else { "remove" }.to_string();

            begin_transaction(
                pkg_id_str,
                String::new(),
                display_name,
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
                String::new(),
                display_name,
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
                    String::new(),
                    name,
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
        let history = nav_history.clone();

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
                h.push(NavEntry { view: "detail".to_string(), detail_id: id_str, ..Default::default() });
                (h.can_go_back(), h.can_go_forward())
            };

            if let Some(app_ref) = app_weak.upgrade() {
                app_ref.set_can_go_back(can_back);
                app_ref.set_can_go_forward(can_fwd);
                app_ref.set_detail_screenshots([].as_slice().into());
                app_ref.set_detail_description_blocks([].as_slice().into());
                app_ref.set_detail_loading(true);
                app_ref.set_current_view("detail".into());
            }
        });
    }

    {
        let app_weak = app.as_weak();
        let proxy_arc = proxy_opt.clone();
        let rt_handle = rt.handle().clone();

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
        let app_weak = app.as_weak();
        let proxy_arc = proxy_opt.clone();
        let rt_handle = rt.handle().clone();
        let store = tx_store.clone();

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
        let app_weak = app.as_weak();

        app.on_eula_cancelled(move || {
            if let Some(app_ref) = app_weak.upgrade() {
                let source = app_ref.get_eula_source_view().to_string();
                app_ref.set_current_view(source.into());
            }
        });
    }

    {
        let settings = settings.clone();
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
        let rt_handle = rt.handle().clone();
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
        let rt_handle = rt.handle().clone();
        app.on_restart_daemon_requested(move || {
            rt_handle.spawn(async move {
                // Kill existing daemon
                let _ = tokio::process::Command::new("pkill")
                    .args(["-x", "arc-daemon"])
                    .status()
                    .await;

                // Brief pause to let it exit cleanly
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
                        String::new(),
                        title,
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
        let app_weak = app.as_weak();
        let history = nav_history.clone();
        app.on_nav_story(move |story| {
            let (can_back, can_fwd) = {
                let mut h = history.lock().unwrap();
                h.push(NavEntry { view: "story".to_string(), ..Default::default() });
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
        let app_weak = app.as_weak();
        let history = nav_history.clone();
        app.on_nav_settings(move || {
            let (can_back, can_fwd) = {
                let mut h = history.lock().unwrap();
                h.push(NavEntry { view: "settings".to_string(), ..Default::default() });
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
        let app_weak = app.as_weak();
        let history = nav_history.clone();
        app.on_nav_installed(move || {
            let (can_back, can_fwd) = {
                let mut h = history.lock().unwrap();
                h.push(NavEntry { view: "installed".to_string(), ..Default::default() });
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
        let app_weak = app.as_weak();
        let history = nav_history.clone();
        app.on_nav_downloads(move || {
            let (can_back, can_fwd) = {
                let mut h = history.lock().unwrap();
                h.push(NavEntry { view: "downloads".to_string(), ..Default::default() });
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
        let app_weak = app.as_weak();
        let proxy_arc = proxy_opt.clone();
        let rt_handle = rt_handle_global.clone();
        let store = tx_store.clone();
        let history = nav_history.clone();
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
            let Some(app_ref) = app_weak.upgrade() else { return };
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
        let app_weak = app.as_weak();
        let proxy_arc = proxy_opt.clone();
        let rt_handle = rt_handle_global.clone();
        let store = tx_store.clone();
        let history = nav_history.clone();
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
            let Some(app_ref) = app_weak.upgrade() else { return };
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
        let proxy_arc = proxy_opt.clone();
        let rt_handle = rt.handle().clone();
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

    app.run()?;
    Ok(())
}
