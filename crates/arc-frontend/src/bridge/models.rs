#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++Qt" {
        include!(<QtCore/QAbstractListModel>);
        #[qobject]
        type QAbstractListModel;
    }

    unsafe extern "C++" {
        include!("cxx-qt-lib/qhash.h");
        type QHash_i32_QByteArray = cxx_qt_lib::QHash<cxx_qt_lib::QHashPair_i32_QByteArray>;

        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;

        include!("cxx-qt-lib/qmodelindex.h");
        type QModelIndex = cxx_qt_lib::QModelIndex;

        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;

        include!("cxx-qt-lib/qstringlist.h");
        type QStringList = cxx_qt_lib::QStringList;

        include!("cxx-qt-lib/qlist.h");
        type QList_i32 = cxx_qt_lib::QList<i32>;
    }

    #[qenum(PackageListModel)]
    enum PackageRoles {
        Id,
        Name,
        Version,
        Description,
        Provider,
        Installed,
        IconUrl,
        Busy,
        Progress,
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[base = QAbstractListModel]
        #[qml_element]
        #[qproperty(bool, loading)]
        #[qproperty(QStringList, providers)]
        #[qproperty(i32, total)]
        type PackageListModel = super::PackageListModelRust;

        #[qinvokable]
        fn search(self: Pin<&mut PackageListModel>, query: QString);

        #[qinvokable]
        #[cxx_name = "loadInstalled"]
        fn load_installed(self: Pin<&mut PackageListModel>);

        #[qinvokable]
        #[cxx_name = "searchCategory"]
        fn search_category(self: Pin<&mut PackageListModel>, category: QString);

        #[qinvokable]
        #[cxx_name = "loadUpdates"]
        fn load_updates(self: Pin<&mut PackageListModel>);

        #[qinvokable]
        #[cxx_name = "checkForUpdates"]
        fn check_for_updates(self: Pin<&mut PackageListModel>);

        #[qinvokable]
        #[cxx_name = "setFilters"]
        fn set_filters(
            self: Pin<&mut PackageListModel>,
            provider: QString,
            installed: i32,
            sort_by_name: bool,
        );

        #[qinvokable]
        #[cxx_name = "setSearchText"]
        fn set_search_text(self: Pin<&mut PackageListModel>, text: QString);

        #[qinvokable]
        fn release(self: Pin<&mut PackageListModel>);

        #[qinvokable]
        fn restore(self: Pin<&mut PackageListModel>);
    }

    extern "RustQt" {
        #[qinvokable]
        #[cxx_override]
        fn data(self: &PackageListModel, index: &QModelIndex, role: i32) -> QVariant;

        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "roleNames"]
        fn role_names(self: &PackageListModel) -> QHash_i32_QByteArray;

        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "rowCount"]
        fn row_count(self: &PackageListModel, _parent: &QModelIndex) -> i32;

        #[cxx_override]
        #[cxx_name = "canFetchMore"]
        fn can_fetch_more(self: &PackageListModel, _parent: &QModelIndex) -> bool;

        #[cxx_override]
        #[cxx_name = "fetchMore"]
        fn fetch_more(self: Pin<&mut PackageListModel>, _parent: &QModelIndex);
    }

    unsafe extern "RustQt" {
        #[inherit]
        #[cxx_name = "beginResetModel"]
        unsafe fn begin_reset_model(self: Pin<&mut PackageListModel>);
        #[inherit]
        #[cxx_name = "endResetModel"]
        unsafe fn end_reset_model(self: Pin<&mut PackageListModel>);
        #[inherit]
        #[cxx_name = "beginRemoveRows"]
        unsafe fn begin_remove_rows(
            self: Pin<&mut PackageListModel>,
            parent: &QModelIndex,
            first: i32,
            last: i32,
        );
        #[inherit]
        #[cxx_name = "endRemoveRows"]
        unsafe fn end_remove_rows(self: Pin<&mut PackageListModel>);
        #[inherit]
        #[cxx_name = "beginInsertRows"]
        unsafe fn begin_insert_rows(
            self: Pin<&mut PackageListModel>,
            parent: &QModelIndex,
            first: i32,
            last: i32,
        );
        #[inherit]
        #[cxx_name = "endInsertRows"]
        unsafe fn end_insert_rows(self: Pin<&mut PackageListModel>);
        #[inherit]
        fn index(
            self: &PackageListModel,
            row: i32,
            column: i32,
            parent: &QModelIndex,
        ) -> QModelIndex;
        #[inherit]
        #[cxx_name = "dataChanged"]
        fn data_changed(
            self: Pin<&mut PackageListModel>,
            top_left: &QModelIndex,
            bottom_right: &QModelIndex,
            roles: &QList_i32,
        );
    }

    impl cxx_qt::Threading for PackageListModel {}

    #[qenum(TransactionsModel)]
    enum TransactionRoles {
        Id,
        PkgId,
        Name,
        IconUrl,
        Progress,
        Status,
        TxType,
        Section,
        Error,
        FinishedAt,
        Automatic,
        BytesDone,
        BytesTotal,
        SpeedBps,
        EtaSecs,
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[base = QAbstractListModel]
        #[qml_element]
        #[qml_singleton]
        #[qproperty(i32, active_count, cxx_name = "activeCount")]
        #[qproperty(i32, updates_count, cxx_name = "updatesCount")]
        #[qproperty(i32, running_count, cxx_name = "runningCount")]
        #[qproperty(i32, queued_count, cxx_name = "queuedCount")]
        #[qproperty(i32, done_count, cxx_name = "doneCount")]
        // pkgId to progress 0..1 for every pending or running transaction
        #[qproperty(QString, busy_packages_json, cxx_name = "busyPackagesJson")]
        #[qproperty(bool, history_loaded, cxx_name = "historyLoaded")]
        #[qproperty(bool, updating_all, cxx_name = "updatingAll")]
        type TransactionsModel = super::TransactionsModelRust;

        #[qinvokable]
        fn init(self: Pin<&mut TransactionsModel>);

        #[qinvokable]
        fn install(
            self: Pin<&mut TransactionsModel>,
            pkg_id: QString,
            name: QString,
            icon_url: QString,
        );

        #[qinvokable]
        #[cxx_name = "updateAll"]
        fn update_all(self: Pin<&mut TransactionsModel>);

        // install() with EULA check which emits eulaRequired instead if needed
        #[qinvokable]
        #[cxx_name = "requestInstall"]
        fn request_install(
            self: Pin<&mut TransactionsModel>,
            pkg_id: QString,
            name: QString,
            icon_url: QString,
        );

        #[qsignal]
        #[cxx_name = "eulaRequired"]
        fn eula_required(
            self: Pin<&mut TransactionsModel>,
            pkg_id: QString,
            name: QString,
            icon_url: QString,
            eula_url: QString,
        );

        #[qinvokable]
        #[cxx_name = "installFlatpakref"]
        fn install_flatpakref(
            self: Pin<&mut TransactionsModel>,
            url_or_path: QString,
            name: QString,
        );

        #[qinvokable]
        #[cxx_name = "installBundle"]
        fn install_bundle(self: Pin<&mut TransactionsModel>, path: QString, name: QString);

        #[qinvokable]
        fn update(
            self: Pin<&mut TransactionsModel>,
            pkg_id: QString,
            name: QString,
            icon_url: QString,
        );

        #[qinvokable]
        fn launch(self: Pin<&mut TransactionsModel>, pkg_id: QString);

        #[qinvokable]
        #[cxx_name = "removePackage"]
        fn remove_package(
            self: Pin<&mut TransactionsModel>,
            pkg_id: QString,
            name: QString,
            icon_url: QString,
        );

        #[qsignal]
        #[cxx_name = "removeConfirmationNeeded"]
        fn remove_confirmation_needed(
            self: Pin<&mut TransactionsModel>,
            pkg_id: QString,
            name: QString,
            icon_url: QString,
        );

        #[qinvokable]
        #[cxx_name = "confirmRemove"]
        fn confirm_remove(
            self: Pin<&mut TransactionsModel>,
            pkg_id: QString,
            name: QString,
            icon_url: QString,
            delete_data: bool,
        );

        #[qinvokable]
        fn cancel(self: Pin<&mut TransactionsModel>, tx_id: QString);

        #[qinvokable]
        #[cxx_name = "cancelForPackage"]
        fn cancel_for_package(self: Pin<&mut TransactionsModel>, pkg_id: QString);

        #[qinvokable]
        #[cxx_name = "clearFinished"]
        fn clear_finished(self: Pin<&mut TransactionsModel>);
    }

    extern "RustQt" {
        #[qinvokable]
        #[cxx_override]
        fn data(self: &TransactionsModel, index: &QModelIndex, role: i32) -> QVariant;

        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "roleNames"]
        fn role_names(self: &TransactionsModel) -> QHash_i32_QByteArray;

        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "rowCount"]
        fn row_count(self: &TransactionsModel, _parent: &QModelIndex) -> i32;
    }

    unsafe extern "RustQt" {
        #[inherit]
        #[cxx_name = "beginInsertRows"]
        unsafe fn begin_insert_rows(
            self: Pin<&mut TransactionsModel>,
            parent: &QModelIndex,
            first: i32,
            last: i32,
        );
        #[inherit]
        #[cxx_name = "endInsertRows"]
        unsafe fn end_insert_rows(self: Pin<&mut TransactionsModel>);
        #[inherit]
        #[cxx_name = "beginRemoveRows"]
        unsafe fn begin_remove_rows(
            self: Pin<&mut TransactionsModel>,
            parent: &QModelIndex,
            first: i32,
            last: i32,
        );
        #[inherit]
        #[cxx_name = "endRemoveRows"]
        unsafe fn end_remove_rows(self: Pin<&mut TransactionsModel>);
        #[inherit]
        #[cxx_name = "beginResetModel"]
        unsafe fn begin_reset_model(self: Pin<&mut TransactionsModel>);
        #[inherit]
        #[cxx_name = "endResetModel"]
        unsafe fn end_reset_model(self: Pin<&mut TransactionsModel>);
        #[inherit]
        fn index(
            self: &TransactionsModel,
            row: i32,
            column: i32,
            parent: &QModelIndex,
        ) -> QModelIndex;
        #[inherit]
        #[cxx_name = "dataChanged"]
        fn data_changed(
            self: Pin<&mut TransactionsModel>,
            top_left: &QModelIndex,
            bottom_right: &QModelIndex,
            roles: &QList_i32,
        );
    }

    impl cxx_qt::Threading for TransactionsModel {}

    #[qenum(HomeFeedModel)]
    enum HomeRoles {
        ItemType,
        Text,
        Title,
        CardsJson,
        HeroItemsJson,
        EditorialItemsJson,
        LinkTitle,
        LinkItemsJson,
        AppCloudOverlay,
        CategoriesJson,
        Loading,
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[base = QAbstractListModel]
        #[qml_element]
        #[qml_singleton]
        #[qproperty(bool, loading)]
        type HomeFeedModel = super::HomeFeedModelRust;

        #[qinvokable]
        fn load(self: Pin<&mut HomeFeedModel>);

        // always refetches unlike load which skips once sections are populated
        // picks up apps dropped earlier while the appstream database was cold
        #[qinvokable]
        fn refresh(self: Pin<&mut HomeFeedModel>);

        // like refresh(), but for a disruptive event (daemon restart) where
        // the old content shouldn't just sit there silently updating in the background.
        // clears the page first so the loading state (page
        // conveyor, then per-row placeholders) is actually visible again
        #[qinvokable]
        fn reload(self: Pin<&mut HomeFeedModel>);
    }

    extern "RustQt" {
        #[qinvokable]
        #[cxx_override]
        fn data(self: &HomeFeedModel, index: &QModelIndex, role: i32) -> QVariant;

        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "roleNames"]
        fn role_names(self: &HomeFeedModel) -> QHash_i32_QByteArray;

        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "rowCount"]
        fn row_count(self: &HomeFeedModel, _parent: &QModelIndex) -> i32;
    }

    unsafe extern "RustQt" {
        #[inherit]
        #[cxx_name = "beginResetModel"]
        unsafe fn begin_reset_model(self: Pin<&mut HomeFeedModel>);
        #[inherit]
        #[cxx_name = "endResetModel"]
        unsafe fn end_reset_model(self: Pin<&mut HomeFeedModel>);
        #[inherit]
        #[cxx_name = "beginInsertRows"]
        unsafe fn begin_insert_rows(
            self: Pin<&mut HomeFeedModel>,
            parent: &QModelIndex,
            first: i32,
            last: i32,
        );
        #[inherit]
        #[cxx_name = "endInsertRows"]
        unsafe fn end_insert_rows(self: Pin<&mut HomeFeedModel>);
        #[inherit]
        fn index(self: &HomeFeedModel, row: i32, column: i32, parent: &QModelIndex) -> QModelIndex;
        #[inherit]
        #[cxx_name = "dataChanged"]
        fn data_changed(
            self: Pin<&mut HomeFeedModel>,
            top_left: &QModelIndex,
            bottom_right: &QModelIndex,
            roles: &QList_i32,
        );
    }

    impl cxx_qt::Threading for HomeFeedModel {}

    #[qenum(RemotesModel)]
    enum RemoteRoles {
        Name,
        Url,
        Protected,
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[base = QAbstractListModel]
        #[qml_element]
        #[qml_singleton]
        #[qproperty(bool, loading)]
        type RemotesModel = super::RemotesModelRust;

        #[qinvokable]
        fn load(self: Pin<&mut RemotesModel>);

        #[qinvokable]
        fn add(self: Pin<&mut RemotesModel>, name: QString, url: QString);

        #[qinvokable]
        #[cxx_name = "addFlatpakrepo"]
        fn add_flatpakrepo(self: Pin<&mut RemotesModel>, content: QString);

        #[qinvokable]
        fn remove(self: Pin<&mut RemotesModel>, name: QString);

        #[qsignal]
        #[cxx_name = "actionFailed"]
        fn action_failed(self: Pin<&mut RemotesModel>, message: QString);

        #[qsignal]
        #[cxx_name = "actionSucceeded"]
        fn action_succeeded(self: Pin<&mut RemotesModel>, message: QString);
    }

    extern "RustQt" {
        #[qinvokable]
        #[cxx_override]
        fn data(self: &RemotesModel, index: &QModelIndex, role: i32) -> QVariant;

        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "roleNames"]
        fn role_names(self: &RemotesModel) -> QHash_i32_QByteArray;

        #[qinvokable]
        #[cxx_override]
        #[cxx_name = "rowCount"]
        fn row_count(self: &RemotesModel, _parent: &QModelIndex) -> i32;
    }

    unsafe extern "RustQt" {
        #[inherit]
        #[cxx_name = "beginResetModel"]
        unsafe fn begin_reset_model(self: Pin<&mut RemotesModel>);
        #[inherit]
        #[cxx_name = "endResetModel"]
        unsafe fn end_reset_model(self: Pin<&mut RemotesModel>);
    }

    impl cxx_qt::Threading for RemotesModel {}
}

use crate::runtime;
use cxx_qt::{CxxQtThread, CxxQtType, Threading};
use cxx_qt_lib::{
    QByteArray, QHash, QHashPair_i32_QByteArray, QList, QModelIndex, QString, QStringList, QVariant,
};
use futures_util::future::join_all;
use futures_util::StreamExt;
use libarc::{ArcDaemonProxy, Package};
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tokio::task::spawn_blocking;

#[derive(Clone)]
struct PackageRow {
    pkg: Package,
    busy: bool,
    progress: f32,
}

fn provider_str(provider: &libarc::Provider) -> &'static str {
    match provider {
        libarc::Provider::Flatpak => "Flatpak",
        libarc::Provider::Distrobox => "Distrobox",
        libarc::Provider::Lutris => "Lutris",
        libarc::Provider::AppImage => "AppImage",
        libarc::Provider::Pwa => "PWA",
    }
}

#[derive(Default, PartialEq, Clone, Copy)]
enum ListMode {
    #[default]
    Search,
    Installed,
    Updates,
}

#[derive(Clone)]
enum PageSource {
    Search(String),
    Category(String),
}

const PAGE_SIZE: u64 = 60;

pub struct PrefetchedCategory {
    category: String,
    rows: Vec<PackageRow>,
    providers: Vec<String>,
    total: u64,
    at: Instant,
}

const CATEGORY_PREFETCH_FRESH: Duration = Duration::from_secs(60);

static PREFETCHED_CATEGORY: Mutex<Option<PrefetchedCategory>> = Mutex::new(None);

fn take_prefetched_category(category: &str) -> Option<PrefetchedCategory> {
    let mut slot = PREFETCHED_CATEGORY.lock().unwrap_or_else(|p| p.into_inner());
    let fresh = slot.as_ref().is_some_and(|p| p.category == category && p.at.elapsed() < CATEGORY_PREFETCH_FRESH);
    if fresh {
        slot.take()
    } else {
        None
    }
}

pub fn prefetch_category(category: String) {
    let fresh = PREFETCHED_CATEGORY
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .as_ref()
        .is_some_and(|p| p.category == category && p.at.elapsed() < CATEGORY_PREFETCH_FRESH);
    if fresh {
        return;
    }
    runtime::spawn(async move {
        let source = PageSource::Category(category.clone());
        let Ok(mut page) = fetch_page(&source, &libarc::SearchFilter::default(), 0, PAGE_SIZE).await else {
            return;
        };
        crate::services::images::resolve_packages(&mut page.items).await;
        let Ok((rows, providers, total)) = spawn_blocking(move || page_rows(page)).await else {
            return;
        };
        *PREFETCHED_CATEGORY.lock().unwrap_or_else(|p| p.into_inner()) =
            Some(PrefetchedCategory { category, rows, providers, total, at: Instant::now() });
    });
}

const PREFETCH_TOP: usize = 6;

pub struct PackageListModelRust {
    loading: bool,
    total: i32,
    source: Option<PageSource>,
    fetching: bool,
    // Installed drops rows on remove and Updates drops rows on update
    // while Search only flips the installed flag
    mode: ListMode,
    packages: Vec<PackageRow>,
    all_packages: Vec<PackageRow>,
    filter_provider: String,
    filter_text: String,
    filter_installed: i32,
    sort_by_name: bool,
    providers: QStringList,
    request_seq: u64,
    registered: bool,
}

impl Default for PackageListModelRust {
    fn default() -> Self {
        Self {
            loading: true,
            total: 0,
            source: None,
            fetching: false,
            mode: ListMode::default(),
            packages: Vec::new(),
            all_packages: Vec::new(),
            filter_provider: String::new(),
            filter_text: String::new(),
            filter_installed: 0,
            sort_by_name: false,
            providers: QStringList::default(),
            request_seq: 0,
            registered: false,
        }
    }
}

static PACKAGE_LIST_QT_THREADS: Mutex<Vec<CxxQtThread<qobject::PackageListModel>>> =
    Mutex::new(Vec::new());

fn catalog_lang() -> String {
    sys_locale::get_locale().unwrap_or_default()
}

fn provider_filter_name(display: &str) -> String {
    match display {
        "PWA" => "Pwa".to_string(),
        other => other.to_string(),
    }
}

async fn fetch_page(
    source: &PageSource,
    filter: &libarc::SearchFilter,
    offset: u64,
    limit: u64,
) -> anyhow::Result<libarc::PackagePage> {
    let proxy = runtime::proxy()
        .await
        .ok_or_else(|| anyhow::anyhow!("no daemon connection"))?;
    let lang = catalog_lang();
    match source {
        PageSource::Search(query) => {
            runtime::call(
                "search",
                proxy.search_packages_page(query, &lang, filter, offset, limit),
            )
            .await
        }
        PageSource::Category(id) => {
            let key = id.clone();
            let priority = spawn_blocking(move || crate::cache::forge_lists().get(&key))
                .await
                .ok()
                .flatten()
                .unwrap_or_default();
            runtime::call(
                "category",
                proxy.category_packages_page(id, &lang, priority, offset, limit),
            )
            .await
        }
    }
}

fn page_rows(page: libarc::PackagePage) -> (Vec<PackageRow>, Vec<String>, u64) {
    let total = page.total;
    let mut packages = page.items;
    for pkg in &mut packages {
        pkg.icon_url = Some(crate::services::icons::resolve(
            &pkg.id,
            pkg.icon_url.as_deref(),
        ));
    }
    let providers = page
        .providers
        .iter()
        .map(|p| provider_str(p).to_string())
        .collect();
    let (rows, _) = rows_and_providers(packages);
    (rows, providers, total)
}

impl qobject::PackageListModel {
    pub fn search(mut self: Pin<&mut Self>, query: QString) {
        {
            let mut rust = self.as_mut().rust_mut();
            rust.mode = ListMode::Search;
            rust.source = Some(PageSource::Search(query.to_string()));
            rust.filter_provider = String::new();
            rust.filter_installed = 0;
            rust.sort_by_name = false;
        }
        self.load_page(true);
    }

    pub fn load_installed(mut self: Pin<&mut Self>) {
        {
            let mut rust = self.as_mut().rust_mut();
            rust.mode = ListMode::Installed;
            rust.source = None;
        }
        self.as_mut().load_with(
            async move {
                let proxy = runtime::proxy()
                    .await
                    .ok_or_else(|| anyhow::anyhow!("no daemon connection"))?;
                runtime::call("load installed", proxy.installed_packages()).await
            },
            "load installed",
        );
    }

    pub fn search_category(mut self: Pin<&mut Self>, category: QString) {
        let category = category.to_string();
        {
            let mut rust = self.as_mut().rust_mut();
            rust.mode = ListMode::Search;
            rust.source = Some(PageSource::Category(category.clone()));
            rust.filter_provider = String::new();
            rust.filter_installed = 0;
            rust.sort_by_name = false;
        }
        let seq = match take_prefetched_category(&category) {
            Some(page) => self.as_mut().apply_first_page(page),
            None => self.as_mut().load_page(true),
        };

        // the curated order is refreshed after the first page is already on screen
        let Some(slug) = crate::services::forge::category_list_slug(&category) else {
            return;
        };
        let qt_thread = self.qt_thread();
        runtime::spawn(async move {
            let Some(list) = crate::services::forge::fetch_list(slug).await else {
                return;
            };
            let fresh: Vec<String> = list.apps.into_iter().map(|a| a.app_ref).collect();
            let key = category.clone();
            let changed = spawn_blocking(move || {
                let changed = crate::cache::forge_lists().get(&key).as_ref() != Some(&fresh);
                if changed {
                    crate::cache::forge_lists().insert(key, fresh);
                }
                changed
            })
            .await
            .unwrap_or(false);
            if changed {
                let _ = qt_thread.queue(move |this| {
                    if this.request_seq == seq {
                        this.load_page(true);
                    }
                });
            }
        });
    }

    // asks the daemon for the next page or with reset for the first one and returns the request number
    fn apply_first_page(mut self: Pin<&mut Self>, page: PrefetchedCategory) -> u64 {
        self.as_mut().register();
        let seq = {
            let mut rust = self.as_mut().rust_mut();
            rust.request_seq += 1;
            rust.fetching = false;
            rust.request_seq
        };
        let mut rows = page.rows;
        carry_busy_state(&self.all_packages, &mut rows);
        unsafe {
            self.as_mut().begin_reset_model();
            let mut rust = self.as_mut().rust_mut();
            rust.all_packages = rows.clone();
            rust.packages = rows;
            drop(rust);
            self.as_mut().end_reset_model();
        }
        self.as_mut().set_providers(provider_qlist(&page.providers));
        self.as_mut().set_total(page.total as i32);
        self.as_mut().set_loading(false);
        seq
    }

    fn load_page(mut self: Pin<&mut Self>, reset: bool) -> u64 {
        let Some(source) = self.source.clone() else {
            return self.request_seq;
        };
        if !reset && self.fetching {
            return self.request_seq;
        }
        self.as_mut().register();
        let qt_thread = self.qt_thread();
        let (seq, offset) = {
            let mut rust = self.as_mut().rust_mut();
            if reset {
                rust.request_seq += 1;
            }
            rust.fetching = true;
            (
                rust.request_seq,
                if reset {
                    0
                } else {
                    rust.all_packages.len() as u64
                },
            )
        };
        if reset {
            self.as_mut().set_loading(true);
        }
        let filter = libarc::SearchFilter {
            provider: provider_filter_name(&self.filter_provider),
            installed: self.filter_installed,
            sort_by_name: self.sort_by_name,
        };
        runtime::spawn(async move {
            let page = fetch_page(&source, &filter, offset, PAGE_SIZE).await;
            let built = match page {
                Ok(mut page) => {
                    crate::services::images::resolve_packages(&mut page.items).await;
                    spawn_blocking(move || page_rows(page)).await.ok()
                }
                Err(e) => {
                    tracing::warn!("list page failed: {e}");
                    None
                }
            };
            let _ = qt_thread.queue(move |mut this| {
                if this.request_seq != seq {
                    return;
                }
                this.as_mut().rust_mut().fetching = false;
                let Some((mut rows, providers, total)) = built else {
                    this.as_mut().set_loading(false);
                    return;
                };
                if reset {
                    let top: Vec<String> = rows
                        .iter()
                        .take(PREFETCH_TOP)
                        .map(|r| r.pkg.id.clone())
                        .collect();
                    crate::bridge::detail_controller::prefetch_first_shots(top);
                    carry_busy_state(&this.all_packages, &mut rows);
                    unsafe {
                        this.as_mut().begin_reset_model();
                        let mut rust = this.as_mut().rust_mut();
                        rust.all_packages = rows.clone();
                        rust.packages = rows;
                        drop(rust);
                        this.as_mut().end_reset_model();
                    }
                    this.as_mut().set_providers(provider_qlist(&providers));
                } else if !rows.is_empty() {
                    let first = this.packages.len() as i32;
                    let last = first + rows.len() as i32 - 1;
                    unsafe {
                        this.as_mut()
                            .begin_insert_rows(&QModelIndex::default(), first, last);
                        let mut rust = this.as_mut().rust_mut();
                        rust.all_packages.extend(rows.iter().cloned());
                        rust.packages.extend(rows);
                        drop(rust);
                        this.as_mut().end_insert_rows();
                    }
                }
                this.as_mut().set_total(total as i32);
                this.as_mut().set_loading(false);
            });
        });
        seq
    }

    fn refresh_loaded_pages(mut self: Pin<&mut Self>) {
        let Some(source) = self.source.clone() else {
            return;
        };
        if self.fetching || self.all_packages.is_empty() {
            return;
        }
        let qt_thread = self.qt_thread();
        let seq = self.request_seq;
        let limit = (self.all_packages.len() as u64).max(PAGE_SIZE);
        let filter = libarc::SearchFilter {
            provider: provider_filter_name(&self.filter_provider),
            installed: self.filter_installed,
            sort_by_name: self.sort_by_name,
        };
        self.as_mut().rust_mut().fetching = true;
        runtime::spawn(async move {
            let built = match fetch_page(&source, &filter, 0, limit).await {
                Ok(mut page) => {
                    crate::services::images::resolve_packages(&mut page.items).await;
                    spawn_blocking(move || page_rows(page)).await.ok()
                }
                Err(_) => None,
            };
            let _ = qt_thread.queue(move |mut this| {
                if this.request_seq != seq {
                    return;
                }
                this.as_mut().rust_mut().fetching = false;
                let Some((mut rows, providers, total)) = built else {
                    return;
                };
                carry_busy_state(&this.all_packages, &mut rows);
                if same_ids(&this.all_packages, &rows) {
                    let count = rows.len() as i32;
                    {
                        let mut rust = this.as_mut().rust_mut();
                        rust.all_packages = rows.clone();
                        rust.packages = rows;
                    }
                    if count > 0 {
                        let top = this.index(0, 0, &QModelIndex::default());
                        let bottom = this.index(count - 1, 0, &QModelIndex::default());
                        this.as_mut()
                            .data_changed(&top, &bottom, &QList::<i32>::default());
                    }
                } else {
                    unsafe {
                        this.as_mut().begin_reset_model();
                        let mut rust = this.as_mut().rust_mut();
                        rust.all_packages = rows.clone();
                        rust.packages = rows;
                        drop(rust);
                        this.as_mut().end_reset_model();
                    }
                }
                this.as_mut().set_providers(provider_qlist(&providers));
                this.as_mut().set_total(total as i32);
            });
        });
    }

    fn register(mut self: Pin<&mut Self>) {
        if !self.registered {
            let qt_thread = self.qt_thread();
            PACKAGE_LIST_QT_THREADS.lock().unwrap().push(qt_thread);
            self.as_mut().rust_mut().registered = true;
        }
    }

    pub fn release(mut self: Pin<&mut Self>) {
        unsafe {
            self.as_mut().begin_reset_model();
            let mut rust = self.as_mut().rust_mut();
            rust.request_seq += 1;
            rust.fetching = false;
            rust.all_packages = Vec::new();
            rust.packages = Vec::new();
            drop(rust);
            self.as_mut().end_reset_model();
        }
        self.as_mut().set_total(0);
    }

    pub fn restore(mut self: Pin<&mut Self>) {
        if !self.all_packages.is_empty() {
            return;
        }
        if self.source.is_some() {
            self.load_page(true);
            return;
        }
        match self.mode {
            ListMode::Installed => self.as_mut().load_installed(),
            ListMode::Updates => self.as_mut().load_updates(),
            ListMode::Search => {}
        }
    }

    pub fn can_fetch_more(&self, _parent: &QModelIndex) -> bool {
        self.source.is_some() && !self.fetching && (self.all_packages.len() as i32) < self.total
    }

    pub fn fetch_more(self: Pin<&mut Self>, _parent: &QModelIndex) {
        self.load_page(false);
    }

    pub fn check_for_updates(mut self: Pin<&mut Self>) {
        self.as_mut().set_loading(true);
        self.as_mut().load_updates();
        runtime::spawn(async move {
            if let Some(proxy) = runtime::proxy().await {
                if let Err(e) = proxy.refresh_catalog().await {
                    tracing::warn!("refresh_catalog failed: {e}");
                }
            }
        });
    }

    pub fn load_updates(mut self: Pin<&mut Self>) {
        {
            let mut rust = self.as_mut().rust_mut();
            rust.mode = ListMode::Updates;
            rust.source = None;
        }
        self.as_mut().load_with(
            async move {
                let proxy = runtime::proxy()
                    .await
                    .ok_or_else(|| anyhow::anyhow!("no daemon connection"))?;
                runtime::call("load updates", proxy.updates_packages()).await
            },
            "load updates",
        );
    }

    pub fn set_filters(
        mut self: Pin<&mut Self>,
        provider: QString,
        installed: i32,
        sort_by_name: bool,
    ) {
        {
            let mut rust = self.as_mut().rust_mut();
            rust.filter_provider = provider.to_string();
            rust.filter_installed = installed;
            rust.sort_by_name = sort_by_name;
        }
        if self.source.is_some() {
            self.load_page(true);
        } else {
            self.apply_rows();
        }
    }

    pub fn set_search_text(mut self: Pin<&mut Self>, text: QString) {
        let text = text.to_string().trim().to_lowercase();
        if self.filter_text == text {
            return;
        }
        self.as_mut().rust_mut().filter_text = text;
        self.apply_rows();
    }

    fn apply_rows(mut self: Pin<&mut Self>) {
        let providers = providers_list(&self.all_packages);
        self.as_mut().set_providers(providers);

        let mut rows: Vec<PackageRow> = self
            .all_packages
            .iter()
            .filter(|r| {
                self.filter_provider.is_empty()
                    || provider_str(&r.pkg.provider) == self.filter_provider
            })
            .filter(|r| match self.filter_installed {
                1 => r.pkg.installed,
                2 => !r.pkg.installed,
                _ => true,
            })
            .filter(|r| {
                self.filter_text.is_empty()
                    || r.pkg.name.to_lowercase().contains(&self.filter_text)
                    || r.pkg.description.to_lowercase().contains(&self.filter_text)
            })
            .cloned()
            .collect();
        if self.sort_by_name {
            rows.sort_by(|a, b| a.pkg.name.to_lowercase().cmp(&b.pkg.name.to_lowercase()));
        }

        let count = rows.len() as i32;
        unsafe {
            self.as_mut().begin_reset_model();
            self.as_mut().rust_mut().packages = rows;
            self.as_mut().end_reset_model();
        }
        self.as_mut().set_total(count);
    }

    fn load_with(
        mut self: Pin<&mut Self>,
        fetch: impl std::future::Future<Output = anyhow::Result<Vec<Package>>> + Send + 'static,
        what: &'static str,
    ) {
        self.as_mut().register();
        let qt_thread = self.qt_thread();
        let seq = {
            let mut rust = self.as_mut().rust_mut();
            rust.filter_provider = String::new();
            rust.filter_text = String::new();
            rust.filter_installed = 0;
            rust.sort_by_name = false;
            rust.request_seq += 1;
            rust.request_seq
        };
        if self.all_packages.is_empty() {
            self.as_mut().set_loading(true);
        }

        runtime::spawn(async move {
            let mut packages = match fetch.await {
                Ok(packages) => packages,
                Err(e) => {
                    tracing::warn!("{what} failed: {e}");
                    let _ = qt_thread.queue(move |mut this| {
                        if this.request_seq == seq {
                            this.as_mut().set_loading(false);
                        }
                    });
                    return;
                }
            };

            crate::services::images::resolve_packages(&mut packages).await;
            let built = spawn_blocking(move || {
                let mut packages = packages;
                for pkg in &mut packages {
                    pkg.icon_url = Some(crate::services::icons::resolve(
                        &pkg.id,
                        pkg.icon_url.as_deref(),
                    ));
                }
                rows_and_providers(packages)
            })
            .await
            .unwrap_or_default();

            fill_incrementally(qt_thread, seq, built, true);
        });
    }

    pub fn data(&self, index: &QModelIndex, role: i32) -> QVariant {
        let Some(row) = self.packages.get(index.row() as usize) else {
            return QVariant::default();
        };
        let pkg = &row.pkg;
        let role = qobject::PackageRoles { repr: role };
        match role {
            qobject::PackageRoles::Id => QVariant::from(&QString::from(&pkg.id)),
            qobject::PackageRoles::Name => QVariant::from(&QString::from(&pkg.name)),
            qobject::PackageRoles::Version => QVariant::from(&QString::from(&pkg.version)),
            qobject::PackageRoles::Description => QVariant::from(&QString::from(&pkg.description)),
            qobject::PackageRoles::Provider => {
                QVariant::from(&QString::from(provider_str(&pkg.provider)))
            }
            qobject::PackageRoles::Installed => QVariant::from(&pkg.installed),
            qobject::PackageRoles::IconUrl => {
                QVariant::from(&QString::from(pkg.icon_url.as_deref().unwrap_or("")))
            }
            qobject::PackageRoles::Busy => QVariant::from(&row.busy),
            qobject::PackageRoles::Progress => QVariant::from(&row.progress),
            _ => QVariant::default(),
        }
    }

    pub fn role_names(&self) -> QHash<QHashPair_i32_QByteArray> {
        let mut roles = QHash::<QHashPair_i32_QByteArray>::default();

        roles.insert(qobject::PackageRoles::Id.repr, QByteArray::from("pkgId"));
        roles.insert(qobject::PackageRoles::Name.repr, QByteArray::from("name"));
        roles.insert(
            qobject::PackageRoles::Version.repr,
            QByteArray::from("version"),
        );
        roles.insert(
            qobject::PackageRoles::Description.repr,
            QByteArray::from("description"),
        );
        roles.insert(
            qobject::PackageRoles::Provider.repr,
            QByteArray::from("provider"),
        );
        roles.insert(
            qobject::PackageRoles::Installed.repr,
            QByteArray::from("installed"),
        );
        roles.insert(
            qobject::PackageRoles::IconUrl.repr,
            QByteArray::from("iconUrl"),
        );
        roles.insert(qobject::PackageRoles::Busy.repr, QByteArray::from("busy"));
        roles.insert(
            qobject::PackageRoles::Progress.repr,
            QByteArray::from("progress"),
        );
        roles
    }

    pub fn row_count(&self, _parent: &QModelIndex) -> i32 {
        self.packages.len() as i32
    }
}

fn same_ids(a: &[PackageRow], b: &[PackageRow]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.pkg.id == y.pkg.id)
}

fn providers_list(rows: &[PackageRow]) -> QStringList {
    let mut providers: Vec<&'static str> = Vec::new();
    for row in rows {
        let p = provider_str(&row.pkg.provider);
        if !providers.contains(&p) {
            providers.push(p);
        }
    }
    providers.sort_unstable();
    let mut names: Vec<String> = providers.iter().map(|p| p.to_string()).collect();
    names.sort_unstable();
    provider_qlist(&names)
}

fn provider_qlist(names: &[String]) -> QStringList {
    let mut list = QList::<QString>::default();
    for n in names {
        list.append(QString::from(n));
    }
    QStringList::from(&list)
}

fn rows_and_providers(packages: Vec<Package>) -> (Vec<PackageRow>, Vec<String>) {
    let rows: Vec<PackageRow> = packages
        .into_iter()
        .map(|pkg| PackageRow {
            pkg,
            busy: false,
            progress: 0.0,
        })
        .collect();
    let mut names: Vec<&'static str> = Vec::new();
    for row in &rows {
        let p = provider_str(&row.pkg.provider);
        if !names.contains(&p) {
            names.push(p);
        }
    }
    names.sort_unstable();
    (rows, names.into_iter().map(|s| s.to_string()).collect())
}

fn fill_incrementally(
    qt_thread: CxxQtThread<qobject::PackageListModel>,
    seq: u64,
    built: (Vec<PackageRow>, Vec<String>),
    skip_if_same: bool,
) {
    let _ = qt_thread.queue(move |mut this| {
        if this.request_seq != seq {
            return;
        }
        let (mut rows, provider_names) = built;
        if skip_if_same
            && same_ids(&this.all_packages, &rows)
            && rows
                .iter()
                .zip(this.all_packages.iter())
                .all(|(a, b)| a.pkg.installed == b.pkg.installed)
        {
            this.as_mut().set_loading(false);
            return;
        }
        let count = rows.len() as i32;
        carry_busy_state(&this.all_packages, &mut rows);
        this.as_mut().set_providers(provider_qlist(&provider_names));
        unsafe {
            this.as_mut().begin_reset_model();
            {
                let mut rust = this.as_mut().rust_mut();
                rust.all_packages = rows.clone();
                rust.packages = rows;
            }
            this.as_mut().end_reset_model();
        }
        this.as_mut().set_total(count);
        this.as_mut().set_loading(false);
    });
}

fn carry_busy_state(old: &[PackageRow], rows: &mut [PackageRow]) {
    let busy: HashMap<&str, f32> = old
        .iter()
        .filter(|r| r.busy)
        .map(|r| (r.pkg.id.as_str(), r.progress))
        .collect();
    if busy.is_empty() {
        return;
    }
    for row in rows.iter_mut() {
        if let Some(&progress) = busy.get(row.pkg.id.as_str()) {
            row.busy = true;
            row.progress = progress;
        }
    }
}

pub(crate) fn reload_package_lists() {
    PACKAGE_LIST_QT_THREADS.lock().unwrap().retain(|qt_thread| {
        qt_thread
            .queue(|mut model| {
                if model.all_packages.is_empty() {
                    return;
                }
                let mode = model.mode;
                match mode {
                    ListMode::Updates => model.as_mut().load_updates(),
                    ListMode::Installed => model.as_mut().load_installed(),
                    ListMode::Search => model.as_mut().refresh_loaded_pages(),
                }
            })
            .is_ok()
    });
}

fn sync_package_busy(pkg_id: &str, busy: bool, progress: f32) {
    crate::bridge::detail_controller::sync_busy(pkg_id, busy, progress);

    let pkg_id = pkg_id.to_string();

    PACKAGE_LIST_QT_THREADS.lock().unwrap().retain(|qt_thread| {
        let pkg_id = pkg_id.clone();
        qt_thread
            .queue(move |mut model| {
                let mut changed_row: Option<i32> = None;
                {
                    let mut rust = model.as_mut().rust_mut();
                    for r in rust.all_packages.iter_mut() {
                        if r.pkg.id == pkg_id {
                            r.busy = busy;
                            r.progress = progress;
                        }
                    }
                    for (i, r) in rust.packages.iter_mut().enumerate() {
                        if r.pkg.id == pkg_id {
                            r.busy = busy;
                            r.progress = progress;
                            changed_row = Some(i as i32);
                        }
                    }
                }
                if let Some(row) = changed_row {
                    let idx = model.index(row, 0, &QModelIndex::default());
                    let roles = QList::<i32>::default();
                    model.as_mut().data_changed(&idx, &idx, &roles);
                }
            })
            .is_ok()
    });
}

fn sync_package_finished(pkg_id: &str, tx_type: &str, success: bool) {
    let is_remove = tx_type == "remove" || tx_type == "remove_with_data";
    if success {
        crate::bridge::detail_controller::sync_installed(pkg_id, !is_remove);
        crate::bridge::detail_controller::refresh_extensions_for_current();
        sync_home_installed(pkg_id, !is_remove);
        if !is_remove {
            let id = pkg_id.to_string();
            runtime::spawn(async move { crate::services::forge::post_install(id).await });
        }
    } else {
        crate::bridge::detail_controller::sync_busy(pkg_id, false, 0.0);
    }

    let pkg_id = pkg_id.to_string();
    let is_update = tx_type == "update";
    PACKAGE_LIST_QT_THREADS.lock().unwrap().retain(|qt_thread| {
        let pkg_id = pkg_id.clone();
        qt_thread
            .queue(move |mut model| {
                if !model
                    .packages
                    .iter()
                    .chain(model.all_packages.iter())
                    .any(|r| r.pkg.id == pkg_id)
                {
                    return;
                }
                let drop_row = success
                    && ((is_remove
                        && matches!(model.mode, ListMode::Installed | ListMode::Updates))
                        || (is_update && model.mode == ListMode::Updates));

                {
                    let mut rust = model.as_mut().rust_mut();
                    if drop_row {
                        rust.all_packages.retain(|r| r.pkg.id != pkg_id);
                    } else {
                        for r in rust.all_packages.iter_mut() {
                            if r.pkg.id == pkg_id {
                                r.busy = false;
                                r.progress = 0.0;
                                if success {
                                    r.pkg.installed = !is_remove;
                                }
                            }
                        }
                    }
                }

                let visible_row = model.packages.iter().position(|r| r.pkg.id == pkg_id);
                let Some(row) = visible_row else { return };
                if drop_row {
                    unsafe {
                        model.as_mut().begin_remove_rows(
                            &QModelIndex::default(),
                            row as i32,
                            row as i32,
                        );
                        model.as_mut().rust_mut().packages.remove(row);
                        model.as_mut().end_remove_rows();
                    }
                } else {
                    {
                        let mut rust = model.as_mut().rust_mut();
                        let r = &mut rust.packages[row];
                        r.busy = false;
                        r.progress = 0.0;
                        if success {
                            r.pkg.installed = !is_remove;
                        }
                    }
                    let idx = model.index(row as i32, 0, &QModelIndex::default());
                    let roles = QList::<i32>::default();
                    model.as_mut().data_changed(&idx, &idx, &roles);
                }
            })
            .is_ok()
    });
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
enum TxStatus {
    #[default]
    Pending,
    Running,
    Completed,
    Failed,
}

#[derive(Clone, Default)]
struct Tx {
    id: String,
    pkg_id: String,
    name: String,
    icon_url: String,
    progress: f32,
    status: TxStatus,
    tx_type: String,
    error: String,
    seq: u64,
    finished_at: u64,
    automatic: bool,
    bytes_done: u64,
    bytes_total: u64,
    speed_bps: f64,
    eta_secs: i32,
    // recent (time, bytes) samples for the rolling speed window
    samples: std::collections::VecDeque<(std::time::Instant, u64)>,
}

impl Tx {
    // rolling average over the last few seconds like steam shows it
    fn push_sample(&mut self, bytes_done: u64, bytes_total: u64) {
        let now = std::time::Instant::now();
        // bytes dropping means a new operation started (app -> runtime -> locale)
        if bytes_done < self.bytes_done {
            self.samples.clear();
        }
        self.bytes_done = bytes_done;
        self.bytes_total = bytes_total;
        self.samples.push_back((now, bytes_done));
        while let Some((t, _)) = self.samples.front() {
            if now.duration_since(*t).as_secs_f64() > 6.0 && self.samples.len() > 2 {
                self.samples.pop_front();
            } else {
                break;
            }
        }
        if let (Some((t0, b0)), Some((t1, b1))) = (self.samples.front(), self.samples.back()) {
            let dt = t1.duration_since(*t0).as_secs_f64();
            if dt > 0.5 && b1 > b0 {
                self.speed_bps = (b1 - b0) as f64 / dt;
                let remaining = bytes_total.saturating_sub(bytes_done) as f64;
                self.eta_secs = if self.speed_bps > 0.0 {
                    (remaining / self.speed_bps).ceil() as i32
                } else {
                    -1
                };
            }
        }
    }
}

#[derive(Default)]
pub struct TransactionsModelRust {
    transactions: Vec<Tx>,
    next_seq: u64,
    active_count: i32,
    updates_count: i32,
    running_count: i32,
    queued_count: i32,
    done_count: i32,
    busy_packages_json: QString,
    history_loaded: bool,
    updating_all: bool,
}

static QT_THREAD: OnceLock<CxxQtThread<qobject::TransactionsModel>> = OnceLock::new();
const DAEMON_STARTUP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

async fn retry_until_daemon_answers<F, Fut>(mut call: F)
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    for _ in 0..5 {
        if tokio::time::timeout(DAEMON_STARTUP_TIMEOUT, call())
            .await
            .unwrap_or(false)
        {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}

fn has_ongoing(transactions: &[Tx], pkg_id: &str) -> bool {
    transactions
        .iter()
        .any(|tx| tx.pkg_id == pkg_id && matches!(tx.status, TxStatus::Pending | TxStatus::Running))
}

impl qobject::TransactionsModel {
    pub fn init(mut self: Pin<&mut Self>) {
        let qt_thread = self.as_mut().qt_thread();
        if QT_THREAD.set(qt_thread.clone()).is_err() {
            return; // already initialized
        }

        runtime::spawn(async move {
            let Some(proxy) = runtime::proxy().await else {
                let _ = qt_thread.queue(|mut this| this.as_mut().set_history_loaded(true));
                return;
            };

            retry_until_daemon_answers(|| load_running_transactions(&proxy)).await;
            let _ = qt_thread.queue(|mut this| this.as_mut().set_history_loaded(true));
            retry_until_daemon_answers(|| refresh_updates_count(&proxy)).await;
            let pad_proxy = proxy.clone();
            runtime::spawn(async move {
                crate::bridge::startup_controller::after_first_frame().await;
                prepare_installed_icons(&pad_proxy).await;
            });

            let mut proxy = proxy;
            loop {
                run_signal_listener(proxy.clone()).await;
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                runtime::invalidate_proxy();
                let Some(fresh) = runtime::proxy().await else {
                    continue;
                };
                proxy = fresh;
                retry_until_daemon_answers(|| load_running_transactions(&proxy)).await;
                retry_until_daemon_answers(|| refresh_updates_count(&proxy)).await;
                reload_package_lists();
                crate::bridge::detail_controller::refresh_current();
            }
        });
    }

    pub fn install(mut self: Pin<&mut Self>, pkg_id: QString, name: QString, icon_url: QString) {
        self.as_mut().start_transaction(
            pkg_id.to_string(),
            name.to_string(),
            icon_url.to_string(),
            "install",
        );
    }

    pub fn request_install(
        self: Pin<&mut Self>,
        pkg_id: QString,
        name: QString,
        icon_url: QString,
    ) {
        let pkg_id = pkg_id.to_string();
        let name = name.to_string();
        let icon_url = icon_url.to_string();
        runtime::spawn(async move {
            let eula_url = match runtime::proxy().await {
                Some(proxy) => proxy
                    .app_metadata(&pkg_id)
                    .await
                    .ok()
                    .and_then(|m| m.eula_url)
                    .unwrap_or_default(),
                None => String::new(),
            };

            let Some(qt_thread) = QT_THREAD.get() else {
                return;
            };
            let _ = qt_thread.queue(move |mut model| {
                if eula_url.is_empty() {
                    model
                        .as_mut()
                        .start_transaction(pkg_id, name, icon_url, "install");
                } else {
                    model.as_mut().eula_required(
                        QString::from(&pkg_id),
                        QString::from(&name),
                        QString::from(&icon_url),
                        QString::from(&eula_url),
                    );
                }
            });
        });
    }

    pub fn install_flatpakref(mut self: Pin<&mut Self>, url_or_path: QString, name: QString) {
        self.as_mut().start_transaction(
            url_or_path.to_string(),
            name.to_string(),
            String::new(),
            "flatpakref",
        );
    }

    pub fn install_bundle(mut self: Pin<&mut Self>, path: QString, name: QString) {
        self.as_mut().start_transaction(
            path.to_string(),
            name.to_string(),
            String::new(),
            "bundle",
        );
    }

    pub fn update(mut self: Pin<&mut Self>, pkg_id: QString, name: QString, icon_url: QString) {
        self.as_mut().start_transaction(
            pkg_id.to_string(),
            name.to_string(),
            icon_url.to_string(),
            "update",
        );
    }

    pub fn launch(self: Pin<&mut Self>, pkg_id: QString) {
        let pkg_id = pkg_id.to_string();
        runtime::spawn(async move {
            if let Some(proxy) = runtime::proxy().await {
                if let Err(e) = proxy.run_package(&pkg_id).await {
                    tracing::warn!("failed to launch {pkg_id}: {e}");
                }
            }
        });
    }

    pub fn update_all(mut self: Pin<&mut Self>) {
        self.as_mut().set_updating_all(true);
        runtime::spawn(async move {
            let Some(qt_thread) = QT_THREAD.get() else {
                return;
            };

            let Some(proxy) = runtime::proxy().await else {
                let _ = qt_thread.queue(|mut model| model.as_mut().set_updating_all(false));
                return;
            };
            let Ok(updates) = proxy.updates_packages().await else {
                let _ = qt_thread.queue(|mut model| model.as_mut().set_updating_all(false));
                return;
            };

            let updates: Vec<(Package, String)> = spawn_blocking(move || {
                updates
                    .into_iter()
                    .map(|pkg| {
                        let icon =
                            crate::services::icons::resolve(&pkg.id, pkg.icon_url.as_deref());
                        (pkg, icon)
                    })
                    .collect()
            })
            .await
            .unwrap_or_default();

            let _ = qt_thread.queue(move |mut model| {
                for (pkg, icon) in updates {
                    model
                        .as_mut()
                        .start_transaction(pkg.id, pkg.name, icon, "update");
                }
                model.as_mut().set_updating_all(false);
            });
        });
    }

    pub fn remove_package(
        mut self: Pin<&mut Self>,
        pkg_id: QString,
        name: QString,
        icon_url: QString,
    ) {
        let pkg_id = pkg_id.to_string();
        let name = name.to_string();
        let name = if name.is_empty() {
            pkg_id.clone()
        } else {
            name
        };
        self.as_mut().remove_confirmation_needed(
            QString::from(&pkg_id),
            QString::from(&name),
            icon_url,
        );
    }

    pub fn confirm_remove(
        mut self: Pin<&mut Self>,
        pkg_id: QString,
        name: QString,
        icon_url: QString,
        delete_data: bool,
    ) {
        let pkg_id = pkg_id.to_string();
        let name = name.to_string();
        let name = if name.is_empty() {
            pkg_id.clone()
        } else {
            name
        };
        let tx_type = if delete_data {
            "remove_with_data"
        } else {
            "remove"
        };
        self.as_mut()
            .start_transaction(pkg_id, name, icon_url.to_string(), tx_type);
    }

    pub fn cancel(self: Pin<&mut Self>, tx_id: QString) {
        let tx_id = tx_id.to_string();
        runtime::spawn(async move {
            if let Some(proxy) = runtime::proxy().await {
                let _ = proxy.cancel_transaction(&tx_id).await;
            }
        });
    }

    pub fn clear_finished(mut self: Pin<&mut Self>) {
        unsafe {
            self.as_mut().begin_reset_model();
            {
                let mut rust = self.as_mut().rust_mut();
                rust.transactions
                    .retain(|tx| matches!(tx.status, TxStatus::Pending | TxStatus::Running));
            }
            self.as_mut().end_reset_model();
        }
        sync_launcher_badge(self);
        runtime::spawn(async move {
            if let Some(proxy) = runtime::proxy().await {
                let _ = proxy.clear_transaction_history().await;
            }
        });
    }

    pub fn cancel_for_package(self: Pin<&mut Self>, pkg_id: QString) {
        let pkg_id = pkg_id.to_string();
        let tx_id = self
            .transactions
            .iter()
            .find(|tx| {
                tx.pkg_id == pkg_id && matches!(tx.status, TxStatus::Pending | TxStatus::Running)
            })
            .map(|tx| tx.id.clone());
        let Some(tx_id) = tx_id.filter(|id| !id.is_empty()) else {
            return;
        };
        runtime::spawn(async move {
            if let Some(proxy) = runtime::proxy().await {
                let _ = proxy.cancel_transaction(&tx_id).await;
            }
        });
    }

    fn start_transaction(
        mut self: Pin<&mut Self>,
        pkg_id: String,
        name: String,
        icon_url: String,
        tx_type: &'static str,
    ) {
        if has_ongoing(&self.transactions, &pkg_id) {
            return;
        }

        let seq = next_seq(self.as_mut());
        let row = self.transactions.len() as i32;
        unsafe {
            self.as_mut()
                .begin_insert_rows(&QModelIndex::default(), row, row);
            self.as_mut().rust_mut().transactions.push(Tx {
                pkg_id: pkg_id.clone(),
                name,
                icon_url,
                status: TxStatus::Pending,
                tx_type: tx_type.to_string(),
                seq,
                ..Default::default()
            });
            self.as_mut().end_insert_rows();
        }
        rebuild_rows(self.as_mut());
        sync_launcher_badge(self.as_mut());
        sync_package_busy(&pkg_id, true, 0.0);

        runtime::spawn(async move {
            let Some(proxy) = runtime::proxy().await else {
                queue_update(&pkg_id, |tx| {
                    tx.status = TxStatus::Failed;
                    tx.error = "Failed to connect to daemon".into();
                });
                return;
            };

            // the GUI always opts into notifications and lets frontend_visible
            // (see SettingsController::set_frontend_visible) decide whether one
            // actually shows
            let result = match tx_type {
                "install" => proxy.install_package(&pkg_id, true).await,
                "remove" => proxy.remove_package(&pkg_id, true).await,
                "remove_with_data" => proxy.remove_package_with_data(&pkg_id, true, true).await,
                "update" => proxy.update_package(&pkg_id, true).await,
                "flatpakref" => proxy.install_flatpakref(&pkg_id, true).await,
                "bundle" => proxy.install_flatpak_bundle(&pkg_id, true).await,
                _ => return,
            };

            match result {
                Ok(tx_id) => queue_update(&pkg_id, move |tx| {
                    tx.id = tx_id.clone();
                    tx.status = TxStatus::Running;
                }),
                Err(e) => queue_update(&pkg_id, move |tx| {
                    tx.status = TxStatus::Failed;
                    tx.error = e.to_string();
                }),
            }
        });
    }

    pub fn data(&self, index: &QModelIndex, role: i32) -> QVariant {
        let Some(tx) = self.transactions.get(index.row() as usize) else {
            return QVariant::default();
        };
        let role = qobject::TransactionRoles { repr: role };
        match role {
            qobject::TransactionRoles::Id => QVariant::from(&QString::from(&tx.id)),
            qobject::TransactionRoles::PkgId => QVariant::from(&QString::from(&tx.pkg_id)),
            qobject::TransactionRoles::Name => QVariant::from(&QString::from(&tx.name)),
            qobject::TransactionRoles::IconUrl => QVariant::from(&QString::from(&tx.icon_url)),
            qobject::TransactionRoles::Progress => QVariant::from(&tx.progress),
            qobject::TransactionRoles::Status => {
                QVariant::from(&QString::from(status_str(tx.status)))
            }
            qobject::TransactionRoles::TxType => QVariant::from(&QString::from(&tx.tx_type)),
            qobject::TransactionRoles::Section => {
                QVariant::from(&QString::from(section_str(tx.status)))
            }
            qobject::TransactionRoles::Error => QVariant::from(&QString::from(&tx.error)),
            qobject::TransactionRoles::FinishedAt => QVariant::from(&(tx.finished_at as f64)),
            qobject::TransactionRoles::Automatic => QVariant::from(&tx.automatic),
            qobject::TransactionRoles::BytesDone => QVariant::from(&(tx.bytes_done as f64)),
            qobject::TransactionRoles::BytesTotal => QVariant::from(&(tx.bytes_total as f64)),
            qobject::TransactionRoles::SpeedBps => QVariant::from(&tx.speed_bps),
            qobject::TransactionRoles::EtaSecs => QVariant::from(&tx.eta_secs),
            _ => QVariant::default(),
        }
    }

    pub fn role_names(&self) -> QHash<QHashPair_i32_QByteArray> {
        let mut roles = QHash::<QHashPair_i32_QByteArray>::default();

        roles.insert(qobject::TransactionRoles::Id.repr, QByteArray::from("txId"));
        roles.insert(
            qobject::TransactionRoles::PkgId.repr,
            QByteArray::from("pkgId"),
        );
        roles.insert(
            qobject::TransactionRoles::Name.repr,
            QByteArray::from("name"),
        );
        roles.insert(
            qobject::TransactionRoles::IconUrl.repr,
            QByteArray::from("iconUrl"),
        );
        roles.insert(
            qobject::TransactionRoles::Progress.repr,
            QByteArray::from("progress"),
        );
        roles.insert(
            qobject::TransactionRoles::Status.repr,
            QByteArray::from("status"),
        );
        roles.insert(
            qobject::TransactionRoles::TxType.repr,
            QByteArray::from("txType"),
        );
        roles.insert(
            qobject::TransactionRoles::Section.repr,
            QByteArray::from("section"),
        );
        roles.insert(
            qobject::TransactionRoles::Error.repr,
            QByteArray::from("error"),
        );
        roles.insert(
            qobject::TransactionRoles::FinishedAt.repr,
            QByteArray::from("finishedAt"),
        );
        roles.insert(
            qobject::TransactionRoles::Automatic.repr,
            QByteArray::from("automatic"),
        );
        roles.insert(
            qobject::TransactionRoles::BytesDone.repr,
            QByteArray::from("bytesDone"),
        );
        roles.insert(
            qobject::TransactionRoles::BytesTotal.repr,
            QByteArray::from("bytesTotal"),
        );
        roles.insert(
            qobject::TransactionRoles::SpeedBps.repr,
            QByteArray::from("speedBps"),
        );
        roles.insert(
            qobject::TransactionRoles::EtaSecs.repr,
            QByteArray::from("etaSecs"),
        );
        roles
    }

    pub fn row_count(&self, _parent: &QModelIndex) -> i32 {
        self.transactions.len() as i32
    }
}

fn status_str(status: TxStatus) -> &'static str {
    match status {
        TxStatus::Pending => "pending",
        TxStatus::Running => "running",
        TxStatus::Completed => "completed",
        TxStatus::Failed => "failed",
    }
}

fn section_str(status: TxStatus) -> &'static str {
    match status {
        TxStatus::Running => "active",
        TxStatus::Pending => "pending",
        TxStatus::Completed | TxStatus::Failed => "completed",
    }
}

fn queue_update(pkg_id: &str, f: impl FnOnce(&mut Tx) + Send + 'static) {
    let Some(qt_thread) = QT_THREAD.get() else {
        return;
    };
    let pkg_id = pkg_id.to_string();
    let _ = qt_thread.queue(move |mut model| {
        if let Some(row) = model.transactions.iter().position(|tx| {
            tx.pkg_id == pkg_id && matches!(tx.status, TxStatus::Pending | TxStatus::Running)
        }) {
            f(&mut model.as_mut().rust_mut().transactions[row]);
            let tx = model.transactions[row].clone();
            apply_row_change(model, row as i32, &tx);
            sync_package_row(&tx);
        }
    });
}

fn queue_update_or_resync(
    tx_id: &str,
    proxy: ArcDaemonProxy<'static>,
    f: impl FnOnce(&mut Tx) + Send + 'static,
) {
    let Some(qt_thread) = QT_THREAD.get() else {
        return;
    };
    let tx_id = tx_id.to_string();
    let _ = qt_thread.queue(move |mut model| {
        let Some(row) = model.transactions.iter().position(|tx| tx.id == tx_id) else {
            runtime::spawn(async move {
                load_running_transactions(&proxy).await;
            });
            return;
        };
        f(&mut model.as_mut().rust_mut().transactions[row]);
        let tx = model.transactions[row].clone();
        apply_row_change(model, row as i32, &tx);
        sync_package_row(&tx);
    });
}

fn apply_row_change(mut model: Pin<&mut qobject::TransactionsModel>, row: i32, tx: &Tx) {
    emit_tx_row_changed(model.as_mut(), row);
    if is_finished(tx.status) {
        rebuild_rows(model);
    }
}

fn queue_update_by_id(tx_id: &str, f: impl FnOnce(&mut Tx) + Send + 'static) {
    let Some(qt_thread) = QT_THREAD.get() else {
        return;
    };
    let tx_id = tx_id.to_string();
    let _ = qt_thread.queue(move |mut model| {
        if let Some(row) = model.transactions.iter().position(|tx| tx.id == tx_id) {
            f(&mut model.as_mut().rust_mut().transactions[row]);
            let tx = model.transactions[row].clone();
            apply_row_change(model, row as i32, &tx);
            sync_package_row(&tx);
        }
    });
}

fn sync_package_row(tx: &Tx) {
    match tx.status {
        TxStatus::Pending | TxStatus::Running => sync_package_busy(&tx.pkg_id, true, tx.progress),
        TxStatus::Completed => sync_package_finished(&tx.pkg_id, &tx.tx_type, true),
        TxStatus::Failed => sync_package_finished(&tx.pkg_id, &tx.tx_type, false),
    }
}

fn is_finished(status: TxStatus) -> bool {
    matches!(status, TxStatus::Completed | TxStatus::Failed)
}

fn is_dependency_id(pkg_id: &str) -> bool {
    matches!(
        pkg_id.rsplit('.').next(),
        Some("Locale" | "Debug" | "Sources" | "Docs" | "Sdk" | "BaseApp")
    ) || pkg_id.contains(".Platform.")
        || pkg_id.ends_with(".Platform")
}

fn next_seq(mut model: Pin<&mut qobject::TransactionsModel>) -> u64 {
    let mut rust = model.as_mut().rust_mut();
    rust.next_seq += 1;
    rust.next_seq
}

fn ordered_rows(mut rows: Vec<Tx>) -> Vec<Tx> {
    rows.retain(|tx| !is_finished(tx.status) || !is_dependency_id(&tx.pkg_id));

    let mut newest: HashMap<&str, u64> = HashMap::new();
    for tx in rows.iter().filter(|tx| is_finished(tx.status)) {
        let slot = newest.entry(tx.pkg_id.as_str()).or_default();
        *slot = (*slot).max(tx.seq);
    }
    let newest: HashMap<String, u64> = newest
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    rows.retain(|tx| !is_finished(tx.status) || newest.get(&tx.pkg_id) == Some(&tx.seq));

    rows.sort_by(
        |a, b| match (is_finished(a.status), is_finished(b.status)) {
            (false, true) => std::cmp::Ordering::Less,
            (true, false) => std::cmp::Ordering::Greater,
            (true, true) => (b.finished_at, b.seq).cmp(&(a.finished_at, a.seq)),
            (false, false) => a.seq.cmp(&b.seq),
        },
    );
    let running = rows.iter().filter(|tx| !is_finished(tx.status)).count();
    rows.truncate(running + MAX_FINISHED_ROWS);
    rows
}

const MAX_FINISHED_ROWS: usize = 100;

fn rebuild_rows(mut model: Pin<&mut qobject::TransactionsModel>) {
    let rows = ordered_rows(model.transactions.clone());

    let unchanged = rows.len() == model.transactions.len()
        && rows
            .iter()
            .zip(model.transactions.iter())
            .all(|(a, b)| a.seq == b.seq);
    if unchanged {
        return;
    }

    unsafe {
        model.as_mut().begin_reset_model();
        model.as_mut().rust_mut().transactions = rows;
        model.as_mut().end_reset_model();
    }
    sync_launcher_badge(model);
}

fn emit_tx_row_changed(mut model: Pin<&mut qobject::TransactionsModel>, row: i32) {
    sync_launcher_badge(model.as_mut());
    let idx = model.index(row, 0, &QModelIndex::default());
    let roles = QList::<i32>::default();
    model.as_mut().data_changed(&idx, &idx, &roles);
}

fn sync_launcher_badge(mut model: Pin<&mut qobject::TransactionsModel>) {
    let mut running = 0;
    let mut queued = 0;
    let mut done = 0;
    let mut busy_packages: HashMap<String, f32> = HashMap::new();
    let active: Vec<f64> = model
        .transactions
        .iter()
        .inspect(|tx| match tx.status {
            TxStatus::Running => running += 1,
            TxStatus::Pending => queued += 1,
            TxStatus::Completed | TxStatus::Failed => done += 1,
        })
        .filter(|tx| matches!(tx.status, TxStatus::Pending | TxStatus::Running))
        .inspect(|tx| {
            busy_packages.insert(tx.pkg_id.clone(), tx.progress);
        })
        .map(|tx| tx.progress as f64)
        .collect();
    let progress = if active.is_empty() {
        0.0
    } else {
        active.iter().sum::<f64>() / active.len() as f64
    };
    model.as_mut().set_active_count(active.len() as i32);
    model.as_mut().set_running_count(running);
    model.as_mut().set_queued_count(queued);
    model.as_mut().set_done_count(done);
    model
        .as_mut()
        .set_busy_packages_json(QString::from(&to_json(&busy_packages)));
    libarc::launcher::update_transactions(active.len() as i32, progress);
}

async fn load_running_transactions(proxy: &ArcDaemonProxy<'static>) -> bool {
    let Ok(txs) = proxy.transactions().await else {
        return false;
    };

    let Some(qt_thread) = QT_THREAD.get() else {
        return false;
    };

    let unique_ids: std::collections::HashSet<String> =
        txs.iter().map(|tx| tx.package_id.clone()).collect();
    let resolved: HashMap<String, Package> = join_all(
        unique_ids
            .into_iter()
            .map(|id| async move { (id.clone(), proxy.app_info(&id).await.ok().flatten()) }),
    )
    .await
    .into_iter()
    .filter_map(|(id, pkg)| pkg.map(|pkg| (id, pkg)))
    .collect();

    let lookup: Vec<(String, Option<String>)> = txs
        .iter()
        .map(|tx| {
            (
                tx.package_id.clone(),
                resolved
                    .get(&tx.package_id)
                    .and_then(|p| p.icon_url.clone()),
            )
        })
        .collect();
    let icons: HashMap<String, String> = spawn_blocking(move || {
        lookup
            .into_iter()
            .map(|(id, raw)| {
                let icon = crate::services::icons::resolve(&id, raw.as_deref());
                (id, icon)
            })
            .collect()
    })
    .await
    .unwrap_or_default();

    let _ = qt_thread.queue(move |mut model| {
        for tx in txs {
            let (status, error) = match &tx.status {
                libarc::TransactionStatus::Pending => (TxStatus::Pending, String::new()),
                libarc::TransactionStatus::Running => (TxStatus::Running, String::new()),
                libarc::TransactionStatus::Success => (TxStatus::Completed, String::new()),
                libarc::TransactionStatus::Failed(msg) => (TxStatus::Failed, msg.clone()),
            };
            let id = tx.id.to_string();
            if model.transactions.iter().any(|e| e.id == id) {
                continue;
            }
            if let Some(row) = model.transactions.iter().position(|e| {
                e.id.is_empty()
                    && e.pkg_id == tx.package_id
                    && matches!(e.status, TxStatus::Pending | TxStatus::Running)
            }) {
                let mut rust = model.as_mut().rust_mut();
                rust.transactions[row].id = id;
                continue;
            }
            let tx_type = match tx.transaction_type {
                libarc::TransactionType::Install => "install",
                libarc::TransactionType::Remove => "remove",
                libarc::TransactionType::Update => "update",
            };
            let pkg = resolved.get(&tx.package_id);
            let name = pkg
                .map(|p| p.name.clone())
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| tx.package_id.clone());
            let icon_url = icons.get(&tx.package_id).cloned().unwrap_or_default();
            let seq = next_seq(model.as_mut());
            let row = model.transactions.len() as i32;
            unsafe {
                model
                    .as_mut()
                    .begin_insert_rows(&QModelIndex::default(), row, row);
                model.as_mut().rust_mut().transactions.push(Tx {
                    id,
                    seq,
                    pkg_id: tx.package_id.clone(),
                    name,
                    icon_url,
                    progress: tx.progress as f32 / 100.0,
                    status,
                    tx_type: tx_type.to_string(),
                    error,
                    finished_at: tx.finished_at,
                    automatic: tx.automatic,
                    ..Default::default()
                });
                model.as_mut().end_insert_rows();
            }
        }
        rebuild_rows(model.as_mut());
        sync_launcher_badge(model.as_mut());
    });
    true
}

async fn prepare_installed_icons(proxy: &ArcDaemonProxy<'static>) {
    let Ok(Ok(packages)) =
        tokio::time::timeout(DAEMON_STARTUP_TIMEOUT, proxy.installed_packages()).await
    else {
        return;
    };
    let _ = spawn_blocking(move || {
        for pkg in packages {
            crate::services::icons::resolve(&pkg.id, pkg.icon_url.as_deref());
        }
    })
    .await;
}

async fn refresh_updates_count(proxy: &ArcDaemonProxy<'static>) -> bool {
    let Ok(updates) = proxy.updates_packages().await else {
        return false;
    };
    let count = updates.len();
    libarc::launcher::update_badge(count as i32);
    let Some(qt_thread) = QT_THREAD.get() else {
        return false;
    };
    let _ = qt_thread.queue(move |mut model| {
        model.as_mut().set_updates_count(count as i32);
    });
    true
}

async fn run_signal_listener(proxy: ArcDaemonProxy<'static>) {
    let subscribed = tokio::time::timeout(DAEMON_STARTUP_TIMEOUT, async {
        tokio::join!(
            proxy.receive_transaction_progress(),
            proxy.receive_transaction_finished(),
            proxy.receive_updates_available(),
            proxy.receive_transaction_stats(),
            proxy.receive_transaction_started(),
            proxy.receive_catalog_refreshed(),
            proxy.receive_catalog_changed(),
        )
    })
    .await;

    let Ok((
        Ok(mut progress_stream),
        Ok(mut finished_stream),
        Ok(mut updates_stream),
        Ok(mut stats_stream),
        Ok(mut started_stream),
        Ok(mut catalog_stream),
        Ok(mut changed_stream),
    )) = subscribed
    else {
        tracing::warn!("run_signal_listener: failed to subscribe to transaction signal streams");
        return;
    };

    loop {
        tokio::select! {
            sig = stats_stream.next() => {
                let Some(sig) = sig else { break };
                let Ok(args) = sig.args() else { continue };
                let tx_id = args.transaction_id().to_string();
                let bytes_done = *args.bytes_done();
                let bytes_total = *args.bytes_total();
                queue_update_by_id(&tx_id, move |tx| {
                    tx.push_sample(bytes_done, bytes_total);
                });
            }
            sig = updates_stream.next() => {
                let Some(sig) = sig else { break };
                let Ok(args) = sig.args() else { continue };
                let count = *args.count();
                libarc::launcher::update_badge(count as i32);
                let Some(qt_thread) = QT_THREAD.get() else { continue };
                let _ = qt_thread.queue(move |mut model| {
                    model.as_mut().set_updates_count(count as i32);
                });
            }
            sig = catalog_stream.next() => {
                let Some(_) = sig else { break };
                refresh_updates_count(&proxy).await;
                reload_package_lists();
            }
            sig = changed_stream.next() => {
                let Some(_) = sig else { break };
                reload_package_lists();
                crate::bridge::detail_controller::refresh_current();
            }
            sig = started_stream.next() => {
                let Some(sig) = sig else { break };
                let Ok(args) = sig.args() else { continue };
                let tx_id = args.transaction_id().to_string();
                let pkg_id = args.package_id().to_string();
                let Some(qt_thread) = QT_THREAD.get() else { continue };
                let resync_proxy = proxy.clone();
                let _ = qt_thread.queue(move |model| {
                    let known = model.transactions.iter().any(|tx| {
                        tx.id == tx_id
                            || (tx.pkg_id == pkg_id
                                && matches!(tx.status, TxStatus::Pending | TxStatus::Running))
                    });
                    if known {
                        return;
                    }
                    runtime::spawn(async move {
                        load_running_transactions(&resync_proxy).await;
                    });
                });
            }
            sig = progress_stream.next() => {
                let Some(sig) = sig else { break };
                let Ok(args) = sig.args() else { continue };
                let tx_id = args.transaction_id().to_string();
                let progress = *args.progress() as f32 / 100.0;
                queue_update_by_id(&tx_id, move |tx| {
                    tx.progress = progress;
                    if tx.status == TxStatus::Pending {
                        tx.status = TxStatus::Running;
                    }
                });
            }
            sig = finished_stream.next() => {
                let Some(sig) = sig else { break };
                let Ok(args) = sig.args() else { continue };
                let tx_id = args.transaction_id().to_string();
                let success = *args.success();
                let message = args.message().to_string();
                queue_update_or_resync(&tx_id, proxy.clone(), move |tx| {
                    tx.finished_at = libarc::unix_now();
                    if success {
                        tx.status = TxStatus::Completed;
                        tx.progress = 1.0;
                    } else {
                        tx.status = TxStatus::Failed;
                        tx.error = message;
                    }
                });
                if success {
                    refresh_updates_count(&proxy).await;
                }
            }
        }
    }
}

fn is_empty_section(section: &crate::services::home::HomeSection) -> bool {
    section.cards.is_empty()
        && section.hero_items.is_empty()
        && section.editorial_items.is_empty()
        && section.link_items.is_empty()
        && section.categories.is_empty()
}

fn to_json<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_string(v).unwrap_or_default()
}

pub struct HomeFeedModelRust {
    loading: bool,
    sections: Vec<crate::services::home::HomeSection>,
    raw_sections: Vec<crate::services::home::HomeSection>,
    request_seq: u64,
    pending_count: u32,
    all_stories: Vec<crate::services::home::Story>,
}

impl Default for HomeFeedModelRust {
    fn default() -> Self {
        Self {
            loading: true,
            sections: Vec::new(),
            raw_sections: Vec::new(),
            request_seq: 0,
            pending_count: 0,
            all_stories: Vec::new(),
        }
    }
}

static HOME_FEED_QT_THREAD: OnceLock<CxxQtThread<qobject::HomeFeedModel>> = OnceLock::new();

fn sync_home_installed(pkg_id: &str, installed: bool) {
    let Some(qt_thread) = HOME_FEED_QT_THREAD.get() else {
        return;
    };
    let pkg_id = pkg_id.to_string();
    let _ = qt_thread.queue(move |mut this| {
        let mut changed_rows: Vec<i32> = Vec::new();
        {
            let mut rust = this.as_mut().rust_mut();
            for (row, section) in rust.sections.iter_mut().enumerate() {
                let mut touched = false;
                for card in section.cards.iter_mut() {
                    if card.id == pkg_id && card.installed != installed {
                        card.installed = installed;
                        touched = true;
                    }
                }
                if touched {
                    changed_rows.push(row as i32);
                }
            }
            for section in rust.raw_sections.iter_mut() {
                for card in section.cards.iter_mut().filter(|c| c.id == pkg_id) {
                    card.installed = installed;
                }
            }
        }
        for row in changed_rows {
            let idx = this.index(row, 0, &QModelIndex::default());
            let roles = QList::<i32>::default();
            this.as_mut().data_changed(&idx, &idx, &roles);
        }
    });
}

fn worth_snapshotting(sections: &[crate::services::home::HomeSection]) -> bool {
    sections
        .iter()
        .filter(|s| {
            use crate::services::home::SectionKind;
            matches!(
                s.kind,
                SectionKind::AppRow | SectionKind::AppGrid | SectionKind::AppWideGrid
            )
        })
        .all(|s| !s.cards.is_empty())
}

// frontpage apps are the most opened ones so their first screenshots are fetched once per session in the background
fn prefetch_frontpage(sections: &[crate::services::home::HomeSection]) {
    static DONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if DONE.swap(true, std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    let mut ids: Vec<String> = Vec::new();
    for section in sections {
        let card_ids = section.cards.iter().map(|c| c.id.clone());
        let hero_ids = section
            .hero_items
            .iter()
            .chain(section.editorial_items.iter())
            .map(|h| h.id.clone());
        for id in card_ids.chain(hero_ids) {
            if !id.is_empty() && !id.starts_with("pwa:") && !ids.contains(&id) {
                ids.push(id);
            }
        }
    }
    ids.truncate(24);
    crate::bridge::detail_controller::prefetch_first_shots(ids);
}

// off the GUI thread: writes to disk are cheap to trigger but the actual
// I/O should never run on the thread driving the UI
fn save_home_cache_in_background(
    sections: Vec<crate::services::home::HomeSection>,
    stories: Vec<crate::services::home::Story>,
) {
    if !worth_snapshotting(&sections) {
        return;
    }
    prefetch_frontpage(&sections);
    runtime::spawn(async move {
        let _ = spawn_blocking(move || {
            crate::services::home_cache::save(&sections, &stories);
        })
        .await;
    });
}

fn apply_stored_home(
    mut this: Pin<&mut qobject::HomeFeedModel>,
    preloaded: crate::services::home_cache::Preloaded,
) {
    unsafe {
        this.as_mut().begin_reset_model();
        this.as_mut().rust_mut().sections = preloaded.display;
        this.as_mut().rust_mut().raw_sections = preloaded.raw;
        this.as_mut().end_reset_model();
    }
    this.as_mut().set_loading(false);
    crate::bridge::startup_controller::log_step("home filled from disk");
}

impl qobject::HomeFeedModel {
    pub fn load(mut self: Pin<&mut Self>) {
        if !self.sections.is_empty() {
            return;
        }
        if let Some(preloaded) = crate::services::home_cache::take_preloaded() {
            apply_stored_home(self.as_mut(), preloaded);
        } else {
            let qt_thread = self.qt_thread();
            runtime::spawn(async move {
                let Some(preloaded) = crate::services::home_cache::wait_preloaded().await else {
                    return;
                };
                let _ = qt_thread.queue(move |mut this| {
                    if this.sections.is_empty() {
                        apply_stored_home(this.as_mut(), preloaded);
                    }
                });
            });
        }
        self.as_mut().fetch();
    }

    pub fn refresh(self: Pin<&mut Self>) {
        self.fetch();
    }

    pub fn reload(mut self: Pin<&mut Self>) {
        unsafe {
            self.as_mut().begin_reset_model();
            self.as_mut().rust_mut().sections = Vec::new();
            self.as_mut().rust_mut().raw_sections = Vec::new();
            self.as_mut().end_reset_model();
        }
        self.fetch();
    }

    fn fetch(mut self: Pin<&mut Self>) {
        let has_cached = !self.sections.is_empty();
        self.as_mut().set_loading(!has_cached);

        let seq = {
            let mut rust = self.as_mut().rust_mut();
            rust.request_seq += 1;
            rust.request_seq
        };

        let qt_thread = self.qt_thread();
        let _ = HOME_FEED_QT_THREAD.set(qt_thread.clone());

        runtime::spawn(async move {
            let proxy = runtime::proxy().await;
            let (plan, ctx) = crate::services::home::plan_home(proxy).await;

            let pending_count = plan.pending.len() as u32;
            let raw = plan.sections;
            let mut display = raw.clone();
            let images_proxy = runtime::proxy().await;
            crate::services::images::resolve_sections(images_proxy.as_ref(), &mut display).await;

            let qt_reset = qt_thread.clone();
            let _ = qt_reset.queue(move |mut this| unsafe {
                if this.request_seq != seq {
                    return;
                }
                if this.sections.is_empty() {
                    this.as_mut().begin_reset_model();
                    this.as_mut().rust_mut().sections = display;
                    this.as_mut().rust_mut().raw_sections = raw;
                    this.as_mut().end_reset_model();
                    crate::bridge::startup_controller::log_step("home filled from network");
                }
                this.as_mut().rust_mut().all_stories = Vec::new();
                this.as_mut().rust_mut().pending_count = pending_count;
                this.as_mut().set_loading(false);
                if pending_count == 0 {
                    save_home_cache_in_background(
                        this.raw_sections.clone(),
                        this.all_stories.clone(),
                    );
                }
            });

            for pending in plan.pending {
                let ctx = ctx.clone();
                let qt_thread = qt_thread.clone();
                runtime::spawn(async move {
                    let (row_index, section, stories) =
                        crate::services::home::resolve_pending_section(pending, &ctx).await;
                    let mut display = section.clone();
                    let images_proxy = runtime::proxy().await;
                    crate::services::images::resolve_sections(
                        images_proxy.as_ref(),
                        std::slice::from_mut(&mut display),
                    )
                    .await;
                    let _ = qt_thread.queue(move |mut this| {
                        if this.request_seq != seq {
                            return;
                        }
                        let changed = {
                            let mut rust = this.as_mut().rust_mut();
                            let unchanged = rust.raw_sections.get(row_index) == Some(&section);
                            match (rust.raw_sections.get(row_index).cloned(), unchanged) {
                                (_, true) => false,
                                (Some(old), false)
                                    if is_empty_section(&section) && !is_empty_section(&old) =>
                                {
                                    let was_loading =
                                        rust.sections.get(row_index).is_some_and(|s| s.loading);
                                    if let Some(slot) = rust.sections.get_mut(row_index) {
                                        slot.loading = false;
                                    }
                                    if let Some(slot) = rust.raw_sections.get_mut(row_index) {
                                        slot.loading = false;
                                    }
                                    was_loading
                                }
                                (Some(_), false) => {
                                    rust.raw_sections[row_index] = section;
                                    if let Some(slot) = rust.sections.get_mut(row_index) {
                                        *slot = display;
                                    }
                                    true
                                }
                                (None, _) => false,
                            }
                        };
                        crate::services::stories::remember_all(&stories);
                        this.as_mut().rust_mut().all_stories.extend(stories);

                        if changed {
                            let idx = this.index(row_index as i32, 0, &QModelIndex::default());
                            let roles = QList::<i32>::default();
                            this.as_mut().data_changed(&idx, &idx, &roles);
                        }

                        let remaining = this.pending_count.saturating_sub(1);
                        this.as_mut().rust_mut().pending_count = remaining;
                        if remaining == 0 {
                            save_home_cache_in_background(
                                this.raw_sections.clone(),
                                this.all_stories.clone(),
                            );
                        }
                    });
                });
            }
        });
    }

    pub fn data(&self, index: &QModelIndex, role: i32) -> QVariant {
        let Some(s) = self.sections.get(index.row() as usize) else {
            return QVariant::default();
        };
        let role = qobject::HomeRoles { repr: role };
        match role {
            qobject::HomeRoles::ItemType => QVariant::from(&QString::from(s.kind.as_str())),
            qobject::HomeRoles::Text => QVariant::from(&QString::from(&s.text)),
            qobject::HomeRoles::Title => QVariant::from(&QString::from(&s.title)),
            qobject::HomeRoles::CardsJson => QVariant::from(&QString::from(&to_json(&s.cards))),
            qobject::HomeRoles::HeroItemsJson => {
                QVariant::from(&QString::from(&to_json(&s.hero_items)))
            }
            qobject::HomeRoles::EditorialItemsJson => {
                QVariant::from(&QString::from(&to_json(&s.editorial_items)))
            }
            qobject::HomeRoles::LinkTitle => QVariant::from(&QString::from(&s.link_title)),
            qobject::HomeRoles::LinkItemsJson => {
                QVariant::from(&QString::from(&to_json(&s.link_items)))
            }
            qobject::HomeRoles::AppCloudOverlay => QVariant::from(&s.app_cloud_overlay),
            qobject::HomeRoles::CategoriesJson => {
                QVariant::from(&QString::from(&to_json(&s.categories)))
            }
            qobject::HomeRoles::Loading => QVariant::from(&s.loading),
            _ => QVariant::default(),
        }
    }

    pub fn role_names(&self) -> QHash<QHashPair_i32_QByteArray> {
        let mut roles = QHash::<QHashPair_i32_QByteArray>::default();
        roles.insert(
            qobject::HomeRoles::ItemType.repr,
            QByteArray::from("itemType"),
        );
        roles.insert(qobject::HomeRoles::Text.repr, QByteArray::from("text"));
        roles.insert(qobject::HomeRoles::Title.repr, QByteArray::from("title"));
        roles.insert(
            qobject::HomeRoles::CardsJson.repr,
            QByteArray::from("cardsJson"),
        );
        roles.insert(
            qobject::HomeRoles::HeroItemsJson.repr,
            QByteArray::from("heroItemsJson"),
        );
        roles.insert(
            qobject::HomeRoles::EditorialItemsJson.repr,
            QByteArray::from("editorialItemsJson"),
        );
        roles.insert(
            qobject::HomeRoles::LinkTitle.repr,
            QByteArray::from("linkTitle"),
        );
        roles.insert(
            qobject::HomeRoles::LinkItemsJson.repr,
            QByteArray::from("linkItemsJson"),
        );
        roles.insert(
            qobject::HomeRoles::AppCloudOverlay.repr,
            QByteArray::from("appCloudOverlay"),
        );
        roles.insert(
            qobject::HomeRoles::CategoriesJson.repr,
            QByteArray::from("categoriesJson"),
        );
        roles.insert(
            qobject::HomeRoles::Loading.repr,
            QByteArray::from("loading"),
        );
        roles
    }

    pub fn row_count(&self, _parent: &QModelIndex) -> i32 {
        self.sections.len() as i32
    }
}

pub struct RemotesModelRust {
    loading: bool,
    remotes: Vec<libarc::RemoteInfo>,
}

impl Default for RemotesModelRust {
    fn default() -> Self {
        Self {
            loading: true,
            remotes: Vec::new(),
        }
    }
}

fn emit_failed(qt_thread: &CxxQtThread<qobject::RemotesModel>, message: String) {
    let _ = qt_thread.queue(move |mut this| {
        this.as_mut().action_failed(QString::from(&message));
    });
}

impl qobject::RemotesModel {
    pub fn load(mut self: Pin<&mut Self>) {
        self.as_mut().set_loading(true);

        let qt_thread = self.qt_thread();
        runtime::spawn(async move {
            let remotes = match runtime::proxy().await {
                Some(proxy) => proxy.remotes().await.unwrap_or_default(),
                None => vec![],
            };

            qt_thread
                .queue(move |mut this| unsafe {
                    this.as_mut().begin_reset_model();
                    this.as_mut().rust_mut().remotes = remotes;
                    this.as_mut().end_reset_model();
                    this.as_mut().set_loading(false);
                })
                .ok();
        });
    }

    pub fn add(mut self: Pin<&mut Self>, name: QString, url: QString) {
        let name = name.to_string();
        let url = url.to_string();
        let qt_thread = self.as_mut().qt_thread();
        runtime::spawn(async move {
            let Some(proxy) = runtime::proxy().await else {
                emit_failed(&qt_thread, "Failed to connect to daemon".into());
                return;
            };
            match proxy.add_remote(&name, &url).await {
                Ok(true) => {
                    let msg = format!("Added repository {name}");
                    let _ = qt_thread.queue(move |mut this| {
                        this.as_mut().load();
                        this.as_mut().action_succeeded(QString::from(&msg));
                    });
                }
                Ok(false) => emit_failed(&qt_thread, format!("Could not add repository {name}")),
                Err(e) => emit_failed(&qt_thread, format!("Could not add repository {name}: {e}")),
            }
        });
    }

    pub fn add_flatpakrepo(mut self: Pin<&mut Self>, content: QString) {
        let content = content.to_string();
        let qt_thread = self.as_mut().qt_thread();
        runtime::spawn(async move {
            let Some(proxy) = runtime::proxy().await else {
                emit_failed(&qt_thread, "Failed to connect to daemon".into());
                return;
            };
            match proxy.add_flatpakrepo(&content).await {
                Ok(true) => {
                    let _ = qt_thread.queue(move |mut this| {
                        this.as_mut().load();
                        this.as_mut()
                            .action_succeeded(QString::from("Repository added"));
                    });
                }
                Ok(false) => emit_failed(&qt_thread, "Could not add repository".into()),
                Err(e) => emit_failed(&qt_thread, format!("Could not add repository: {e}")),
            }
        });
    }

    pub fn remove(mut self: Pin<&mut Self>, name: QString) {
        let name = name.to_string();
        let qt_thread = self.as_mut().qt_thread();
        runtime::spawn(async move {
            let Some(proxy) = runtime::proxy().await else {
                emit_failed(&qt_thread, "Failed to connect to daemon".into());
                return;
            };
            match proxy.remove_remote(&name).await {
                Ok(true) => {
                    let msg = format!("Removed repository {name}");
                    let _ = qt_thread.queue(move |mut this| {
                        this.as_mut().load();
                        this.as_mut().action_succeeded(QString::from(&msg));
                    });
                }
                Ok(false) => emit_failed(&qt_thread, format!("Could not remove repository {name}")),
                Err(e) => emit_failed(
                    &qt_thread,
                    format!("Could not remove repository {name}: {e}"),
                ),
            }
        });
    }

    pub fn data(&self, index: &QModelIndex, role: i32) -> QVariant {
        let Some(r) = self.remotes.get(index.row() as usize) else {
            return QVariant::default();
        };
        let role = qobject::RemoteRoles { repr: role };
        match role {
            qobject::RemoteRoles::Name => QVariant::from(&QString::from(&r.name)),
            qobject::RemoteRoles::Url => QVariant::from(&QString::from(&r.url)),
            qobject::RemoteRoles::Protected => QVariant::from(&r.protected),
            _ => QVariant::default(),
        }
    }

    pub fn role_names(&self) -> QHash<QHashPair_i32_QByteArray> {
        let mut roles = QHash::<QHashPair_i32_QByteArray>::default();
        roles.insert(qobject::RemoteRoles::Name.repr, QByteArray::from("name"));
        roles.insert(qobject::RemoteRoles::Url.repr, QByteArray::from("url"));
        roles.insert(
            qobject::RemoteRoles::Protected.repr,
            QByteArray::from("isProtected"),
        );
        roles
    }

    pub fn row_count(&self, _parent: &QModelIndex) -> i32 {
        self.remotes.len() as i32
    }
}
