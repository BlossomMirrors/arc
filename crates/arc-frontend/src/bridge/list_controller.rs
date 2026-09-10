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
        #[qproperty(bool, loading)]
        #[qproperty(QString, slug)]
        #[qproperty(QString, icon_name, cxx_name = "iconName")]
        #[qproperty(QString, color)]
        #[qproperty(QString, description)]
        #[qproperty(QString, apps_json, cxx_name = "appsJson")]
        #[qproperty(QString, defs_json, cxx_name = "defsJson")]
        type ListController = super::ListControllerRust;

        #[qinvokable]
        fn load(self: Pin<&mut ListController>, slug: QString);

        #[qinvokable]
        fn refresh(self: Pin<&mut ListController>);
    }

    impl cxx_qt::Threading for ListController {}
}

use crate::runtime;
use crate::services::forge;
use cxx_qt::CxxQtType;
use cxx_qt::Threading;
use cxx_qt_lib::QString;
use std::pin::Pin;

pub struct ListControllerRust {
    loading: bool,
    slug: QString,
    icon_name: QString,
    color: QString,
    description: QString,
    apps_json: QString,
    defs_json: QString,
    request_seq: u64,
}

impl Default for ListControllerRust {
    fn default() -> Self {
        let defs: Vec<serde_json::Value> = forge::LISTS
            .iter()
            .map(|l| {
                serde_json::json!({
                    "slug": l.slug,
                    "iconName": l.icon_name,
                    "color": l.color,
                })
            })
            .collect();
        Self {
            loading: false,
            slug: QString::default(),
            icon_name: QString::default(),
            color: QString::default(),
            description: QString::default(),
            apps_json: QString::from("[]"),
            defs_json: QString::from(&serde_json::to_string(&defs).unwrap_or_else(|_| "[]".into())),
            request_seq: 0,
        }
    }
}

impl qobject::ListController {
    pub fn load(mut self: Pin<&mut Self>, slug: QString) {
        let slug = slug.to_string();
        if self.slug.to_string() == slug && !self.apps_json.to_string().is_empty() && !self.loading {
            return;
        }
        self.as_mut().start(slug);
    }

    pub fn refresh(mut self: Pin<&mut Self>) {
        let slug = self.slug.to_string();
        if slug.is_empty() {
            return;
        }
        self.as_mut().start(slug);
    }

    fn start(mut self: Pin<&mut Self>, slug: String) {
        let seq = self.request_seq.wrapping_add(1);
        self.as_mut().rust_mut().request_seq = seq;

        let def = forge::list_def(&slug);
        self.as_mut().set_slug(QString::from(&slug));
        self.as_mut()
            .set_icon_name(QString::from(def.map(|d| d.icon_name).unwrap_or("")));
        self.as_mut()
            .set_color(QString::from(def.map(|d| d.color).unwrap_or("")));
        self.as_mut().set_apps_json(QString::from("[]"));
        self.as_mut().set_description(QString::default());
        self.as_mut().set_loading(true);

        let qt_thread = self.qt_thread();
        runtime::spawn(async move {
            let data = crate::services::home::load_list(&slug).await;
            let apps_json = serde_json::to_string(&data.cards).unwrap_or_else(|_| "[]".into());

            let _ = qt_thread.queue(move |mut this| {
                if this.request_seq != seq {
                    return;
                }
                this.as_mut().set_description(QString::from(&data.description));
                this.as_mut().set_apps_json(QString::from(&apps_json));
                this.as_mut().set_loading(false);
            });
        });
    }
}
