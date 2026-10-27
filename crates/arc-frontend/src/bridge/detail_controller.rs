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
        #[qproperty(QString, remote)]
        #[qproperty(QString, name)]
        #[qproperty(QString, summary)]
        #[qproperty(QString, description)]
        #[qproperty(QString, icon_url, cxx_name = "iconUrl")]
        #[qproperty(QString, developer_name, cxx_name = "developerName")]
        #[qproperty(bool, verified)]
        #[qproperty(QString, homepage_url, cxx_name = "homepageUrl")]
        #[qproperty(QString, project_urls_json, cxx_name = "projectUrlsJson")]
        #[qproperty(QString, content_rating, cxx_name = "contentRating")]
        #[qproperty(QString, version)]
        #[qproperty(QString, license)]
        #[qproperty(QString, eula_url, cxx_name = "eulaUrl")]
        #[qproperty(QString, extensions_json, cxx_name = "extensionsJson")]
        #[qproperty(bool, installed)]
        #[qproperty(bool, busy)]
        #[qproperty(f32, progress)]
        #[qproperty(QStringList, screenshots)]
        #[qproperty(QStringList, full_screenshots, cxx_name = "fullScreenshots")]
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
        fn prefetch(self: Pin<&mut DetailController>, pkg_id: QString);

        #[qinvokable]
        fn launch(self: Pin<&mut DetailController>);

        #[qinvokable]
        #[cxx_name = "refreshExtensions"]
        fn refresh_extensions(self: Pin<&mut DetailController>);

        #[qinvokable]
        fn clear(self: Pin<&mut DetailController>);

        #[qinvokable]
        #[cxx_name = "setShotWidth"]
        fn set_shot_width(self: Pin<&mut DetailController>, shot_width: i32);
    }

    impl cxx_qt::Threading for DetailController {}
}

use crate::runtime;
use cxx_qt::{CxxQtThread, Threading};
use cxx_qt_lib::{QList, QString, QStringList};
use std::pin::Pin;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tokio::task::spawn_blocking;

static SHOT_WIDTH: AtomicU32 = AtomicU32::new(0);

fn shot_width() -> Option<u32> {
    let width = SHOT_WIDTH.load(Ordering::Relaxed);
    (width > 0).then_some(width)
}

pub struct DetailControllerRust {
    loading: bool,
    refining: bool,
    id: QString,
    remote: QString,
    name: QString,
    summary: QString,
    description: QString,
    icon_url: QString,
    developer_name: QString,
    verified: bool,
    homepage_url: QString,
    project_urls_json: QString,
    content_rating: QString,
    version: QString,
    license: QString,
    eula_url: QString,
    extensions_json: QString,
    installed: bool,
    busy: bool,
    progress: f32,
    screenshots: QStringList,
    full_screenshots: QStringList,
}

impl Default for DetailControllerRust {
    // starts true so the skeleton covers the very first frame too
    // load()/loadWithSeed() haven't run yet at construction time
    fn default() -> Self {
        Self {
            loading: true,
            refining: false,
            id: QString::default(),
            remote: QString::default(),
            name: QString::default(),
            summary: QString::default(),
            description: QString::default(),
            icon_url: QString::default(),
            developer_name: QString::default(),
            verified: false,
            homepage_url: QString::default(),
            project_urls_json: QString::from("{}"),
            content_rating: QString::default(),
            version: QString::default(),
            license: QString::default(),
            eula_url: QString::default(),
            extensions_json: QString::default(),
            installed: false,
            busy: false,
            progress: 0.0,
            screenshots: QStringList::default(),
            full_screenshots: QStringList::default(),
        }
    }
}

static QT_THREAD: OnceLock<CxxQtThread<qobject::DetailController>> = OnceLock::new();

#[derive(Clone, Default)]
struct Detail {
    remote: String,
    name: String,
    summary: String,
    description: String,
    icon_url: String,
    developer_name: String,
    verified: bool,
    homepage_url: String,
    project_urls_json: String,
    content_rating: String,
    version: String,
    license: String,
    eula_url: String,
    installed: bool,
    screenshots: Vec<String>,
    full_screenshots: Vec<String>,
}

fn apply_detail(mut this: Pin<&mut qobject::DetailController>, d: &Detail) {
    this.as_mut().set_remote(QString::from(&d.remote));
    this.as_mut().set_name(QString::from(&d.name));
    this.as_mut().set_summary(QString::from(&d.summary));
    this.as_mut().set_description(QString::from(&d.description));
    this.as_mut().set_icon_url(QString::from(&d.icon_url));
    this.as_mut()
        .set_developer_name(QString::from(&d.developer_name));
    this.as_mut().set_verified(d.verified);
    this.as_mut()
        .set_homepage_url(QString::from(&d.homepage_url));
    this.as_mut()
        .set_project_urls_json(QString::from(&d.project_urls_json));
    this.as_mut()
        .set_content_rating(QString::from(&d.content_rating));
    this.as_mut().set_version(QString::from(&d.version));
    this.as_mut().set_license(QString::from(&d.license));
    this.as_mut().set_eula_url(QString::from(&d.eula_url));
    this.as_mut().set_installed(d.installed);
    let screenshots: QList<QString> = d
        .screenshots
        .iter()
        .map(|s| QString::from(s.as_str()))
        .collect();
    this.as_mut()
        .set_screenshots(QStringList::from(&screenshots));
    let full: QList<QString> = d
        .full_screenshots
        .iter()
        .map(|s| QString::from(s.as_str()))
        .collect();
    this.as_mut().set_full_screenshots(QStringList::from(&full));
}

fn reset_fields(mut this: Pin<&mut qobject::DetailController>) {
    apply_detail(
        this.as_mut(),
        &Detail {
            project_urls_json: "{}".into(),
            ..Default::default()
        },
    );
    this.as_mut().set_extensions_json(QString::default());
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
    let rows: Vec<ExtensionRow> = runtime::call("extensions", proxy.extensions(pkg_id))
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
            let _ = inner_thread.queue(move |mut this| {
                if this.id.to_string() == pkg_id {
                    this.as_mut().set_extensions_json(QString::from(&json));
                }
            });
        });
    });
}

pub fn refresh_current() {
    *PREFETCHED.lock().unwrap_or_else(|p| p.into_inner()) = None;
    let Some(qt_thread) = QT_THREAD.get() else {
        return;
    };
    let _ = qt_thread.queue(|this| {
        let pkg_id = this.id.to_string();
        if pkg_id.is_empty() {
            return;
        }
        if let Some(inner) = QT_THREAD.get() {
            start_fetch(inner.clone(), pkg_id, None);
        }
    });
}

impl qobject::DetailController {
    pub fn set_shot_width(self: Pin<&mut Self>, shot_width: i32) {
        if shot_width > 0 {
            SHOT_WIDTH.store(shot_width as u32, Ordering::Relaxed);
        }
    }

    pub fn prefetch(self: Pin<&mut Self>, pkg_id: QString) {
        let pkg_id = pkg_id.to_string();
        if is_prefetched(&pkg_id) || is_in_flight(&pkg_id) {
            return;
        }
        *IN_FLIGHT.lock().unwrap_or_else(|p| p.into_inner()) = Some(pkg_id.clone());
        let width = shot_width();
        let qt_thread = self.qt_thread();
        runtime::spawn(async move {
            let built = match runtime::proxy().await {
                Some(proxy) => Some(tokio::join!(
                    build_detail(&proxy, &pkg_id, width),
                    fetch_extensions_json(&proxy, &pkg_id)
                )),
                None => None,
            };
            let _ = qt_thread.queue(move |mut this| {
                let mut in_flight = IN_FLIGHT.lock().unwrap_or_else(|p| p.into_inner());
                if in_flight.as_deref() == Some(pkg_id.as_str()) {
                    *in_flight = None;
                }
                drop(in_flight);
                let waiting = this.id.to_string() == pkg_id && (this.loading || this.refining);
                match built {
                    Some((detail, extensions_json)) if waiting => {
                        apply_detail(this.as_mut(), &detail);
                        this.as_mut().set_extensions_json(QString::from(&extensions_json));
                        this.as_mut().set_loading(false);
                        this.as_mut().set_refining(false);
                    }
                    Some((detail, extensions_json)) => {
                        *PREFETCHED.lock().unwrap_or_else(|p| p.into_inner()) =
                            Some(Prefetched { id: pkg_id, detail, extensions_json, at: Instant::now() });
                    }
                    None if waiting => {
                        let qt_thread = this.qt_thread();
                        start_fetch(qt_thread, pkg_id, None);
                    }
                    None => {}
                }
            });
        });
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
        reset_fields(self.as_mut());
        self.as_mut().set_loading(true);
        self.as_mut().set_refining(false);

        let qt_thread = self.qt_thread();
        let _ = QT_THREAD.set(qt_thread.clone());
        if apply_prefetched(self.as_mut(), &pkg_id_str) || is_in_flight(&pkg_id_str) {
            return;
        }
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
        reset_fields(self.as_mut());
        self.as_mut().set_name(name);
        self.as_mut().set_summary(summary);
        self.as_mut().set_icon_url(icon_url);
        self.as_mut().set_installed(installed);
        self.as_mut().set_refining(true);
        self.as_mut().set_loading(false);

        let qt_thread = self.qt_thread();
        let _ = QT_THREAD.set(qt_thread.clone());
        if apply_prefetched(self.as_mut(), &pkg_id_str) || is_in_flight(&pkg_id_str) {
            return;
        }
        start_fetch(qt_thread, pkg_id_str, None);
    }

    pub fn refresh_extensions(self: Pin<&mut Self>) {
        refresh_extensions_for_current();
    }

    pub fn clear(mut self: Pin<&mut Self>) {
        self.as_mut().set_id(QString::default());
        reset_fields(self.as_mut());
    }
}

pub fn prefetch_first_shots(ids: Vec<String>) {
    if ids.is_empty() {
        return;
    }
    let width = shot_width().unwrap_or(0);
    runtime::spawn(async move {
        use futures_util::StreamExt;
        crate::bridge::startup_controller::after_first_frame().await;
        let Some(proxy) = runtime::proxy().await else {
            return;
        };
        let mut firsts: Vec<String> = futures_util::stream::iter(ids)
            .map(|id| {
                let proxy = proxy.clone();
                async move {
                    runtime::call("metadata", proxy.app_metadata(&id))
                        .await
                        .ok()?
                        .screenshots
                        .into_iter()
                        .next()
                }
            })
            .buffer_unordered(4)
            .filter_map(|first| async move { first })
            .collect()
            .await;
        crate::services::images::resolve_urls(
            Some(&proxy),
            firsts.iter_mut().collect(),
            libarc::ImageKind::Image,
            width,
        )
        .await;
    });
}

async fn build_detail(
    proxy: &libarc::ArcDaemonProxy<'static>,
    pkg_id: &str,
    shot_width: Option<u32>,
) -> Detail {
    let (info, meta) = tokio::join!(
        runtime::call("app info", proxy.app_info(pkg_id)),
        runtime::call("metadata", proxy.app_metadata(pkg_id)),
    );
    let package = info.ok().flatten();
    let meta = meta.unwrap_or_default();

    let screenshots = if meta.screenshots.is_empty() {
        package
            .as_ref()
            .map(|p| p.screenshots.clone())
            .unwrap_or_default()
    } else {
        meta.screenshots.clone()
    };

    let mut shown = screenshots.clone();
    let width = shot_width.or_else(self::shot_width).unwrap_or(0);
    crate::services::images::resolve_urls(
        Some(proxy),
        shown.iter_mut().collect(),
        libarc::ImageKind::Image,
        width,
    )
    .await;

    let pkg_id_for_icon = pkg_id.to_string();
    let raw_icon_url = package.as_ref().and_then(|p| p.icon_url.clone());
    let mut icon_url = spawn_blocking(move || {
        crate::services::icons::resolve(&pkg_id_for_icon, raw_icon_url.as_deref())
    })
    .await
    .unwrap_or_default();
    crate::services::images::resolve_urls(
        Some(proxy),
        vec![&mut icon_url],
        libarc::ImageKind::Icon,
        0,
    )
    .await;

    let summary = if meta.summary.is_empty() {
        package
            .as_ref()
            .map(|p| p.description.clone())
            .unwrap_or_default()
    } else {
        meta.summary
    };
    Detail {
        remote: package
            .as_ref()
            .and_then(|p| p.remote.clone())
            .unwrap_or_default(),
        name: package.as_ref().map(|p| p.name.clone()).unwrap_or_default(),
        summary,
        description: meta.description,
        icon_url,
        developer_name: meta.developer_name.unwrap_or_default(),
        verified: meta.verified,
        homepage_url: meta.homepage_url.unwrap_or_default(),
        project_urls_json: serde_json::to_string(&meta.project_urls)
            .unwrap_or_else(|_| "{}".to_string()),
        content_rating: meta.content_rating,
        version: package
            .as_ref()
            .map(|p| p.version.clone())
            .unwrap_or_default(),
        license: meta.license.unwrap_or_default(),
        eula_url: meta.eula_url.unwrap_or_default(),
        installed: package.as_ref().map(|p| p.installed).unwrap_or(false),
        screenshots: shown,
        full_screenshots: screenshots,
    }
}

struct Prefetched {
    id: String,
    detail: Detail,
    extensions_json: String,
    at: Instant,
}

const PREFETCH_FRESH: Duration = Duration::from_secs(60);

static PREFETCHED: Mutex<Option<Prefetched>> = Mutex::new(None);
static IN_FLIGHT: Mutex<Option<String>> = Mutex::new(None);

fn is_in_flight(id: &str) -> bool {
    IN_FLIGHT.lock().unwrap_or_else(|p| p.into_inner()).as_deref() == Some(id)
}

fn take_prefetched(id: &str) -> Option<Prefetched> {
    let mut slot = PREFETCHED.lock().unwrap_or_else(|p| p.into_inner());
    let fresh = slot
        .as_ref()
        .is_some_and(|p| p.id == id && p.at.elapsed() < PREFETCH_FRESH);
    if fresh {
        slot.take()
    } else {
        None
    }
}

fn is_prefetched(id: &str) -> bool {
    let slot = PREFETCHED.lock().unwrap_or_else(|p| p.into_inner());
    slot.as_ref()
        .is_some_and(|p| p.id == id && p.at.elapsed() < PREFETCH_FRESH)
}

fn apply_prefetched(mut this: Pin<&mut qobject::DetailController>, id: &str) -> bool {
    let Some(p) = take_prefetched(id) else {
        return false;
    };
    if p.detail.name.is_empty() && p.detail.description.is_empty() {
        return false;
    }
    apply_detail(this.as_mut(), &p.detail);
    this.as_mut()
        .set_extensions_json(QString::from(&p.extensions_json));
    this.as_mut().set_loading(false);
    this.as_mut().set_refining(false);
    true
}

fn start_fetch(
    qt_thread: CxxQtThread<qobject::DetailController>,
    pkg_id: String,
    shot_width: Option<u32>,
) {
    let ext_pkg_id = pkg_id.clone();
    let ext_qt_thread = qt_thread.clone();
    runtime::spawn(async move {
        let Some(proxy) = runtime::proxy().await else {
            return;
        };
        let json = fetch_extensions_json(&proxy, &ext_pkg_id).await;
        let _ = ext_qt_thread.queue(move |mut this| {
            if this.id.to_string() == ext_pkg_id {
                this.as_mut().set_extensions_json(QString::from(&json));
            }
        });
    });

    runtime::spawn(async move {
        let Some(proxy) = runtime::proxy().await else {
            let _ = qt_thread.queue(|mut this| {
                this.as_mut().set_loading(false);
                this.as_mut().set_refining(false);
            });
            return;
        };
        let detail = build_detail(&proxy, &pkg_id, shot_width).await;
        let _ = qt_thread.queue(move |mut this| {
            if this.id.to_string() != pkg_id {
                return;
            }
            let found = !detail.name.is_empty() || !detail.description.is_empty();
            if found {
                let seeded_name = this.name.to_string();
                let mut detail = detail;
                if detail.name.is_empty() {
                    detail.name = seeded_name;
                }
                apply_detail(this.as_mut(), &detail);
            }
            this.as_mut().set_loading(false);
            this.as_mut().set_refining(false);
        });
    });
}
