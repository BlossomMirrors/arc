#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;

        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        #[qproperty(bool, loading)]
        #[qproperty(bool, refining)]
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
        #[qproperty(QStringList, screenshots)]
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
        fn prefetch(self: Pin<&mut DetailController>, pkg_id: QString, shot_width: i32);

        #[qinvokable]
        fn launch(self: Pin<&mut DetailController>);

        #[qinvokable]
        #[cxx_name = "refreshExtensions"]
        fn refresh_extensions(self: Pin<&mut DetailController>);
    }

    impl cxx_qt::Threading for DetailController {}
}

use crate::runtime;
use cxx_qt::{CxxQtThread, Threading};
use cxx_qt_lib::{QList, QString, QStringList};
use libarc::cache::PersistentMap;
use std::pin::Pin;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::task::spawn_blocking;

pub struct DetailControllerRust {
    loading: bool,
    refining: bool,
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
    screenshots: QStringList,
}

impl Default for DetailControllerRust {
    // starts true so the skeleton covers the very first frame too
    // load()/loadWithSeed() haven't run yet at construction time
    fn default() -> Self {
        Self {
            loading: true,
            refining: false,
            id: QString::default(),
            name: QString::default(),
            summary: QString::default(),
            description: QString::default(),
            icon_url: QString::default(),
            developer_name: QString::default(),
            homepage_url: QString::default(),
            content_rating: QString::default(),
            version: QString::default(),
            license: QString::default(),
            eula_url: QString::default(),
            extensions_json: QString::default(),
            installed: false,
            busy: false,
            progress: 0.0,
            screenshots: QStringList::default(),
        }
    }
}

static QT_THREAD: OnceLock<CxxQtThread<qobject::DetailController>> = OnceLock::new();

static WARM_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn warm_client() -> &'static reqwest::Client {
    WARM_CLIENT.get_or_init(reqwest::Client::new)
}

// repeat visits render this instantly then revalidate in the background
#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
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

static DETAILS: OnceLock<PersistentMap<CachedDetail>> = OnceLock::new();

fn details() -> &'static PersistentMap<CachedDetail> {
    DETAILS.get_or_init(|| {
        PersistentMap::new("frontend", "details.json")
            .with_entry_ttl(Duration::from_secs(7 * 24 * 3600))
            .with_capacity_limit(500)
    })
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
    this.as_mut().set_screenshots(QStringList::from(&screenshots));
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
    details().update(pkg_id, |c| c.installed = installed);

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

#[derive(serde::Serialize)]
struct ExtensionRow {
    id: String,
    name: String,
    installed: bool,
}

async fn fetch_extensions_json(proxy: &libarc::ArcDaemonProxy<'static>, pkg_id: &str) -> String {
    let rows: Vec<ExtensionRow> = proxy
        .extensions(pkg_id)
        .await
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
            details().update(&pkg_id, |c| c.extensions_json = json.clone());
            let _ = inner_thread.queue(move |mut this| {
                this.as_mut().set_extensions_json(QString::from(&json));
            });
        });
    });
}

impl qobject::DetailController {
    pub fn prefetch(self: Pin<&mut Self>, pkg_id: QString, shot_width: i32) {
        let pkg_id_str = pkg_id.to_string();
        let width = (shot_width > 0).then_some(shot_width as u32);
        if let Some(cached) = details().get(&pkg_id_str) {
            if !cached.screenshots.is_empty() {
                warm_screenshots(cached.screenshots, width);
            }
            return;
        }
        let qt_thread = self.qt_thread();
        let _ = QT_THREAD.set(qt_thread.clone());
        start_fetch(qt_thread, pkg_id_str, width);
    }

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

        let cached = details().get(&pkg_id_str);
        if let Some(cached) = cached {
            apply_cached(self.as_mut(), &cached);
            self.as_mut().set_loading(false);
            self.as_mut().set_refining(false);
        } else {
            self.as_mut().set_loading(true);
            self.as_mut().set_refining(false);
        }

        let qt_thread = self.qt_thread();
        let _ = QT_THREAD.set(qt_thread.clone());
        start_fetch(qt_thread, pkg_id_str, None);
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

        let cached = details().get(&pkg_id_str);
        if let Some(cached) = cached {
            apply_cached(self.as_mut(), &cached);
            self.as_mut().set_refining(false);
        } else {
            self.as_mut().set_name(name);
            self.as_mut().set_summary(summary);
            self.as_mut().set_icon_url(icon_url);
            self.as_mut().set_installed(installed);
            self.as_mut().set_description(QString::default());
            self.as_mut().set_developer_name(QString::default());
            self.as_mut().set_homepage_url(QString::default());
            self.as_mut().set_content_rating(QString::default());
            self.as_mut().set_version(QString::default());
            self.as_mut().set_license(QString::default());
            self.as_mut().set_eula_url(QString::default());
            self.as_mut().set_extensions_json(QString::default());
            self.as_mut().set_screenshots(QStringList::default());
            self.as_mut().set_refining(true);
        }
        self.as_mut().set_loading(false);

        let qt_thread = self.qt_thread();
        let _ = QT_THREAD.set(qt_thread.clone());
        start_fetch(qt_thread, pkg_id_str, None);
    }

    pub fn refresh_extensions(self: Pin<&mut Self>) {
        refresh_extensions_for_current();
    }
}

fn warm_screenshots(urls: Vec<String>, width: Option<u32>) {
    runtime::spawn(async move {
        use futures_util::StreamExt;
        futures_util::stream::iter(urls)
            .for_each_concurrent(3, |url| async move {
                let target = match width {
                    Some(w) => {
                        let sep = if url.contains('?') { '&' } else { '?' };
                        format!("{url}{sep}w={w}")
                    }
                    None => url,
                };
                let _ = warm_client().get(&target).send().await;
            })
            .await;
    });
}

fn start_fetch(qt_thread: CxxQtThread<qobject::DetailController>, pkg_id: String, shot_width: Option<u32>) {
    let ext_pkg_id = pkg_id.clone();
    let ext_qt_thread = qt_thread.clone();
    runtime::spawn(async move {
        let Some(proxy) = runtime::proxy().await else {
            return;
        };
        let json = fetch_extensions_json(&proxy, &ext_pkg_id).await;
        details().entry_or_default(&ext_pkg_id, |c| c.extensions_json = json.clone());
        let _ = ext_qt_thread.queue(move |mut this| {
            if this.id.to_string() == ext_pkg_id {
                this.as_mut().set_extensions_json(QString::from(&json));
            }
        });
    });

    runtime::spawn(async move {
        let Some(proxy) = runtime::proxy().await else {
            qt_thread
                .queue(|mut this| {
                    this.as_mut().set_loading(false);
                    this.as_mut().set_refining(false);
                })
                .ok();
            return;
        };
        let mut package = None;
        let mut meta = libarc::AppMetadata::default();
        let mut warmed_screenshots = false;
        for attempt in 0..4 {
            let (info, m) = tokio::join!(proxy.app_info(&pkg_id), proxy.app_metadata(&pkg_id));
            package = info.ok().flatten();
            meta = m.unwrap_or_default();

            if !warmed_screenshots {
                let shots = if !meta.screenshots.is_empty() {
                    meta.screenshots.clone()
                } else {
                    package.as_ref().map(|p| p.screenshots.clone()).unwrap_or_default()
                };
                if !shots.is_empty() {
                    warmed_screenshots = true;
                    warm_screenshots(shots, shot_width);
                }
            }

            if package.is_some() || !meta.description.is_empty() {
                break;
            }
            if attempt < 3 {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        }

        let pkg_id_for_icon = pkg_id.clone();
        let raw_icon_url = package.as_ref().and_then(|p| p.icon_url.clone());
        let icon_url = spawn_blocking(move || {
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

        let screenshots = if meta.screenshots.is_empty() {
            package.as_ref().map(|p| p.screenshots.clone()).unwrap_or_default()
        } else {
            meta.screenshots
        };

        let mut cached = CachedDetail {
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
            extensions_json: String::new(),
            installed,
            screenshots,
        };

        if let Some(existing) = details().get(&pkg_id) {
            if !existing.extensions_json.is_empty() {
                cached.extensions_json = existing.extensions_json.clone();
            }
        }
        details().insert(pkg_id.clone(), cached.clone());

        qt_thread
            .queue(move |mut this| {
                // don't clobber if the user navigated elsewhere mid-fetch
                if this.id.to_string() != pkg_id {
                    return;
                }
                apply_cached(this.as_mut(), &cached);
                this.as_mut().set_loading(false);
                this.as_mut().set_refining(false);
            })
            .ok();
    });
}
