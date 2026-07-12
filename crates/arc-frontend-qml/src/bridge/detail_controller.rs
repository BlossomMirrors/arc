#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;

        include!("cxx-qt-lib/core/qlist/qlist_QString.h");
        type QList_QString = cxx_qt_lib::QList<cxx_qt_lib::QString>;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        #[qproperty(bool, loading)]
        #[qproperty(QString, id)]
        #[qproperty(QString, name)]
        #[qproperty(QString, summary)]
        #[qproperty(QString, description)]
        #[qproperty(QString, icon_url, cxx_name = "iconUrl")]
        #[qproperty(QString, developer_name, cxx_name = "developerName")]
        #[qproperty(QString, homepage_url, cxx_name = "homepageUrl")]
        #[qproperty(QString, content_rating, cxx_name = "contentRating")]
        #[qproperty(QString, version)]
        #[qproperty(QString, license)]
        #[qproperty(QString, eula_url, cxx_name = "eulaUrl")]
        #[qproperty(QString, extensions_json, cxx_name = "extensionsJson")]
        #[qproperty(bool, installed)]
        #[qproperty(bool, busy)]
        #[qproperty(f32, progress)]
        #[qproperty(QList_QString, screenshots)]
        type DetailController = super::DetailControllerRust;

        #[qinvokable]
        fn load(self: Pin<&mut DetailController>, pkg_id: QString);

        // load() but pre-filled from list row data skips the spinner
        #[qinvokable]
        #[cxx_name = "loadWithSeed"]
        fn load_with_seed(
            self: Pin<&mut DetailController>,
            pkg_id: QString,
            name: QString,
            summary: QString,
            icon_url: QString,
            installed: bool,
        );

        #[qinvokable]
        fn launch(self: Pin<&mut DetailController>);

        #[qinvokable]
        #[cxx_name = "refreshExtensions"]
        fn refresh_extensions(self: Pin<&mut DetailController>);

        // detail page reports when it closes so the daemon shows job
        // notifications for this app again
        #[qinvokable]
        #[cxx_name = "pageClosed"]
        fn page_closed(self: Pin<&mut DetailController>, pkg_id: QString);
    }

    impl cxx_qt::Threading for DetailController {}
}

use crate::runtime;
use cxx_qt::{CxxQtThread, Threading};
use cxx_qt_lib::{QList, QString};
use serde::Deserialize;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::{Mutex, OnceLock};

#[derive(Default)]
pub struct DetailControllerRust {
    loading: bool,
    id: QString,
    name: QString,
    summary: QString,
    description: QString,
    icon_url: QString,
    developer_name: QString,
    homepage_url: QString,
    content_rating: QString,
    version: QString,
    license: QString,
    eula_url: QString,
    extensions_json: QString,
    installed: bool,
    busy: bool,
    progress: f32,
    screenshots: QList<QString>,
}

static QT_THREAD: OnceLock<CxxQtThread<qobject::DetailController>> = OnceLock::new();

// repeat visits render this instantly then revalidate in the background
#[derive(Clone, Default)]
struct CachedDetail {
    name: String,
    summary: String,
    description: String,
    icon_url: String,
    developer_name: String,
    homepage_url: String,
    content_rating: String,
    version: String,
    license: String,
    eula_url: String,
    extensions_json: String,
    installed: bool,
    screenshots: Vec<String>,
}

static DETAIL_CACHE: OnceLock<Mutex<HashMap<String, CachedDetail>>> = OnceLock::new();

fn detail_cache() -> &'static Mutex<HashMap<String, CachedDetail>> {
    DETAIL_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn apply_cached(mut this: Pin<&mut qobject::DetailController>, cached: &CachedDetail) {
    this.as_mut().set_name(QString::from(&cached.name));
    this.as_mut().set_summary(QString::from(&cached.summary));
    this.as_mut().set_description(QString::from(&cached.description));
    this.as_mut().set_icon_url(QString::from(&cached.icon_url));
    this.as_mut().set_developer_name(QString::from(&cached.developer_name));
    this.as_mut().set_homepage_url(QString::from(&cached.homepage_url));
    this.as_mut().set_content_rating(QString::from(&cached.content_rating));
    this.as_mut().set_version(QString::from(&cached.version));
    this.as_mut().set_license(QString::from(&cached.license));
    this.as_mut().set_eula_url(QString::from(&cached.eula_url));
    this.as_mut().set_extensions_json(QString::from(&cached.extensions_json));
    this.as_mut().set_installed(cached.installed);
    let screenshots: QList<QString> = cached.screenshots.iter().map(|s| QString::from(s.as_str())).collect();
    this.as_mut().set_screenshots(screenshots);
}

pub fn sync_busy(pkg_id: &str, busy: bool, progress: f32) {
    let Some(qt_thread) = QT_THREAD.get() else {
        return;
    };
    let pkg_id = pkg_id.to_string();
    let _ = qt_thread.queue(move |mut this| {
        if this.id.to_string() == pkg_id {
            this.as_mut().set_busy(busy);
            this.as_mut().set_progress(progress);
        }
    });
}

pub fn sync_installed(pkg_id: &str, installed: bool) {
    if let Some(cached) = detail_cache().lock().unwrap().get_mut(pkg_id) {
        cached.installed = installed;
    }

    let Some(qt_thread) = QT_THREAD.get() else {
        return;
    };
    let pkg_id = pkg_id.to_string();
    let _ = qt_thread.queue(move |mut this| {
        if this.id.to_string() == pkg_id {
            this.as_mut().set_installed(installed);
            this.as_mut().set_busy(false);
            this.as_mut().set_progress(0.0);
        }
    });
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct AppMetadata {
    summary: String,
    description: String,
    icon_url: Option<String>,
    screenshots: Vec<String>,
    homepage_url: Option<String>,
    content_rating: String,
    developer_name: Option<String>,
    license: Option<String>,
    eula_url: Option<String>,
}

#[derive(serde::Serialize)]
struct ExtensionRow {
    id: String,
    name: String,
    installed: bool,
}

async fn fetch_extensions_json(proxy: &libarc::ArcDaemonProxy<'static>, pkg_id: &str) -> String {
    let rows: Vec<ExtensionRow> = proxy
        .list_extensions(pkg_id)
        .await
        .ok()
        .and_then(|j| serde_json::from_str::<Vec<libarc::Package>>(&j).ok())
        .unwrap_or_default()
        .into_iter()
        .map(|p| ExtensionRow {
            id: p.id,
            name: p.name,
            installed: p.installed,
        })
        .collect();
    serde_json::to_string(&rows).unwrap_or_else(|_| "[]".into())
}

pub fn refresh_extensions_for_current() {
    let Some(qt_thread) = QT_THREAD.get() else {
        return;
    };
    let _ = qt_thread.queue(|this| {
        let pkg_id = this.id.to_string();
        if pkg_id.is_empty() {
            return;
        }
        let Some(inner_thread) = QT_THREAD.get() else {
            return;
        };
        let inner_thread = inner_thread.clone();
        runtime::spawn(async move {
            let Some(proxy) = runtime::proxy().await else {
                return;
            };
            let json = fetch_extensions_json(&proxy, &pkg_id).await;
            if let Some(cached) = detail_cache().lock().unwrap().get_mut(&pkg_id) {
                cached.extensions_json = json.clone();
            }
            let _ = inner_thread.queue(move |mut this| {
                this.as_mut().set_extensions_json(QString::from(&json));
            });
        });
    });
}

impl qobject::DetailController {
    pub fn launch(self: Pin<&mut Self>) {
        let pkg_id = self.id.to_string();
        runtime::spawn(async move {
            if let Some(proxy) = runtime::proxy().await {
                if let Err(e) = proxy.run_package(&pkg_id).await {
                    tracing::warn!("failed to launch {pkg_id}: {e}");
                }
            }
        });
    }

    pub fn load(mut self: Pin<&mut Self>, pkg_id: QString) {
        let pkg_id_str = pkg_id.to_string();
        self.as_mut().set_id(pkg_id);

        let cached = detail_cache().lock().unwrap().get(&pkg_id_str).cloned();
        if let Some(cached) = cached {
            apply_cached(self.as_mut(), &cached);
            self.as_mut().set_loading(false);
        } else {
            self.as_mut().set_loading(true);
        }

        set_foreground(pkg_id_str.clone());

        let qt_thread = self.qt_thread();
        let _ = QT_THREAD.set(qt_thread.clone());
        start_fetch(qt_thread, pkg_id_str);
    }

    pub fn load_with_seed(
        mut self: Pin<&mut Self>,
        pkg_id: QString,
        name: QString,
        summary: QString,
        icon_url: QString,
        installed: bool,
    ) {
        let pkg_id_str = pkg_id.to_string();
        self.as_mut().set_id(pkg_id);

        let cached = detail_cache().lock().unwrap().get(&pkg_id_str).cloned();
        if let Some(cached) = cached {
            apply_cached(self.as_mut(), &cached);
        } else {
            self.as_mut().set_name(name);
            self.as_mut().set_summary(summary);
            self.as_mut().set_icon_url(icon_url);
            self.as_mut().set_installed(installed);
        }
        self.as_mut().set_loading(false);

        set_foreground(pkg_id_str.clone());

        let qt_thread = self.qt_thread();
        let _ = QT_THREAD.set(qt_thread.clone());
        start_fetch(qt_thread, pkg_id_str);
    }

    pub fn page_closed(self: Pin<&mut Self>, pkg_id: QString) {
        // only clear if no newer detail page took over in the meantime
        if self.id.to_string() == pkg_id.to_string() {
            set_foreground(String::new());
        }
    }

    pub fn refresh_extensions(self: Pin<&mut Self>) {
        refresh_extensions_for_current();
    }
}

fn set_foreground(pkg_id: String) {
    runtime::spawn(async move {
        if let Some(proxy) = runtime::proxy().await {
            let _ = proxy.set_foreground_package(&pkg_id).await;
        }
    });
}

fn start_fetch(qt_thread: CxxQtThread<qobject::DetailController>, pkg_id: String) {
    runtime::spawn(async move {
        let Some(proxy) = runtime::proxy().await else {
            qt_thread.queue(|mut this| this.as_mut().set_loading(false)).ok();
            return;
        };

        let (info_json, meta_json, extensions_json) = tokio::join!(
            proxy.get_app_info(&pkg_id),
            proxy.get_app_metadata(&pkg_id),
            fetch_extensions_json(&proxy, &pkg_id),
        );

        let package: Option<libarc::Package> = info_json
            .ok()
            .and_then(|j| serde_json::from_str(&j).ok())
            .flatten();
        let meta: AppMetadata = meta_json
            .ok()
            .and_then(|j| serde_json::from_str(&j).ok())
            .unwrap_or_default();

        let pkg_id_for_icon = pkg_id.clone();
        let raw_icon_url = meta
            .icon_url
            .clone()
            .or_else(|| package.as_ref().and_then(|p| p.icon_url.clone()));
        let icon_url = tokio::task::spawn_blocking(move || {
            crate::services::icons::resolve(&pkg_id_for_icon, raw_icon_url.as_deref())
        })
        .await
        .unwrap_or_default();

        let name = package.as_ref().map(|p| p.name.clone()).unwrap_or_default();
        let installed = package.as_ref().map(|p| p.installed).unwrap_or(false);
        let version = package.as_ref().map(|p| p.version.clone()).unwrap_or_default();
        let summary = if meta.summary.is_empty() {
            package.as_ref().map(|p| p.description.clone()).unwrap_or_default()
        } else {
            meta.summary
        };

        let cached = CachedDetail {
            name,
            summary,
            description: meta.description,
            icon_url,
            developer_name: meta.developer_name.unwrap_or_default(),
            homepage_url: meta.homepage_url.unwrap_or_default(),
            content_rating: meta.content_rating,
            version,
            license: meta.license.unwrap_or_default(),
            eula_url: meta.eula_url.unwrap_or_default(),
            extensions_json,
            installed,
            screenshots: meta.screenshots,
        };
        detail_cache().lock().unwrap().insert(pkg_id.clone(), cached.clone());

        qt_thread
            .queue(move |mut this| {
                // don't clobber if the user navigated elsewhere mid-fetch
                if this.id.to_string() != pkg_id {
                    return;
                }
                apply_cached(this.as_mut(), &cached);
                this.as_mut().set_loading(false);
            })
            .ok();
    });
}
