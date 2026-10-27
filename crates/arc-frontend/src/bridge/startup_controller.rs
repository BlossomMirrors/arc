#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        #[qproperty(bool, first_frame_shown, cxx_name = "firstFrameShown")]
        #[qproperty(bool, standby)]
        #[qproperty(bool, start_hidden, cxx_name = "startHidden")]
        type StartupController = super::StartupControllerRust;

        #[qinvokable]
        #[cxx_name = "firstFrame"]
        fn first_frame(self: Pin<&mut StartupController>);

        #[qinvokable]
        #[cxx_name = "showWindow"]
        fn show_window(self: Pin<&mut StartupController>);

        #[qinvokable]
        #[cxx_name = "preloadEnabled"]
        fn preload_enabled(self: &StartupController) -> bool;

        #[qinvokable]
        #[cxx_name = "returnToStandby"]
        fn return_to_standby(self: Pin<&mut StartupController>);
    }
}

use std::os::unix::process::CommandExt;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::Instant;
use tokio::sync::Notify;

static PROCESS_START: OnceLock<Instant> = OnceLock::new();
static FIRST_FRAME: AtomicBool = AtomicBool::new(false);
static FIRST_FRAME_NOTIFY: Notify = Notify::const_new();
static STANDBY: AtomicBool = AtomicBool::new(false);

pub const MAIN_QML: &str = "qrc:/qt/qml/org/blossomos/arc/src/qml/Main.qml";
pub const STANDBY_ARG: &str = "--standby";

pub fn set_standby() {
    STANDBY.store(true, Ordering::Release);
}

pub fn is_standby() -> bool {
    STANDBY.load(Ordering::Acquire)
}

pub fn mark_process_start() {
    let _ = PROCESS_START.set(Instant::now());
}

pub fn log_step(step: &str) {
    if let Some(start) = PROCESS_START.get() {
        tracing::debug!("startup: {step} after {} ms", start.elapsed().as_millis());
    }
}

pub async fn after_first_frame() {
    let notified = FIRST_FRAME_NOTIFY.notified();
    if FIRST_FRAME.load(Ordering::Acquire) {
        return;
    }
    notified.await;
}

extern "C" {
    fn arc_show_window();
    fn arc_show_window_later(delay_ms: i32);
}

const WINDOW_WAIT_MS: i32 = 1500;

pub fn show_window_later() {
    unsafe { arc_show_window_later(WINDOW_WAIT_MS) };
}

pub struct StartupControllerRust {
    first_frame_shown: bool,
    standby: bool,
    start_hidden: bool,
}

impl Default for StartupControllerRust {
    fn default() -> Self {
        Self {
            first_frame_shown: false,
            standby: is_standby(),
            start_hidden: is_standby() || crate::services::deeplink::pending_opens_page(),
        }
    }
}

impl qobject::StartupController {
    pub fn first_frame(mut self: Pin<&mut Self>) {
        if FIRST_FRAME.swap(true, Ordering::AcqRel) {
            return;
        }
        log_step("first frame");
        FIRST_FRAME_NOTIFY.notify_waiters();
        self.as_mut().set_first_frame_shown(true);
    }

    pub fn preload_enabled(&self) -> bool {
        crate::services::autostart::is_enabled()
    }

    pub fn show_window(self: Pin<&mut Self>) {
        unsafe { arc_show_window() };
    }

    pub fn return_to_standby(self: Pin<&mut Self>) {
        let Ok(exe) = std::env::current_exe() else {
            std::process::exit(0);
        };
        let error = std::process::Command::new(exe).arg(STANDBY_ARG).exec();
        tracing::error!("could not restart into standby: {error}");
        std::process::exit(0);
    }
}
