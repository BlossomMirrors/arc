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
    pub name: String,
    pub icon: Option<RawIcon>,
    pub progress: f32,
    pub status: TxStatus,
    pub tx_type: String,
    pub error: String,
    pub installed_after: bool,
    pub refresh_detail: bool,
    pub saved_pkg: Option<SavedPkgData>,
}

pub type TxStore = Arc<Mutex<Vec<TrackedTx>>>;

pub fn has_ongoing_transaction_for_package(store: &TxStore, pkg_id: &str) -> bool {
    let s = store.lock().unwrap();
    s.iter().any(|tx| {
        tx.pkg_id == pkg_id && (tx.status == TxStatus::Pending || tx.status == TxStatus::Running)
    })
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
        fn to_slint_items(txs: Vec<TrackedTx>) -> Vec<crate::TransactionItem> {
            txs.into_iter()
                .map(|tx| crate::TransactionItem {
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
        let current_detail_pkg = app.get_detail_app().flatpak_id.to_string();
        if !current_detail_pkg.is_empty() {
            let is_busy = has_ongoing_transaction_for_package(&store, &current_detail_pkg);
            app.set_detail_busy(is_busy);
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
    display_name: String,
    tx_type: String,
    installed_after: bool,
    refresh_detail: bool,
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
            name: display_name.clone(),
            icon: None,
            progress: 0.0,
            status: TxStatus::Pending,
            tx_type: tx_type.clone(),
            error: String::new(),
            installed_after,
            refresh_detail,
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

                if let Some((pkg_id, installed_after, refresh_detail, ok, _name, tx_type)) =
                    side_effect
                {
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
                            if tx_type == "update" {
                                remove_from_available_updates(&app, &pkg_id);
                            }
                        } else {

                        }
                        if tx_type == "update" {
                            app.set_updates_all_queued(false);
                        }
                        let current_detail_pkg = app.get_detail_app().flatpak_id.to_string();
                        if !current_detail_pkg.is_empty() {
                            let is_busy = has_ongoing_transaction_for_package(
                                &store_for_closure,
                                &current_detail_pkg,
                            );
                            app.set_detail_busy(is_busy);
                        }
                    });
                }
            }
        }
    }
}
