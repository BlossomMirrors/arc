use crate::icons::{self, RawIcon};
use futures_util::StreamExt;
use libarc::ArcDaemonProxy;
use slint::Model;
use std::sync::{Arc, Mutex};

#[derive(Clone, PartialEq)]
pub enum TxStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

#[derive(Clone)]
pub struct SavedPkgData {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub installed: bool,
}

#[derive(Clone)]
pub struct TrackedTx {
    pub id: String,
    pub pkg_id: String,
    pub parent_id: String,
    pub name: String,
    pub icon: Option<RawIcon>,
    pub progress: f32,
    pub status: TxStatus,
    pub tx_type: String,
    pub error: String,
    pub installed_after: bool,
    pub refresh_detail: bool,
    pub refresh_extensions: bool,
    pub saved_pkg: Option<SavedPkgData>,
}

pub type TxStore = Arc<Mutex<Vec<TrackedTx>>>;

pub fn has_ongoing_transaction_for_package(store: &TxStore, pkg_id: &str) -> bool {
    let s = store.lock().unwrap();
    s.iter().any(|tx| {
        tx.pkg_id == pkg_id && (tx.status == TxStatus::Pending || tx.status == TxStatus::Running)
    })
}

pub fn progress_for_package(store: &TxStore, pkg_id: &str) -> f32 {
    let s = store.lock().unwrap();
    s.iter()
        .find(|tx| tx.pkg_id == pkg_id && (tx.status == TxStatus::Pending || tx.status == TxStatus::Running))
        .map(|tx| tx.progress)
        .unwrap_or(0.0)
}

pub fn transaction_id_for_package(store: &TxStore, pkg_id: &str) -> String {
    let s = store.lock().unwrap();
    s.iter()
        .find(|tx| tx.pkg_id == pkg_id && (tx.status == TxStatus::Pending || tx.status == TxStatus::Running))
        .map(|tx| tx.id.clone())
        .unwrap_or_default()
}

pub fn add_to_available_updates(
    app: &crate::AppWindow,
    pkg: SavedPkgData,
    icon: Option<&RawIcon>,
) {
    let model = app.get_available_updates();
    let mut items: Vec<crate::PackageItem> =
        (0..model.row_count()).filter_map(|i| model.row_data(i)).collect();
    if !items.iter().any(|p| p.id.as_str() == pkg.id.as_str()) {
        items.push(crate::PackageItem {
            id: pkg.id.into(),
            name: pkg.name.into(),
            version: pkg.version.into(),
            description: pkg.description.into(),
            installed: pkg.installed,
            icon: icon.map(|r| r.to_slint_image()).unwrap_or_default(),
            busy: false,
            progress: 0.0,
            transaction_id: Default::default(),
        });
        let count = items.len() as i32;
        app.set_available_updates(items.as_slice().into());
        app.set_update_count(count);
    }
}

pub fn remove_from_available_updates(app: &crate::AppWindow, pkg_id: &str) {
    let model = app.get_available_updates();
    let items: Vec<crate::PackageItem> = (0..model.row_count())
        .filter_map(|i| model.row_data(i))
        .filter(|p| p.id.as_str() != pkg_id)
        .collect();
    let count = items.len() as i32;
    app.set_available_updates(items.as_slice().into());
    app.set_update_count(count);
}

pub fn update_package_installed(app: &crate::AppWindow, pkg_id: &str, installed: bool) {
    let model = app.get_packages();
    let count = model.row_count();
    let mut items: Vec<crate::PackageItem> = (0..count).filter_map(|i| model.row_data(i)).collect();
    for item in &mut items {
        if item.id == pkg_id {
            item.installed = installed;
        }
    }
    app.set_packages(items.as_slice().into());
}

pub fn remove_from_packages_list(app: &crate::AppWindow, pkg_id: &str) {
    let model = app.get_packages();
    let items: Vec<crate::PackageItem> = (0..model.row_count())
        .filter_map(|i| model.row_data(i))
        .filter(|p| p.id.as_str() != pkg_id)
        .collect();
    app.set_packages(items.as_slice().into());
}

pub fn push_transactions_to_ui(store: TxStore, app_weak: &slint::Weak<crate::AppWindow>) {
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

    let _ = app_weak.upgrade_in_event_loop(move |app| {
        // Build a fallback icon map from already-loaded package list icons.
        // This covers the case where a fresh install has no local icon yet.
        let pkg_model = app.get_packages();
        let pkg_icons: std::collections::HashMap<String, slint::Image> = (0..pkg_model.row_count())
            .filter_map(|i| pkg_model.row_data(i))
            .map(|p| (p.id.to_string(), p.icon))
            .collect();

        fn to_slint_items(
            txs: Vec<TrackedTx>,
            pkg_icons: &std::collections::HashMap<String, slint::Image>,
        ) -> Vec<crate::TransactionItem> {
            txs.into_iter()
                .map(|tx| {
                    let icon = tx.icon.as_ref().map(|r| r.to_slint_image())
                        .or_else(|| pkg_icons.get(&tx.pkg_id).cloned())
                        .unwrap_or_default();
                    crate::TransactionItem {
                        id: tx.id.into(),
                        pkg_id: tx.pkg_id.into(),
                        parent_id: tx.parent_id.into(),
                        name: tx.name.into(),
                        icon,
                        progress: match tx.status {
                            TxStatus::Completed | TxStatus::Failed => 1.0,
                            _ => tx.progress,
                        },
                        status: match tx.status {
                            TxStatus::Pending => "pending",
                            TxStatus::Running => "running",
                            TxStatus::Completed => "completed",
                            TxStatus::Failed => "failed",
                        }
                        .into(),
                        tx_type: tx.tx_type.into(),
                        error: tx.error.into(),
                    }
                })
                .collect()
        }

        app.set_active_transactions(to_slint_items(active, &pkg_icons).as_slice().into());
        app.set_pending_transactions(to_slint_items(pending, &pkg_icons).as_slice().into());
        app.set_completed_transactions(to_slint_items(completed, &pkg_icons).as_slice().into());
        app.set_active_transaction_count(active_count);

        // Sync busy/progress into the packages list so search/installed rows show progress.
        {
            let model = app.get_packages();
            let count = model.row_count();
            if count > 0 {
                let mut items: Vec<crate::PackageItem> =
                    (0..count).filter_map(|i| model.row_data(i)).collect();
                let mut changed = false;
                for item in &mut items {
                    let new_busy = has_ongoing_transaction_for_package(&store, item.id.as_str());
                    let new_progress = if new_busy {
                        progress_for_package(&store, item.id.as_str())
                    } else {
                        0.0
                    };
                    let progress_drift = new_busy && (item.progress - new_progress).abs() > 0.005;
                    if item.busy != new_busy || progress_drift {
                        let new_tx_id = if new_busy {
                            transaction_id_for_package(&store, item.id.as_str())
                        } else {
                            String::new()
                        };
                        item.busy = new_busy;
                        item.progress = new_progress;
                        item.transaction_id = new_tx_id.into();
                        changed = true;
                    }
                }
                if changed {
                    app.set_packages(items.as_slice().into());
                }
            }
        }

        let detail = app.get_detail_app();
        let current_detail_pkg = if !detail.flatpak_id.is_empty() {
            detail.flatpak_id.to_string()
        } else {
            detail.appimage_id.to_string()
        };
        if !current_detail_pkg.is_empty() {
            let is_busy = has_ongoing_transaction_for_package(&store, &current_detail_pkg);
            let progress = progress_for_package(&store, &current_detail_pkg);
            let tx_id = transaction_id_for_package(&store, &current_detail_pkg);
            app.set_detail_busy(is_busy);
            app.set_detail_progress(progress);
            app.set_detail_transaction_id(tx_id.into());
        }
    });
}

pub async fn load_icon_for_pkg(pkg_id: &str, name: &str) -> Option<RawIcon> {
    if pkg_id.starts_with("lutris:") {
        return None;
    }
    if pkg_id.starts_with("appimage:") {
        let stem = pkg_id.trim_start_matches("appimage:").to_string();
        return tokio::task::spawn_blocking(move || icons::load_appimage_icon(None, &stem))
            .await
            .unwrap_or(None);
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

pub fn begin_transaction(
    pkg_id: String,
    parent_id: String,
    display_name: String,
    tx_type: String,
    installed_after: bool,
    refresh_detail: bool,
    refresh_extensions: bool,
    saved_pkg: Option<SavedPkgData>,
    store: TxStore,
    proxy_arc: Arc<Mutex<Option<ArcDaemonProxy<'static>>>>,
    app_weak: slint::Weak<crate::AppWindow>,
    rt_handle: tokio::runtime::Handle,
) {
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
            parent_id: parent_id.clone(),
            name: display_name.clone(),
            icon: None,
            progress: 0.0,
            status: TxStatus::Pending,
            tx_type: tx_type.clone(),
            error: String::new(),
            installed_after,
            refresh_detail,
            refresh_extensions,
            saved_pkg,
        });
        s.len() - 1
    };
    push_transactions_to_ui(store.clone(), &app_weak);

    let name_for_icon = display_name.clone();
    let pkg_id_for_icon = pkg_id.clone();

    rt_handle.spawn(async move {
        let tx_id_result = match crate::helpers::get_or_connect(&proxy_arc).await {
            Some(p) => match tx_type.as_str() {
                "install" => p.install_package(&pkg_id).await.ok(),
                "flatpakref" => p.install_flatpakref(&pkg_id).await.ok(),
                "remove" => p.remove_package(&pkg_id).await.ok(),
                "update" => p.update_package(&pkg_id).await.ok(),
                "bundle" => p.install_flatpak_bundle(&pkg_id).await.ok(),
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

pub async fn run_signal_listener(
    proxy: ArcDaemonProxy<'static>,
    store: TxStore,
    app_weak: slint::Weak<crate::AppWindow>,
    cache_invalidate_tx: tokio::sync::mpsc::Sender<()>,
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
                            e.refresh_extensions,
                            success,
                            e.name.clone(),
                            e.tx_type.clone(),
                        ))
                    } else {
                        None
                    }
                };

                push_transactions_to_ui(store.clone(), &app_weak);
                let _ = cache_invalidate_tx.try_send(());

                if let Some((pkg_id, installed_after, refresh_detail, refresh_extensions, ok, name, tx_type)) =
                    side_effect
                {
                    // Detect completed AppImage installs so we can patch the transaction
                    // entry with the real name/icon extracted by the daemon.
                    let appimage_stem = if ok && tx_type == "install" {
                        std::path::Path::new(&pkg_id)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .and_then(|file| {
                                if file.to_lowercase().ends_with(".appimage") {
                                    Some(file[..file.len() - ".appimage".len()].to_string())
                                } else {
                                    None
                                }
                            })
                    } else {
                        None
                    };

                    // Detect DEB/RPM/Arch installs for post-completion icon loading.
                    let native_name = if ok && tx_type == "install" && appimage_stem.is_none() {
                        let lower = pkg_id.to_lowercase();
                        if lower.ends_with(".deb")
                            || lower.ends_with(".rpm")
                            || lower.ends_with(".pkg.tar.zst")
                            || lower.ends_with(".pkg.tar.xz")
                        {
                            Some(name.clone())
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    let store_for_closure = store.clone();
                    let _ = app_weak.upgrade_in_event_loop(move |app| {
                        if ok {
                            if tx_type == "remove" && app.get_current_view() == "installed" {
                                remove_from_packages_list(&app, &pkg_id);
                            } else {
                                update_package_installed(&app, &pkg_id, installed_after);
                            }
                            if refresh_detail && app.get_current_view() == "detail" {
                                app.invoke_detail_requested(pkg_id.clone().into());
                            }
                            if refresh_extensions && app.get_current_view() == "detail" {
                                app.invoke_refresh_extensions_requested();
                            }
                            if tx_type == "update" {
                                remove_from_available_updates(&app, &pkg_id);
                            }
                        } else {

                        }
                        if tx_type == "update" {
                            app.set_updates_all_queued(false);
                        }
                        let detail = app.get_detail_app();
                        let current_detail_pkg = if !detail.flatpak_id.is_empty() {
                            detail.flatpak_id.to_string()
                        } else {
                            detail.appimage_id.to_string()
                        };
                        if !current_detail_pkg.is_empty() {
                            let is_busy = has_ongoing_transaction_for_package(
                                &store_for_closure,
                                &current_detail_pkg,
                            );
                            let progress = progress_for_package(&store_for_closure, &current_detail_pkg);
                            app.set_detail_busy(is_busy);
                            app.set_detail_progress(progress);
                        }
                    });

                    // For DEB/RPM/Arch installs, read the exported .desktop file to get
                    // the real display name and icon, since the install helper writes
                    // EXPORTED_APPS to the .info file pointing at the desktop entry.
                    if let Some(pkg_name) = native_name {
                        let tx_id_for_native = tx_id.clone();
                        let store_for_native = store.clone();
                        let app_weak_for_native = app_weak.clone();
                        tokio::spawn(async move {
                            let home = std::env::var("HOME")
                                .unwrap_or_else(|_| "/root".to_string());

                            // Try all three containers to find the .info file.
                            let (info_content, found_container) = {
                                let mut found = None;
                                let mut found_c = None;
                                for c in &["arc-debian", "arc-fedora", "arc-arch"] {
                                    let p = format!(
                                        "{}/.local/share/arc/packages/{}___{}.info",
                                        home, c, pkg_name
                                    );
                                    if let Ok(content) = tokio::fs::read_to_string(&p).await {
                                        found = Some(content);
                                        found_c = Some(c.to_string());
                                        break;
                                    }
                                }
                                (found, found_c)
                            };

                            let (real_name, desktop_name, icon_value, pkg_type_str) =
                                if let Some(ref info) = info_content {
                                    let real = info
                                        .lines()
                                        .find_map(|l| l.strip_prefix("REAL_NAME=").map(|v| v.to_string()))
                                        .filter(|n| !n.is_empty());

                                    let ptype = info
                                        .lines()
                                        .find_map(|l| l.strip_prefix("PKG_TYPE=").map(|v| v.to_string()))
                                        .filter(|n| !n.is_empty());

                                    // Read the first exported .desktop file for Name= and Icon=.
                                    let (dname, icon) = info
                                        .lines()
                                        .find_map(|l| l.strip_prefix("EXPORTED_APPS=").map(|v| v.to_string()))
                                        .and_then(|apps| apps.split_whitespace().next().map(|s| s.to_string()))
                                        .and_then(|app| {
                                            let dp = format!(
                                                "{}/.local/share/applications/{}.desktop",
                                                home, app
                                            );
                                            std::fs::read_to_string(dp).ok()
                                        })
                                        .map(|desktop| {
                                            let mut in_entry = false;
                                            let mut name = None::<String>;
                                            let mut icon = None::<String>;
                                            for line in desktop.lines() {
                                                let t = line.trim();
                                                if t.starts_with('[') {
                                                    in_entry = t == "[Desktop Entry]";
                                                    continue;
                                                }
                                                if !in_entry { continue; }
                                                if let Some(v) = line.strip_prefix("Name=") {
                                                    name = Some(v.to_string());
                                                } else if let Some(v) = line.strip_prefix("Icon=") {
                                                    icon = Some(v.to_string());
                                                }
                                            }
                                            (name, icon)
                                        })
                                        .unwrap_or((None, None));

                                    (real, dname, icon, ptype)
                                } else {
                                    (None, None, None, None)
                                };

                            let display_name = desktop_name.or(real_name).unwrap_or(pkg_name.clone());
                            let icon_str = icon_value;

                            // Reconstruct the proper distrobox package ID so that clicking
                            // the completed transaction navigates to the right detail page.
                            let new_pkg_id = match (found_container, pkg_type_str) {
                                (Some(container), Some(ptype)) => {
                                    Some(format!("distrobox:{}:{}:{}", container, pkg_name, ptype))
                                }
                                _ => None,
                            };

                            let icon_data = tokio::task::spawn_blocking(move || {
                                icons::load_distrobox_icon(icon_str.as_deref(), &pkg_name)
                            })
                            .await
                            .unwrap_or(None);

                            {
                                let mut s = store_for_native.lock().unwrap();
                                if let Some(e) = s.iter_mut().find(|e| e.id == tx_id_for_native) {
                                    e.name = display_name;
                                    if let Some(id) = new_pkg_id {
                                        e.pkg_id = id;
                                    }
                                    if let Some(data) = icon_data {
                                        e.icon = Some(data);
                                    }
                                }
                            }
                            push_transactions_to_ui(store_for_native, &app_weak_for_native);
                        });
                    }

                    // After a successful AppImage install, patch the completed
                    // transaction entry with the real name and icon from the .info file.
                    if let Some(stem) = appimage_stem {
                        let tx_id_for_update = tx_id.clone();
                        let store_for_update = store.clone();
                        let app_weak_for_update = app_weak.clone();
                        tokio::spawn(async move {
                            let home = std::env::var("HOME")
                                .unwrap_or_else(|_| "/root".to_string());
                            let info_path = std::path::PathBuf::from(home)
                                .join(".local/share/arc/appimages")
                                .join(format!("{}.info", stem));

                            let (real_name, icon_url) = tokio::fs::read_to_string(&info_path)
                                .await
                                .ok()
                                .map(|content| {
                                    let name = content.lines()
                                        .find_map(|l| l.strip_prefix("NAME=").map(|v| v.to_string()))
                                        .filter(|n| !n.is_empty());
                                    let icon = content.lines()
                                        .find_map(|l| l.strip_prefix("ICON_PATH=").map(|v| v.to_string()))
                                        .filter(|p| !p.is_empty());
                                    (name, icon)
                                })
                                .unwrap_or((None, None));

                            let stem_for_icon = stem.clone();
                            let new_icon = tokio::task::spawn_blocking(move || {
                                icons::load_appimage_icon(icon_url.as_deref(), &stem_for_icon)
                            })
                            .await
                            .unwrap_or(None);

                            {
                                let mut s = store_for_update.lock().unwrap();
                                if let Some(e) = s.iter_mut().find(|e| e.id == tx_id_for_update) {
                                    e.pkg_id = format!("appimage:{}", stem);
                                    if let Some(name) = real_name {
                                        e.name = name;
                                    }
                                    if let Some(icon_data) = new_icon {
                                        e.icon = Some(icon_data);
                                    }
                                }
                            }
                            push_transactions_to_ui(store_for_update, &app_weak_for_update);
                        });
                    }
                }
            }
        }
    }
}
