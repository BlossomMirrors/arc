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

    // Allow overriding the bind host via env var so the HTTP API is reachable
    // when running inside a container (set ARC_HTTP_HOST=0.0.0.0).
    let bind_host = std::env::var("ARC_HTTP_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let addr: std::net::SocketAddr = format!("{bind_host}:{HTTP_PORT}").parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("HTTP API listening on http://{}", addr);
    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, http_api::router()).await {
            tracing::error!("HTTP API error: {}", e);
        }
    });

    // ARC_HTTP_ONLY=1 skips the D-Bus daemon (useful in containers where no
    // session bus is available) and keeps the process alive serving only HTTP.
    if std::env::var_os("ARC_HTTP_ONLY").is_some() {
        info!("Running in HTTP-only mode (no D-Bus)");
        tokio::signal::ctrl_c().await?;
        return Ok(());
    }

    let daemon = daemon::Daemon::new().await?;
    daemon.run().await?;

    Ok(())
}
