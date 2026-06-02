mod appstream_db;
mod daemon;
mod dbus_interface;
mod http_api;
mod providers;
mod transaction_manager;

use anyhow::Result;
use tracing::info;
use tracing_subscriber::EnvFilter;

const HTTP_PORT: u16 = 1312;

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

    // Start the read-only HTTP API in the background.
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], HTTP_PORT));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("HTTP API listening on http://{}", addr);
    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, http_api::router()).await {
            tracing::error!("HTTP API error: {}", e);
        }
    });

    let daemon = daemon::Daemon::new().await?;
    daemon.run().await?;

    Ok(())
}
