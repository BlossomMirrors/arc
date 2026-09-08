import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Window
import org.kde.kirigami as Kirigami
import org.kde.kirigami.layouts as KL
import org.blossomos.arc

Kirigami.ApplicationWindow {
    id: root

    title: i18n("Arc Store")
    minimumWidth: Kirigami.Units.gridUnit * 45
    minimumHeight: Kirigami.Units.gridUnit * 32
    width: Kirigami.Units.gridUnit * 66
    height: Kirigami.Units.gridUnit * 44

    readonly property string currentView: NavController.currentView

    readonly property bool frontendVisible: root.visibility !== Window.Minimized && root.visibility !== Window.Hidden
    onFrontendVisibleChanged: SettingsController.setFrontendVisible(root.frontendVisible)

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

            HomePage {}
            SearchPage {
                id: searchPageItem
            }
            InstalledPage {
                id: installedPageItem
            }
            DownloadsPage {
                id: downloadsPageItem
            }
            SettingsPage {}

            // InstalledPage and DownloadsPage share the PackageListModel singleton,
            // so only the active tab may trigger a load or their requests race
            // and clobber each other's results.
            onCurrentIndexChanged: {
                if (currentIndex === root.tabIndex.installed) {
                    installedPageItem.load();
                } else if (currentIndex === root.tabIndex.downloads) {
                    downloadsPageItem.load();
                }
            }
        }
    }

    property bool detailVisible: false

    property bool categoryVisible: false

    CategoryPage {
        id: categoryPageItem
        y: 0
        width: parent.width
        height: parent.height
        x: root.categoryVisible ? 0 : width
        visible: x < width

        Behavior on x {
            NumberAnimation {
                duration: 220
                easing.type: Easing.OutCubic
            }
        }
    }

    DetailPage {
        id: detailPageItem
        y: 0
        width: parent.width
        height: parent.height
        x: root.detailVisible ? 0 : width
        visible: x < width

        Behavior on x {
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
        case "story":
            return storyPageComponent;
        case "flatpakref":
            return installFlatpakrefPageComponent;
        case "addrepo":
            return addRepoPageComponent;
        case "installfile":
            return installFilePageComponent;
        }
        return storyPageComponent;
    }

    function entryProps(entry) {
        switch (entry.kind) {
        case "story":
            return {
                storyId: entry.a
            };
        }
        return {};
    }

    function pushEntry(entry) {
        pageStack.push(entryComponent(entry), entryProps(entry));
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

        function popOne() {
            const kind = root.localStack.pop();
            if (kind === "detail")
                root.detailVisible = false;
            else if (kind === "category")
                root.categoryVisible = false;
            else
                pageStack.pop();
        }

        switch (op.action) {
        case "tab":
            root.detailVisible = false;
            root.categoryVisible = false;
            root.localStack = [];
            if (pageStack.depth > 1)
                pageStack.pop(pageStack.get(0));
            tabView.currentIndex = root.tabIndex[op.entry.kind] ?? 0;
            if (op.entry.kind === "search")
                searchPageItem.query = op.entry.a ?? "";
            break;
        case "push":
            if (op.entry.kind === "detail") {
                detailPageItem.openEntry(op.entry.a, op.entry.c ?? null);
                root.detailVisible = true;
            } else if (op.entry.kind === "category") {
                root.detailVisible = false;
                categoryPageItem.openCategory(op.entry.a, op.entry.b, op.entry.c ?? "", op.entry.d ?? "");
                root.categoryVisible = true;
            } else {
                root.detailVisible = false;
                root.categoryVisible = false;
                pushEntry(op.entry);
            }
            root.localStack.push(op.entry.kind === "detail" || op.entry.kind === "category" ? op.entry.kind : "page");
            break;
        case "pop":
            popOne();
            break;
        case "popTo":
            while (root.localStack.length > op.depth)
                popOne();
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
    function liveSearch(query) {
        if (currentView === "search") {
            searchPageItem.query = query;
            NavController.updateQuery(query);
            return;
        }
        goSearch(query);
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
        NavController.openChild(JSON.stringify({
            kind: "story",
            a: storyId
        }));
    }

    header: TopBar {
        id: topBar
        currentView: root.currentView

        onHomeRequested: root.goHome()
        onSearchRequested: query => root.goSearch(query)
        onSearchTextEdited: query => root.liveSearch(query)
        onInstalledRequested: root.goInstalled()
        onDownloadsRequested: root.goDownloads()
        onSettingsRequested: root.goSettings()
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
        topBar.focusSearch(t);
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
            installedPageItem.load();
            downloadsPageItem.load();
            // a freshly (re)started daemon always boots with
            // frontend_visible=false; since this property's QML value
            // hasn't actually changed across the restart, onFrontendVisibleChanged
            // never re-fires on its own to tell the new process the window
            // is actually on screen
            SettingsController.setFrontendVisible(root.frontendVisible);
        }
        function onIconCacheCleared() {
            HomeFeedModel.reload();
            installedPageItem.load();
            downloadsPageItem.load();
        }
    }

    Component {
        id: storyPageComponent
        StoryPage {}
    }

    Component {
        id: installFlatpakrefPageComponent
        InstallFlatpakrefPage {}
    }

    Component {
        id: addRepoPageComponent
        AddRepoPage {}
    }

    Component {
        id: installFilePageComponent
        InstallFilePage {}
    }

    Component {
        id: eulaPageComponent
        EulaPage {}
    }

    Connections {
        target: TransactionsModel
        function onEulaRequired(pkgId, name, iconUrl, eulaUrl) {
            root.pageStack.push(eulaPageComponent, {
                pkgId: pkgId,
                appName: name,
                iconUrl: iconUrl,
                eulaUrl: eulaUrl
            });
        }
    }

    Connections {
        target: DeepLinkController
        function onKindChanged() {
            switch (DeepLinkController.kind) {
            case "detail":
                root.navigate([
                    {
                        kind: "detail",
                        a: DeepLinkController.pkgId
                    }
                ]);
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
    }

    Component.onCompleted: {
        TransactionsModel.init();
        goHome();
        DeepLinkController.resolve();
        pageStack.Keys.pressed.connect(handleTypeAhead);
        SettingsController.setFrontendVisible(root.frontendVisible);
    }
}
