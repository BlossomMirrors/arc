#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        type WindowController = super::WindowControllerRust;

        #[qinvokable]
        #[cxx_name = "setVisible"]
        fn set_visible(self: Pin<&mut WindowController>, visible: bool);
    }
}

use crate::runtime;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};

static REPORTED: AtomicBool = AtomicBool::new(false);

#[derive(Default)]
pub struct WindowControllerRust;

impl qobject::WindowController {
    pub fn set_visible(self: Pin<&mut Self>, visible: bool) {
        report_visible(visible);
    }
}

pub fn report_visible(visible: bool) {
    if REPORTED.swap(visible, Ordering::Relaxed) == visible {
        return;
    }
    runtime::spawn(async move {
        if let Some(proxy) = runtime::proxy().await {
            let _ = proxy.set_frontend_visible(visible).await;
        }
    });
}
