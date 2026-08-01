mod bridge;
mod runtime;
mod services;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QQuickStyle, QString, QUrl};
use cxx_qt_lib_extras::QApplication;
use std::env;
use std::ffi::{c_void, CString};
use std::pin::Pin;

extern "C" {
    fn arc_setup_i18n(engine: *mut c_void, domain: *const std::ffi::c_char);
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    runtime::init();
    services::deeplink::init();

    let mut app = QApplication::new();

    QGuiApplication::set_desktop_file_name(&QString::from("org.blossomos.Arc"));

    if env::var("QT_QUICK_CONTROLS_STYLE").is_err() {
        QQuickStyle::set_style(&QString::from("org.kde.desktop"));
    }

    let mut engine = QQmlApplicationEngine::new();
    if let Some(mut engine) = engine.as_mut() {
        let domain = CString::new("arc-frontend").unwrap();
        unsafe {
            let raw: *mut QQmlApplicationEngine = Pin::as_mut(&mut engine).get_unchecked_mut();
            arc_setup_i18n(raw as *mut c_void, domain.as_ptr());
        }

        engine.load(&QUrl::from(
            "qrc:/qt/qml/org/blossomos/arc/src/qml/Main.qml",
        ));
    }

    if let Some(app) = app.as_mut() {
        app.exec();
    }

    services::launcher::clear_blocking();
    clear_notification_blocking();
}

// the daemon suppresses notifications for the app whose detail page is open
// so make sure that reservation dies with the frontend
fn clear_notification_blocking() {
    let Ok(conn) = zbus::blocking::Connection::session() else {
        return;
    };
    let _ = conn.call_method(
        Some("org.blossomos.arc.daemon"),
        "/org/blossomos/arc/daemon",
        Some("org.blossomos.arc.daemon"),
        "SetForegroundPackage",
        &("",),
    );
    let _ = conn.call_method(
        Some("org.blossomos.arc.daemon"),
        "/org/blossomos/arc/daemon",
        Some("org.blossomos.arc.daemon"),
        "SetFrontendVisible",
        &(false,),
    );
}
