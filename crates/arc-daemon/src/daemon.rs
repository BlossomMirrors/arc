use crate::dbus_interface::ArcDaemonInterface;
use crate::providers::appimage::AppImageProvider;
use crate::providers::distrobox::DistroboxProvider;
use crate::providers::flatpak::FlatpakProvider;
use crate::providers::lutris::LutrisProvider;
use crate::providers::pwa::PwaProvider;
use crate::providers::{MultiProvider, PackageProvider};
use crate::transaction_manager::TransactionManager;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::process::Command;
use tracing::{info, warn};
use zbus::connection::Builder as ConnectionBuilder;

pub struct Daemon {
    provider: Arc<MultiProvider>,
    transaction_manager: Arc<TransactionManager>,
}

impl Daemon {
    pub async fn new() -> Result<Self> {
        info!("Refreshing AppStream data...");
        match Command::new("flatpak").args(["update", "--appstream"]).status().await {
            Ok(status) if status.success() => info!("AppStream data refreshed"),
            Ok(status) => warn!("flatpak update --appstream exited with {}", status),
            Err(e) => warn!("Failed to run flatpak update --appstream: {}", e),
        }

        let native = DistroboxProvider::new();
        let flatpak = FlatpakProvider::new();
        let lutris = LutrisProvider::new();
        let appimage = AppImageProvider::new();
        let pwa = PwaProvider::new();

        let provider = Arc::new(MultiProvider::new(native, flatpak, lutris, appimage, pwa));

        // load both flatpak and system packages into memory right away so the
        // first search request is fast instead of blocking on a cold provider
        info!("Pre-warming package cache...");
        if let Err(e) = provider.refresh_cache().await {
            warn!("Initial cache warm-up failed: {}", e);
        } else {
            info!("Package cache ready");
        }

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
                if libarc::Settings::load().auto_updates {
                    info!("Auto-update: checking for updates...");
                    match au_provider.list_updates().await {
                        Err(e) => warn!("Auto-update: list_updates failed: {}", e),
                        Ok(updates) if updates.is_empty() => {
                            info!("Auto-update: nothing to update");
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
                        }
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

    pub async fn run(self) -> Result<()> {
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
        let concurrent = (settings.concurrent_downloads as usize).max(1);
        let interface = ArcDaemonInterface {
            provider: self.provider,
            transaction_manager: self.transaction_manager.clone(),
            download_semaphore: Arc::new(Semaphore::new(concurrent)),
        };

        // register on the session dbus under our name so clients
        // can find and connect to us by that name
        let _conn = ConnectionBuilder::session()?
            .name("dev.arc.ArcDaemon1")?
            .serve_at("/dev/arc/ArcDaemon1", interface)?
            .build()
            .await?;

        info!("D-Bus service registered at dev.arc.ArcDaemon1");
        info!("Arc daemon running. Press Ctrl+C to stop.");

        // wait here until ctrl+c, the actual work happens in the dbus callbacks
        tokio::signal::ctrl_c().await?;

        // Cancel all running transactions before shutdown
        info!("Cancelling all running transactions...");
        self.transaction_manager.cancel_all().await;

        info!("Shutting down Arc daemon");

        Ok(())
    }
}
