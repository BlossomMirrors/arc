mod appstream_db;
mod daemon;
mod dbus_interface;
mod forge_cache;
mod http_api;
mod kio;
mod providers;
mod transaction_manager;

use anyhow::Result;
use tracing::info;
use tracing_subscriber::EnvFilter;

include!(concat!(env!("OUT_DIR"), "/locale/translators.rs"));

const HTTP_PORT: u16 = 1312;

#[tokio::main]
async fn main() {
    let code = match run().await {
        Ok(()) => 0,
        Err(e) => {
            tracing::error!("{e:?}");
            1
        }
    };

    // Some background work (e.g. the AppStreamDb cold parse, which can take
    // well over a minute) runs on spawn_blocking OS threads that can't be
    // cancelled, and libflatpak's GLib bindings register their own atexit
    // cleanup; both would otherwise delay process exit by however long they
    // have left to run. All our real cleanup (cancelling transactions)
    // already happened inside run(), so skip Rust's normal exit() (which
    // waits for the runtime and runs atexit handlers) and terminate via the
    // raw syscall instead — a daemon replaced early in its life should die
    // immediately, not linger.
    unsafe { libc::_exit(code) };
}

async fn run() -> Result<()> {
    let locale = sys_locale::get_locale().unwrap_or_else(|| "en".to_string());
    translators::set_locale(&locale);

    // RUST_LOG=debug
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    info!("Arc Communication Daemon (ACD) starting");

    // ARC_HTTP_ONLY runs without a session bus (e.g. in a container), so it
    // can't claim the D-Bus name and doesn't need to replace anything.
    let bus_claim = if std::env::var_os("ARC_HTTP_ONLY").is_none() {
        Some(daemon::claim_bus_name().await?)
    } else {
        None
    };

    // AppStreamDb::get_static() is a lazy, self-memoizing OnceLock: whichever
    // caller (this warm-up or a live search) reaches it first pays the parse
    // cost, everyone else gets the cached result instantly. Warm it in the
    // background instead of blocking D-Bus/HTTP startup on it, so the daemon
    // (and search) are usable immediately; the first search after a cold
    // start just pays the parse cost inline instead of the whole app.
    info!("Loading appstream database in the background...");
    tokio::spawn(async {
        tokio::task::spawn_blocking(appstream_db::AppStreamDb::get_static)
            .await
            .ok();
        info!("Appstream database ready");

        info!("Pre-fetching Forge home page...");
        forge_cache::refresh().await;

        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
            forge_cache::refresh().await;
        }
    });

    // Allow overriding the bind host via env var so the HTTP API is reachable
    // when running inside a container (set ARC_HTTP_HOST=0.0.0.0).
    let bind_host = std::env::var("ARC_HTTP_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let addr: std::net::SocketAddr = format!("{bind_host}:{HTTP_PORT}").parse()?;
    // claiming the D-Bus name only reassigns ownership at the broker; an old
    // instance still has to notice NameLost and actually exit before it lets
    // go of this port, so a fresh daemon can lose the bind race by a beat.
    // retry instead of dying outright, since we already know we're about to
    // own this port.
    let listener = {
        let mut last_err = None;
        let mut bound = None;
        for _ in 0..40 {
            match tokio::net::TcpListener::bind(addr).await {
                Ok(l) => {
                    bound = Some(l);
                    break;
                }
                Err(e) => {
                    last_err = Some(e);
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }
            }
        }
        bound.ok_or_else(|| {
            last_err.unwrap_or_else(|| std::io::Error::other("failed to bind HTTP port"))
        })?
    };
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

    let (conn, name_lost) = bus_claim.expect("checked above: not ARC_HTTP_ONLY");
    let daemon = daemon::Daemon::new().await?;
    daemon.run(conn, name_lost).await?;

    Ok(())
}
