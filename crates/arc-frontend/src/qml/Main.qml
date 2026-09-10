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
            SearchPage {
                id: searchPageItem
            }
            InstalledPage {
                id: installedPageItem
            }
            DownloadsPage {
                id: downloadsPageItem
            }
            SettingsPage {
                onNotificationRequested: message => root.showPassiveNotification(message)
            }

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

    property bool listVisible: false

    ListPage {
        id: listPageItem
        y: 0
        width: parent.width
        height: parent.height
        x: root.listVisible ? 0 : width
        visible: x < width

        Behavior on x {
            NumberAnimation {
                duration: 220
                easing.type: Easing.OutCubic
            }
        }
    }

    property bool storyVisible: false

    StoryPage {
        id: storyPageItem
        y: 0
        width: parent.width
        height: parent.height
        x: root.storyVisible ? 0 : width
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
        case "flatpakref":
            return installFlatpakrefPageComponent;
        case "addrepo":
            return addRepoPageComponent;
        case "installfile":
            return installFilePageComponent;
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
            return root.detailVisible && detailPageItem.pkgId === entry.a;
        case "category":
            return root.categoryVisible && categoryPageItem.categoryId === entry.a;
        case "story":
            return root.storyVisible && storyPageItem.storyId === entry.a;
        case "list":
            return root.listVisible && listPageItem.slug === entry.a;
        }
        return false;
    }

    function openOverlay(entry) {
        switch (entry.kind) {
        case "detail":
            detailPageItem.openEntry(entry.a, entry.c ?? null);
            root.detailVisible = true;
            break;
        case "category":
            categoryPageItem.openCategory(entry.a, entry.b, entry.c ?? "", entry.d ?? "");
            root.categoryVisible = true;
            break;
        case "story":
            storyPageItem.openStory(entry.a);
            root.storyVisible = true;
            break;
        case "list":
            listPageItem.openList(entry.a, entry.b ?? "");
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
                searchPageItem.query = op.entry.a ?? "";
                searchPageItem.focusSearch();
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
            searchPageItem.focusSearch(prefill);
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
            const page = root.pageStack.push(eulaPageComponent, {
                pkgId: pkgId,
                appName: name,
                iconUrl: iconUrl,
                eulaUrl: eulaUrl
            });
            page.closeRequested.connect(() => root.pageStack.pop());
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
    }

    Component.onCompleted: {
        TransactionsModel.init();
        goHome();
        DeepLinkController.resolve();
        pageStack.Keys.pressed.connect(handleTypeAhead);
        SettingsController.setFrontendVisible(root.frontendVisible);
    }
}
