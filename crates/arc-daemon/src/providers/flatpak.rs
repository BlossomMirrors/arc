use super::PackageProvider;
use crate::appstream_db::{entry_to_flatpak_package, AppStreamDb};
use async_trait::async_trait;
use libarc::{ArcError, Package, Provider};
use libflatpak::glib;
use libflatpak::prelude::*;

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
    }
}

impl FlatpakProvider {
    pub async fn fetch_all(&self) -> Result<Vec<Package>, ArcError> {
        // libflatpak uses glib under the hood which is not tokio aware, so all (we hate glib btw)
        // calls to it have to go through spawn_blocking or they'll block the runtime
        tokio::task::spawn_blocking(|| -> Result<Vec<Package>, ArcError> {
            let cancel = libflatpak::gio::Cancellable::NONE;
            let db = AppStreamDb::load_flatpak();

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
            Ok(AppStreamDb::load_flatpak()
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
            Ok(AppStreamDb::load_flatpak()
                .find_by_id(&app_id)
                .map(|e| entry_to_flatpak_package(e, false)))
        })
        .await
        .map_err(|e| ArcError::ProviderError(e.to_string()))?
    }
}

#[async_trait]
impl PackageProvider for FlatpakProvider {
    async fn search(&self, query: &str) -> Result<Vec<Package>, ArcError> {
        let query = query.to_string();
        tokio::task::spawn_blocking(move || -> Result<Vec<Package>, ArcError> {
            Ok(AppStreamDb::load_flatpak()
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
            let db = AppStreamDb::load_flatpak();
            let remote_name = db
                .find_by_id(&package_id)
                .and_then(|e| e.remote)
                .unwrap_or_else(|| "flathub".to_string());

            let inst = installation_with_remote(&remote_name)?;
            let remote = inst
                .remote_by_name(&remote_name, cancel)
                .map_err(|e: glib::Error| ArcError::TransactionFailed(e.to_string()))?;
            let branch = remote
                .default_branch()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "stable".into());
            let remote_ref = inst
                .fetch_remote_ref_sync(
                    &remote_name,
                    libflatpak::RefKind::App,
                    &package_id,
                    None,
                    Some(branch.as_str()),
                    cancel,
                )
                .map_err(|e: glib::Error| ArcError::TransactionFailed(e.to_string()))?;
            let full_ref = remote_ref
                .format_ref()
                .ok_or_else(|| ArcError::TransactionFailed("could not format ref".into()))?;
            let tx = libflatpak::Transaction::for_installation(&inst, cancel)
                .map_err(|e: glib::Error| ArcError::TransactionFailed(e.to_string()))?;
            tx.set_no_interaction(true);
            tx.add_install(&remote_name, &full_ref, &[])
                .map_err(|e: glib::Error| ArcError::TransactionFailed(e.to_string()))?;
            tx.run(cancel)
                .map_err(|e: glib::Error| ArcError::TransactionFailed(e.to_string()))
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
