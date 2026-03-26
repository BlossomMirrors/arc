use crate::dbus_interface::ArcDaemonInterface;
use crate::providers::flatpak::FlatpakProvider;
use crate::providers::packagekit::PackageKitProvider;
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
        let native = PackageKitProvider::new()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to initialize PackageKit provider: {}", e))?;
        let flatpak = FlatpakProvider::new();

        let provider = Arc::new(MultiProvider::new(native, flatpak));

        info!("Pre-warming package cache...");
        if let Err(e) = provider.refresh_cache().await {
            warn!("Initial cache warm-up failed: {}", e);
        } else {
            info!("Package cache ready");
        }

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

        let _conn = ConnectionBuilder::session()?
            .name("dev.arc.ArcDaemon1")?
            .serve_at("/dev/arc/ArcDaemon1", interface)?
            .build()
            .await?;

        info!("D-Bus service registered at dev.arc.ArcDaemon1");
        info!("Arc daemon running. Press Ctrl+C to stop.");

        tokio::signal::ctrl_c().await?;
        info!("Shutting down Arc daemon");

        Ok(())
    }
}
