mod appstream_db;
mod daemon;
mod dbus_interface;
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

    // Parse appstream data before accepting connections so get_home_apps()
    // returns instantly when the frontend first calls it.
    info!("Loading appstream database...");
    tokio::task::spawn_blocking(appstream_db::AppStreamDb::get_static)
        .await
        .ok();
    info!("Appstream database ready");

    let daemon = daemon::Daemon::new().await?;
    daemon.run().await?;

    Ok(())
}
