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
    fn arc_single_instance(
        on_activate: extern "C" fn(args: *const *const std::ffi::c_char, count: i32) -> bool,
    );
    fn arc_prepare_hidden(engine: *mut c_void);
}

// glibc gives every thread its own heap arena that keeps freed memory so this has to happen before any thread starts
fn tune_allocator() {
    unsafe {
        libc::mallopt(libc::M_ARENA_MAX, 2);
        libc::mallopt(libc::M_MMAP_THRESHOLD, 128 * 1024);
    }
}

fn forward_to_running_instance() -> bool {
    let Ok(connection) = zbus::blocking::Connection::session() else {
        return false;
    };
    // calling the name while nobody owns it would make dbus start another arc through the service file
    let running = zbus::blocking::fdo::DBusProxy::new(&connection)
        .ok()
        .and_then(|dbus| {
            dbus.name_has_owner(zbus::names::BusName::from_static_str("org.blossomos.Arc").ok()?)
                .ok()
        })
        .unwrap_or(false);
    if !running {
        return false;
    }
    let args: Vec<String> = env::args().collect();
    let working_dir = env::current_dir()
        .map(|d| d.display().to_string())
        .unwrap_or_default();
    let mut platform_data: std::collections::HashMap<&str, zbus::zvariant::Value> =
        std::collections::HashMap::new();
    if let Ok(token) = env::var("XDG_ACTIVATION_TOKEN") {
        platform_data.insert("activation-token", token.into());
    }
    if let Ok(id) = env::var("DESKTOP_STARTUP_ID") {
        platform_data.insert("desktop-startup-id", id.into());
    }
    connection
        .call_method(
            Some("org.blossomos.Arc"),
            "/org/blossomos/Arc",
            Some("org.kde.KDBusService"),
            "CommandLine",
            &(args, working_dir, platform_data),
        )
        .is_ok()
}

fn main() {
    bridge::startup_controller::mark_process_start();
    tune_allocator();
    let standby = env::args().any(|a| a == bridge::startup_controller::STANDBY_ARG);
    if !standby && forward_to_running_instance() {
        return;
    }
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

        if standby {
            bridge::startup_controller::set_standby();
        } else if services::deeplink::pending_opens_page() {
            bridge::startup_controller::show_window_later();
        }

        Pin::as_mut(&mut engine).load(&QUrl::from(bridge::startup_controller::MAIN_QML));

        let has_root = unsafe {
            let raw: *mut QQmlApplicationEngine = Pin::as_mut(&mut engine).get_unchecked_mut();
            arc_engine_has_root(raw as *mut c_void)
        };
        if !has_root {
            tracing::error!("Main.qml failed to load, see the qml errors above");
            std::process::exit(1);
        }
        bridge::startup_controller::log_step("qml loaded");
        if standby {
            unsafe {
                let raw: *mut QQmlApplicationEngine = Pin::as_mut(&mut engine).get_unchecked_mut();
                arc_prepare_hidden(raw as *mut c_void);
            }
            bridge::startup_controller::log_step("hidden window ready");
        }
    }

    if let Some(app) = app.as_mut() {
        bridge::startup_controller::log_step("event loop");
        app.exec();
    }

    libarc::launcher::clear_blocking();
}
