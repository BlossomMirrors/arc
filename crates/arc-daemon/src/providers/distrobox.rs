use super::PackageProvider;
use async_trait::async_trait;
use libarc::{ArcError, Package, Provider};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::process::Command;
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;
use tracing::info;
use uuid::Uuid;
extern crate libc;

const CONTAINER_DEB: &str = "arc-debian";
const CONTAINER_RPM: &str = "arc-fedora";
const CONTAINER_ARCH: &str = "arc-arch";

const IMAGE_DEB: &str = "quay.io/toolbx-images/debian-toolbox:13";
const IMAGE_RPM: &str = "registry.fedoraproject.org/fedora-toolbox:44";
const IMAGE_ARCH: &str = "docker.io/archlinux:latest";

#[derive(Debug, Clone, Copy, PartialEq)]
enum PkgType {
    Deb,
    Rpm,
    Pacman,
}

impl PkgType {
    fn as_str(self) -> &'static str {
        match self {
            PkgType::Deb => "deb",
            PkgType::Rpm => "rpm",
            PkgType::Pacman => "pacman",
        }
    }

    fn from_str(s: &str) -> Option<Self> {
        match s {
            "deb" => Some(PkgType::Deb),
            "rpm" => Some(PkgType::Rpm),
            "pacman" => Some(PkgType::Pacman),
            _ => None,
        }
    }
}

fn classify_file(path: &Path) -> Option<(String, String, PkgType)> {
    let name = path.file_name()?.to_str()?;
    if name.ends_with(".deb") {
        Some((CONTAINER_DEB.into(), IMAGE_DEB.into(), PkgType::Deb))
    } else if name.ends_with(".rpm") {
        Some((CONTAINER_RPM.into(), IMAGE_RPM.into(), PkgType::Rpm))
    } else if name.ends_with(".pkg.tar.xz") || name.ends_with(".pkg.tar.zst") {
        Some((CONTAINER_ARCH.into(), IMAGE_ARCH.into(), PkgType::Pacman))
    } else {
        None
    }
}

fn guess_pkg_name(path: &Path, pkg_type: PkgType) -> String {
    let base = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");
    match pkg_type {
        PkgType::Deb => base.split('_').next().unwrap_or(base).to_string(),
        PkgType::Rpm => {
            let no_ext = base.strip_suffix(".rpm").unwrap_or(base);
            strip_version_suffix(no_ext)
        }
        PkgType::Pacman => {
            let no_ext = base
                .strip_suffix(".pkg.tar.zst")
                .or_else(|| base.strip_suffix(".pkg.tar.xz"))
                .unwrap_or(base);
            // Arch format: <name>-<version>-<pkgrel>-<arch>
            // Strip the last 3 fields to recover the name. Searching for the first
            // digit-starting field fails for names like "1password" or "0ad".
            let parts: Vec<&str> = no_ext.split('-').collect();
            let name_end = parts.len().saturating_sub(3);
            if name_end > 0 {
                parts[..name_end].join("-")
            } else {
                parts[0].to_string()
            }
        }
    }
}

fn strip_version_suffix(s: &str) -> String {
    // "name-1.0-1.arch" → "name"
    if let Some(i) = s.find('-') {
        if s[i + 1..].starts_with(|c: char| c.is_ascii_digit()) {
            return s[..i].to_string();
        }
    }
    s.to_string()
}

pub struct DistroboxProvider {
    packages_dir: PathBuf,
    home: String,
}

impl DistroboxProvider {
    pub fn new() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        let data_dir = PathBuf::from(&home).join(".local/share/arc");
        let packages_dir = data_dir.join("packages");
        Self { packages_dir, home }
    }

    fn info_file(&self, container: &str, pkg_name: &str) -> PathBuf {
        self.packages_dir
            .join(format!("{}___{}.info", container, pkg_name))
    }

    async fn container_exists(&self, name: &str) -> bool {
        let Ok(out) = Command::new("distrobox")
            .args(["list", "--no-color"])
            .output()
            .await
        else {
            return false;
        };
        let stdout = String::from_utf8_lossy(&out.stdout);
        stdout.lines().skip(1).any(|line| {
            line.split('|')
                .nth(1)
                .map(|s| s.trim() == name)
                .unwrap_or(false)
        })
    }

    async fn ensure_container(
        &self,
        name: &str,
        image: &str,
        cancel_token: &CancellationToken,
    ) -> Result<(), ArcError> {
        if self.container_exists(name).await {
            return Ok(());
        }
        info!("Creating distrobox container {} ({})", name, image);
        let status = run_cancellable(
            Command::new("distrobox").args([
                "create", "--name", name, "--image", image, "--init", "--nvidia", "--yes",
            ]),
            cancel_token,
        )
        .await?;
        if !status.success() {
            return Err(ArcError::ProviderError(format!(
                "Failed to create container '{}'",
                name
            )));
        }
        // initialise the container so it is ready for use
        let _ = run_cancellable(
            Command::new("distrobox").args(["enter", name, "--", "true"]),
            cancel_token,
        )
        .await;
        info!("Container {} ready", name);
        Ok(())
    }

    async fn install_and_export(
        &self,
        container: &str,
        pkg_file: &Path,
        pkg_type: PkgType,
        guessed_name: &str,
        cancel_token: &CancellationToken,
    ) -> Result<(), ArcError> {
        fs::create_dir_all(&self.packages_dir)
            .await
            .map_err(|e| ArcError::ProviderError(e.to_string()))?;

        let work_dir =
            PathBuf::from(&self.home).join(format!(".arc-distrobox-{}", Uuid::new_v4().simple()));
        fs::create_dir_all(&work_dir)
            .await
            .map_err(|e| ArcError::ProviderError(e.to_string()))?;

        let helper_path = work_dir.join("install.sh");
        let export_log = work_dir.join("exported.log");
        let pkg_dest = work_dir.join(pkg_file.file_name().unwrap());

        fs::write(&helper_path, INSTALL_HELPER)
            .await
            .map_err(|e| ArcError::ProviderError(e.to_string()))?;
        Command::new("chmod")
            .args(["+x", helper_path.to_str().unwrap()])
            .status()
            .await
            .ok();
        fs::copy(pkg_file, &pkg_dest)
            .await
            .map_err(|e| ArcError::ProviderError(format!("copy package: {}", e)))?;

        let status = run_cancellable(
            Command::new("distrobox").args([
                "enter",
                container,
                "--",
                helper_path.to_str().unwrap(),
                pkg_dest.to_str().unwrap(),
                pkg_type.as_str(),
                export_log.to_str().unwrap(),
            ]),
            cancel_token,
        )
        .await;

        let log_content = fs::read_to_string(&export_log).await.unwrap_or_default();
        let _ = fs::remove_dir_all(&work_dir).await;

        let status = status?;
        if !status.success() {
            return Err(ArcError::ProviderError(format!(
                "Installation of {} failed",
                pkg_file.display()
            )));
        }

        let mut real_name = guessed_name.to_string();
        let mut apps: Vec<String> = Vec::new();
        let mut bins: Vec<String> = Vec::new();

        for line in log_content.lines() {
            if let Some(v) = line.strip_prefix("pkgname:") {
                real_name = v.to_string();
            } else if let Some(v) = line.strip_prefix("app:") {
                apps.push(v.to_string());
            } else if let Some(v) = line.strip_prefix("bin:") {
                bins.push(v.to_string());
            }
        }

        let info = format!(
            "GUESSED_NAME={}\nREAL_NAME={}\nPKG_TYPE={}\nCONTAINER={}\nEXPORTED_APPS={}\nEXPORTED_BINS={}\n",
            guessed_name,
            real_name,
            pkg_type.as_str(),
            container,
            apps.join(" "),
            bins.join(" "),
        );
        fs::write(self.info_file(container, guessed_name), info)
            .await
            .map_err(|e| ArcError::ProviderError(e.to_string()))?;

        Ok(())
    }

    async fn uninstall_package(
        &self,
        container: &str,
        guessed_name: &str,
        real_name: &str,
        pkg_type: PkgType,
        apps: &[String],
        bins: &[String],
    ) -> Result<(), ArcError> {
        let work_dir =
            PathBuf::from(&self.home).join(format!(".arc-distrobox-{}", Uuid::new_v4().simple()));
        fs::create_dir_all(&work_dir)
            .await
            .map_err(|e| ArcError::ProviderError(e.to_string()))?;

        let helper_path = work_dir.join("uninstall.sh");
        fs::write(&helper_path, UNINSTALL_HELPER)
            .await
            .map_err(|e| ArcError::ProviderError(e.to_string()))?;
        Command::new("chmod")
            .args(["+x", helper_path.to_str().unwrap()])
            .status()
            .await
            .ok();

        let status = Command::new("distrobox")
            .args([
                "enter",
                container,
                "--",
                helper_path.to_str().unwrap(),
                real_name,
                pkg_type.as_str(),
            ])
            .status()
            .await
            .map_err(|e| ArcError::ProviderError(format!("distrobox enter: {}", e)))?;

        let _ = fs::remove_dir_all(&work_dir).await;

        if !status.success() {
            return Err(ArcError::ProviderError(format!(
                "Uninstall of {} failed",
                real_name
            )));
        }

        let home = PathBuf::from(&self.home);
        for app in apps.iter().filter(|s| !s.is_empty()) {
            let _ = fs::remove_file(
                home.join(".local/share/applications")
                    .join(format!("{}.desktop", app)),
            )
            .await;
        }
        for bin in bins.iter().filter(|s| !s.is_empty()) {
            let _ = fs::remove_file(home.join(".local/bin").join(bin)).await;
        }
        let _ = fs::remove_file(self.info_file(container, guessed_name)).await;

        Ok(())
    }

    async fn existing_containers(&self) -> Vec<String> {
        let Ok(out) = Command::new("distrobox")
            .args(["list", "--no-color"])
            .output()
            .await
        else {
            return Vec::new();
        };
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .skip(1)
            .filter_map(|line| {
                line.split('|')
                    .nth(1)
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
            })
            .collect()
    }

    async fn read_installed(&self) -> Result<Vec<Package>, ArcError> {
        let mut entries = match fs::read_dir(&self.packages_dir).await {
            Ok(e) => e,
            Err(_) => return Ok(Vec::new()),
        };

        let live_containers = self.existing_containers().await;

        let mut packages = Vec::new();
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("info") {
                continue;
            }
            let content = match fs::read_to_string(&path).await {
                Ok(c) => c,
                Err(_) => continue,
            };

            // Find which container this package belongs to.
            let container = content
                .lines()
                .find_map(|l| l.strip_prefix("CONTAINER=").map(|v| v.to_string()))
                .unwrap_or_default();

            if !container.is_empty() && !live_containers.contains(&container) {
                // Container was deleted, remove the index entry and any exported files.
                let home = std::path::PathBuf::from(&self.home);
                for line in content.lines() {
                    if let Some(apps) = line.strip_prefix("EXPORTED_APPS=") {
                        for app in apps.split_whitespace().filter(|s| !s.is_empty()) {
                            let _ = fs::remove_file(
                                home.join(".local/share/applications")
                                    .join(format!("{}.desktop", app)),
                            )
                            .await;
                        }
                    } else if let Some(bins) = line.strip_prefix("EXPORTED_BINS=") {
                        for bin in bins.split_whitespace().filter(|s| !s.is_empty()) {
                            let _ = fs::remove_file(home.join(".local/bin").join(bin)).await;
                        }
                    }
                }
                let _ = fs::remove_file(&path).await;
                continue;
            }

            if let Some(pkg) = parse_info(&content, &self.home) {
                packages.push(pkg);
            }
        }
        Ok(packages)
    }

    pub async fn fetch_all(&self) -> Result<Vec<Package>, ArcError> {
        self.read_installed().await
    }
}

fn parse_info(content: &str, home: &str) -> Option<Package> {
    let mut guessed_name = String::new();
    let mut real_name = String::new();
    let mut pkg_type = String::new();
    let mut container = String::new();
    let mut exported_apps: Vec<String> = Vec::new();

    for line in content.lines() {
        if let Some(v) = line.strip_prefix("GUESSED_NAME=") {
            guessed_name = v.to_string();
        } else if let Some(v) = line.strip_prefix("REAL_NAME=") {
            real_name = v.to_string();
        } else if let Some(v) = line.strip_prefix("PKG_TYPE=") {
            pkg_type = v.to_string();
        } else if let Some(v) = line.strip_prefix("CONTAINER=") {
            container = v.to_string();
        } else if let Some(v) = line.strip_prefix("EXPORTED_APPS=") {
            exported_apps = v.split_whitespace().map(|s| s.to_string()).collect();
        }
    }

    if guessed_name.is_empty() || container.is_empty() {
        return None;
    }

    // Read the exported .desktop file for the real display name, icon, and comment.
    let (desktop_name, desktop_icon, desktop_comment) = exported_apps
        .first()
        .and_then(|app| {
            let path = format!("{}/.local/share/applications/{}.desktop", home, app);
            std::fs::read_to_string(path).ok()
        })
        .map(|desktop| {
            let mut in_entry = false;
            let mut name = None::<String>;
            let mut icon = None::<String>;
            let mut comment = None::<String>;
            for line in desktop.lines() {
                let t = line.trim();
                if t.starts_with('[') {
                    in_entry = t == "[Desktop Entry]";
                    continue;
                }
                if !in_entry {
                    continue;
                }
                if let Some(v) = line.strip_prefix("Name=") {
                    name = Some(v.to_string());
                } else if let Some(v) = line.strip_prefix("Icon=") {
                    icon = Some(v.to_string());
                } else if let Some(v) = line.strip_prefix("Comment=") {
                    comment = Some(v.to_string());
                }
            }
            (name, icon, comment)
        })
        .unwrap_or((None, None, None));

    let name = desktop_name.unwrap_or_else(|| {
        if real_name.is_empty() {
            guessed_name.clone()
        } else {
            real_name
        }
    });

    let container_label = match pkg_type.as_str() {
        "deb" => format!("Installed in Debian container ({})", container),
        "rpm" => format!("Installed in Fedora container ({})", container),
        "pacman" => format!("Installed in Arch container ({})", container),
        _ => format!("Installed in container ({})", container),
    };
    let description = match desktop_comment {
        Some(ref c) if !c.is_empty() => format!("{} — {}", c, container_label),
        _ => container_label,
    };

    Some(Package {
        id: format!("distrobox:{}:{}:{}", container, guessed_name, pkg_type),
        name,
        version: String::new(),
        description,
        provider: Provider::Distrobox,
        installed: true,
        icon_url: desktop_icon,
        remote: None,
        screenshots: vec![],
        developer_name: None,
        homepage_url: None,
        content_rating: None,
        is_runtime: false,
    })
}

#[async_trait]
impl PackageProvider for DistroboxProvider {
    async fn search(&self, query: &str) -> Result<Vec<Package>, ArcError> {
        let q = query.to_lowercase();
        let installed = self.read_installed().await?;
        Ok(installed
            .into_iter()
            .filter(|p| p.name.to_lowercase().contains(&q))
            .collect())
    }

    async fn list_installed(&self) -> Result<Vec<Package>, ArcError> {
        self.read_installed().await
    }

    async fn list_updates(&self) -> Result<Vec<Package>, ArcError> {
        Ok(Vec::new())
    }

    async fn install(&self, package_id: &str) -> Result<(), ArcError> {
        let cancel_token = CancellationToken::new();
        let path = PathBuf::from(package_id);
        let (container, image, pkg_type) = classify_file(&path).ok_or_else(|| {
            ArcError::ProviderError(format!("Unsupported package format: {}", path.display()))
        })?;
        let guessed_name = guess_pkg_name(&path, pkg_type);
        self.ensure_container(&container, &image, &cancel_token)
            .await?;
        if container == CONTAINER_DEB {
            self.ensure_debian_compat(&cancel_token).await?;
        }
        self.install_and_export(&container, &path, pkg_type, &guessed_name, &cancel_token)
            .await
    }

    async fn remove(&self, package_id: &str) -> Result<(), ArcError> {
        // package_id format: "distrobox:CONTAINER:GUESSED_NAME:PKG_TYPE"
        let parts: Vec<&str> = package_id.splitn(4, ':').collect();
        if parts.len() < 4 || parts[0] != "distrobox" {
            return Err(ArcError::ProviderError(format!(
                "Invalid distrobox package id: {}",
                package_id
            )));
        }
        let (container, guessed_name, pkg_type_str) = (parts[1], parts[2], parts[3]);
        let pkg_type = PkgType::from_str(pkg_type_str).ok_or_else(|| {
            ArcError::ProviderError(format!("Unknown pkg type: {}", pkg_type_str))
        })?;

        let info_path = self.info_file(container, guessed_name);
        let content = fs::read_to_string(&info_path)
            .await
            .map_err(|_| ArcError::PackageNotFound(guessed_name.to_string()))?;

        let mut real_name = guessed_name.to_string();
        let mut apps: Vec<String> = Vec::new();
        let mut bins: Vec<String> = Vec::new();

        for line in content.lines() {
            if let Some(v) = line.strip_prefix("REAL_NAME=") {
                real_name = v.to_string();
            } else if let Some(v) = line.strip_prefix("EXPORTED_APPS=") {
                apps = v.split_whitespace().map(|s| s.to_string()).collect();
            } else if let Some(v) = line.strip_prefix("EXPORTED_BINS=") {
                bins = v.split_whitespace().map(|s| s.to_string()).collect();
            }
        }

        self.uninstall_package(container, guessed_name, &real_name, pkg_type, &apps, &bins)
            .await
    }

    async fn update(&self, _package_id: &str) -> Result<(), ArcError> {
        Err(ArcError::ProviderError(
            "Updates are managed through distrobox directly".to_string(),
        ))
    }

    async fn run(&self, package_id: &str) -> Result<(), ArcError> {
        // package_id format: "distrobox:CONTAINER:GUESSED_NAME:PKG_TYPE"
        let parts: Vec<&str> = package_id.splitn(4, ':').collect();
        if parts.len() < 4 || parts[0] != "distrobox" {
            return Err(ArcError::ProviderError(format!(
                "Invalid distrobox package id: {}",
                package_id
            )));
        }
        let (container, guessed_name) = (parts[1], parts[2]);

        let info_path = self.info_file(container, guessed_name);
        let content = fs::read_to_string(&info_path)
            .await
            .map_err(|_| ArcError::PackageNotFound(guessed_name.to_string()))?;

        let app_name = content
            .lines()
            .find_map(|l| l.strip_prefix("EXPORTED_APPS=").map(|v| v.to_string()))
            .and_then(|apps| apps.split_whitespace().next().map(|s| s.to_string()));

        if let Some(app) = app_name {
            Command::new("gtk-launch")
                .arg(&app)
                .spawn()
                .map_err(|e| ArcError::ProviderError(format!("gtk-launch: {}", e)))?;
            return Ok(());
        }

        // No desktop app. Try the first exported binary instead.
        let bin_name = content
            .lines()
            .find_map(|l| l.strip_prefix("EXPORTED_BINS=").map(|v| v.to_string()))
            .and_then(|bins| bins.split_whitespace().next().map(|s| s.to_string()));

        if let Some(bin) = bin_name {
            let bin_path = PathBuf::from(&self.home).join(".local/bin").join(&bin);
            Command::new(&bin_path)
                .spawn()
                .map_err(|e| ArcError::ProviderError(format!("launch {}: {}", bin, e)))?;
            return Ok(());
        }

        Err(ArcError::ProviderError(format!(
            "No exported app or binary found for {}",
            guessed_name
        )))
    }

    async fn search_category(&self, _category: &str) -> Result<Vec<Package>, ArcError> {
        // Distrobox doesn't have categories, return empty
        Ok(Vec::new())
    }

    async fn get_app_info(&self, package_id: &str) -> Result<Option<Package>, ArcError> {
        // Look up the package from installed packages
        let installed = self.read_installed().await?;
        Ok(installed.into_iter().find(|p| p.id == package_id))
    }
}

async fn run_cancellable(
    cmd: &mut Command,
    cancel_token: &CancellationToken,
) -> Result<std::process::ExitStatus, ArcError> {
    // setsid() makes the child its own process group leader (pgid == pid),
    // so we can kill the whole tree (distrobox → podman exec → apt-get/dnf/pacman)
    // with a single kill(-pgid, SIGKILL).
    unsafe {
        cmd.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| ArcError::ProviderError(e.to_string()))?;

    // Spawn a separate task that kills the process group when the token fires.
    // This mirrors the GIO-bridge pattern for flatpak: it fires independently of
    // whether the outer install future is still being polled, so the process is
    // killed even if the outer tokio::select! drops this future first.
    let pid = child.id();
    let killer_token = cancel_token.clone();
    let killer = tokio::spawn(async move {
        killer_token.cancelled().await;
        if let Some(pid) = pid {
            unsafe {
                libc::kill(-(pid as libc::pid_t), libc::SIGKILL);
            }
        }
    });

    let result = child
        .wait()
        .await
        .map_err(|e| ArcError::ProviderError(e.to_string()));
    killer.abort();

    if cancel_token.is_cancelled() {
        return Err(ArcError::ProviderError("Cancelled".to_string()));
    }
    result
}

/// Spawns a task that drip-feeds progress from `from` to `ceiling` every
/// `interval_secs` seconds. Abort the returned handle when the phase ends.
fn slow_tick(
    tx: UnboundedSender<u8>,
    from: u8,
    ceiling: u8,
    interval_secs: u64,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut p = from;
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(interval_secs)).await;
            p = p.saturating_add(1).min(ceiling);
            if tx.send(p).is_err() {
                break;
            }
        }
    })
}

impl DistroboxProvider {
    pub async fn install_with_progress(
        &self,
        package_id: &str,
        progress_tx: UnboundedSender<u8>,
        cancel_token: CancellationToken,
    ) -> Result<(), ArcError> {
        let path = PathBuf::from(package_id);
        let (container, image, pkg_type) = classify_file(&path).ok_or_else(|| {
            ArcError::ProviderError(format!("Unsupported package format: {}", path.display()))
        })?;
        let guessed_name = guess_pkg_name(&path, pkg_type);

        // Phase 1: ensure container (fast if it exists, slow if it needs creating)
        let _ = progress_tx.send(5);
        let ticker = slow_tick(progress_tx.clone(), 5, 18, 2);
        let result = self
            .ensure_container(&container, &image, &cancel_token)
            .await;
        ticker.abort();
        result?;

        // Phase 2: one-time compat setup
        let _ = progress_tx.send(20);
        if container == CONTAINER_DEB {
            let ticker = slow_tick(progress_tx.clone(), 20, 55, 3);
            let result = self.ensure_debian_compat(&cancel_token).await;
            ticker.abort();
            result?;
        } else if container == CONTAINER_ARCH {
            let ticker = slow_tick(progress_tx.clone(), 20, 55, 3);
            let result = self.ensure_arch_compat(&cancel_token).await;
            ticker.abort();
            result?;
        }

        // Phase 3: install + export inside the container
        let _ = progress_tx.send(60);
        let ticker = slow_tick(progress_tx.clone(), 60, 92, 2);
        let result = self
            .install_and_export(&container, &path, pkg_type, &guessed_name, &cancel_token)
            .await;
        ticker.abort();
        result?;

        let _ = progress_tx.send(95);
        Ok(())
    }

    async fn ensure_debian_compat(&self, cancel_token: &CancellationToken) -> Result<(), ArcError> {
        let arc_dir = PathBuf::from(&self.home).join(".local/share/arc");
        let marker = arc_dir.join("arc-debian-compat-v2.done");
        if marker.exists() {
            return Ok(());
        }

        let work_dir =
            PathBuf::from(&self.home).join(format!(".arc-distrobox-{}", Uuid::new_v4().simple()));
        fs::create_dir_all(&work_dir)
            .await
            .map_err(|e| ArcError::ProviderError(e.to_string()))?;

        let helper_path = work_dir.join("compat.sh");
        fs::write(&helper_path, DEBIAN_COMPAT_HELPER)
            .await
            .map_err(|e| ArcError::ProviderError(e.to_string()))?;
        Command::new("chmod")
            .args(["+x", helper_path.to_str().unwrap()])
            .status()
            .await
            .ok();

        let status = run_cancellable(
            Command::new("distrobox").args([
                "enter",
                CONTAINER_DEB,
                "--",
                helper_path.to_str().unwrap(),
                marker.to_str().unwrap(),
            ]),
            cancel_token,
        )
        .await;

        let _ = fs::remove_dir_all(&work_dir).await;

        let status = status?;
        if !status.success() {
            return Err(ArcError::ProviderError(
                "Failed to set up Debian compatibility packages".to_string(),
            ));
        }
        Ok(())
    }

    async fn ensure_arch_compat(&self, cancel_token: &CancellationToken) -> Result<(), ArcError> {
        let arc_dir = PathBuf::from(&self.home).join(".local/share/arc");
        let marker = arc_dir.join("arc-arch-compat-v1.done");
        if marker.exists() {
            return Ok(());
        }

        let work_dir =
            PathBuf::from(&self.home).join(format!(".arc-distrobox-{}", Uuid::new_v4().simple()));
        fs::create_dir_all(&work_dir)
            .await
            .map_err(|e| ArcError::ProviderError(e.to_string()))?;

        let helper_path = work_dir.join("compat.sh");
        fs::write(&helper_path, ARCH_COMPAT_HELPER)
            .await
            .map_err(|e| ArcError::ProviderError(e.to_string()))?;
        Command::new("chmod")
            .args(["+x", helper_path.to_str().unwrap()])
            .status()
            .await
            .ok();

        let status = run_cancellable(
            Command::new("distrobox").args([
                "enter",
                CONTAINER_ARCH,
                "--",
                helper_path.to_str().unwrap(),
                marker.to_str().unwrap(),
            ]),
            cancel_token,
        )
        .await;

        let _ = fs::remove_dir_all(&work_dir).await;

        let status = status?;
        if !status.success() {
            return Err(ArcError::ProviderError(
                "Failed to set up Arch compatibility packages".to_string(),
            ));
        }
        Ok(())
    }
}

const INSTALL_HELPER: &str = r#"#!/bin/bash
set -euo pipefail

DEST="$1"
PKG_TYPE="$2"
EXPORT_LOG="$3"

CNAME="$(grep '^name=' /run/.containerenv 2>/dev/null | cut -d'"' -f2 || true)"
ENTER_PREFIX="distrobox-enter${CNAME:+ -n $CNAME} --"

export_desktop_file() {
    local src="$1"
    [ -f "$src" ] || return 0
    local app_name dest_dir dest
    app_name="$(basename "$src" .desktop)"
    dest_dir="$HOME/.local/share/applications"
    dest="$dest_dir/${app_name}.desktop"
    mkdir -p "$dest_dir"

    while IFS= read -r line; do
        if [[ "$line" =~ ^Exec= ]]; then
            printf 'Exec=%s %s\n' "$ENTER_PREFIX" "${line#Exec=}"
        elif [[ "$line" =~ ^TryExec= ]]; then
            :
        elif [[ "$line" =~ ^Icon=(/usr/share/(icons|pixmaps)/.+)$ ]]; then
            printf 'Icon=%s/.local/share/%s\n' "$HOME" "${BASH_REMATCH[1]#/usr/share/}"
        else
            printf '%s\n' "$line"
        fi
    done < "$src" > "$dest"

    printf 'app:%s\n' "$app_name" >> "$EXPORT_LOG"
}

export_binary() {
    local bin="$1"
    [ -f "$bin" ] && [ -x "$bin" ] || return 0
    local name; name="$(basename "$bin")"
    mkdir -p "$HOME/.local/bin"

    if distrobox-export --bin "$bin" --export-path "$HOME/.local/bin" 2>/dev/null; then
        printf 'bin:%s\n' "$name" >> "$EXPORT_LOG"
        return
    fi

    local wrapper="$HOME/.local/bin/$name"
    cat > "$wrapper" << WRAPPER
#!/bin/sh
exec $ENTER_PREFIX "$bin" "\$@"
WRAPPER
    chmod +x "$wrapper"
    printf 'bin:%s\n' "$name" >> "$EXPORT_LOG"
}

copy_icons() {
    local file_list="$1"
    while IFS= read -r f; do
        [[ "$f" == /usr/share/icons/* ]] || [[ "$f" == /usr/share/pixmaps/* ]] || continue
        [ -f "$f" ] || continue
        local rel="${f#/usr/share/}"
        local dest="$HOME/.local/share/$rel"
        mkdir -p "$(dirname "$dest")"
        cp "$f" "$dest" 2>/dev/null || true
    done <<< "$file_list"
    gtk-update-icon-cache "$HOME/.local/share/icons/hicolor" -q 2>/dev/null || true
}

export_all() {
    local file_list="$1"
    [ -z "$file_list" ] && return 0

    copy_icons "$file_list"

    while IFS= read -r f; do
        [[ "$f" == *.desktop ]] || continue
        [[ "$f" == */applications/* ]] || continue
        export_desktop_file "$f"
    done <<< "$file_list"

    while IFS= read -r f; do
        [[ "$f" =~ ^(/usr(/local)?/bin|/bin)/[^/]+$ ]] || continue
        export_binary "$f"
    done <<< "$file_list"

    update-desktop-database "$HOME/.local/share/applications" 2>/dev/null || true
}

touch "$EXPORT_LOG"

# Some postinst scripts call xdg-desktop-menu to register a menu entry.
# There's no writable system menu dir / desktop session in a distrobox
# container, so it exits nonzero and can fail the whole package install.
# export_all() below does our own desktop integration, so shadow it with
# a no-op for the duration of the install.
sudo tee /usr/local/bin/xdg-desktop-menu > /dev/null << 'STUB'
#!/bin/sh
exit 0
STUB
sudo chmod +x /usr/local/bin/xdg-desktop-menu
trap 'sudo rm -f /usr/local/bin/xdg-desktop-menu' EXIT

case "$PKG_TYPE" in
    deb)
        sudo rm -f /var/lib/dpkg/lock-frontend /var/lib/dpkg/lock /var/cache/apt/archives/lock
        sudo dpkg --configure -a 2>/dev/null || true
        sudo apt-get update -qq 2>/dev/null || true
        pkg_name="$(dpkg-deb --field "$DEST" Package)"
        sudo apt-get install -y "$DEST" || (sudo dpkg -i "$DEST" && sudo apt-get install -f -y)
        file_list="$(dpkg -L "$pkg_name" 2>/dev/null || true)"
        ;;
    rpm)
        pkg_name="$(rpm -qp --queryformat '%{NAME}' "$DEST" 2>/dev/null)"
        sudo dnf install -y "$DEST"
        file_list="$(rpm -ql "$pkg_name" 2>/dev/null || true)"
        ;;
    pacman)
        sudo pacman -Sy --noconfirm
        pkg_name="$(pacman -Qip "$DEST" 2>/dev/null | awk '/^Name[[:space:]]/{print $3}')"
        sudo pacman -U --noconfirm "$DEST"
        file_list="$(pacman -Ql "$pkg_name" 2>/dev/null | awk '{print $2}' || true)"
        ;;
esac

printf 'pkgname:%s\n' "$pkg_name" >> "$EXPORT_LOG"

# Refresh package lists after install. The package may have added new apt
# repos (e.g. steam-launcher adds Valve's repo). Running update here means
# the cache timestamp is current so apps like Steam don't pop up an
# "apt-get update" xterm dialog on first launch.
if [ "$PKG_TYPE" = "deb" ]; then
    sudo apt-get update -qq 2>/dev/null || true

    # If steam-launcher was just installed, pre-install all runtime deps so
    # Steam never needs to call apt-get via polkit on launch.
    if dpkg -s steam-launcher &>/dev/null 2>&1; then
        DEBIAN_FRONTEND=noninteractive sudo apt-get install -y \
            libegl1:i386 libgbm1:i386 xdg-desktop-portal-kde \
            steam-libs-amd64 2>/dev/null || true
    fi
fi

export_all "$file_list"
"#;

const DEBIAN_COMPAT_HELPER: &str = r#"#!/bin/bash
set -euo pipefail
MARKER="$1"

# Already done on a previous install
[ -f "$MARKER" ] && exit 0

# Enable 32-bit architecture support (required for Steam and most Windows-compat libs)
sudo dpkg --add-architecture i386
sudo apt-get update -qq

# KDE / xdg-desktop-portal support. Lets apps use native file pickers, portals,
# and the secret service. Installed silently; nothing is exported to the host.
sudo apt-get install -y --no-install-recommends \
    xdg-desktop-portal-kde \
    libsecret-1-0

# lib32 / multiarch libraries required by Steam and similar apps.
# Installing all of these here means Steam never needs to run apt-get at
# launch time (which would pop up a polkit/xterm dialog).
sudo apt-get install -y \
    libc6:i386 \
    libegl1:i386 \
    libgbm1:i386 \
    libgl1:i386 \
    libgl1-mesa-dri:i386 \
    libstdc++6:i386 \
    libgcc-s1:i386 \
    libxss1:i386 \
    libxtst6:i386 \
    libnss3:i386

touch "$MARKER"
"#;

const ARCH_COMPAT_HELPER: &str = r#"#!/bin/bash
set -euo pipefail
MARKER="$1"

[ -f "$MARKER" ] && exit 0

sudo pacman -Sy --noconfirm

# Audio: PipeWire client libraries + PulseAudio and ALSA compatibility layers.
# The host PipeWire socket is bind-mounted into the container by distrobox;
# these packages let containerised apps connect to it via PA or ALSA APIs.
sudo pacman -S --noconfirm --needed \
    pipewire \
    pipewire-alsa \
    pipewire-pulse \
    libpulse \
    alsa-lib \
    alsa-plugins

touch "$MARKER"
"#;

const UNINSTALL_HELPER: &str = r#"#!/bin/bash
set -euo pipefail
PKG_REAL="$1"
PKG_TYPE="$2"
case "$PKG_TYPE" in
    deb)    sudo apt-get remove -y "$PKG_REAL" ;;
    rpm)    sudo dnf remove -y "$PKG_REAL" ;;
    pacman) sudo pacman -R --noconfirm "$PKG_REAL" ;;
esac
"#;
