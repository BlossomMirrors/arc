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

        #[qinvokable]
        fn prefetch(self: Pin<&mut ListController>, slug: QString);

        #[qinvokable]
        #[cxx_name = "prefetchCategory"]
        fn prefetch_category(self: Pin<&mut ListController>, category: QString);

        #[qinvokable]
        fn clear(self: Pin<&mut ListController>);
    }

    impl cxx_qt::Threading for ListController {}
}

use crate::runtime;
use crate::services::forge;
use cxx_qt::CxxQtType;
use cxx_qt::Threading;
use cxx_qt_lib::QString;
use std::pin::Pin;
use std::sync::Mutex;
use std::time::{Duration, Instant};

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
        if self.slug.to_string() == slug && !self.apps_json.to_string().is_empty() && !self.loading
        {
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

    pub fn clear(mut self: Pin<&mut Self>) {
        let seq = self.request_seq.wrapping_add(1);
        self.as_mut().rust_mut().request_seq = seq;
        self.as_mut().set_slug(QString::default());
        self.as_mut().set_apps_json(QString::from("[]"));
        self.as_mut().set_description(QString::default());
        self.as_mut().set_loading(false);
    }

    pub fn prefetch(self: Pin<&mut Self>, slug: QString) {
        let slug = slug.to_string();
        if prefetched_list_is_fresh(&slug) {
            return;
        }
        runtime::spawn(async move {
            let key = list_key(&slug);
            let stored = tokio::task::spawn_blocking(move || crate::cache::list_pages().get(&key))
                .await
                .ok()
                .flatten();
            let raw = match stored {
                Some(data) => data,
                None => match crate::services::home::load_list(&slug).await {
                    Some(data) => {
                        let key = list_key(&slug);
                        let store = data.clone();
                        let _ = tokio::task::spawn_blocking(move || crate::cache::list_pages().insert(key, store)).await;
                        data
                    }
                    None => return,
                },
            };
            let display = resolved(raw.clone()).await;
            *PREFETCHED_LIST.lock().unwrap_or_else(|p| p.into_inner()) =
                Some(PrefetchedList { slug, raw, display, at: Instant::now() });
        });
    }

    pub fn prefetch_category(self: Pin<&mut Self>, category: QString) {
        crate::bridge::models::prefetch_category(category.to_string());
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

        let ready = take_prefetched_list(&slug);
        match &ready {
            Some(p) => {
                self.as_mut().set_description(QString::from(&p.display.description));
                self.as_mut().set_apps_json(QString::from(&serde_json::to_string(&p.display.cards).unwrap_or_else(|_| "[]".into())));
                self.as_mut().set_loading(false);
            }
            None => {
                self.as_mut().set_apps_json(QString::from("[]"));
                self.as_mut().set_description(QString::default());
                self.as_mut().set_loading(true);
            }
        }

        let qt_thread = self.qt_thread();
        runtime::spawn(async move {
            let key = list_key(&slug);
            let stored = match ready {
                Some(p) => Some(p.raw),
                None => {
                    let stored_key = key.clone();
                    let stored = tokio::task::spawn_blocking(move || crate::cache::list_pages().get(&stored_key))
                        .await
                        .ok()
                        .flatten();
                    if let Some(data) = &stored {
                        show(&qt_thread, seq, &resolved(data.clone()).await);
                    }
                    stored
                }
            };
            let stored_json = stored.as_ref().map(|d| serde_json::to_string(d).unwrap_or_default());

            let fresh = crate::services::home::load_list(&slug).await;
            match fresh {
                Some(data) => {
                    let fresh_json = serde_json::to_string(&data).unwrap_or_default();
                    if stored_json.as_deref() != Some(fresh_json.as_str()) {
                        show(&qt_thread, seq, &resolved(data.clone()).await);
                        let _ = tokio::task::spawn_blocking(move || crate::cache::list_pages().insert(key, data)).await;
                    }
                }
                None if stored_json.is_none() => show(&qt_thread, seq, &Default::default()),
                None => {}
            }
        });
    }
}

struct PrefetchedList {
    slug: String,
    raw: crate::services::home::ListData,
    display: crate::services::home::ListData,
    at: Instant,
}

const PREFETCH_FRESH: Duration = Duration::from_secs(60);

static PREFETCHED_LIST: Mutex<Option<PrefetchedList>> = Mutex::new(None);

fn list_key(slug: &str) -> String {
    format!("{}:{slug}", forge::user_lang())
}

fn prefetched_list_is_fresh(slug: &str) -> bool {
    PREFETCHED_LIST
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .as_ref()
        .is_some_and(|p| p.slug == slug && p.at.elapsed() < PREFETCH_FRESH)
}

fn take_prefetched_list(slug: &str) -> Option<PrefetchedList> {
    let mut slot = PREFETCHED_LIST.lock().unwrap_or_else(|p| p.into_inner());
    let fresh = slot.as_ref().is_some_and(|p| p.slug == slug && p.at.elapsed() < PREFETCH_FRESH);
    if fresh {
        slot.take()
    } else {
        None
    }
}

async fn resolved(mut data: crate::services::home::ListData) -> crate::services::home::ListData {
    let proxy = runtime::proxy().await;
    crate::services::images::resolve_cards(proxy.as_ref(), &mut data.cards).await;
    data
}

fn show(
    qt_thread: &cxx_qt::CxxQtThread<qobject::ListController>,
    seq: u64,
    data: &crate::services::home::ListData,
) {
    let description = data.description.clone();
    let apps_json = serde_json::to_string(&data.cards).unwrap_or_else(|_| "[]".into());
    let _ = qt_thread.queue(move |mut this| {
        if this.request_seq != seq {
            return;
        }
        this.as_mut().set_description(QString::from(&description));
        this.as_mut().set_apps_json(QString::from(&apps_json));
        this.as_mut().set_loading(false);
    });
}
