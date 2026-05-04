mod appstream_db;
mod daemon;
mod dbus_interface;
mod icon_cache;
mod providers;
mod transaction_manager;

use anyhow::Result;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    // RUST_LOG=debug
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    info!("Arc Communication Daemon (ACD) starting");

    let daemon = daemon::Daemon::new().await?;
    daemon.run().await?;

    Ok(())
}
