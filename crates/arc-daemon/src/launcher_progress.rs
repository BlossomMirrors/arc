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

pub fn update(visible: bool, progress: f64) {
    let Some(conn) = CONNECTION.get().cloned() else {
        return;
    };
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
