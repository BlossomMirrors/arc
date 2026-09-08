// Broadcasts install/remove progress via the "Unity LauncherEntry" D-Bus
// protocol (also implemented by Plasma's Task Manager) so Arc's taskbar/dock
// icon keeps showing a progress bar even while the OS-level job notification
// itself is suppressed because the frontend window is on screen.
use std::collections::HashMap;
use std::sync::OnceLock;
use tokio::spawn;
use tracing::warn;
use zbus::zvariant::Value;
use zbus::Connection;

// matches the .desktop file id Arc installs under (org.blossomos.Arc.desktop)
const APP_URI: &str = "application://org.blossomos.Arc.desktop";

static CONNECTION: OnceLock<Connection> = OnceLock::new();

pub fn init(conn: Connection) {
    let _ = CONNECTION.set(conn);
}

// progress is 0.0-1.0; visible is false once nothing is running so the
// taskbar clears the bar instead of leaving it stuck at its last value
pub fn update(visible: bool, progress: f64) {
    let Some(conn) = CONNECTION.get().cloned() else { return };
    spawn(async move {
        let mut props: HashMap<&str, Value> = HashMap::new();
        props.insert("progress-visible", Value::from(visible));
        props.insert("progress", Value::from(progress.clamp(0.0, 1.0)));
        let result = conn
            .emit_signal(
                Option::<&str>::None,
                "/",
                "com.canonical.Unity.LauncherEntry",
                "Update",
                &(APP_URI, props),
            )
            .await;
        if let Err(e) = result {
            warn!("failed to emit LauncherEntry progress signal: {e}");
        }
    });
}
