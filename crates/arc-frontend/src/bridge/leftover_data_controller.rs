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
        #[qproperty(bool, has_data, cxx_name = "hasData")]
        #[qproperty(bool, busy)]
        #[qproperty(QString, leftover_json, cxx_name = "leftoverJson")]
        type LeftoverDataController = super::LeftoverDataControllerRust;

        #[qinvokable]
        fn check(self: Pin<&mut LeftoverDataController>);

        #[qinvokable]
        fn cleanup(self: Pin<&mut LeftoverDataController>);

        #[qinvokable]
        fn dismiss(self: Pin<&mut LeftoverDataController>);
    }

    impl cxx_qt::Threading for LeftoverDataController {}
}

use crate::runtime;
use cxx_qt::Threading;
use cxx_qt_lib::QString;
use std::pin::Pin;

pub struct LeftoverDataControllerRust {
    has_data: bool,
    busy: bool,
    leftover_json: QString,
}

impl Default for LeftoverDataControllerRust {
    fn default() -> Self {
        Self {
            has_data: false,
            busy: false,
            leftover_json: QString::from("[]"),
        }
    }
}

impl qobject::LeftoverDataController {
    pub fn check(self: Pin<&mut Self>) {
        let qt_thread = self.qt_thread();
        runtime::spawn(async move {
            let Some(proxy) = runtime::proxy().await else {
                return;
            };
            let Ok(json) = proxy.list_leftover_data().await else {
                return;
            };
            let has_data = serde_json::from_str::<Vec<serde_json::Value>>(&json)
                .map(|v| !v.is_empty())
                .unwrap_or(false);

            let _ = qt_thread.queue(move |mut this| {
                this.as_mut().set_leftover_json(QString::from(&json));
                this.as_mut().set_has_data(has_data);
            });
        });
    }

    pub fn cleanup(mut self: Pin<&mut Self>) {
        #[derive(serde::Deserialize)]
        struct Entry {
            id: String,
        }

        let ids: Vec<String> = serde_json::from_str::<Vec<Entry>>(&self.leftover_json.to_string())
            .unwrap_or_default()
            .into_iter()
            .map(|e| e.id)
            .collect();
        if ids.is_empty() {
            return;
        }

        self.as_mut().set_busy(true);
        let qt_thread = self.qt_thread();
        runtime::spawn(async move {
            if let Some(proxy) = runtime::proxy().await {
                let _ = proxy.delete_leftover_data(ids).await;
            }
            let _ = qt_thread.queue(move |mut this| {
                this.as_mut().set_busy(false);
                this.as_mut().set_has_data(false);
                this.as_mut().set_leftover_json(QString::from("[]"));
            });
        });
    }

    pub fn dismiss(mut self: Pin<&mut Self>) {
        self.as_mut().set_has_data(false);
    }
}
