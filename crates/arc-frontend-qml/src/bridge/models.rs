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
        #[qml_singleton]
        #[qproperty(bool, loading)]
        #[qproperty(QStringList, providers)]
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
        #[cxx_name = "setFilters"]
        fn set_filters(self: Pin<&mut PackageListModel>, provider: QString, installed: i32, sort_by_name: bool);
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
        unsafe fn begin_remove_rows(self: Pin<&mut PackageListModel>, parent: &QModelIndex, first: i32, last: i32);
        #[inherit]
        #[cxx_name = "endRemoveRows"]
        unsafe fn end_remove_rows(self: Pin<&mut PackageListModel>);
        #[inherit]
        #[cxx_name = "beginInsertRows"]
        unsafe fn begin_insert_rows(self: Pin<&mut PackageListModel>, parent: &QModelIndex, first: i32, last: i32);
        #[inherit]
        #[cxx_name = "endInsertRows"]
        unsafe fn end_insert_rows(self: Pin<&mut PackageListModel>);
        #[inherit]
        fn index(self: &PackageListModel, row: i32, column: i32, parent: &QModelIndex) -> QModelIndex;
        #[inherit]
        #[cxx_name = "dataChanged"]
        fn data_changed(self: Pin<&mut PackageListModel>, top_left: &QModelIndex, bottom_right: &QModelIndex, roles: &QList_i32);
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
        type TransactionsModel = super::TransactionsModelRust;

        #[qinvokable]
        fn init(self: Pin<&mut TransactionsModel>);

        #[qinvokable]
        fn install(self: Pin<&mut TransactionsModel>, pkg_id: QString, name: QString, icon_url: QString);

        #[qinvokable]
        #[cxx_name = "updateAll"]
        fn update_all(self: Pin<&mut TransactionsModel>);

        // install() with EULA check which emits eulaRequired instead if needed
        #[qinvokable]
        #[cxx_name = "requestInstall"]
        fn request_install(self: Pin<&mut TransactionsModel>, pkg_id: QString, name: QString, icon_url: QString);

        #[qsignal]
        #[cxx_name = "eulaRequired"]
        fn eula_required(self: Pin<&mut TransactionsModel>, pkg_id: QString, name: QString, icon_url: QString, eula_url: QString);

        #[qinvokable]
        #[cxx_name = "installFlatpakref"]
        fn install_flatpakref(self: Pin<&mut TransactionsModel>, url_or_path: QString, name: QString);

        #[qinvokable]
        #[cxx_name = "installBundle"]
        fn install_bundle(self: Pin<&mut TransactionsModel>, path: QString, name: QString);

        #[qinvokable]
        fn update(self: Pin<&mut TransactionsModel>, pkg_id: QString, name: QString, icon_url: QString);

        #[qinvokable]
        fn launch(self: Pin<&mut TransactionsModel>, pkg_id: QString);

        #[qinvokable]
        #[cxx_name = "removePackage"]
        fn remove_package(self: Pin<&mut TransactionsModel>, pkg_id: QString);

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
        unsafe fn begin_insert_rows(self: Pin<&mut TransactionsModel>, parent: &QModelIndex, first: i32, last: i32);
        #[inherit]
        #[cxx_name = "endInsertRows"]
        unsafe fn end_insert_rows(self: Pin<&mut TransactionsModel>);
        #[inherit]
        #[cxx_name = "beginRemoveRows"]
        unsafe fn begin_remove_rows(self: Pin<&mut TransactionsModel>, parent: &QModelIndex, first: i32, last: i32);
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
        fn index(self: &TransactionsModel, row: i32, column: i32, parent: &QModelIndex) -> QModelIndex;
        #[inherit]
        #[cxx_name = "dataChanged"]
        fn data_changed(self: Pin<&mut TransactionsModel>, top_left: &QModelIndex, bottom_right: &QModelIndex, roles: &QList_i32);
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
use cxx_qt_lib::{QByteArray, QHash, QHashPair_i32_QByteArray, QList, QModelIndex, QString, QStringList, QVariant};
use futures_util::StreamExt;
use libarc::{ArcDaemonProxy, Package};
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::{Mutex, OnceLock};

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

pub struct PackageListModelRust {
    loading: bool,
    // Installed drops rows on remove and Updates drops rows on update
    // while Search only flips the installed flag
    mode: ListMode,
    packages: Vec<PackageRow>,
    all_packages: Vec<PackageRow>,
    filter_provider: String,
    filter_installed: i32,
    sort_by_name: bool,
    providers: QStringList,
    request_seq: u64,
}

impl Default for PackageListModelRust {
    fn default() -> Self {
        Self {
            loading: true,
            mode: ListMode::default(),
            packages: Vec::new(),
            all_packages: Vec::new(),
            filter_provider: String::new(),
            filter_installed: 0,
            sort_by_name: false,
            providers: QStringList::default(),
            request_seq: 0,
        }
    }
}

static PACKAGE_LIST_QT_THREAD: OnceLock<CxxQtThread<qobject::PackageListModel>> = OnceLock::new();

// keyed by "search:<q>" / "installed" / "updates" / "category:<id>",
// cleared wholesale after any successful transaction
static PACKAGE_CACHE: OnceLock<Mutex<HashMap<String, Vec<Package>>>> = OnceLock::new();

fn package_cache() -> &'static Mutex<HashMap<String, Vec<Package>>> {
    PACKAGE_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn invalidate_package_cache() {
    package_cache().lock().unwrap().clear();
}

impl qobject::PackageListModel {
    pub fn search(mut self: Pin<&mut Self>, query: QString) {
        let query = query.to_string();
        self.as_mut().rust_mut().mode = ListMode::Search;
        let cache_key = format!("search:{query}");
        self.as_mut().load_with(
            cache_key,
            async move {
                let proxy = runtime::proxy().await.ok_or_else(|| anyhow::anyhow!("no daemon connection"))?;
                proxy.search_packages(&query).await
            },
            "search",
        );
    }

    pub fn load_installed(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().mode = ListMode::Installed;
        self.as_mut().load_with(
            "installed".to_string(),
            async move {
                let proxy = runtime::proxy().await.ok_or_else(|| anyhow::anyhow!("no daemon connection"))?;
                proxy.installed_packages().await
            },
            "load installed",
        );
    }

    pub fn search_category(mut self: Pin<&mut Self>, category: QString) {
        let category = category.to_string();
        self.as_mut().rust_mut().mode = ListMode::Search;
        let cache_key = format!("category:{category}");
        self.as_mut().load_with(
            cache_key,
            async move {
                let proxy = runtime::proxy().await.ok_or_else(|| anyhow::anyhow!("no daemon connection"))?;
                proxy.search_category_packages(&category).await
            },
            "search category",
        );
    }

    pub fn load_updates(mut self: Pin<&mut Self>) {
        self.as_mut().rust_mut().mode = ListMode::Updates;
        self.as_mut().load_with(
            "updates".to_string(),
            async move {
                let proxy = runtime::proxy().await.ok_or_else(|| anyhow::anyhow!("no daemon connection"))?;
                proxy.updates_packages().await
            },
            "load updates",
        );
    }

    pub fn set_filters(mut self: Pin<&mut Self>, provider: QString, installed: i32, sort_by_name: bool) {
        {
            let mut rust = self.as_mut().rust_mut();
            rust.filter_provider = provider.to_string();
            rust.filter_installed = installed;
            rust.sort_by_name = sort_by_name;
        }
        self.apply_rows();
    }
    fn apply_rows(mut self: Pin<&mut Self>) {
        let providers = providers_list(&self.all_packages);
        self.as_mut().set_providers(providers);

        let mut rows: Vec<PackageRow> = self
            .all_packages
            .iter()
            .filter(|r| {
                self.filter_provider.is_empty() || provider_str(&r.pkg.provider) == self.filter_provider
            })
            .filter(|r| match self.filter_installed {
                1 => r.pkg.installed,
                2 => !r.pkg.installed,
                _ => true,
            })
            .cloned()
            .collect();
        if self.sort_by_name {
            rows.sort_by(|a, b| a.pkg.name.to_lowercase().cmp(&b.pkg.name.to_lowercase()));
        }

        unsafe {
            self.as_mut().begin_reset_model();
            self.as_mut().rust_mut().packages = rows;
            self.as_mut().end_reset_model();
        }
    }

    fn load_with(
        mut self: Pin<&mut Self>,
        cache_key: String,
        fetch: impl std::future::Future<Output = anyhow::Result<Vec<Package>>> + Send + 'static,
        what: &'static str,
    ) {
        let qt_thread = self.qt_thread();
        let _ = PACKAGE_LIST_QT_THREAD.set(qt_thread.clone());

        let seq = {
            let mut rust = self.as_mut().rust_mut();
            rust.filter_provider = String::new();
            rust.filter_installed = 0;
            rust.sort_by_name = false;
            rust.request_seq += 1;
            rust.request_seq
        };
        self.as_mut().set_loading(true);

        runtime::spawn(async move {
            let cached = {
                let guard = package_cache().lock().unwrap();
                guard.get(&cache_key).cloned()
            };
            let had_cache = cached.is_some();
            if let Some(cached) = cached {
                let qt = qt_thread.clone();
                if let Ok(built) = tokio::task::spawn_blocking(move || rows_and_providers(cached)).await {
                    fill_incrementally(qt, seq, built, false);
                }
            }

            let packages = match fetch.await {
                Ok(packages) => packages,
                Err(e) => {
                    tracing::warn!("{what} failed: {e}");
                    if !had_cache {
                        let qt_thread = qt_thread.clone();
                        let _ = qt_thread.queue(move |mut this| {
                            if this.request_seq == seq {
                                this.as_mut().set_loading(false);
                            }
                        });
                    }
                    return;
                }
            };

            let built = tokio::task::spawn_blocking(move || {
                let mut packages = packages;
                for pkg in &mut packages {
                    pkg.icon_url = Some(crate::services::icons::resolve(&pkg.id, pkg.icon_url.as_deref()));
                }
                package_cache().lock().unwrap().insert(cache_key, packages.clone());
                rows_and_providers(packages)
            })
            .await
            .unwrap_or_default();

            fill_incrementally(qt_thread, seq, built, had_cache);
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
            qobject::PackageRoles::Provider => QVariant::from(&QString::from(provider_str(&pkg.provider))),
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
        roles.insert(qobject::PackageRoles::Version.repr, QByteArray::from("version"));
        roles.insert(
            qobject::PackageRoles::Description.repr,
            QByteArray::from("description"),
        );
        roles.insert(qobject::PackageRoles::Provider.repr, QByteArray::from("provider"));
        roles.insert(
            qobject::PackageRoles::Installed.repr,
            QByteArray::from("installed"),
        );
        roles.insert(qobject::PackageRoles::IconUrl.repr, QByteArray::from("iconUrl"));
        roles.insert(qobject::PackageRoles::Busy.repr, QByteArray::from("busy"));
        roles.insert(qobject::PackageRoles::Progress.repr, QByteArray::from("progress"));
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
        .map(|pkg| PackageRow { pkg, busy: false, progress: 0.0 })
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
        if skip_if_same && same_ids(&this.all_packages, &rows) {
            this.as_mut().set_loading(false);
            return;
        }
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

fn sync_package_busy(pkg_id: &str, busy: bool, progress: f32) {
    crate::bridge::detail_controller::sync_busy(pkg_id, busy, progress);

    let Some(qt_thread) = PACKAGE_LIST_QT_THREAD.get() else {
        return;
    };
    let pkg_id = pkg_id.to_string();
    let _ = qt_thread.queue(move |mut model| {
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
    });
}

fn sync_package_finished(pkg_id: &str, tx_type: &str, success: bool) {
    let is_remove = tx_type == "remove" || tx_type == "remove_with_data";
    if success {
        invalidate_package_cache();
        crate::bridge::detail_controller::sync_installed(pkg_id, !is_remove);
        crate::bridge::detail_controller::refresh_extensions_for_current();
        if !is_remove {
            let id = pkg_id.to_string();
            runtime::spawn(async move { crate::services::forge::post_install(id).await });
        }
    } else {
        crate::bridge::detail_controller::sync_busy(pkg_id, false, 0.0);
    }

    let Some(qt_thread) = PACKAGE_LIST_QT_THREAD.get() else {
        return;
    };
    let pkg_id = pkg_id.to_string();
    let is_update = tx_type == "update";
    let _ = qt_thread.queue(move |mut model| {
        if !model.packages.iter().chain(model.all_packages.iter()).any(|r| r.pkg.id == pkg_id) {
            return;
        }
        let drop_row = success
            && ((is_remove && model.mode == ListMode::Installed) || (is_update && model.mode == ListMode::Updates));

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
                model.as_mut().begin_remove_rows(&QModelIndex::default(), row as i32, row as i32);
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
    active_count: i32,
    updates_count: i32,
    running_count: i32,
    queued_count: i32,
    done_count: i32,
    busy_packages_json: QString,
}

static QT_THREAD: OnceLock<CxxQtThread<qobject::TransactionsModel>> = OnceLock::new();

fn has_ongoing(transactions: &[Tx], pkg_id: &str) -> bool {
    transactions
        .iter()
        .any(|tx| tx.pkg_id == pkg_id && matches!(tx.status, TxStatus::Pending | TxStatus::Running))
}

impl qobject::TransactionsModel {
    pub fn init(mut self: Pin<&mut Self>) {
        if QT_THREAD.set(self.as_mut().qt_thread()).is_err() {
            return; // already initialized
        }

        runtime::spawn(async move {
            let Some(proxy) = runtime::proxy().await else { return };
            load_running_transactions(&proxy).await;
            refresh_updates_count(&proxy).await;

            loop {
                run_signal_listener(proxy.clone()).await;
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
        });
    }

    pub fn install(mut self: Pin<&mut Self>, pkg_id: QString, name: QString, icon_url: QString) {
        self.as_mut()
            .start_transaction(pkg_id.to_string(), name.to_string(), icon_url.to_string(), "install");
    }

    pub fn request_install(self: Pin<&mut Self>, pkg_id: QString, name: QString, icon_url: QString) {
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
                    model.as_mut().start_transaction(pkg_id, name, icon_url, "install");
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
        self.as_mut()
            .start_transaction(url_or_path.to_string(), name.to_string(), String::new(), "flatpakref");
    }

    pub fn install_bundle(mut self: Pin<&mut Self>, path: QString, name: QString) {
        self.as_mut()
            .start_transaction(path.to_string(), name.to_string(), String::new(), "bundle");
    }

    pub fn update(mut self: Pin<&mut Self>, pkg_id: QString, name: QString, icon_url: QString) {
        self.as_mut()
            .start_transaction(pkg_id.to_string(), name.to_string(), icon_url.to_string(), "update");
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

    pub fn update_all(self: Pin<&mut Self>) {
        runtime::spawn(async move {
            let Some(proxy) = runtime::proxy().await else {
                return;
            };
            let Ok(updates) = proxy.updates_packages().await else {
                return;
            };
            let Some(qt_thread) = QT_THREAD.get() else {
                return;
            };
            let _ = qt_thread.queue(move |mut model| {
                for pkg in updates {
                    let icon = crate::services::icons::resolve(&pkg.id, pkg.icon_url.as_deref());
                    model.as_mut().start_transaction(pkg.id, pkg.name, icon, "update");
                }
            });
        });
    }

    pub fn remove_package(mut self: Pin<&mut Self>, pkg_id: QString) {
        let pkg_id = pkg_id.to_string();
        let name = pkg_id.clone();
        self.as_mut().start_transaction(pkg_id, name, String::new(), "remove");
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
            .find(|tx| tx.pkg_id == pkg_id && matches!(tx.status, TxStatus::Pending | TxStatus::Running))
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

    fn start_transaction(mut self: Pin<&mut Self>, pkg_id: String, name: String, icon_url: String, tx_type: &'static str) {
        if has_ongoing(&self.transactions, &pkg_id) {
            return;
        }

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
                ..Default::default()
            });
            self.as_mut().end_insert_rows();
        }
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

            let result = match tx_type {
                "install" => proxy.install_package(&pkg_id).await,
                "remove" => proxy.remove_package(&pkg_id).await,
                "update" => proxy.update_package(&pkg_id).await,
                "flatpakref" => proxy.install_flatpakref(&pkg_id).await,
                "bundle" => proxy.install_flatpak_bundle(&pkg_id).await,
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
            qobject::TransactionRoles::Status => QVariant::from(&QString::from(status_str(tx.status))),
            qobject::TransactionRoles::TxType => QVariant::from(&QString::from(&tx.tx_type)),
            qobject::TransactionRoles::Section => QVariant::from(&QString::from(section_str(tx.status))),
            qobject::TransactionRoles::Error => QVariant::from(&QString::from(&tx.error)),
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
        roles.insert(qobject::TransactionRoles::PkgId.repr, QByteArray::from("pkgId"));
        roles.insert(qobject::TransactionRoles::Name.repr, QByteArray::from("name"));
        roles.insert(qobject::TransactionRoles::IconUrl.repr, QByteArray::from("iconUrl"));
        roles.insert(qobject::TransactionRoles::Progress.repr, QByteArray::from("progress"));
        roles.insert(qobject::TransactionRoles::Status.repr, QByteArray::from("status"));
        roles.insert(qobject::TransactionRoles::TxType.repr, QByteArray::from("txType"));
        roles.insert(qobject::TransactionRoles::Section.repr, QByteArray::from("section"));
        roles.insert(qobject::TransactionRoles::Error.repr, QByteArray::from("error"));
        roles.insert(qobject::TransactionRoles::BytesDone.repr, QByteArray::from("bytesDone"));
        roles.insert(qobject::TransactionRoles::BytesTotal.repr, QByteArray::from("bytesTotal"));
        roles.insert(qobject::TransactionRoles::SpeedBps.repr, QByteArray::from("speedBps"));
        roles.insert(qobject::TransactionRoles::EtaSecs.repr, QByteArray::from("etaSecs"));
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
        if let Some(row) = model
            .transactions
            .iter()
            .position(|tx| tx.pkg_id == pkg_id && matches!(tx.status, TxStatus::Pending | TxStatus::Running))
        {
            f(&mut model.as_mut().rust_mut().transactions[row]);
            let tx = model.transactions[row].clone();
            emit_tx_row_changed(model, row as i32);
            sync_package_row(&tx);
        }
    });
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
            emit_tx_row_changed(model, row as i32);
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
    model.as_mut().set_busy_packages_json(QString::from(&to_json(&busy_packages)));
    libarc::launcher::update_transactions(active.len() as i32, progress);
}

async fn load_running_transactions(proxy: &ArcDaemonProxy<'static>) {
    let Ok(txs) = proxy.transactions().await else {
        return;
    };

    let Some(qt_thread) = QT_THREAD.get() else {
        return;
    };
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
            let tx_type = match tx.transaction_type {
                libarc::TransactionType::Install => "install",
                libarc::TransactionType::Remove => "remove",
                libarc::TransactionType::Update => "update",
            };
            let icon_url = crate::services::icons::resolve(&tx.package_id, None);
            let row = model.transactions.len() as i32;
            unsafe {
                model
                    .as_mut()
                    .begin_insert_rows(&QModelIndex::default(), row, row);
                model.as_mut().rust_mut().transactions.push(Tx {
                    id,
                    pkg_id: tx.package_id.clone(),
                    name: tx.package_id,
                    icon_url,
                    progress: tx.progress as f32 / 100.0,
                    status,
                    tx_type: tx_type.to_string(),
                    error,
                    ..Default::default()
                });
                model.as_mut().end_insert_rows();
            }
        }
        sync_launcher_badge(model.as_mut());
    });
}

async fn refresh_updates_count(proxy: &ArcDaemonProxy<'static>) {
    let Ok(updates) = proxy.updates_packages().await else {
        return;
    };
    let count = updates.len();
    libarc::launcher::update_badge(count as i32);
    let Some(qt_thread) = QT_THREAD.get() else {
        return;
    };
    let _ = qt_thread.queue(move |mut model| {
        model.as_mut().set_updates_count(count as i32);
    });
}

async fn run_signal_listener(proxy: ArcDaemonProxy<'static>) {
    let (Ok(mut progress_stream), Ok(mut finished_stream), Ok(mut updates_stream), Ok(mut stats_stream)) = tokio::join!(
        proxy.receive_transaction_progress(),
        proxy.receive_transaction_finished(),
        proxy.receive_updates_available(),
        proxy.receive_transaction_stats(),
    ) else {
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
                queue_update_by_id(&tx_id, move |tx| {
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

fn to_json<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_string(v).unwrap_or_default()
}

pub struct HomeFeedModelRust {
    loading: bool,
    sections: Vec<crate::services::home::HomeSection>,
}

impl Default for HomeFeedModelRust {
    fn default() -> Self {
        Self { loading: true, sections: Vec::new() }
    }
}

impl qobject::HomeFeedModel {
    pub fn load(mut self: Pin<&mut Self>) {
        if !self.sections.is_empty() {
            return;
        }
        if let Some((sections, stories)) = crate::services::home_cache::load() {
            crate::bridge::home_model::stash_stories(stories);
            unsafe {
                self.as_mut().begin_reset_model();
                self.as_mut().rust_mut().sections = sections;
                self.as_mut().end_reset_model();
            }
        }
        self.fetch();
    }

    pub fn refresh(self: Pin<&mut Self>) {
        self.fetch();
    }

    fn fetch(mut self: Pin<&mut Self>) {
        let has_cached = !self.sections.is_empty();
        self.as_mut().set_loading(!has_cached);

        let qt_thread = self.qt_thread();
        runtime::spawn(async move {
            let proxy = runtime::proxy().await;
            let (sections, stories) = crate::services::home::load_home(proxy).await;
            crate::services::home_cache::save(&sections, &stories);
            crate::bridge::home_model::stash_stories(stories);

            qt_thread
                .queue(move |mut this| unsafe {
                    this.as_mut().begin_reset_model();
                    this.as_mut().rust_mut().sections = sections;
                    this.as_mut().end_reset_model();
                    this.as_mut().set_loading(false);
                })
                .ok();
        });
    }

    pub fn data(&self, index: &QModelIndex, role: i32) -> QVariant {
        let Some(s) = self.sections.get(index.row() as usize) else {
            return QVariant::default();
        };
        let role = qobject::HomeRoles { repr: role };
        match role {
            qobject::HomeRoles::ItemType => QVariant::from(&QString::from(s.item_type)),
            qobject::HomeRoles::Text => QVariant::from(&QString::from(&s.text)),
            qobject::HomeRoles::Title => QVariant::from(&QString::from(&s.title)),
            qobject::HomeRoles::CardsJson => QVariant::from(&QString::from(&to_json(&s.cards))),
            qobject::HomeRoles::HeroItemsJson => QVariant::from(&QString::from(&to_json(&s.hero_items))),
            qobject::HomeRoles::EditorialItemsJson => {
                QVariant::from(&QString::from(&to_json(&s.editorial_items)))
            }
            qobject::HomeRoles::LinkTitle => QVariant::from(&QString::from(&s.link_title)),
            qobject::HomeRoles::LinkItemsJson => QVariant::from(&QString::from(&to_json(&s.link_items))),
            qobject::HomeRoles::AppCloudOverlay => QVariant::from(&s.app_cloud_overlay),
            qobject::HomeRoles::CategoriesJson => QVariant::from(&QString::from(&to_json(&s.categories))),
            _ => QVariant::default(),
        }
    }

    pub fn role_names(&self) -> QHash<QHashPair_i32_QByteArray> {
        let mut roles = QHash::<QHashPair_i32_QByteArray>::default();
        roles.insert(qobject::HomeRoles::ItemType.repr, QByteArray::from("itemType"));
        roles.insert(qobject::HomeRoles::Text.repr, QByteArray::from("text"));
        roles.insert(qobject::HomeRoles::Title.repr, QByteArray::from("title"));
        roles.insert(qobject::HomeRoles::CardsJson.repr, QByteArray::from("cardsJson"));
        roles.insert(qobject::HomeRoles::HeroItemsJson.repr, QByteArray::from("heroItemsJson"));
        roles.insert(
            qobject::HomeRoles::EditorialItemsJson.repr,
            QByteArray::from("editorialItemsJson"),
        );
        roles.insert(qobject::HomeRoles::LinkTitle.repr, QByteArray::from("linkTitle"));
        roles.insert(qobject::HomeRoles::LinkItemsJson.repr, QByteArray::from("linkItemsJson"));
        roles.insert(
            qobject::HomeRoles::AppCloudOverlay.repr,
            QByteArray::from("appCloudOverlay"),
        );
        roles.insert(qobject::HomeRoles::CategoriesJson.repr, QByteArray::from("categoriesJson"));
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
        Self { loading: true, remotes: Vec::new() }
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
                        this.as_mut().action_succeeded(QString::from("Repository added"));
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
                Err(e) => emit_failed(&qt_thread, format!("Could not remove repository {name}: {e}")),
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
        roles.insert(qobject::RemoteRoles::Protected.repr, QByteArray::from("isProtected"));
        roles
    }

    pub fn row_count(&self, _parent: &QModelIndex) -> i32 {
        self.remotes.len() as i32
    }
}
