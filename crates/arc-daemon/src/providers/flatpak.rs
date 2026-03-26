use super::PackageProvider;
use async_trait::async_trait;
use libarc::{ArcError, Package, Provider};
use libflatpak::glib;
use libflatpak::prelude::*;
use tokio::process::Command;

pub struct FlatpakProvider;

impl FlatpakProvider {
    pub fn new() -> Self {
        Self
    }
}

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

fn parse_search_output(out: &str) -> Vec<Package> {
    out.lines()
        .filter_map(|line| {
            let mut c = line.splitn(4, '\t');
            let id = c.next().unwrap_or("").trim().to_string();
            if id.is_empty() || !id.contains('.') {
                return None;
            }
            Some(Package {
                id: id.clone(),
                name: c.next().unwrap_or("").trim().to_string(),
                version: c.next().unwrap_or("").trim().to_string(),
                description: c.next().unwrap_or("").trim().to_string(),
                provider: Provider::Flatpak,
                installed: false,
            })
        })
        .collect()
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
    }
}

impl FlatpakProvider {
    pub async fn fetch_all(&self) -> Result<Vec<Package>, ArcError> {
        let out = Command::new("flatpak")
            .args([
                "remote-ls",
                "--app",
                "--columns=application,name,version,description",
            ])
            .output()
            .await
            .map_err(|e| ArcError::ProviderError(format!("flatpak remote-ls: {}", e)))?;
        Ok(parse_search_output(&String::from_utf8_lossy(&out.stdout)))
    }
}

#[async_trait]
impl PackageProvider for FlatpakProvider {
    async fn search(&self, query: &str) -> Result<Vec<Package>, ArcError> {
        let out = Command::new("flatpak")
            .args([
                "search",
                "--columns=application,name,version,description",
                query,
            ])
            .output()
            .await
            .map_err(|e| ArcError::ProviderError(format!("flatpak search: {}", e)))?;
        Ok(parse_search_output(&String::from_utf8_lossy(&out.stdout)))
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
            let inst = installation_with_remote("flathub")?;
            let remote = inst
                .remote_by_name("flathub", cancel)
                .map_err(|e: glib::Error| ArcError::TransactionFailed(e.to_string()))?;
            let branch = remote
                .default_branch()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "stable".into());
            let remote_ref = inst
                .fetch_remote_ref_sync(
                    "flathub",
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
            tx.add_install("flathub", &full_ref, &[])
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
}
