mod appstream_db;
mod icons;

use crate::icons::RawIcon;
use anyhow::Result;
use appstream_db::AppStreamDb;
use futures_util::{future::join_all, StreamExt};
use libarc::{connect, ArcDaemonProxy, Package, Provider, Settings};
use slint::{Model, SharedString};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

slint::include_modules!();

// ── Transaction tracking ──────────────────────────────────────────────────────

#[derive(Clone, PartialEq)]
enum TxStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

#[derive(Clone)]
struct TrackedTx {
    id: String,
    pkg_id: String,
    name: String,
    icon: Option<RawIcon>,
    progress: f32,
    status: TxStatus,
    tx_type: String,
    error: String,
    installed_after: bool,
    refresh_detail: bool,
}

type TxStore = Arc<Mutex<Vec<TrackedTx>>>;

fn has_ongoing_transaction_for_package(store: &TxStore, pkg_id: &str) -> bool {
    let s = store.lock().unwrap();
    s.iter().any(|tx| {
        tx.pkg_id == pkg_id && (tx.status == TxStatus::Pending || tx.status == TxStatus::Running)
    })
}

fn push_transactions_to_ui(store: TxStore, app_weak: &slint::Weak<AppWindow>) {
    let (active, pending, completed) = {
        let s = store.lock().unwrap();
        let mut a: Vec<TrackedTx> = Vec::new();
        let mut p: Vec<TrackedTx> = Vec::new();
        let mut c: Vec<TrackedTx> = Vec::new();
        for tx in s.iter() {
            match tx.status {
                TxStatus::Running => a.push(tx.clone()),
                TxStatus::Pending => p.push(tx.clone()),
                TxStatus::Completed | TxStatus::Failed => c.push(tx.clone()),
            }
        }
        (a, p, c)
    };

    let active_count = (active.len() + pending.len()) as i32;

    let status_text = if let Some(tx) = active.first() {
        let verb = match tx.tx_type.as_str() {
            "install" | "flatpakref" => "Installing",
            "remove" => "Removing",
            _ => "Updating",
        };
        format!("{} {} ({}%)", verb, tx.name, (tx.progress * 100.0) as i32)
    } else if !pending.is_empty() {
        format!("{} operation(s) queued", pending.len())
    } else {
        String::new()
    };

    let _ = app_weak.upgrade_in_event_loop(move |app| {
        fn to_slint_items(txs: Vec<TrackedTx>) -> Vec<TransactionItem> {
            txs.into_iter()
                .map(|tx| TransactionItem {
                    id: tx.id.into(),
                    name: tx.name.into(),
                    icon: tx.icon.map(|r| r.to_slint_image()).unwrap_or_default(),
                    progress: tx.progress,
                    status: match tx.status {
                        TxStatus::Pending => "pending",
                        TxStatus::Running => "running",
                        TxStatus::Completed => "completed",
                        TxStatus::Failed => "failed",
                    }
                    .into(),
                    tx_type: tx.tx_type.into(),
                    error: tx.error.into(),
                })
                .collect()
        }

        app.set_active_transactions(to_slint_items(active).as_slice().into());
        app.set_pending_transactions(to_slint_items(pending).as_slice().into());
        app.set_completed_transactions(to_slint_items(completed).as_slice().into());
        app.set_active_transaction_count(active_count);
        if !status_text.is_empty() {
            app.set_status_text(status_text.into());
        }
        // Update detail-busy based on whether the current detail package has an ongoing transaction
        let current_detail_pkg = app.get_detail_app().flatpak_id.to_string();
        if !current_detail_pkg.is_empty() {
            let is_busy = has_ongoing_transaction_for_package(&store, &current_detail_pkg);
            app.set_detail_busy(is_busy);
        }
    });
}

async fn load_icon_for_pkg(pkg_id: &str, name: &str) -> Option<RawIcon> {
    if pkg_id.starts_with("lutris:") {
        return None;
    }
    let is_flatpak = !pkg_id.contains('/')
        && !pkg_id.contains(';')
        && !pkg_id.starts_with("distrobox:")
        && pkg_id.matches('.').count() >= 2;
    if is_flatpak {
        let id = pkg_id.to_string();
        tokio::task::spawn_blocking(move || icons::load_local_flatpak_icon(&id))
            .await
            .unwrap_or(None)
    } else {
        let n = name.to_string();
        tokio::task::spawn_blocking(move || icons::load_native_package_icon(&n))
            .await
            .unwrap_or(None)
    }
}

fn begin_transaction(
    pkg_id: String,
    display_name: String,
    tx_type: String,
    installed_after: bool,
    refresh_detail: bool,
    store: TxStore,
    proxy_arc: Arc<Mutex<Option<ArcDaemonProxy<'static>>>>,
    app_weak: slint::Weak<AppWindow>,
    rt_handle: tokio::runtime::Handle,
) {
    // Don't queue duplicates for the same package
    {
        let s = store.lock().unwrap();
        if s.iter().any(|e| {
            e.pkg_id == pkg_id && (e.status == TxStatus::Pending || e.status == TxStatus::Running)
        }) {
            return;
        }
    }

    let entry_idx = {
        let mut s = store.lock().unwrap();
        s.push(TrackedTx {
            id: String::new(),
            pkg_id: pkg_id.clone(),
            name: display_name.clone(),
            icon: None,
            progress: 0.0,
            status: TxStatus::Pending,
            tx_type: tx_type.clone(),
            error: String::new(),
            installed_after,
            refresh_detail,
        });
        s.len() - 1
    };
    push_transactions_to_ui(store.clone(), &app_weak);

    let name_for_icon = display_name.clone();
    let pkg_id_for_icon = pkg_id.clone();

    rt_handle.spawn(async move {
        let tx_id_result = match get_or_connect(&proxy_arc).await {
            Some(p) => match tx_type.as_str() {
                "install" => p.install_package(&pkg_id).await.ok(),
                "flatpakref" => p.install_flatpakref(&pkg_id).await.ok(),
                "remove" => p.remove_package(&pkg_id).await.ok(),
                "update" => p.update_package(&pkg_id).await.ok(),
                _ => None,
            },
            None => None,
        };

        match tx_id_result {
            Some(tx_id) => {
                {
                    let mut s = store.lock().unwrap();
                    if let Some(e) = s.get_mut(entry_idx) {
                        e.id = tx_id;
                        e.status = TxStatus::Running;
                    }
                }
                push_transactions_to_ui(store.clone(), &app_weak);

                let icon = load_icon_for_pkg(&pkg_id_for_icon, &name_for_icon).await;
                {
                    let mut s = store.lock().unwrap();
                    if let Some(e) = s.get_mut(entry_idx) {
                        e.icon = icon;
                    }
                }
                push_transactions_to_ui(store.clone(), &app_weak);
            }
            None => {
                {
                    let mut s = store.lock().unwrap();
                    if let Some(e) = s.get_mut(entry_idx) {
                        e.status = TxStatus::Failed;
                        e.error = "Failed to connect to daemon".to_string();
                    }
                }
                push_transactions_to_ui(store.clone(), &app_weak);
            }
        }
    });
}

async fn run_signal_listener(
    proxy: ArcDaemonProxy<'static>,
    store: TxStore,
    app_weak: slint::Weak<AppWindow>,
) {
    let (mut progress_stream, mut finished_stream) = match tokio::join!(
        proxy.receive_transaction_progress(),
        proxy.receive_transaction_finished(),
    ) {
        (Ok(ps), Ok(fs)) => (ps, fs),
        _ => return,
    };

    loop {
        tokio::select! {
            sig = progress_stream.next() => {
                let Some(sig) = sig else { break; };
                let Ok(args) = sig.args() else { continue; };
                let tx_id = args.transaction_id().to_string();
                let progress = *args.progress() as f32 / 100.0;
                {
                    let mut s = store.lock().unwrap();
                    if let Some(e) = s.iter_mut().find(|e| e.id == tx_id) {
                        e.progress = progress;
                        if e.status == TxStatus::Pending {
                            e.status = TxStatus::Running;
                        }
                    }
                }
                push_transactions_to_ui(store.clone(), &app_weak);
            }
            sig = finished_stream.next() => {
                let Some(sig) = sig else { break; };
                let Ok(args) = sig.args() else { continue; };
                let tx_id = args.transaction_id().to_string();
                let success = *args.success();
                let msg = args.message().to_string();

                let side_effect = {
                    let mut s = store.lock().unwrap();
                    if let Some(e) = s.iter_mut().find(|e| e.id == tx_id) {
                        if success {
                            e.status = TxStatus::Completed;
                            e.progress = 1.0;
                        } else {
                            e.status = TxStatus::Failed;
                            e.error = msg;
                        }
                        Some((
                            e.pkg_id.clone(),
                            e.installed_after,
                            e.refresh_detail,
                            success,
                            e.name.clone(),
                            e.tx_type.clone(),
                        ))
                    } else {
                        None
                    }
                };

                push_transactions_to_ui(store.clone(), &app_weak);

                if let Some((pkg_id, installed_after, refresh_detail, ok, name, tx_type)) =
                    side_effect
                {
                    let store_for_closure = store.clone();
                    let _ = app_weak.upgrade_in_event_loop(move |app| {
                        if ok {
                            update_package_installed(&app, &pkg_id, installed_after);
                            let verb = match tx_type.as_str() {
                                "remove" => "Removed",
                                "update" => "Updated",
                                _ => "Installed",
                            };
                            app.set_status_text(format!("{} {}.", verb, name).into());
                            if refresh_detail && app.get_current_view() == "detail" {
                                app.invoke_detail_requested(pkg_id.into());
                            }
                        } else {
                            app.set_status_text("Operation failed.".into());
                        }
                        // Update detail-busy state after transaction completes
                        let current_detail_pkg = app.get_detail_app().flatpak_id.to_string();
                        if !current_detail_pkg.is_empty() {
                            let is_busy = has_ongoing_transaction_for_package(&store_for_closure, &current_detail_pkg);
                            app.set_detail_busy(is_busy);
                        }
                    });
                }
            }
        }
    }
}

// ── Shared helpers ────────────────────────────────────────────────────────────

fn parse_flatpakref(content: &str) -> (String, String, String) {
    let mut name = String::new();
    let mut title = String::new();
    let mut url = String::new();
    for line in content.lines() {
        let line = line.trim();
        if let Some(v) = line.strip_prefix("Name=") {
            name = v.to_string();
        } else if let Some(v) = line.strip_prefix("Title=") {
            title = v.to_string();
        } else if let Some(v) = line.strip_prefix("Url=") {
            url = v.to_string();
        }
    }
    if title.is_empty() {
        title = name.clone();
    }
    (title, name, url)
}

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

fn dedup_by_preference(pkgs: Vec<libarc::Package>, settings: &Settings) -> Vec<libarc::Package> {
    use std::collections::HashSet;

    let flatpak_id_by_name: HashMap<String, String> = pkgs
        .iter()
        .filter(|p| p.provider == Provider::Flatpak)
        .map(|p| (p.name.to_lowercase(), p.id.clone()))
        .collect();
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
                    !native_names.contains(&name)
                        || settings.preferred_for(&p.id) == Provider::Flatpak
                }
                Provider::Distrobox => {
                    let Some(flatpak_id) = flatpak_id_by_name.get(&name) else {
                        return true;
                    };
                    settings.preferred_for(flatpak_id) == Provider::Distrobox
                }
                Provider::Lutris => {
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

fn get_display_name(app: &AppWindow, pkg_id: &str) -> String {
    let detail = app.get_detail_app();
    if detail.flatpak_id.as_str() == pkg_id
        || detail.native_id.as_str() == pkg_id
        || detail.lutris_id.as_str() == pkg_id
    {
        return detail.name.to_string();
    }
    let model = app.get_packages();
    for i in 0..model.row_count() {
        if let Some(p) = model.row_data(i) {
            if p.id.as_str() == pkg_id {
                return p.name.to_string();
            }
        }
    }
    let upd = app.get_available_updates();
    for i in 0..upd.row_count() {
        if let Some(p) = upd.row_data(i) {
            if p.id.as_str() == pkg_id {
                return p.name.to_string();
            }
        }
    }
    pkg_id.to_string()
}

// ── Data loading ──────────────────────────────────────────────────────────────

struct RawCard {
    id: String,
    name: String,
    summary: String,
    icon: Option<RawIcon>,
    installed: bool,
}

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
    license: String,
    homepage_url: String,
    content_rating: String,
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
            installed: self.installed,
        }
    }
}

async fn load_home(app_weak: slint::Weak<AppWindow>, proxy: Option<ArcDaemonProxy<'static>>) {
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

async fn load_package_icons(pkgs: Vec<libarc::Package>) -> Vec<RawPackage> {
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
                Provider::Distrobox => {
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

// ── Main ──────────────────────────────────────────────────────────────────────

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

    // Transaction store shared across all handlers
    let tx_store: TxStore = Arc::new(Mutex::new(Vec::new()));

    // Spawn the global signal listener once if the daemon is available
    if let Some(proxy) = get_proxy(&proxy_opt) {
        let store = tx_store.clone();
        let app_weak = app.as_weak();
        rt.handle()
            .spawn(run_signal_listener(proxy, store, app_weak));
    }

    // Load home in background
    {
        let app_weak = app.as_weak();
        let proxy = proxy_opt.lock().unwrap().clone();
        app.set_home_loading(true);
        rt.handle().spawn(async move {
            tokio::task::spawn_blocking(AppStreamDb::get_static)
                .await
                .unwrap();
            load_home(app_weak, proxy).await;
        });
    }

    // Silently check for updates on startup (no UI loading state)
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

    // ── Search ────────────────────────────────────────────────────────────────
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

                let all_pkgs = dedup_by_preference(daemon_pkgs, &s);
                let raw_pkgs = load_package_icons(all_pkgs).await;
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

    // ── Installed list ────────────────────────────────────────────────────────
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

    // ── Install ───────────────────────────────────────────────────────────────
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
                store.clone(),
                proxy_arc.clone(),
                app_weak.clone(),
                rt_handle.clone(),
            );
        });
    }

    // ── Remove ────────────────────────────────────────────────────────────────
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
                store.clone(),
                proxy_arc.clone(),
                app_weak.clone(),
                rt_handle.clone(),
            );
        });
    }

    // ── Update (single) ───────────────────────────────────────────────────────
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

            begin_transaction(
                pkg_id_str,
                display_name,
                "update".to_string(),
                true,
                in_detail,
                store.clone(),
                proxy_arc.clone(),
                app_weak.clone(),
                rt_handle.clone(),
            );
        });
    }

    // ── Update All ────────────────────────────────────────────────────────────
    {
        let app_weak = app.as_weak();
        let proxy_arc = proxy_opt.clone();
        let rt_handle = rt.handle().clone();
        let store = tx_store.clone();

        app.on_update_all_requested(move || {
            let updates: Vec<(String, String)> = app_weak
                .upgrade()
                .map(|a| {
                    let model = a.get_available_updates();
                    (0..model.row_count())
                        .filter_map(|i| model.row_data(i))
                        .map(|p| (p.id.to_string(), p.name.to_string()))
                        .collect()
                })
                .unwrap_or_default();

            for (pkg_id, name) in updates {
                begin_transaction(
                    pkg_id,
                    name,
                    "update".to_string(),
                    true,
                    false,
                    store.clone(),
                    proxy_arc.clone(),
                    app_weak.clone(),
                    rt_handle.clone(),
                );
            }
        });
    }

    // ── Clear completed ───────────────────────────────────────────────────────
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

    // ── Cancel transaction ────────────────────────────────────────────────────
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
                        // Update local state to reflect cancellation
                        let mut s = store.lock().unwrap();
                        if let Some(tx) = s.iter_mut().find(|t| t.id == tx_id) {
                            tx.status = TxStatus::Failed;
                            tx.error = "Cancelled".to_string();
                        }
                        drop(s);
                        push_transactions_to_ui(store.clone(), &app_weak);
                    }
                }
            });
        });
    }

    // ── Category ──────────────────────────────────────────────────────────────
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

    // ── Refresh updates (populates download manager's Available section) ───────
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
                let raw_pkgs = load_package_icons(updates).await;

                let _ = app_weak2.upgrade_in_event_loop(move |app| {
                    let pkgs: Vec<PackageItem> = raw_pkgs.iter().map(|r| r.to_slint()).collect();
                    app.set_available_updates(pkgs.as_slice().into());
                    app.set_update_count(count as i32);
                    app.set_updates_loading(false);
                    if count == 0 {
                        app.set_status_text("Everything is up to date.".into());
                    } else {
                        app.set_status_text(format!("{} update(s) available.", count).into());
                    }
                });
            });

            if let Some(app_ref) = app_weak.upgrade() {
                app_ref.set_updates_loading(true);
                app_ref.set_status_text("Checking for updates...".into());
            }
        });
    }

    // ── Detail view ───────────────────────────────────────────────────────────
    {
        let app_weak = app.as_weak();
        let proxy_arc = proxy_opt.clone();
        let rt_handle = rt.handle().clone();
        let settings = settings.clone();
        let store = tx_store.clone();

        app.on_detail_requested(move |id| {
            let app_id = id.to_string();
            let app_weak2 = app_weak.clone();
            let proxy = get_proxy(&proxy_arc);
            let _s = settings.lock().unwrap().clone();
            let store = store.clone();

            rt_handle.spawn(async move {
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
                        license: raw.license.into(),
                        homepage_url: raw.homepage_url.into(),
                        content_rating: raw.content_rating.into(),
                    });
                    app.set_detail_loading(false);
                    // Check if this package has an ongoing transaction
                    let is_busy =
                        has_ongoing_transaction_for_package(&store, &pkg_id_for_busy_check);
                    app.set_detail_busy(is_busy);
                });

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

    // ── Settings ──────────────────────────────────────────────────────────────
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

    // ── Deep-link / CLI args ──────────────────────────────────────────────────
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
                    .to_string()
            });
        let flatpakref_url = std::env::args()
            .find(|a| a.starts_with("flatpak+https://") || a.starts_with("flatpak+http://"))
            .map(|a| a.trim_start_matches("flatpak+").to_string());

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
        }
    }

    // ── Package file (MIME association) ───────────────────────────────────────
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

            if let Some(app_ref) = app_weak.upgrade() {
                app_ref.set_install_file_path(file_path.clone().into());
                app_ref.set_install_file_name(file_name.clone().into());
                app_ref.set_install_file_has_flatpak(false);
                app_ref.set_current_view("install-file".into());
            }

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
