import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Window
import org.kde.kirigami as Kirigami
import org.kde.kirigami.layouts as KL
import org.blossomos.arc
import org.kde.ki18n

Kirigami.ApplicationWindow {
    id: root

    title: KI18n.i18n("Arc Store")
    minimumWidth: Kirigami.Units.gridUnit * 45
    minimumHeight: Kirigami.Units.gridUnit * 32
    width: Kirigami.Units.gridUnit * 66
    height: Kirigami.Units.gridUnit * 44

    readonly property string currentView: NavController.currentView

    readonly property bool frontendVisible: root.visibility !== Window.Minimized && root.visibility !== Window.Hidden
    onFrontendVisibleChanged: SettingsController.setFrontendVisible(root.frontendVisible)

    onFrameSwapped: StartupController.firstFrame()
    visible: !StartupController.startHidden
    onVisibleChanged: if (visible && StartupController.standby) HomeFeedModel.refresh()
    onClosing: close => {
        if (StartupController.preloadEnabled()) {
            close.accepted = false;
            StartupController.returnToStandby();
        }
    }

    pageStack.globalToolBar.style: Kirigami.ApplicationHeaderStyle.None

    Binding {
        target: root.pageStack.columnView
        property: "columnResizeMode"
        value: KL.ColumnView.SingleColumn
    }

    pageStack.initialPage: Kirigami.Page {
        padding: 0

        Controls.SwipeView {
            id: tabView
            anchors.fill: parent
            interactive: false

            Binding {
                target: tabView.contentItem
                property: "highlightMoveDuration"
                value: 0
            }

            HomePage {
                id: homePageItem
            }
            LazyPage {
                id: searchPage
        order: 0
                source: "pages/SearchPage.qml"
            }
            LazyPage {
                id: installedPage
        order: 1
                source: "pages/InstalledPage.qml"
                onLoaded: item.load()
            }
            LazyPage {
                id: downloadsPage
        order: 2
                source: "pages/DownloadsPage.qml"
                onLoaded: item.load()
            }
            LazyPage {
                id: settingsPage
        order: 3
                source: "pages/SettingsPage.qml"
            }

            property int previousIndex: 0

            onCurrentIndexChanged: {
                if (previousIndex === root.tabIndex.search) {
                    searchPage.item?.release();
                }
                previousIndex = currentIndex;

                if (currentIndex === root.tabIndex.installed) {
                    installedPage.page().load();
                } else if (currentIndex === root.tabIndex.downloads) {
                    downloadsPage.page().load();
                } else if (currentIndex === root.tabIndex.search) {
                    searchPage.page().restore();
                }
            }
        }
    }

    property bool detailVisible: false

    // pages opened from a link appear at once instead of sliding in
    property bool instantNav: false

    property bool categoryVisible: false

    LazyPage {
        id: categoryPage
        order: 5
        source: "pages/CategoryPage.qml"
        y: 0
        width: parent.width
        height: parent.height
        x: root.categoryVisible ? 0 : width
        visible: x < width

        Behavior on x {
            enabled: !root.instantNav
            NumberAnimation {
                duration: 220
                easing.type: Easing.OutCubic
            }
        }
    }

    property bool listVisible: false

    LazyPage {
        id: listPage
        order: 6
        source: "pages/ListPage.qml"
        y: 0
        width: parent.width
        height: parent.height
        x: root.listVisible ? 0 : width
        visible: x < width

        Behavior on x {
            enabled: !root.instantNav
            NumberAnimation {
                duration: 220
                easing.type: Easing.OutCubic
            }
        }
    }

    property bool storyVisible: false

    LazyPage {
        id: storyPage
        order: 7
        source: "pages/StoryPage.qml"
        y: 0
        width: parent.width
        height: parent.height
        x: root.storyVisible ? 0 : width
        visible: x < width

        Behavior on x {
            enabled: !root.instantNav
            NumberAnimation {
                duration: 220
                easing.type: Easing.OutCubic
            }
        }
    }

    LazyPage {
        id: detailPage
        order: 4
        active: StartupController.standby
        source: "pages/DetailPage.qml"
        y: 0
        width: parent.width
        height: parent.height
        x: root.detailVisible ? 0 : width
        visible: x < width

        Behavior on x {
            enabled: !root.instantNav
            NumberAnimation {
                duration: 220
                easing.type: Easing.OutCubic
            }
        }
    }

    property bool eulaVisible: false

    LazyPage {
        id: eulaPage
        order: 8
        source: "pages/dialogs/EulaPage.qml"
        y: 0
        width: parent.width
        height: parent.height
        x: root.eulaVisible ? 0 : width
        visible: x < width

        Behavior on x {
            enabled: !root.instantNav
            NumberAnimation {
                duration: 220
                easing.type: Easing.OutCubic
            }
        }
    }

    TapHandler {
        acceptedButtons: Qt.BackButton
        onTapped: NavController.goBack()
    }

    TapHandler {
        acceptedButtons: Qt.ForwardButton
        onTapped: NavController.goForward()
    }

    function entryComponent(entry) {
        switch (entry.kind) {
        case "flatpakref":
            return Qt.resolvedUrl("pages/dialogs/InstallFlatpakrefPage.qml");
        case "addrepo":
            return Qt.resolvedUrl("pages/dialogs/AddRepoPage.qml");
        case "installfile":
            return Qt.resolvedUrl("pages/dialogs/InstallFilePage.qml");
        }
        return null;
    }

    function pushEntry(entry) {
        const component = entryComponent(entry);
        if (component)
            pageStack.push(component);
    }

    function isOverlayKind(kind) {
        return kind === "detail" || kind === "category" || kind === "story" || kind === "list";
    }

    function overlayShows(entry) {
        switch (entry.kind) {
        case "detail":
            return root.detailVisible && detailPage.item?.pkgId === entry.a;
        case "category":
            return root.categoryVisible && categoryPage.item?.categoryId === entry.a;
        case "story":
            return root.storyVisible && storyPage.item?.storyId === entry.a;
        case "list":
            return root.listVisible && listPage.item?.slug === entry.a;
        }
        return false;
    }

    function openOverlay(entry) {
        switch (entry.kind) {
        case "detail":
            detailPage.page().openEntry(entry.a, entry.c ?? null);
            root.detailVisible = true;
            break;
        case "category":
            categoryPage.page().openCategory(entry.a, entry.b, entry.c ?? "", entry.d ?? "");
            root.categoryVisible = true;
            break;
        case "story":
            storyPage.page().openStory(entry.a);
            root.storyVisible = true;
            break;
        case "list":
            listPage.page().openList(entry.a, entry.b ?? "");
            root.listVisible = true;
            break;
        }
    }

    readonly property var tabIndex: ({
            home: 0,
            search: 1,
            installed: 2,
            downloads: 3,
            settings: 4
        })

    property var localStack: []

    function navigate(spec) {
        NavController.navigate(JSON.stringify(spec));
    }

    function runNavOp(json) {
        const op = JSON.parse(json);

        function popOne(revealed) {
            const kind = root.localStack.pop();
            const restore = revealed && root.isOverlayKind(revealed.kind) && !root.overlayShows(revealed);
            if (!restore || revealed.kind !== kind) {
                if (kind === "detail")
                    root.detailVisible = false;
                else if (kind === "category")
                    root.categoryVisible = false;
                else if (kind === "story")
                    root.storyVisible = false;
                else if (kind === "list")
                    root.listVisible = false;
                else
                    pageStack.pop();
            }
            if (restore)
                root.openOverlay(revealed);
        }

        switch (op.action) {
        case "tab":
            root.detailVisible = false;
            root.categoryVisible = false;
            root.storyVisible = false;
            root.listVisible = false;
            root.localStack = [];
            if (pageStack.depth > 1)
                pageStack.pop(pageStack.get(0));
            tabView.currentIndex = root.tabIndex[op.entry.kind] ?? 0;
            if (op.entry.kind === "search") {
                const search = searchPage.page();
                search.query = op.entry.a ?? "";
                search.focusSearch();
            } else if (op.entry.kind === "home") {
                homePageItem.searchText = "";
            }
            break;
        case "push":
            if (root.isOverlayKind(op.entry.kind)) {
                if (op.entry.kind !== "detail")
                    root.detailVisible = false;
                if (op.entry.kind === "category" || op.entry.kind === "list")
                    root.storyVisible = false;
                root.openOverlay(op.entry);
            } else {
                root.detailVisible = false;
                root.categoryVisible = false;
                root.storyVisible = false;
                root.listVisible = false;
                pushEntry(op.entry);
            }
            root.localStack.push(root.isOverlayKind(op.entry.kind) ? op.entry.kind : "page");
            break;
        case "pop":
            popOne(op.top);
            break;
        case "popTo":
            while (root.localStack.length > op.depth)
                popOne(root.localStack.length === op.depth + 1 ? op.top : null);
            break;
        }
    }

    function goHome() {
        navigate([
            {
                kind: "home"
            }
        ]);
    }
    function goSearch(query) {
        navigate([
            {
                kind: "search",
                a: query
            }
        ]);
    }
    function goInstalled() {
        navigate([
            {
                kind: "installed"
            }
        ]);
    }
    function goDownloads() {
        navigate([
            {
                kind: "downloads"
            }
        ]);
    }
    function goSettings() {
        navigate([
            {
                kind: "settings"
            }
        ]);
    }
    function openCategory(categoryId, categoryLabel, categoryColor, categoryIcon) {
        navigate([
            {
                kind: "category",
                a: categoryId,
                b: categoryLabel,
                c: categoryColor ?? "",
                d: categoryIcon ?? ""
            }
        ]);
    }
    function openApp(pkgId, seed) {
        navigate([
            {
                kind: "detail",
                a: pkgId,
                c: seed ?? null
            }
        ]);
    }
    function openStory(storyId) {
        navigate([
            {
                kind: "story",
                a: storyId
            }
        ]);
    }
    function openList(slug, label) {
        navigate([
            {
                kind: "list",
                a: slug,
                b: label ?? ""
            }
        ]);
    }

    header: TopBar {
        id: topBar
        currentView: root.currentView

        onHomeRequested: root.goHome()
        onInstalledRequested: root.goInstalled()
        onDownloadsRequested: root.goDownloads()
        onSettingsRequested: root.goSettings()
        onSearchFocusRequested: prefill => root.focusSearch(prefill)
    }

    function focusSearch(prefill) {
        if (root.currentView === "home") {
            homePageItem.focusSearch(prefill);
        } else if (root.currentView === "search") {
            searchPage.page().focusSearch(prefill);
        } else {
            root.goSearch(prefill ?? "");
        }
    }

    function handleTypeAhead(event) {
        if (event.modifiers !== Qt.NoModifier && event.modifiers !== Qt.ShiftModifier) {
            return;
        }
        if (event.key === Qt.Key_Escape || event.key === Qt.Key_Tab || event.key === Qt.Key_Backtab || event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
            return;
        }
        var t = event.text;
        if (t.length === 0 || t.charCodeAt(0) < 0x20 || t.charCodeAt(0) === 0x7f) {
            return;
        }
        root.focusSearch(t);
        event.accepted = true;
    }

    Connections {
        target: NavController
        function onNavOp(op) {
            root.runNavOp(op);
        }
    }

    Connections {
        target: SettingsController
        function onDaemonReconnected() {
            HomeFeedModel.reload();
            installedPage.item?.load();
            downloadsPage.item?.load();
            SettingsController.setFrontendVisible(root.frontendVisible);
        }
        function onIconCacheCleared() {
            HomeFeedModel.reload();
            installedPage.item?.load();
            downloadsPage.item?.load();
        }
    }

    Connections {
        target: settingsPage.item
        function onNotificationRequested(message) {
            root.showPassiveNotification(message);
        }
    }

    Connections {
        target: eulaPage.item
        function onCloseRequested() {
            root.eulaVisible = false;
        }
    }

    Connections {
        target: TransactionsModel
        function onEulaRequired(pkgId, name, iconUrl, eulaUrl) {
            eulaPage.page().open(pkgId, name, iconUrl, eulaUrl);
            root.eulaVisible = true;
        }
    }

    Kirigami.PromptDialog {
        id: removeConfirmDialog

        property string pkgId: ""
        property string appName: ""
        property string iconUrl: ""

        title: KI18n.i18n("Remove %1?", removeConfirmDialog.appName)
        subtitle: KI18n.i18n("The application will be uninstalled from your system.")
        standardButtons: Kirigami.Dialog.Cancel
        showCloseButton: false

        Controls.CheckBox {
            id: deleteDataCheckBox
            text: KI18n.i18n("Also delete app data")
        }

        customFooterActions: [
            Kirigami.Action {
                text: KI18n.i18n("Remove")
                icon.name: "delete"
                onTriggered: {
                    TransactionsModel.confirmRemove(
                        removeConfirmDialog.pkgId,
                        removeConfirmDialog.appName,
                        removeConfirmDialog.iconUrl,
                        deleteDataCheckBox.checked);
                    removeConfirmDialog.close();
                }
            }
        ]

        onOpened: deleteDataCheckBox.checked = false
    }

    Connections {
        target: TransactionsModel
        function onRemoveConfirmationNeeded(pkgId, name, iconUrl) {
            removeConfirmDialog.pkgId = pkgId;
            removeConfirmDialog.appName = name;
            removeConfirmDialog.iconUrl = iconUrl;
            removeConfirmDialog.open();
        }
    }

    Connections {
        target: DeepLinkController
        function onKindChanged() {
            root.instantNav = true;
            root.openDeepLink();
            root.instantNav = false;
            if (DeepLinkController.kind === "detail" || DeepLinkController.kind === "list")
                StartupController.showWindow();
        }
    }

    function openDeepLink() {
        switch (DeepLinkController.kind) {
        case "detail":
            root.navigate([
                {
                    kind: "detail",
                    a: DeepLinkController.pkgId
                }
            ]);
            break;
        case "list":
            root.openList(DeepLinkController.listSlug, "");
            break;
        case "flatpakref":
            root.navigate([
                {
                    kind: "flatpakref"
                }
            ]);
            break;
        case "addrepo":
            root.navigate([
                {
                    kind: "addrepo"
                }
            ]);
            break;
        case "installfile":
            root.navigate([
                {
                    kind: "installfile"
                }
            ]);
            break;
        }
    }

    Component.onCompleted: {
        DetailController.setShotWidth(Math.round(Kirigami.Units.gridUnit * 14 * 16 / 9 * 2));
        SettingsController.load();
        TransactionsModel.init();
        goHome();
        DeepLinkController.resolve();
        pageStack.Keys.pressed.connect(handleTypeAhead);
        SettingsController.setFrontendVisible(root.frontendVisible);
    }
}
