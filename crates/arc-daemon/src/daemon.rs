use crate::dbus_interface::ArcDaemonInterface;
use crate::providers::bottles::BottlesProvider;
use crate::providers::distrobox::DistroboxProvider;
use crate::providers::flatpak::FlatpakProvider;
use crate::providers::MultiProvider;
use crate::transaction_manager::TransactionManager;
use anyhow::Result;
use std::sync::Arc;
use tracing::{info, warn};
use zbus::connection::Builder as ConnectionBuilder;

pub struct Daemon {
    provider: Arc<MultiProvider>,
    transaction_manager: Arc<TransactionManager>,
}

impl Daemon {
    pub async fn new() -> Result<Self> {
        let native = DistroboxProvider::new();
        let flatpak = FlatpakProvider::new();
        let bottles = BottlesProvider::new();

        let provider = Arc::new(MultiProvider::new(native, flatpak, bottles));

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

        Ok(Self {
            provider,
            transaction_manager: Arc::new(TransactionManager::new()),
        })
    }

    pub async fn run(self) -> Result<()> {
        info!("Starting Arc Communication Daemon");

        let interface = ArcDaemonInterface {
            provider: self.provider,
            transaction_manager: self.transaction_manager,
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
        info!("Shutting down Arc daemon");

        Ok(())
    }
}
