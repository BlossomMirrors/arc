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
        #[qproperty(bool, busy)]
        #[qproperty(QString, leftover_json, cxx_name = "leftoverJson")]
        type LeftoverDataController = super::LeftoverDataControllerRust;

        #[qinvokable]
        fn check(self: Pin<&mut LeftoverDataController>);

        #[qinvokable]
        #[cxx_name = "cleanupOne"]
        fn cleanup_one(self: Pin<&mut LeftoverDataController>, id: QString);
    }

    impl cxx_qt::Threading for LeftoverDataController {}
}

use crate::runtime;
use cxx_qt::Threading;
use cxx_qt_lib::QString;
use std::pin::Pin;

pub struct LeftoverDataControllerRust {
    busy: bool,
    leftover_json: QString,
}

impl Default for LeftoverDataControllerRust {
    fn default() -> Self {
        Self {
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

            let _ = qt_thread.queue(move |mut this| {
                this.as_mut().set_leftover_json(QString::from(&json));
            });
        });
    }

    pub fn cleanup_one(mut self: Pin<&mut Self>, id: QString) {
        let id = id.to_string();

        let remaining: Vec<serde_json::Value> =
            serde_json::from_str::<Vec<serde_json::Value>>(&self.leftover_json.to_string())
                .unwrap_or_default()
                .into_iter()
                .filter(|e| e.get("id").and_then(|v| v.as_str()) != Some(id.as_str()))
                .collect();
        let remaining_json = serde_json::to_string(&remaining).unwrap_or_else(|_| "[]".to_string());

        self.as_mut().set_busy(true);
        let qt_thread = self.qt_thread();
        runtime::spawn(async move {
            if let Some(proxy) = runtime::proxy().await {
                let _ = proxy.delete_leftover_data(vec![id]).await;
            }
            let _ = qt_thread.queue(move |mut this| {
                this.as_mut().set_busy(false);
                this.as_mut().set_leftover_json(QString::from(&remaining_json));
            });
        });
    }
}
