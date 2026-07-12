#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        #[qproperty(bool, can_go_back, cxx_name = "canGoBack")]
        #[qproperty(bool, can_go_forward, cxx_name = "canGoForward")]
        type NavController = super::NavControllerRust;

        #[qinvokable]
        fn record(self: Pin<&mut NavController>, state: QString);

        #[qinvokable]
        #[cxx_name = "goBack"]
        fn go_back(self: Pin<&mut NavController>);

        #[qinvokable]
        #[cxx_name = "goForward"]
        fn go_forward(self: Pin<&mut NavController>);

        #[qsignal]
        #[cxx_name = "restoreRequested"]
        fn restore_requested(self: Pin<&mut NavController>, state: QString);
    }
}

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use std::pin::Pin;

#[derive(Default)]
pub struct NavControllerRust {
    entries: Vec<String>,
    cursor: usize,
    can_go_back: bool,
    can_go_forward: bool,
}

impl qobject::NavController {
    fn update_flags(mut self: Pin<&mut Self>) {
        let back = self.cursor > 0;
        let forward = !self.entries.is_empty() && self.cursor < self.entries.len() - 1;
        self.as_mut().set_can_go_back(back);
        self.as_mut().set_can_go_forward(forward);
    }

    pub fn record(mut self: Pin<&mut Self>, state: QString) {
        let state = state.to_string();
        if self.entries.get(self.cursor) == Some(&state) {
            return;
        }
        {
            let cursor = self.cursor;
            let mut rust = self.as_mut().rust_mut();
            if !rust.entries.is_empty() {
                rust.entries.truncate(cursor + 1);
            }
            rust.entries.push(state);
            rust.cursor = rust.entries.len() - 1;
        }
        self.update_flags();
    }

    pub fn go_back(mut self: Pin<&mut Self>) {
        if self.cursor == 0 {
            return;
        }
        self.as_mut().rust_mut().cursor -= 1;
        let state = self.entries[self.cursor].clone();
        self.as_mut().update_flags();
        self.restore_requested(QString::from(&state));
    }

    pub fn go_forward(mut self: Pin<&mut Self>) {
        if self.entries.is_empty() || self.cursor >= self.entries.len() - 1 {
            return;
        }
        self.as_mut().rust_mut().cursor += 1;
        let state = self.entries[self.cursor].clone();
        self.as_mut().update_flags();
        self.restore_requested(QString::from(&state));
    }
}
