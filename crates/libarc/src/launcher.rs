use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::OnceLock;
use zbus::zvariant::Value;

const APP_URI: &str = "application://org.blossomos.Arc.desktop";

enum Msg {
    Transactions { active: i32, progress: f64 },
    Updates(i32),
}

static SENDER: OnceLock<mpsc::Sender<Msg>> = OnceLock::new();

fn sender() -> &'static mpsc::Sender<Msg> {
    SENDER.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || run(rx));
        tx
    })
}

pub fn update_transactions(active: i32, progress: f64) {
    let _ = sender().send(Msg::Transactions { active, progress });
}

pub fn update_badge(updates: i32) {
    let _ = sender().send(Msg::Updates(updates));
}

pub fn clear_blocking() {
    if let Ok(conn) = zbus::blocking::Connection::session() {
        emit(&conn, 0, false, 0.0, false);
    }
}

fn emit(
    conn: &zbus::blocking::Connection,
    count: i64,
    count_visible: bool,
    progress: f64,
    progress_visible: bool,
) {
    let mut props: HashMap<&str, Value> = HashMap::new();
    props.insert("count", count.into());
    props.insert("count-visible", count_visible.into());
    props.insert("progress", progress.into());
    props.insert("progress-visible", progress_visible.into());
    let _ = conn.emit_signal(
        None::<zbus::names::BusName>,
        "/org/blossomos/arc/LauncherEntry",
        "com.canonical.Unity.LauncherEntry",
        "Update",
        &(APP_URI, props),
    );
}

fn run(rx: mpsc::Receiver<Msg>) {
    let Ok(conn) = zbus::blocking::Connection::session() else {
        while rx.recv().is_ok() {}
        return;
    };
    let mut active = 0i32;
    let mut progress = 0f64;
    let mut updates = 0i32;
    while let Ok(msg) = rx.recv() {
        match msg {
            Msg::Transactions {
                active: a,
                progress: p,
            } => {
                active = a;
                progress = p;
            }
            Msg::Updates(u) => updates = u,
        }
        let count = (active + updates) as i64;
        emit(&conn, count, count > 0, progress, active > 0);
    }
}
