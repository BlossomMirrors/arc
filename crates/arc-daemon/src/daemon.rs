use crate::dbus_interface::ArcDaemonInterface;
use crate::download_queue::DownloadQueue;
use crate::providers::appimage::AppImageProvider;
use crate::providers::distrobox::DistroboxProvider;
use crate::providers::flatpak::FlatpakProvider;
use crate::providers::lutris::LutrisProvider;
use crate::providers::pwa::PwaProvider;
use crate::providers::{MultiProvider, PackageProvider};
use crate::transaction_manager::TransactionManager;
use anyhow::Result;
use futures_util::StreamExt;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::process::Command;
use tracing::{info, warn};
use zbus::connection::Builder as ConnectionBuilder;
use zbus::fdo::{DBusProxy, NameLostStream, RequestNameFlags, RequestNameReply};
use zbus::names::BusName;
use zbus::Connection;

const BUS_NAME: &str = "org.blossomos.arc.daemon";

/// claims [`BUS_NAME`] on the session bus replacing any daemon
/// with a still-running old instance such as binding the HTTP port
pub async fn claim_bus_name() -> Result<(Connection, NameLostStream)> {
    let conn = ConnectionBuilder::session()?.build().await?;

    let dbus = DBusProxy::new(&conn).await?;
    let name_lost = dbus.receive_name_lost().await?;

    let flags = RequestNameFlags::AllowReplacement
        | RequestNameFlags::ReplaceExisting
        | RequestNameFlags::DoNotQueue;
    let mut reply = conn.request_name_with_flags(BUS_NAME, flags).await?;

    if reply == RequestNameReply::Exists {
        // owner didnt set AllowReplacement // kill it directly and retry
        warn!("{BUS_NAME} is held by a non-replaceable daemon, terminating it");
        let owner = dbus.get_name_owner(BusName::try_from(BUS_NAME)?).await?;
        let pid = dbus.get_connection_unix_process_id(owner.into()).await?;
        unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };
        for _ in 0..20 {
            tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
            reply = conn.request_name_with_flags(BUS_NAME, flags).await?;
            if reply != RequestNameReply::Exists {
                break;
            }
        }
    }
    if !matches!(
        reply,
        RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner
    ) {
        anyhow::bail!("could not take over bus name {BUS_NAME}: {reply:?}");
    }

    info!("D-Bus name {BUS_NAME} claimed");
    Ok((conn, name_lost))
}

pub struct Daemon {
    provider: Arc<MultiProvider>,
    transaction_manager: Arc<TransactionManager>,
}

impl Daemon {
    pub async fn new() -> Result<Self> {
        let native = DistroboxProvider::new();
        let flatpak = FlatpakProvider::new();
        let lutris = LutrisProvider::new();
        let appimage = AppImageProvider::new();
        let pwa = PwaProvider::new();

        let provider = Arc::new(MultiProvider::new(native, flatpak, lutris, appimage, pwa));

        // both the appstream refresh (network + flatpak subprocess) and the
        // initial cache warm-up (e.g. Lutris's catalog fetch) can take a long
        // time on a slow network; run them in the background instead of
        // blocking D-Bus/HTTP startup on them. Callers just pay the cold-start
        // cost inline on their first request instead of the whole daemon being
        // unreachable until this finishes.
        let warmup_provider = Arc::clone(&provider);
        tokio::spawn(async move {
            info!("Refreshing AppStream data...");
            match Command::new("flatpak").args(["update", "--appstream"]).status().await {
                Ok(status) if status.success() => info!("AppStream data refreshed"),
                Ok(status) => warn!("flatpak update --appstream exited with {}", status),
                Err(e) => warn!("Failed to run flatpak update --appstream: {}", e),
            }

            info!("Pre-warming package cache...");
            if let Err(e) = warmup_provider.refresh_cache().await {
                warn!("Initial cache warm-up failed: {}", e);
            } else {
                info!("Package cache ready");
            }
        });

        // arc clone is a reference counted pointer so both the spawn and the
        // daemon struct share the same provider without copying it
        let bg_provider = Arc::clone(&provider);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(15 * 60)).await;
                if let Err(e) = bg_provider.refresh_cache().await {
                    warn!("Background cache refresh failed: {}", e);
                } else {
                    info!("Package cache refreshed");
                }
            }
        });

        let au_provider = Arc::clone(&provider);
        tokio::spawn(async move {
            loop {
                let auto_updates = libarc::Settings::load().auto_updates;
                info!("Checking for updates...");
                match au_provider.refresh_updates().await {
                    Err(e) => warn!("Update check failed: {}", e),
                    Ok(updates) if updates.is_empty() => {
                        info!("No updates available");
                    }
                    Ok(updates) if !auto_updates => {
                        info!("{} update(s) available", updates.len());
                    }
                    Ok(updates) => {
                        info!("Auto-update: updating {} package(s)", updates.len());
                        for pkg in updates {
                            info!("Auto-update: updating {}", pkg.id);
                            if let Err(e) = au_provider.update(&pkg.id).await {
                                warn!("Auto-update: failed to update {}: {}", pkg.id, e);
                            }
                        }
                        info!("Auto-update: done");
                        au_provider.invalidate_package_cache().await;
                    }
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
            }
        });

        Ok(Self {
            provider,
            transaction_manager: Arc::new(TransactionManager::new()),
        })
    }

    pub async fn run(self, conn: Connection, mut name_lost: NameLostStream) -> Result<()> {
        info!("Starting Arc Communication Daemon");

        // scan ~/.appimages on startup to pick up any AppImages placed there manually
        info!("Scanning AppImages directory...");
        self.provider.appimage.scan_and_sync().await;

        // start inotify watcher; keep it alive for the duration of the daemon
        let _appimage_watcher =
            AppImageProvider::start_watcher(Arc::clone(&self.provider.appimage));
        if let Err(ref e) = _appimage_watcher {
            warn!("AppImage watcher failed to start: {}", e);
        }

        let settings = libarc::Settings::load();
        let interface = ArcDaemonInterface {
            provider: self.provider,
            transaction_manager: self.transaction_manager.clone(),
            download_queue: Arc::new(DownloadQueue::new(settings.concurrent_downloads as usize)),
            foreground_package: Arc::new(tokio::sync::RwLock::new(String::new())),
            notifications_suppressed: Arc::new(AtomicBool::new(false)),
        };

        // the bus name was already claimed in claim_bus_name(), before the
        // HTTP listener bind, so an old instance is guaranteed dead by now
        conn.object_server()
            .at("/org/blossomos/arc/daemon", interface)
            .await?;

        info!("D-Bus service registered at {BUS_NAME}");
        info!("Arc daemon running. Press Ctrl+C to stop.");

        // wait here until ctrl+c/sigterm or until a newer daemon replaces us,
        // the actual work happens in the dbus callbacks
        let mut sigterm =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = sigterm.recv() => {
                info!("Received SIGTERM, shutting down");
            }
            _ = async {
                while let Some(signal) = name_lost.next().await {
                    match signal.args() {
                        Ok(args) if args.name.as_str() == BUS_NAME => break,
                        _ => {}
                    }
                }
            } => {
                info!("{BUS_NAME} was taken over by another daemon instance, shutting down");
            }
        }

        // Cancel all running transactions before shutdown
        info!("Cancelling all running transactions...");
        self.transaction_manager.cancel_all().await;

        info!("Shutting down Arc daemon");

        Ok(())
    }
}
