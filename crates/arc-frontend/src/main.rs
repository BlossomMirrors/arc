mod bridge;
mod cache;
mod runtime;
mod services;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QQuickStyle, QString, QUrl};
use cxx_qt_lib_extras::QApplication;
use std::env;
use std::ffi::{c_void, CString};
use std::pin::Pin;

extern "C" {
    fn arc_setup_i18n(engine: *mut c_void, domain: *const std::ffi::c_char);
    fn arc_add_locale_dir(domain: *const std::ffi::c_char, dir: *const std::ffi::c_char);
    fn arc_install_message_handler();
    fn arc_engine_has_root(engine: *mut c_void) -> bool;
    fn arc_set_icon_theme(name: *const std::ffi::c_char);
    fn arc_single_instance(on_activate: extern "C" fn(args: *const *const std::ffi::c_char, count: i32));
}

fn main() {
    bridge::startup_controller::mark_process_start();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    unsafe { arc_install_message_handler() };

    runtime::init();
    services::home_cache::preload();
    services::deeplink::init();

    let mut app = QApplication::new();
    unsafe { arc_single_instance(bridge::deeplink_controller::on_activate) };

    if let Some(theme) = libarc::icons::system_theme_name() {
        if let Ok(theme) = CString::new(theme) {
            unsafe { arc_set_icon_theme(theme.as_ptr()) };
        }
    }

    QGuiApplication::set_desktop_file_name(&QString::from("org.blossomos.Arc"));

    if env::var("QT_QUICK_CONTROLS_STYLE").is_err() {
        QQuickStyle::set_style(&QString::from("org.kde.desktop"));
    }

    let mut engine = QQmlApplicationEngine::new();
    if let Some(mut engine) = engine.as_mut() {
        let domain = CString::new("arc-frontend").unwrap();
        if let Ok(dir) = CString::new(env!("ARC_LOCALE_DIR")) {
            unsafe { arc_add_locale_dir(domain.as_ptr(), dir.as_ptr()) };
        }
        unsafe {
            let raw: *mut QQmlApplicationEngine = Pin::as_mut(&mut engine).get_unchecked_mut();
            arc_setup_i18n(raw as *mut c_void, domain.as_ptr());
        }

        Pin::as_mut(&mut engine).load(&QUrl::from(
            "qrc:/qt/qml/org/blossomos/arc/src/qml/Main.qml",
        ));

        let has_root = unsafe {
            let raw: *mut QQmlApplicationEngine = Pin::as_mut(&mut engine).get_unchecked_mut();
            arc_engine_has_root(raw as *mut c_void)
        };
        if !has_root {
            tracing::error!("Main.qml failed to load, see the qml errors above");
            std::process::exit(1);
        }
        bridge::startup_controller::log_step("qml loaded");
    }

    if let Some(app) = app.as_mut() {
        bridge::startup_controller::log_step("event loop");
        app.exec();
    }

    libarc::launcher::clear_blocking();
}
