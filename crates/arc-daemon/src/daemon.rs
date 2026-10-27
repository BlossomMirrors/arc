use crate::dbus_interface::ArcDaemonInterface;
use crate::providers::appimage::AppImageProvider;
use crate::providers::distrobox::DistroboxProvider;
use crate::providers::flatpak::FlatpakProvider;
use crate::providers::lutris::LutrisProvider;
use crate::providers::pwa::PwaProvider;
use crate::providers::MultiProvider;
use crate::transaction_manager::TransactionManager;
use anyhow::Result;
use futures_util::StreamExt;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::spawn;
use tokio::sync::Semaphore;
use tokio::task::spawn_blocking;
use tracing::{info, warn};
use zbus::connection::Builder as ConnectionBuilder;
use zbus::fdo::{DBusProxy, NameLostStream, RequestNameFlags, RequestNameReply};
use zbus::names::BusName;
use zbus::object_server::SignalEmitter;
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

        let provider = MultiProvider::new(native, flatpak, lutris, appimage, pwa);

        // the catalogue on disk answers right away so all of this runs in the background
        let warmup_provider = Arc::clone(&provider);
        spawn(async move {
            crate::appstream_db::refresh_remotes_if_due().await;
            spawn_blocking(crate::appstream_db::sync_if_stale).await.ok();
            if let Err(e) = warmup_provider.refresh_cache().await {
                warn!("Initial catalogue refresh failed: {}", e);
            }
            if let Err(e) = warmup_provider.refresh_updates().await {
                warn!("Initial update check failed: {}", e);
            }
        });

        // arc clone is a reference counted pointer so both the spawn and the
        // daemon struct share the same provider without copying it
        let bg_provider = Arc::clone(&provider);
        spawn(async move {
            let mut ticks: u32 = 0;
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(15 * 60)).await;
                if let Err(e) = bg_provider.refresh_cache().await {
                    warn!("Background cache refresh failed: {}", e);
                } else {
                    info!("Package cache refreshed");
                }
                // hourly
                ticks += 1;
                if ticks % 4 == 0 {
                    match bg_provider.pwa.refresh_installed_icons().await {
                        0 => {}
                        n => info!("Refreshed {n} PWA icon(s)"),
                    }
                }
            }
        });

        Ok(Self {
            provider,
            transaction_manager: Arc::new(TransactionManager::new()),
        })
    }

    pub async fn run(self, conn: Connection, mut name_lost: NameLostStream) -> Result<()> {
        info!("Starting Arc Communication Daemon");

        crate::launcher_progress::init(conn.clone());

        // scan ~/.appimages on startup to pick up any AppImages placed there manually
        // it runs icon cache tools for every AppImage so it happens after the interface is reachable
        let scan_provider = Arc::clone(&self.provider);
        spawn(async move {
            info!("Scanning AppImages directory...");
            scan_provider.appimage.scan_and_sync().await;
            scan_provider.refresh_appimages().await;
        });

        // start inotify watcher; keep it alive for the duration of the daemon
        let _appimage_watcher =
            AppImageProvider::start_watcher(Arc::clone(&self.provider.appimage));
        if let Err(ref e) = _appimage_watcher {
            warn!("AppImage watcher failed to start: {}", e);
        }

        let settings = libarc::Settings::load();
        let concurrent = (settings.concurrent_downloads as usize).max(1);
        let auto_update_provider = Arc::clone(&self.provider);
        let watch_provider = Arc::clone(&self.provider);
        let interface = ArcDaemonInterface {
            provider: self.provider,
            transaction_manager: self.transaction_manager.clone(),
            download_semaphore: Arc::new(Semaphore::new(concurrent)),
            download_permits: Arc::new(tokio::sync::Mutex::new(concurrent)),
            frontend_visible: Arc::new(tokio::sync::RwLock::new(false)),
        };

        // the bus name was already claimed in claim_bus_name(), before the
        // HTTP listener bind, so an old instance is guaranteed dead by now
        conn.object_server()
            .at("/org/blossomos/arc/daemon", interface)
            .await?;

        info!("D-Bus service registered at {BUS_NAME}");

        let emitter = SignalEmitter::new(&conn, "/org/blossomos/arc/daemon")?.to_owned();

        let changed_emitter = emitter.clone();
        spawn(async move {
            let mut rx = crate::catalog::subscribe();
            while rx.changed().await.is_ok() {
                // a sync commits several times in a row and the frontend only needs to hear about it once
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                let generation = *rx.borrow_and_update();
                let _ = ArcDaemonInterface::catalog_changed(&changed_emitter, generation).await;
            }
        });

        let _flatpak_watcher = start_flatpak_watcher(Arc::clone(&watch_provider));

        spawn(crate::dbus_interface::run_auto_updates(
            auto_update_provider,
            self.transaction_manager.clone(),
            emitter,
        ));

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

fn flatpak_bases() -> Vec<PathBuf> {
    let mut bases = vec![PathBuf::from("/var/lib/flatpak")];
    if let Some(home) = std::env::var_os("HOME") {
        bases.push(PathBuf::from(home).join(".local/share/flatpak"));
    }
    bases
}

// flatpak replaces the active link in each <remote>/<arch> folder when it deploys a new catalogue
fn appstream_arch_dirs() -> Vec<PathBuf> {
    flatpak_bases()
        .into_iter()
        .flat_map(|base| std::fs::read_dir(base.join("appstream")).into_iter().flatten().flatten())
        .flat_map(|remote| std::fs::read_dir(remote.path()).into_iter().flatten().flatten())
        .map(|arch| arch.path())
        .filter(|p| p.is_dir())
        .collect()
}

// installs and catalogue updates made outside arc show up without waiting for the next refresh
// flatpak touches a .changed file in its base folder after every change and that is what this listens for
fn start_flatpak_watcher(provider: Arc<MultiProvider>) -> Option<RecommendedWatcher> {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<()>();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if res.is_ok() {
            let _ = tx.send(());
        }
    })
    .map_err(|e| warn!("flatpak watcher failed to start: {e}"))
    .ok()?;
    for base in flatpak_bases().into_iter().filter(|b| b.is_dir()) {
        let _ = watcher.watch(&base, RecursiveMode::NonRecursive);
    }
    for dir in appstream_arch_dirs() {
        let _ = watcher.watch(&dir, RecursiveMode::NonRecursive);
    }
    spawn(async move {
        while rx.recv().await.is_some() {
            // one install or deploy fires many events so wait until it goes quiet
            loop {
                match tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv()).await {
                    Ok(Some(())) => continue,
                    Ok(None) => return,
                    Err(_) => break,
                }
            }
            spawn_blocking(crate::appstream_db::sync_if_stale).await.ok();
            provider.on_flatpak_changed().await;
        }
    });
    info!("Watching flatpak installations for changes");
    Some(watcher)
}
