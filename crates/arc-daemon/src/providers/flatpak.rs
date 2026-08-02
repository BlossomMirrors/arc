use super::{PackageProvider, Progress};
use crate::appstream_db::{entry_to_flatpak_package, AppStreamDb};
use async_trait::async_trait;
use libarc::{ArcError, Package, Provider, RemoteInfo};
use libflatpak::glib;
use libflatpak::prelude::*;
use std::sync::Arc;
use tokio::sync::{mpsc::UnboundedSender, Semaphore};
use tracing::warn;

pub struct FlatpakProvider {
    network_sem: Arc<Semaphore>,
}

impl FlatpakProvider {
    pub fn new() -> Self {
        Self {
            network_sem: Arc::new(Semaphore::new(1)),
        }
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

// Like installation_with_app but also finds Runtime refs (extensions/addons).
fn installation_with_ref(
    app_id: &str,
) -> Result<(libflatpak::Installation, libflatpak::InstalledRef), ArcError> {
    let cancel = libflatpak::gio::Cancellable::NONE;
    for inst in all_installations() {
        if let Ok(r) = inst.current_installed_app(app_id, cancel) {
            return Ok((inst, r));
        }
        let runtime_ref = inst
            .list_installed_refs_by_kind(libflatpak::RefKind::Runtime, cancel)
            .unwrap_or_default()
            .into_iter()
            .find(|r| r.name().map(|n| n == app_id).unwrap_or(false));
        if let Some(r) = runtime_ref {
            return Ok((inst, r));
        }
    }
    Err(ArcError::TransactionFailed(format!(
        "'{}' is not installed",
        app_id
    )))
}

// Runtime names are ambiguous when multiple branches are installed at once
// (e.g. org.freedesktop.Platform//22.08 alongside //23.08), which is the
// common case since different apps pin different runtime versions. Matching
// by name alone against list_installed_refs_by_kind can resolve to a branch
// that is already current, silently no-opping the real pending update.
// Prefer the ref that list_installed_refs_for_update actually flags first.
fn installation_with_updatable_ref(
    app_id: &str,
) -> Result<(libflatpak::Installation, libflatpak::InstalledRef), ArcError> {
    let cancel = libflatpak::gio::Cancellable::NONE;
    for inst in all_installations() {
        let updatable = inst
            .list_installed_refs_for_update(cancel)
            .unwrap_or_default()
            .into_iter()
            .find(|r| r.name().map(|n| n == app_id).unwrap_or(false));
        if let Some(r) = updatable {
            return Ok((inst, r));
        }
    }
    installation_with_ref(app_id)
}

// Extensions often live on branches named after the extension point version,
// so when branch guessing fails we scan the full remote ref list by name
fn find_remote_refs_by_name(
    inst: &libflatpak::Installation,
    remote_name: &str,
    name: &str,
    cancel: Option<&libflatpak::gio::Cancellable>,
) -> Vec<libflatpak::RemoteRef> {
    let default_arch = libflatpak::functions::default_arch();
    let mut refs: Vec<libflatpak::RemoteRef> = inst
        .list_remote_refs_sync(remote_name, cancel)
        .unwrap_or_default()
        .into_iter()
        .filter(|r| r.name().map(|n| n == name).unwrap_or(false))
        .collect();
    refs.sort_by_key(|r| r.arch() != default_arch);
    refs
}

fn run_install_transaction(
    inst: &libflatpak::Installation,
    remote_name: &str,
    full_ref: &str,
    progress_tx: Option<&UnboundedSender<Progress>>,
    cancel: Option<&libflatpak::gio::Cancellable>,
) -> Result<(), ArcError> {
    let tx = libflatpak::Transaction::for_installation(inst, cancel)
        .map_err(|e: glib::Error| ArcError::TransactionFailed(e.to_string()))?;
    tx.set_no_interaction(true);
    tx.add_install(remote_name, full_ref, &[])
        .map_err(|e: glib::Error| ArcError::TransactionFailed(e.to_string()))?;
    if let Some(sender) = progress_tx {
        let sender = sender.clone();
        tx.connect_new_operation(move |_, op, progress| {
            progress.set_update_frequency(500);
            let total = op.download_size();
            let sender = sender.clone();
            progress.connect_changed(move |p| {
                let _ = sender.send(Progress {
                    percent: p.progress().clamp(0, 100) as u8,
                    bytes_done: p.bytes_transferred(),
                    bytes_total: total,
                });
            });
        });
    }
    tx.run(cancel)
        .map_err(|e| ArcError::TransactionFailed(e.to_string()))
}

fn installed_ref_to_package(r: &libflatpak::InstalledRef) -> Package {
    let id = r.name().map(|s| s.to_string()).unwrap_or_default();
    let is_runtime = r.kind() == libflatpak::RefKind::Runtime;
    let name = r
        .appdata_name()
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| id.split('.').last().unwrap_or(&id).to_string());
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
        developer_name: None,
        homepage_url: None,
        content_rating: None,
        is_runtime,
        categories: vec![],
    }
}

impl FlatpakProvider {
    pub async fn fetch_all(&self) -> Result<Vec<Package>, ArcError> {
        let _permit = self
            .network_sem
            .acquire()
            .await
            .map_err(|e| ArcError::ProviderError(e.to_string()))?;
        tokio::task::spawn_blocking(|| -> Result<Vec<Package>, ArcError> {
            let cancel = libflatpak::gio::Cancellable::NONE;
            let db = AppStreamDb::get();

            // collect installed app ids so we can mark them correctly in search results
            let mut installed_ids: std::collections::HashSet<String> =
                std::collections::HashSet::new();
            for inst in all_installations() {
                let refs = inst
                    .list_installed_refs_by_kind(libflatpak::RefKind::App, cancel)
                    .unwrap_or_default();
                for r in &refs {
                    if let Some(name) = r.name() {
                        installed_ids.insert(name.to_string());
                    }
                }
            }

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
                .filter_map(|id| {
                    let is_installed = installed_ids.contains(&id);
                    if let Some(entry) = db.find_by_id(&id) {
                        Some(entry_to_flatpak_package(entry, is_installed))
                    } else if is_installed {
                        // Installed but absent from any catalog — try the exported metainfo.
                        let pkg = db
                            .load_from_exported_metainfo(&id)
                            .map(|e| entry_to_flatpak_package(e, true))
                            .unwrap_or_else(|| Package {
                                name: id.clone(),
                                id: id.clone(),
                                version: String::new(),
                                description: String::new(),
                                provider: libarc::Provider::Flatpak,
                                installed: true,
                                icon_url: None,
                                remote: None,
                                screenshots: vec![],
                                developer_name: None,
                                homepage_url: None,
                                content_rating: None,
                                is_runtime: false,
                                categories: vec![],
                            });
                        Some(pkg)
                    } else {
                        // Not installed and no catalog entry — skip so it doesn't
                        // show up as a nameless ghost in search results.
                        None
                    }
                })
                .collect())
        })
        .await
        .map_err(|e| ArcError::ProviderError(e.to_string()))?
    }

    pub async fn search_category(&self, category: &str) -> Result<Vec<Package>, ArcError> {
        let category = category.to_string();
        tokio::task::spawn_blocking(move || -> Result<Vec<Package>, ArcError> {
            let cancel = libflatpak::gio::Cancellable::NONE;
            let mut installed_ids: std::collections::HashSet<String> =
                std::collections::HashSet::new();
            for inst in all_installations() {
                let refs = inst
                    .list_installed_refs_by_kind(libflatpak::RefKind::App, cancel)
                    .unwrap_or_default();
                for r in &refs {
                    if let Some(name) = r.name() {
                        installed_ids.insert(name.to_string());
                    }
                }
            }
            Ok(AppStreamDb::get()
                .get_apps_by_category(&category)
                .into_iter()
                .map(|e| {
                    let is_installed = installed_ids.contains(&e.id);
                    entry_to_flatpak_package(e, is_installed)
                })
                .collect())
        })
        .await
        .map_err(|e| ArcError::ProviderError(e.to_string()))?
    }

    pub async fn list_extensions(&self, app_id: &str) -> Result<Vec<Package>, ArcError> {
        let app_id = app_id.to_string();
        let _permit = self
            .network_sem
            .acquire()
            .await
            .map_err(|e| ArcError::ProviderError(e.to_string()))?;
        tokio::task::spawn_blocking(move || -> Result<Vec<Package>, ArcError> {
            let cancel = libflatpak::gio::Cancellable::NONE;
            let prefix = format!("{}.", app_id);

            fn should_filter(suffix: &str) -> bool {
                let l = suffix.to_lowercase();
                l.contains("debug") || l.contains("sources") || l.contains("locale")
            }

            fn fallback_name(suffix: &str) -> String {
                const SKIP: &[&str] = &[
                    "Utility",
                    "CompatibilityTool",
                    "Extension",
                    "Plugin",
                    "Addon",
                    "Compat",
                    "GL",
                    "GL32",
                    "Platform",
                ];
                let parts: Vec<&str> = suffix.split('.').collect();
                let parts: &[&str] = if parts.len() > 1 && SKIP.contains(&parts[0]) {
                    &parts[1..]
                } else {
                    &parts
                };
                let s = parts.join(" ").replace('_', " ").replace('-', " ");
                let mut chars = s.chars();
                match chars.next() {
                    None => suffix.to_string(),
                    Some(c) => c.to_uppercase().to_string() + chars.as_str(),
                }
            }

            // Collect installed extensions; grab appdata_name() while we have the InstalledRef
            let mut installed_names: std::collections::HashMap<String, String> =
                std::collections::HashMap::new();
            for inst in all_installations() {
                for r in inst
                    .list_installed_refs_by_kind(libflatpak::RefKind::Runtime, cancel)
                    .unwrap_or_default()
                {
                    let Some(name) = r.name() else { continue };
                    if !name.starts_with(&prefix) {
                        continue;
                    }
                    let suffix = name.strip_prefix(&prefix).unwrap_or(&name);
                    if should_filter(suffix) {
                        continue;
                    }
                    let display = r
                        .appdata_name()
                        .map(|s| s.to_string())
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| fallback_name(suffix));
                    installed_names.insert(name.to_string(), display);
                }
            }

            let db = AppStreamDb::try_get();
            let mut extensions: Vec<Package> = Vec::new();
            let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

            for inst in all_installations() {
                for remote in inst.list_remotes(cancel).unwrap_or_default() {
                    let Some(remote_name) = remote.name() else {
                        continue;
                    };
                    for r in inst
                        .list_remote_refs_sync(remote_name.as_str(), cancel)
                        .unwrap_or_default()
                    {
                        if r.kind() != libflatpak::RefKind::Runtime {
                            continue;
                        }
                        let Some(name) = r.name() else { continue };
                        if !name.starts_with(&prefix) {
                            continue;
                        }
                        if !seen.insert(name.to_string()) {
                            continue;
                        }
                        let suffix = name.strip_prefix(&prefix).unwrap_or(&name);
                        if should_filter(suffix) {
                            continue;
                        }
                        let installed = installed_names.contains_key(&name.to_string());
                        let display = installed_names
                            .get(&name.to_string())
                            .cloned()
                            .or_else(|| db.find_by_id(&name).map(|e| e.name))
                            .unwrap_or_else(|| fallback_name(suffix));
                        extensions.push(Package {
                            id: name.to_string(),
                            name: display,
                            version: r.branch().map(|s| s.to_string()).unwrap_or_default(),
                            description: String::new(),
                            provider: libarc::Provider::Flatpak,
                            installed,
                            icon_url: None,
                            remote: Some(remote_name.to_string()),
                            screenshots: vec![],
                            developer_name: None,
                            homepage_url: None,
                            content_rating: None,
                            is_runtime: false,
                            categories: vec![],
                        });
                    }
                }
            }

            // include installed extensions not found in any remote listing
            for (id, display) in &installed_names {
                if !seen.contains(id) {
                    extensions.push(Package {
                        id: id.clone(),
                        name: display.clone(),
                        version: String::new(),
                        description: String::new(),
                        provider: libarc::Provider::Flatpak,
                        installed: true,
                        icon_url: None,
                        remote: None,
                        screenshots: vec![],
                        developer_name: None,
                        homepage_url: None,
                        content_rating: None,
                        is_runtime: false,
                        categories: vec![],
                    });
                }
            }

            Ok(extensions)
        })
        .await
        .map_err(|e| ArcError::ProviderError(e.to_string()))?
    }

    pub async fn get_app_info(&self, app_id: &str) -> Result<Option<Package>, ArcError> {
        let app_id = app_id.to_string();
        tokio::task::spawn_blocking(move || -> Result<Option<Package>, ArcError> {
            let cancel = libflatpak::gio::Cancellable::NONE;
            // same lookup list_installed() uses; installed_ref() with no
            // explicit arch/branch doesn't reliably match despite being
            // documented as optional
            let installed = all_installations().iter().any(|inst| {
                inst.list_installed_refs_by_kind(libflatpak::RefKind::App, cancel)
                    .unwrap_or_default()
                    .iter()
                    .any(|r| r.name().map(|n| n.to_string()).as_deref() == Some(app_id.as_str()))
            });
            // fast path: an installed app's own exported metainfo is a
            // single small local file, independent of the shared
            // AppStreamDb OnceLock's full-catalog parse — try it first so
            // clicking into something you already have installed doesn't
            // queue behind that bulk warm-up
            let entry = crate::appstream_db::load_local_metainfo(&app_id)
                .or_else(|| AppStreamDb::try_get().find_by_id(&app_id));
            Ok(entry.map(|e| entry_to_flatpak_package(e, installed)))
        })
        .await
        .map_err(|e| ArcError::ProviderError(e.to_string()))?
    }

    pub async fn install_with_progress(
        &self,
        app_id: &str,
        progress_tx: UnboundedSender<Progress>,
        gio_cancel: libflatpak::gio::Cancellable,
    ) -> Result<(), ArcError> {
        let app_id = app_id.to_string();
        tokio::task::spawn_blocking(move || -> Result<(), ArcError> {
            let cancel = Some(&gio_cancel);
            let primary_remote = AppStreamDb::try_get()
                .find_by_id(&app_id)
                .and_then(|e| e.remote);

            let mut remotes_to_try: Vec<String> = Vec::new();
            if let Some(ref r) = primary_remote {
                remotes_to_try.push(r.clone());
            }
            for inst in all_installations() {
                for remote in inst
                    .list_remotes(libflatpak::gio::Cancellable::NONE)
                    .unwrap_or_default()
                {
                    if let Some(name) = remote.name() {
                        let n = name.to_string();
                        if !remotes_to_try.contains(&n) {
                            remotes_to_try.push(n);
                        }
                    }
                }
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

                let mut refs_to_try: Vec<String> = Vec::new();
                for branch in &branch_candidates {
                    // Try App kind first (normal apps), then Runtime (extensions/addons)
                    let remote_ref = [libflatpak::RefKind::App, libflatpak::RefKind::Runtime]
                        .into_iter()
                        .find_map(|kind| {
                            inst.fetch_remote_ref_sync(
                                &remote_name,
                                kind,
                                &app_id,
                                None,
                                Some(branch.as_str()),
                                cancel,
                            )
                            .ok()
                        });
                    if let Some(f) = remote_ref.and_then(|r| r.format_ref()) {
                        let f = f.to_string();
                        if !refs_to_try.contains(&f) {
                            refs_to_try.push(f);
                        }
                    }
                }

                // Nothing on the guessed branches, look up the actual branch
                if refs_to_try.is_empty() {
                    for r in find_remote_refs_by_name(&inst, &remote_name, &app_id, cancel) {
                        if let Some(f) = r.format_ref() {
                            let f = f.to_string();
                            if !refs_to_try.contains(&f) {
                                refs_to_try.push(f);
                            }
                        }
                    }
                }

                if refs_to_try.is_empty() {
                    last_error = Some(ArcError::TransactionFailed(format!(
                        "{} not found in remote {}",
                        app_id, remote_name
                    )));
                    continue;
                }

                for full_ref in refs_to_try {
                    match run_install_transaction(
                        &inst,
                        &remote_name,
                        &full_ref,
                        Some(&progress_tx),
                        cancel,
                    ) {
                        Ok(_) => return Ok(()),
                        Err(e) => last_error = Some(e),
                    }
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

    pub async fn install_flatpakref_with_progress(
        &self,
        url: &str,
        progress_tx: UnboundedSender<Progress>,
        gio_cancel: libflatpak::gio::Cancellable,
    ) -> Result<(), ArcError> {
        let url = url.to_string();
        tokio::task::spawn_blocking(move || -> Result<(), ArcError> {
            let rt = tokio::runtime::Handle::current();
            let bytes: Vec<u8> = rt.block_on(async {
                if let Some(path) = url.strip_prefix("file://") {
                    tokio::fs::read(path)
                        .await
                        .map_err(|e| ArcError::ProviderError(e.to_string()))
                } else {
                    reqwest::get(&url)
                        .await
                        .map_err(|e| ArcError::ProviderError(e.to_string()))?
                        .bytes()
                        .await
                        .map(|b| b.to_vec())
                        .map_err(|e| ArcError::ProviderError(e.to_string()))
                }
            })?;

            // Parse the flatpakref INI to extract the repository fields so we can
            // add the remote explicitly before running the install transaction.
            let content = String::from_utf8_lossy(&bytes);
            let mut remote_url = String::new();
            let mut suggest_name = String::new();
            let mut gpg_key_b64 = String::new();
            for line in content.lines() {
                let line = line.trim();
                if let Some(v) = line.strip_prefix("Url=") {
                    remote_url = v.to_string();
                } else if let Some(v) = line.strip_prefix("SuggestRemoteName=") {
                    suggest_name = v.to_string();
                } else if let Some(v) = line.strip_prefix("GPGKey=") {
                    gpg_key_b64 = v.to_string();
                }
            }

            let cancel = Some(&gio_cancel);
            let inst = libflatpak::Installation::new_user(cancel)
                .map_err(|e: glib::Error| ArcError::ProviderError(e.to_string()))?;

            // Add the remote from the flatpakref if it is not already present.
            // GPG verification is handled by add_install_flatpakref which reads the
            // key from the flatpakref bytes, so we only need to register the URL here.
            if !remote_url.is_empty() {
                let remote_name = if suggest_name.is_empty() {
                    "flatpakref-remote".to_string()
                } else {
                    suggest_name.clone()
                };
                let already_exists = inst
                    .list_remotes(cancel)
                    .unwrap_or_default()
                    .iter()
                    .any(|r| r.name().map(|n| n == remote_name).unwrap_or(false));

                if !already_exists {
                    let remote = libflatpak::Remote::new(&remote_name);
                    remote.set_url(&remote_url);
                    if !gpg_key_b64.is_empty() {
                        let key_bytes = glib::base64_decode(&gpg_key_b64);
                        remote.set_gpg_verify(true);
                        let key_gbytes = glib::Bytes::from(key_bytes.as_slice());
                        remote.set_gpg_key(&key_gbytes);
                    }
                    inst.add_remote(&remote, true, cancel)
                        .map_err(|e: glib::Error| ArcError::ProviderError(e.to_string()))?;
                }
            }

            // Run the install transaction, add_install_flatpakref resolves the
            // exact ref (name + branch) and handles the RuntimeRepo if needed.
            let glib_bytes = glib::Bytes::from(bytes.as_slice());
            let tx = libflatpak::Transaction::for_installation(&inst, cancel)
                .map_err(|e: glib::Error| ArcError::TransactionFailed(e.to_string()))?;
            tx.set_no_interaction(true);
            tx.add_install_flatpakref(&glib_bytes)
                .map_err(|e: glib::Error| ArcError::TransactionFailed(e.to_string()))?;

            let progress_tx_for_tx = progress_tx.clone();
            tx.connect_new_operation(move |_, op, progress| {
                progress.set_update_frequency(500);
                let total = op.download_size();
                let sender = progress_tx_for_tx.clone();
                progress.connect_changed(move |p| {
                    let _ = sender.send(Progress {
                        percent: p.progress().clamp(0, 100) as u8,
                        bytes_done: p.bytes_transferred(),
                        bytes_total: total,
                    });
                });
            });

            tx.run(cancel)
                .map_err(|e: glib::Error| ArcError::TransactionFailed(e.to_string()))
        })
        .await
        .map_err(|e| ArcError::TransactionFailed(e.to_string()))?
    }

    pub async fn update_with_progress(
        &self,
        app_id: &str,
        progress_tx: UnboundedSender<Progress>,
        gio_cancel: libflatpak::gio::Cancellable,
    ) -> Result<(), ArcError> {
        let app_id = app_id.to_string();
        tokio::task::spawn_blocking(move || -> Result<(), ArcError> {
            let cancel = Some(&gio_cancel);
            let (inst, installed) = installation_with_updatable_ref(&app_id)?;
            let full_ref = installed
                .format_ref()
                .ok_or_else(|| ArcError::TransactionFailed("could not format ref".into()))?
                .to_string();

            // Runtime refs live in the system installation and need flatpak-system-helper
            // for privilege elevation. Delegate to subprocess so polkit handles it.
            if full_ref.starts_with("runtime/") {
                let _ = progress_tx.send(Progress::pct(10));
                let status = std::process::Command::new("flatpak")
                    .args(["update", "-y", "--noninteractive", &full_ref])
                    .status()
                    .map_err(|e| ArcError::TransactionFailed(e.to_string()))?;
                let _ = progress_tx.send(Progress::pct(100));
                return if status.success() {
                    Ok(())
                } else {
                    Err(ArcError::TransactionFailed(format!(
                        "flatpak update failed for {}",
                        app_id
                    )))
                };
            }

            let tx = libflatpak::Transaction::for_installation(&inst, cancel)
                .map_err(|e: glib::Error| ArcError::TransactionFailed(e.to_string()))?;
            tx.set_no_interaction(true);
            tx.add_update(&full_ref, &[], None)
                .map_err(|e: glib::Error| ArcError::TransactionFailed(e.to_string()))?;
            tx.connect_new_operation(move |_, op, progress| {
                progress.set_update_frequency(500);
                let total = op.download_size();
                let sender = progress_tx.clone();
                progress.connect_changed(move |p| {
                    let _ = sender.send(Progress {
                        percent: p.progress().clamp(0, 100) as u8,
                        bytes_done: p.bytes_transferred(),
                        bytes_total: total,
                    });
                });
            });
            tx.run(cancel)
                .map_err(|e: glib::Error| ArcError::TransactionFailed(e.to_string()))
        })
        .await
        .map_err(|e| ArcError::TransactionFailed(e.to_string()))?
    }

    const PROTECTED_REMOTES: &'static [&'static str] = &["flathub", "blossomos"];

    pub fn list_remotes() -> Vec<RemoteInfo> {
        let cancel = libflatpak::gio::Cancellable::NONE;
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        for inst in all_installations() {
            for remote in inst.list_remotes(cancel).unwrap_or_default() {
                let name = remote.name().map(|s| s.to_string()).unwrap_or_default();
                let url = remote.url().map(|s| s.to_string()).unwrap_or_default();
                if seen.insert(name.clone()) {
                    let protected = Self::PROTECTED_REMOTES.contains(&name.as_str());
                    out.push(RemoteInfo {
                        name,
                        url,
                        protected,
                    });
                }
            }
        }
        out
    }

    pub fn add_remote_from_url(name: &str, url: &str) -> Result<(), ArcError> {
        let cancel = libflatpak::gio::Cancellable::NONE;
        let inst = libflatpak::Installation::new_user(cancel)
            .map_err(|e: glib::Error| ArcError::ProviderError(e.to_string()))?;
        let already = inst
            .list_remotes(cancel)
            .unwrap_or_default()
            .iter()
            .any(|r| r.name().map(|n| n == name).unwrap_or(false));
        if already {
            return Err(ArcError::ProviderError(format!(
                "remote '{}' already exists",
                name
            )));
        }
        let remote = libflatpak::Remote::new(name);
        remote.set_url(url);
        inst.add_remote(&remote, false, cancel)
            .map_err(|e: glib::Error| ArcError::ProviderError(e.to_string()))
    }

    pub fn remove_remote(name: &str) -> Result<(), ArcError> {
        if Self::PROTECTED_REMOTES.contains(&name) {
            return Err(ArcError::ProviderError(format!(
                "'{}' is a protected repository and cannot be removed",
                name
            )));
        }
        let cancel = libflatpak::gio::Cancellable::NONE;
        for inst in all_installations() {
            let found = inst
                .list_remotes(cancel)
                .unwrap_or_default()
                .iter()
                .any(|r| r.name().map(|n| n == name).unwrap_or(false));
            if found {
                return inst
                    .remove_remote(name, cancel)
                    .map_err(|e: glib::Error| ArcError::ProviderError(e.to_string()));
            }
        }
        Err(ArcError::ProviderError(format!(
            "remote '{}' not found",
            name
        )))
    }

    pub fn add_remote_from_flatpakrepo(content: &str) -> Result<(), ArcError> {
        let mut title = String::new();
        let mut url = String::new();
        let mut gpg_key_b64 = String::new();
        for line in content.lines() {
            let line = line.trim();
            if let Some(v) = line.strip_prefix("Title=") {
                title = v.trim().to_string();
            } else if let Some(v) = line.strip_prefix("Url=") {
                url = v.trim().to_string();
            } else if let Some(v) = line.strip_prefix("GPGKey=") {
                gpg_key_b64 = v.trim().to_string();
            }
        }
        if url.is_empty() {
            return Err(ArcError::ProviderError(
                "No Url= found in .flatpakrepo".into(),
            ));
        }
        // Derive a safe remote name from the title
        let name = if title.is_empty() {
            "imported-repo".to_string()
        } else {
            title
                .to_lowercase()
                .replace(' ', "-")
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '-')
                .collect()
        };
        let cancel = libflatpak::gio::Cancellable::NONE;
        let inst = libflatpak::Installation::new_user(cancel)
            .map_err(|e: glib::Error| ArcError::ProviderError(e.to_string()))?;
        // Don't fail if the remote already exists, just update it
        let remote = libflatpak::Remote::new(&name);
        remote.set_url(&url);
        if !gpg_key_b64.is_empty() {
            let key_bytes = glib::base64_decode(&gpg_key_b64);
            remote.set_gpg_verify(true);
            let key_gbytes = glib::Bytes::from(key_bytes.as_slice());
            remote.set_gpg_key(&key_gbytes);
        }
        inst.add_remote(&remote, true, cancel)
            .map_err(|e: glib::Error| ArcError::ProviderError(e.to_string()))
    }

    pub async fn install_bundle_with_progress(
        &self,
        path: &str,
        progress_tx: UnboundedSender<Progress>,
        gio_cancel: libflatpak::gio::Cancellable,
    ) -> Result<(), ArcError> {
        let path = path.to_string();
        tokio::task::spawn_blocking(move || -> Result<(), ArcError> {
            let cancel = Some(&gio_cancel);
            let inst = libflatpak::Installation::new_user(cancel)
                .map_err(|e: glib::Error| ArcError::ProviderError(e.to_string()))?;
            let file = libflatpak::gio::File::for_path(&path);
            let tx = libflatpak::Transaction::for_installation(&inst, cancel)
                .map_err(|e: glib::Error| ArcError::TransactionFailed(e.to_string()))?;
            tx.set_no_interaction(true);
            tx.add_install_bundle(&file, None::<&glib::Bytes>)
                .map_err(|e: glib::Error| ArcError::TransactionFailed(e.to_string()))?;
            let progress_tx_for_tx = progress_tx.clone();
            tx.connect_new_operation(move |_, op, progress| {
                progress.set_update_frequency(500);
                let total = op.download_size();
                let sender = progress_tx_for_tx.clone();
                progress.connect_changed(move |p| {
                    let _ = sender.send(Progress {
                        percent: p.progress().clamp(0, 100) as u8,
                        bytes_done: p.bytes_transferred(),
                        bytes_total: total,
                    });
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
            Ok(AppStreamDb::get()
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
        let _permit = self
            .network_sem
            .acquire()
            .await
            .map_err(|e| ArcError::ProviderError(e.to_string()))?;
        tokio::task::spawn_blocking(|| -> Result<Vec<Package>, ArcError> {
            let cancel = libflatpak::gio::Cancellable::NONE;
            let mut packages = Vec::new();
            for inst in all_installations() {
                let mut attempt = 0;
                loop {
                    match inst.list_installed_refs_for_update(cancel) {
                        Ok(refs) => {
                            packages.extend(refs.iter().map(installed_ref_to_package));
                            break;
                        }
                        Err(e) if attempt < 2 => {
                            attempt += 1;
                            warn!("list_installed_refs_for_update failed, retrying: {e}");
                            std::thread::sleep(std::time::Duration::from_millis(300));
                        }
                        Err(e) => {
                            warn!("list_installed_refs_for_update failed: {e}");
                            break;
                        }
                    }
                }
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

            // a cold miss falls through to the flathub fallback below
            let primary_remote = AppStreamDb::try_get()
                .find_by_id(&package_id)
                .and_then(|e| e.remote);

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

                let mut refs_to_try: Vec<String> = Vec::new();
                for branch in &branch_candidates {
                    let remote_ref = [libflatpak::RefKind::App, libflatpak::RefKind::Runtime]
                        .into_iter()
                        .find_map(|kind| {
                            inst.fetch_remote_ref_sync(
                                &remote_name,
                                kind,
                                &package_id,
                                None,
                                Some(branch.as_str()),
                                cancel,
                            )
                            .ok()
                        });
                    if let Some(f) = remote_ref.and_then(|r| r.format_ref()) {
                        let f = f.to_string();
                        if !refs_to_try.contains(&f) {
                            refs_to_try.push(f);
                        }
                    }
                }

                // Nothing on the guessed branches, look up the actual branch
                if refs_to_try.is_empty() {
                    for r in find_remote_refs_by_name(&inst, &remote_name, &package_id, cancel) {
                        if let Some(f) = r.format_ref() {
                            let f = f.to_string();
                            if !refs_to_try.contains(&f) {
                                refs_to_try.push(f);
                            }
                        }
                    }
                }

                if refs_to_try.is_empty() {
                    last_error = Some(ArcError::TransactionFailed(format!(
                        "{} not found in remote {}",
                        package_id, remote_name
                    )));
                    continue;
                }

                for full_ref in refs_to_try {
                    match run_install_transaction(&inst, &remote_name, &full_ref, None, cancel) {
                        Ok(_) => return Ok(()),
                        Err(e) => last_error = Some(e),
                    }
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
            let (inst, installed) = installation_with_ref(&package_id)?;
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
            let (inst, installed) = installation_with_updatable_ref(&package_id)?;
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
