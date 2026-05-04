use super::PackageProvider;
use crate::appstream_db::{entry_to_flatpak_package, AppStreamDb};
use async_trait::async_trait;
use libarc::{ArcError, Package, Provider};
use libflatpak::glib;
use libflatpak::prelude::*;
use tokio::sync::mpsc::UnboundedSender;

pub struct FlatpakProvider;

impl FlatpakProvider {
    pub fn new() -> Self {
        Self
    }
}

// flatpak can have multiple "installations", the system one (all users) and
// a per user one under ~/.local/share/flatpak. we want to cover both
fn all_installations() -> Vec<libflatpak::Installation> {
    let cancel = libflatpak::gio::Cancellable::NONE;
    let mut out = Vec::new();
    if let Ok(i) = libflatpak::Installation::new_user(cancel) {
        out.push(i);
    }
    if let Ok(i) = libflatpak::Installation::new_system(cancel) {
        out.push(i);
    }
    out
}

fn installation_with_remote(remote: &str) -> Result<libflatpak::Installation, ArcError> {
    let cancel = libflatpak::gio::Cancellable::NONE;
    for inst in all_installations() {
        let remotes = inst.list_remotes(cancel).unwrap_or_default();
        if remotes
            .iter()
            .any(|r| r.name().map(|n| n == remote).unwrap_or(false))
        {
            return Ok(inst);
        }
    }
    Err(ArcError::ProviderError(format!(
        "remote '{}' not found",
        remote
    )))
}

fn installation_with_app(
    app_id: &str,
) -> Result<(libflatpak::Installation, libflatpak::InstalledRef), ArcError> {
    let cancel = libflatpak::gio::Cancellable::NONE;
    for inst in all_installations() {
        if let Ok(r) = inst.current_installed_app(app_id, cancel) {
            return Ok((inst, r));
        }
    }
    Err(ArcError::TransactionFailed(format!(
        "'{}' is not installed",
        app_id
    )))
}

fn installed_ref_to_package(r: &libflatpak::InstalledRef) -> Package {
    let id = r.name().map(|s| s.to_string()).unwrap_or_default();
    let name = r
        .appdata_name()
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| id.clone());
    Package {
        id: id.clone(),
        name,
        version: r
            .appdata_version()
            .or_else(|| r.branch())
            .map(|s| s.to_string())
            .unwrap_or_default(),
        description: r
            .appdata_summary()
            .map(|s| s.to_string())
            .unwrap_or_default(),
        provider: Provider::Flatpak,
        installed: true,
        icon_url: None,
        remote: r.origin().map(|s| s.to_string()),
        screenshots: vec![],
    }
}

impl FlatpakProvider {
    pub async fn fetch_all(&self) -> Result<Vec<Package>, ArcError> {
        // libflatpak uses glib under the hood which is not tokio aware, so all (we hate glib btw)
        // calls to it have to go through spawn_blocking or they'll block the runtime
        tokio::task::spawn_blocking(|| -> Result<Vec<Package>, ArcError> {
            let cancel = libflatpak::gio::Cancellable::NONE;
            let db = AppStreamDb::get_static();

            // collect unique app ids from all remotes across all installations
            let mut app_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
            for inst in all_installations() {
                let remotes = inst.list_remotes(cancel).unwrap_or_default();
                for remote in remotes {
                    let Some(remote_name) = remote.name() else {
                        continue;
                    };
                    let refs = inst
                        .list_remote_refs_sync(remote_name.as_str(), cancel)
                        .unwrap_or_default();
                    for r in refs {
                        if r.kind() == libflatpak::RefKind::App {
                            if let Some(name) = r.name() {
                                app_ids.insert(name.to_string());
                            }
                        }
                    }
                }
            }

            Ok(app_ids
                .into_iter()
                .map(|id| {
                    // enrich with appstream metadata if we have it, otherwise
                    // just return a bare package with the id as the name
                    db.find_by_id(&id)
                        .map(|e| entry_to_flatpak_package(e, false))
                        .unwrap_or_else(|| Package {
                            name: id.clone(),
                            id: id.clone(),
                            version: String::new(),
                            description: String::new(),
                            provider: libarc::Provider::Flatpak,
                            installed: false,
                            icon_url: None,
                            remote: None,
                            screenshots: vec![],
                        })
                })
                .collect())
        })
        .await
        .map_err(|e| ArcError::ProviderError(e.to_string()))?
    }

    pub async fn search_category(&self, category: &str) -> Result<Vec<Package>, ArcError> {
        let category = category.to_string();
        tokio::task::spawn_blocking(move || -> Result<Vec<Package>, ArcError> {
            Ok(AppStreamDb::get_static()
                .get_apps_by_category(&category)
                .into_iter()
                .map(|e| entry_to_flatpak_package(e, false))
                .collect())
        })
        .await
        .map_err(|e| ArcError::ProviderError(e.to_string()))?
    }

    pub async fn get_app_info(&self, app_id: &str) -> Result<Option<Package>, ArcError> {
        let app_id = app_id.to_string();
        tokio::task::spawn_blocking(move || -> Result<Option<Package>, ArcError> {
            Ok(AppStreamDb::get_static()
                .find_by_id(&app_id)
                .map(|e| entry_to_flatpak_package(e, false)))
        })
        .await
        .map_err(|e| ArcError::ProviderError(e.to_string()))?
    }

    pub async fn install_with_progress(
        &self,
        app_id: &str,
        progress_tx: UnboundedSender<u8>,
    ) -> Result<(), ArcError> {
        let app_id = app_id.to_string();
        tokio::task::spawn_blocking(move || -> Result<(), ArcError> {
            let cancel = libflatpak::gio::Cancellable::NONE;
            let db = AppStreamDb::get_static();

            // Build a list of remotes to try: primary from AppStream DB, then flathub as fallback
            let primary_remote = db.find_by_id(&app_id).and_then(|e| e.remote);

            let mut remotes_to_try = Vec::new();
            if let Some(remote) = &primary_remote {
                remotes_to_try.push(remote.clone());
            }
            // Always try flathub as fallback if it's not the primary
            if primary_remote.as_deref() != Some("flathub") {
                remotes_to_try.push("flathub".to_string());
            }

            // Branches to try for each remote
            let branches_to_try = ["stable", "master", "beta", "main"];

            let mut last_error = None;

            for remote_name in remotes_to_try {
                let inst = match installation_with_remote(&remote_name) {
                    Ok(i) => i,
                    Err(e) => {
                        last_error = Some(e);
                        continue;
                    }
                };

                let remote = match inst.remote_by_name(&remote_name, cancel) {
                    Ok(r) => r,
                    Err(e) => {
                        last_error = Some(ArcError::TransactionFailed(e.to_string()));
                        continue;
                    }
                };

                // Try multiple branches: start with remote's default, then try common alternatives
                let mut branch_candidates = Vec::new();
                if let Some(default_branch) = remote.default_branch().filter(|s| !s.is_empty()) {
                    branch_candidates.push(default_branch.to_string());
                }
                for branch in branches_to_try {
                    if !branch_candidates.iter().any(|b| b == branch) {
                        branch_candidates.push(branch.to_string());
                    }
                }

                let mut branch_tried = false;
                for branch in branch_candidates {
                    branch_tried = true;
                    let remote_ref = match inst.fetch_remote_ref_sync(
                        &remote_name,
                        libflatpak::RefKind::App,
                        &app_id,
                        None,
                        Some(branch.as_str()),
                        cancel,
                    ) {
                        Ok(r) => r,
                        Err(e) => {
                            last_error = Some(ArcError::TransactionFailed(e.to_string()));
                            continue;
                        }
                    };

                    let full_ref = remote_ref.format_ref().ok_or_else(|| {
                        ArcError::TransactionFailed("could not format ref".into())
                    })?;
                    let tx = libflatpak::Transaction::for_installation(&inst, cancel)
                        .map_err(|e: glib::Error| ArcError::TransactionFailed(e.to_string()))?;
                    tx.set_no_interaction(true);
                    tx.add_install(&remote_name, &full_ref, &[])
                        .map_err(|e: glib::Error| ArcError::TransactionFailed(e.to_string()))?;
                    let progress_tx_for_tx = progress_tx.clone();
                    tx.connect_new_operation(move |_, _op, progress| {
                        progress.set_update_frequency(1500);
                        let sender = progress_tx_for_tx.clone();
                        progress.connect_changed(move |p| {
                            let _ = sender.send(p.progress().clamp(0, 100) as u8);
                        });
                    });

                    match tx.run(cancel) {
                        Ok(_) => return Ok(()),
                        Err(e) => {
                            last_error = Some(ArcError::TransactionFailed(e.to_string()));
                            continue;
                        }
                    }
                }

                if !branch_tried {
                    last_error = Some(ArcError::TransactionFailed(format!(
                        "No branches available for {} in remote {}",
                        app_id, remote_name
                    )));
                }
            }

            Err(last_error.unwrap_or_else(|| {
                ArcError::TransactionFailed(format!(
                    "Failed to install {} from any available remote",
                    app_id
                ))
            }))
        })
        .await
        .map_err(|e| ArcError::TransactionFailed(e.to_string()))?
    }

    pub async fn update_with_progress(
        &self,
        app_id: &str,
        progress_tx: UnboundedSender<u8>,
    ) -> Result<(), ArcError> {
        let app_id = app_id.to_string();
        tokio::task::spawn_blocking(move || -> Result<(), ArcError> {
            let cancel = libflatpak::gio::Cancellable::NONE;
            let (inst, installed) = installation_with_app(&app_id)?;
            let full_ref = installed
                .format_ref()
                .ok_or_else(|| ArcError::TransactionFailed("could not format ref".into()))?;
            let tx = libflatpak::Transaction::for_installation(&inst, cancel)
                .map_err(|e: glib::Error| ArcError::TransactionFailed(e.to_string()))?;
            tx.set_no_interaction(true);
            tx.add_update(&full_ref, &[], None)
                .map_err(|e: glib::Error| ArcError::TransactionFailed(e.to_string()))?;
            tx.connect_new_operation(move |_, _op, progress| {
                progress.set_update_frequency(1500);
                let sender = progress_tx.clone();
                progress.connect_changed(move |p| {
                    let _ = sender.send(p.progress().clamp(0, 100) as u8);
                });
            });
            tx.run(cancel)
                .map_err(|e: glib::Error| ArcError::TransactionFailed(e.to_string()))
        })
        .await
        .map_err(|e| ArcError::TransactionFailed(e.to_string()))?
    }
}

#[async_trait]
impl PackageProvider for FlatpakProvider {
    async fn search(&self, query: &str) -> Result<Vec<Package>, ArcError> {
        let query = query.to_string();
        tokio::task::spawn_blocking(move || -> Result<Vec<Package>, ArcError> {
            Ok(AppStreamDb::get_static()
                .search_apps(&query)
                .into_iter()
                .map(|e| entry_to_flatpak_package(e, false))
                .collect())
        })
        .await
        .map_err(|e| ArcError::ProviderError(e.to_string()))?
    }

    async fn search_category(&self, category: &str) -> Result<Vec<Package>, ArcError> {
        self.search_category(category).await
    }

    async fn get_app_info(&self, app_id: &str) -> Result<Option<Package>, ArcError> {
        self.get_app_info(app_id).await
    }

    async fn list_installed(&self) -> Result<Vec<Package>, ArcError> {
        tokio::task::spawn_blocking(|| -> Result<Vec<Package>, ArcError> {
            let cancel = libflatpak::gio::Cancellable::NONE;
            let mut packages = Vec::new();
            for inst in all_installations() {
                let refs = inst
                    .list_installed_refs_by_kind(libflatpak::RefKind::App, cancel)
                    .unwrap_or_default();
                packages.extend(refs.iter().map(installed_ref_to_package));
            }
            Ok(packages)
        })
        .await
        .map_err(|e| ArcError::ProviderError(e.to_string()))?
    }

    async fn list_updates(&self) -> Result<Vec<Package>, ArcError> {
        tokio::task::spawn_blocking(|| -> Result<Vec<Package>, ArcError> {
            let cancel = libflatpak::gio::Cancellable::NONE;
            let mut packages = Vec::new();
            for inst in all_installations() {
                let refs = inst
                    .list_installed_refs_for_update(cancel)
                    .unwrap_or_default();
                packages.extend(
                    refs.iter()
                        .filter(|r| r.kind() == libflatpak::RefKind::App)
                        .map(installed_ref_to_package),
                );
            }
            Ok(packages)
        })
        .await
        .map_err(|e| ArcError::ProviderError(e.to_string()))?
    }

    async fn install(&self, package_id: &str) -> Result<(), ArcError> {
        let package_id = package_id.to_string();
        tokio::task::spawn_blocking(move || -> Result<(), ArcError> {
            let cancel = libflatpak::gio::Cancellable::NONE;

            // Get the package info to determine which remote it comes from
            let db = AppStreamDb::get_static();
            let primary_remote = db.find_by_id(&package_id).and_then(|e| e.remote);

            // Build a list of remotes to try: primary from AppStream DB, then flathub as fallback
            let mut remotes_to_try = Vec::new();
            if let Some(remote) = &primary_remote {
                remotes_to_try.push(remote.clone());
            }
            // Always try flathub as fallback if it's not the primary
            if primary_remote.as_deref() != Some("flathub") {
                remotes_to_try.push("flathub".to_string());
            }

            // Branches to try for each remote
            let branches_to_try = ["stable", "master", "beta", "main"];

            let mut last_error = None;

            for remote_name in remotes_to_try {
                let inst = match installation_with_remote(&remote_name) {
                    Ok(i) => i,
                    Err(e) => {
                        last_error = Some(e);
                        continue;
                    }
                };

                let remote = match inst.remote_by_name(&remote_name, cancel) {
                    Ok(r) => r,
                    Err(e) => {
                        last_error = Some(ArcError::TransactionFailed(e.to_string()));
                        continue;
                    }
                };

                // Try multiple branches: start with remote's default, then try common alternatives
                let mut branch_candidates = Vec::new();
                if let Some(default_branch) = remote.default_branch().filter(|s| !s.is_empty()) {
                    branch_candidates.push(default_branch.to_string());
                }
                for branch in branches_to_try {
                    if !branch_candidates.iter().any(|b| b == branch) {
                        branch_candidates.push(branch.to_string());
                    }
                }

                let mut branch_tried = false;
                for branch in branch_candidates {
                    branch_tried = true;
                    let remote_ref = match inst.fetch_remote_ref_sync(
                        &remote_name,
                        libflatpak::RefKind::App,
                        &package_id,
                        None,
                        Some(branch.as_str()),
                        cancel,
                    ) {
                        Ok(r) => r,
                        Err(e) => {
                            last_error = Some(ArcError::TransactionFailed(e.to_string()));
                            continue;
                        }
                    };

                    let full_ref = remote_ref.format_ref().ok_or_else(|| {
                        ArcError::TransactionFailed("could not format ref".into())
                    })?;
                    let tx = libflatpak::Transaction::for_installation(&inst, cancel)
                        .map_err(|e: glib::Error| ArcError::TransactionFailed(e.to_string()))?;
                    tx.set_no_interaction(true);
                    tx.add_install(&remote_name, &full_ref, &[])
                        .map_err(|e: glib::Error| ArcError::TransactionFailed(e.to_string()))?;

                    match tx.run(cancel) {
                        Ok(_) => return Ok(()),
                        Err(e) => {
                            last_error = Some(ArcError::TransactionFailed(e.to_string()));
                            continue;
                        }
                    }
                }

                if !branch_tried {
                    last_error = Some(ArcError::TransactionFailed(format!(
                        "No branches available for {} in remote {}",
                        package_id, remote_name
                    )));
                }
            }

            Err(last_error.unwrap_or_else(|| {
                ArcError::TransactionFailed(format!(
                    "Failed to install {} from any available remote",
                    package_id
                ))
            }))
        })
        .await
        .map_err(|e| ArcError::TransactionFailed(e.to_string()))?
    }

    async fn remove(&self, package_id: &str) -> Result<(), ArcError> {
        let package_id = package_id.to_string();
        tokio::task::spawn_blocking(move || -> Result<(), ArcError> {
            let cancel = libflatpak::gio::Cancellable::NONE;
            let (inst, installed) = installation_with_app(&package_id)?;
            let full_ref = installed
                .format_ref()
                .ok_or_else(|| ArcError::TransactionFailed("could not format ref".into()))?;
            let tx = libflatpak::Transaction::for_installation(&inst, cancel)
                .map_err(|e: glib::Error| ArcError::TransactionFailed(e.to_string()))?;
            tx.set_no_interaction(true);
            tx.add_uninstall(&full_ref)
                .map_err(|e: glib::Error| ArcError::TransactionFailed(e.to_string()))?;
            tx.run(cancel)
                .map_err(|e: glib::Error| ArcError::TransactionFailed(e.to_string()))
        })
        .await
        .map_err(|e| ArcError::TransactionFailed(e.to_string()))?
    }

    async fn update(&self, package_id: &str) -> Result<(), ArcError> {
        let package_id = package_id.to_string();
        tokio::task::spawn_blocking(move || -> Result<(), ArcError> {
            let cancel = libflatpak::gio::Cancellable::NONE;
            let (inst, installed) = installation_with_app(&package_id)?;
            let full_ref = installed
                .format_ref()
                .ok_or_else(|| ArcError::TransactionFailed("could not format ref".into()))?;
            let tx = libflatpak::Transaction::for_installation(&inst, cancel)
                .map_err(|e: glib::Error| ArcError::TransactionFailed(e.to_string()))?;
            tx.set_no_interaction(true);
            tx.add_update(&full_ref, &[], None)
                .map_err(|e: glib::Error| ArcError::TransactionFailed(e.to_string()))?;
            tx.run(cancel)
                .map_err(|e: glib::Error| ArcError::TransactionFailed(e.to_string()))
        })
        .await
        .map_err(|e| ArcError::TransactionFailed(e.to_string()))?
    }

    async fn run(&self, package_id: &str) -> Result<(), ArcError> {
        let package_id = package_id.to_string();
        tokio::task::spawn_blocking(move || -> Result<(), ArcError> {
            let _cancel = libflatpak::gio::Cancellable::NONE;
            let (_inst, _installed) = installation_with_app(&package_id)?;

            // Use flatpak run to launch the application
            let status = std::process::Command::new("flatpak")
                .args(["run", &package_id])
                .status()
                .map_err(|e| ArcError::TransactionFailed(e.to_string()))?;

            if !status.success() {
                return Err(ArcError::TransactionFailed(format!(
                    "Failed to run {}",
                    package_id
                )));
            }
            Ok(())
        })
        .await
        .map_err(|e| ArcError::TransactionFailed(e.to_string()))?
    }
}
