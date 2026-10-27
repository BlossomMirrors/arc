#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        #[qproperty(bool, first_frame_shown, cxx_name = "firstFrameShown")]
        type StartupController = super::StartupControllerRust;

        #[qinvokable]
        #[cxx_name = "firstFrame"]
        fn first_frame(self: Pin<&mut StartupController>);
    }
}

use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::Instant;
use tokio::sync::Notify;

static PROCESS_START: OnceLock<Instant> = OnceLock::new();
static FIRST_FRAME: AtomicBool = AtomicBool::new(false);
static FIRST_FRAME_NOTIFY: Notify = Notify::const_new();

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

#[derive(Default)]
pub struct StartupControllerRust {
    first_frame_shown: bool,
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

}
