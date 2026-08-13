#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        #[qproperty(QString, preferred_provider, cxx_name = "preferredProvider")]
        #[qproperty(bool, ignore_native_preference, cxx_name = "ignoreNativePreference")]
        #[qproperty(bool, auto_updates, cxx_name = "autoUpdates")]
        #[qproperty(i32, concurrent_downloads, cxx_name = "concurrentDownloads")]
        #[qproperty(bool, show_security_warnings, cxx_name = "showSecurityWarnings")]
        type SettingsController = super::SettingsControllerRust;

        #[qinvokable]
        fn load(self: Pin<&mut SettingsController>);

        #[qinvokable]
        fn save(self: Pin<&mut SettingsController>);

        #[qinvokable]
        #[cxx_name = "forceUpdate"]
        fn force_update(self: Pin<&mut SettingsController>);

        #[qinvokable]
        #[cxx_name = "restartDaemon"]
        fn restart_daemon(self: Pin<&mut SettingsController>);

        #[qsignal]
        #[cxx_name = "actionFailed"]
        fn action_failed(self: Pin<&mut SettingsController>, message: QString);

        #[qsignal]
        #[cxx_name = "actionSucceeded"]
        fn action_succeeded(self: Pin<&mut SettingsController>, message: QString);

        #[qsignal]
        #[cxx_name = "daemonReconnected"]
        fn daemon_reconnected(self: Pin<&mut SettingsController>);
    }

    impl cxx_qt::Threading for SettingsController {}
}

use crate::runtime;
use cxx_qt::{CxxQtThread, Threading};
use cxx_qt_lib::QString;
use libarc::{Provider, Settings};
use std::pin::Pin;

#[derive(Default)]
pub struct SettingsControllerRust {
    preferred_provider: QString,
    ignore_native_preference: bool,
    auto_updates: bool,
    concurrent_downloads: i32,
    show_security_warnings: bool,
}

fn emit_result(qt_thread: &CxxQtThread<qobject::SettingsController>, message: String, success: bool) {
    let _ = qt_thread.queue(move |mut this| {
        if success {
            this.as_mut().action_succeeded(QString::from(&message));
        } else {
            this.as_mut().action_failed(QString::from(&message));
        }
    });
}

// resolves the actual executable path of whatever process is currently
// running as `arc-daemon` — a dev session (`cargo run -p arc-dev`) runs
// target/debug/arc-daemon, not the system-installed /usr/bin/arc-daemon a
// hardcoded path would restart instead, silently leaving the one actually
// in use untouched (and possibly reviving a stale system install instead)
fn find_running_arc_daemon_exe() -> Option<std::path::PathBuf> {
    let entries = std::fs::read_dir("/proc").ok()?;
    for entry in entries.flatten() {
        if entry.file_name().to_string_lossy().parse::<u32>().is_err() {
            continue;
        }
        let Ok(comm) = std::fs::read_to_string(entry.path().join("comm")) else {
            continue;
        };
        if comm.trim() != "arc-daemon" {
            continue;
        }
        if let Ok(exe) = std::fs::read_link(entry.path().join("exe")) {
            return Some(exe);
        }
    }
    None
}

fn provider_to_string(p: &Provider) -> &'static str {
    match p {
        Provider::Flatpak => "Flatpak",
        Provider::Distrobox => "Distrobox",
        Provider::Lutris => "Lutris",
        Provider::AppImage => "AppImage",
        Provider::Pwa => "Pwa",
    }
}

fn provider_from_string(s: &str) -> Provider {
    match s {
        "Distrobox" => Provider::Distrobox,
        "Lutris" => Provider::Lutris,
        "AppImage" => Provider::AppImage,
        "Pwa" => Provider::Pwa,
        _ => Provider::Flatpak,
    }
}

impl qobject::SettingsController {
    pub fn load(mut self: Pin<&mut Self>) {
        let s = Settings::load();
        self.as_mut()
            .set_preferred_provider(QString::from(provider_to_string(&s.preferred_provider)));
        self.as_mut().set_ignore_native_preference(s.ignore_native_preference);
        self.as_mut().set_auto_updates(s.auto_updates);
        self.as_mut().set_concurrent_downloads(s.concurrent_downloads as i32);
        self.as_mut().set_show_security_warnings(s.show_security_warnings);
    }

    pub fn save(self: Pin<&mut Self>) {
        let concurrent_downloads = self.concurrent_downloads.max(1) as u32;
        let s = Settings {
            preferred_provider: provider_from_string(&self.preferred_provider.to_string()),
            ignore_native_preference: self.ignore_native_preference,
            auto_updates: self.auto_updates,
            concurrent_downloads,
            show_security_warnings: self.show_security_warnings,
        };
        if let Err(e) = s.save() {
            tracing::warn!("failed to save settings: {e}");
        }

        runtime::spawn(async move {
            if let Some(proxy) = runtime::proxy().await {
                let _ = proxy.set_concurrent_downloads(concurrent_downloads).await;
            }
        });
    }

    pub fn force_update(mut self: Pin<&mut Self>) {
        let qt_thread = self.as_mut().qt_thread();
        runtime::spawn(async move {
            let result = tokio::process::Command::new("flatpak")
                .args(["update", "-y"])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .await;
            match result {
                Ok(status) if status.success() => {
                    emit_result(&qt_thread, "Update finished".into(), true);
                }
                Ok(status) => {
                    emit_result(&qt_thread, format!("flatpak update exited with {status}"), false);
                }
                Err(e) => emit_result(&qt_thread, format!("Failed to run flatpak update: {e}"), false),
            }
        });
    }

    pub fn restart_daemon(mut self: Pin<&mut Self>) {
        let qt_thread = self.as_mut().qt_thread();
        runtime::spawn(async move {
            let daemon_path = find_running_arc_daemon_exe()
                .unwrap_or_else(|| std::path::PathBuf::from("/usr/bin/arc-daemon"));

            let _ = tokio::process::Command::new("pkill").args(["-x", "arc-daemon"]).status().await;
            tokio::time::sleep(std::time::Duration::from_millis(800)).await;
            let spawned = std::process::Command::new("setsid")
                .arg("--fork")
                .arg(&daemon_path)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
            if let Err(e) = spawned {
                emit_result(&qt_thread, format!("Failed to start {}: {e}", daemon_path.display()), false);
                return;
            }

            for _ in 0..20 {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                let Ok(proxy) = libarc::connect().await else { continue };
                let ready = tokio::time::timeout(
                    std::time::Duration::from_secs(2),
                    proxy.list_installed(),
                )
                .await
                .map(|r| r.is_ok())
                .unwrap_or(false);
                if ready {
                    runtime::invalidate_proxy();
                    emit_result(&qt_thread, "Daemon restarted".into(), true);
                    let _ = qt_thread.queue(|mut this| {
                        this.as_mut().daemon_reconnected();
                    });
                    return;
                }
            }
            emit_result(&qt_thread, "Daemon restart timed out".into(), false);
        });
    }
}
