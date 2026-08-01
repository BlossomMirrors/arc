import QtQuick
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

    property string currentView: "home"

    property var navSpec: []
    property bool navRestoring: false

    pageStack.globalToolBar.style: Kirigami.ApplicationHeaderStyle.None
    pageStack.defaultColumnWidth: Kirigami.Units.gridUnit * 25

    readonly property bool splitAllowed: pageStack.depth >= 2
        && pageStack.width >= pageStack.defaultColumnWidth * 2
        && pageStack.get(pageStack.depth - 2).isListPage === true

    Binding {
        target: root.pageStack.columnView
        property: "columnResizeMode"
        value: root.splitAllowed ? KL.ColumnView.FixedColumns : KL.ColumnView.SingleColumn
    }

    function pushEntry(entry) {
        switch (entry.kind) {
        case "home": pageStack.push(homePageComponent); break;
        case "search": pageStack.push(searchPageComponent, { query: entry.a ?? "" }); break;
        case "installed": pageStack.push(installedPageComponent); break;
        case "category": pageStack.push(categoryPageComponent, { categoryId: entry.a, categoryLabel: entry.b }); break;
        case "downloads": pageStack.push(downloadsPageComponent); break;
        case "settings": pageStack.push(settingsPageComponent); break;
        case "detail": pageStack.push(detailPageComponent, { pkgId: entry.a, seed: entry.c ?? null }); break;
        case "story": pageStack.push(storyPageComponent, { storyId: entry.a }); break;
        }
    }

    function sameEntry(x, y) {
        return x.kind === y.kind && (x.a ?? "") === (y.a ?? "") && (x.b ?? "") === (y.b ?? "");
    }

    function navigate(spec) {
        var common = 0;
        while (common < navSpec.length && common < spec.length && sameEntry(navSpec[common], spec[common])) {
            common++;
        }
        if (common === navSpec.length && spec.length > navSpec.length && pageStack.depth === navSpec.length) {
            for (var i = common; i < spec.length; i++) {
                pushEntry(spec[i]);
            }
        } else {
            while (pageStack.depth > 0) {
                pageStack.pop();
            }
            for (var j = 0; j < spec.length; j++) {
                pushEntry(spec[j]);
            }
        }
        navSpec = spec;
        currentView = spec.length > 0
            ? (spec[0].kind === "category" ? "search" : spec[0].kind)
            : "home";
        if (!navRestoring) {
            NavController.record(JSON.stringify(spec));
        }
    }

    function goHome() { navigate([{ kind: "home" }]) }
    function goSearch(query) { navigate([{ kind: "search", a: query }]) }
    function goInstalled() { navigate([{ kind: "installed" }]) }
    function goDownloads() { navigate([{ kind: "downloads" }]) }
    function goSettings() { navigate([{ kind: "settings" }]) }
    function openCategory(categoryId, categoryLabel) { navigate([{ kind: "category", a: categoryId, b: categoryLabel }]) }
    function openApp(pkgId, seed) { navigate(navSpec.concat([{ kind: "detail", a: pkgId, c: seed ?? null }])) }
    function openStory(storyId) { navigate(navSpec.concat([{ kind: "story", a: storyId }])) }

    header: TopBar {
        currentView: root.currentView

        onHomeRequested: root.goHome()
        onSearchRequested: query => root.goSearch(query)
        onInstalledRequested: root.goInstalled()
        onDownloadsRequested: root.goDownloads()
        onSettingsRequested: root.goSettings()
    }

    readonly property bool onScreen: visible && visibility !== Window.Minimized && visibility !== Window.Hidden
    onOnScreenChanged: WindowController.setVisible(onScreen)

    Connections {
        target: NavController
        function onRestoreRequested(state) {
            root.navRestoring = true;
            root.navigate(JSON.parse(state));
            root.navRestoring = false;
        }
    }

    Component {
        id: homePageComponent
        HomePage {}
    }

    Component {
        id: searchPageComponent
        SearchPage {}
    }

    Component {
        id: installedPageComponent
        InstalledPage {}
    }

    Component {
        id: categoryPageComponent
        CategoryPage {}
    }

    Component {
        id: downloadsPageComponent
        DownloadsPage {}
    }

    Component {
        id: settingsPageComponent
        SettingsPage {}
    }

    Component {
        id: detailPageComponent
        DetailPage {}
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
                root.navigate([{ kind: "detail", a: DeepLinkController.pkgId }]);
                break;
            case "flatpakref":
                root.pageStack.clear();
                root.pageStack.push(installFlatpakrefPageComponent);
                break;
            case "addrepo":
                root.pageStack.clear();
                root.pageStack.push(addRepoPageComponent);
                break;
            case "installfile":
                root.pageStack.clear();
                root.pageStack.push(installFilePageComponent);
                break;
            }
        }
    }

    Component.onCompleted: {
        TransactionsModel.init();
        WindowController.setVisible(root.onScreen);
        goHome();
        DeepLinkController.resolve();
    }
}
