pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.blossomos.arc
import org.kde.ki18n

Kirigami.ScrollablePage {
    id: root

    property string slug: ""
    property string label: ""

    readonly property var apps: JSON.parse(ListController.appsJson || "[]")
    readonly property bool loading: ListController.loading

    property var selection: ({})

    readonly property var selectableApps: root.apps.filter(a => !a.installed)
    readonly property int selectedCount: Object.keys(root.selection).length
    readonly property bool allSelected: root.selectableApps.length > 0
        && root.selectedCount === root.selectableApps.length

    title: root.label.length > 0 ? root.label : listLabels.labelFor(root.slug)

    Kirigami.ColumnView.fillWidth: true
    padding: 0

    readonly property real maxContentWidth: Kirigami.Units.gridUnit * 70
    readonly property real sideInset: Math.max(Kirigami.Units.largeSpacing,
        (root.width - root.maxContentWidth) / 2)

    ListLabels {
        id: listLabels
    }

    Binding {
        target: root.contentItem
        property: "rightPadding"
        value: 0
    }

    function openList(slug, label) {
        root.slug = slug;
        root.label = label ?? "";
        root.selection = {};
        ListController.load(slug);
    }

    function toggle(pkgId) {
        const next = Object.assign({}, root.selection);
        if (next[pkgId]) {
            delete next[pkgId];
        } else {
            next[pkgId] = true;
        }
        root.selection = next;
    }

    function selectAll() {
        const next = {};
        for (const app of root.selectableApps) {
            next[app.id] = true;
        }
        root.selection = next;
    }

    function clearSelection() {
        root.selection = {};
    }

    function installSelected() {
        for (const app of root.apps) {
            if (root.selection[app.id]) {
                TransactionsModel.requestInstall(app.id, app.name, app.icon_url);
            }
        }
        root.clearSelection();
    }

    onAppsChanged: {
        const next = {};
        for (const app of root.apps) {
            if (root.selection[app.id] && !app.installed) {
                next[app.id] = true;
            }
        }
        root.selection = next;
    }

    header: Controls.ToolBar {
        position: Controls.ToolBar.Header

        contentItem: RowLayout {
            spacing: Kirigami.Units.smallSpacing

            Kirigami.Icon {
                source: ListController.iconName
                visible: source.length > 0
                color: ListController.color.length > 0 ? ListController.color : Kirigami.Theme.textColor
                implicitWidth: Kirigami.Units.iconSizes.medium
                implicitHeight: Kirigami.Units.iconSizes.medium
            }

            Kirigami.Heading {
                level: 2
                text: root.title
                elide: Text.ElideRight
            }

            Controls.Label {
                Layout.fillWidth: true
                visible: ListController.description.length > 0
                text: ListController.description
                elide: Text.ElideRight
                opacity: 0.7
            }

            Item {
                Layout.fillWidth: true
                visible: ListController.description.length === 0
            }

            Controls.Button {
                text: root.allSelected ? KI18n.i18n("Select none") : KI18n.i18n("Select all")
                enabled: root.selectableApps.length > 0
                onClicked: root.allSelected ? root.clearSelection() : root.selectAll()
            }

            Controls.Button {
                highlighted: true
                enabled: root.selectedCount > 0
                text: KI18n.i18np("Install %1 app", "Install %1 apps", root.selectedCount)
                icon.name: "install"
                onClicked: root.installSelected()
            }
        }
    }

    LoadingOverlay {
        visible: root.loading
    }

    Kirigami.PlaceholderMessage {
        anchors.centerIn: parent
        width: parent.width - Kirigami.Units.gridUnit * 4
        visible: !root.loading && root.apps.length === 0
        text: KI18n.i18n("This list is empty")
    }

    ColumnLayout {
        width: root.width
        visible: !root.loading && root.apps.length > 0
        spacing: 0

        GridLayout {
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing
            Layout.bottomMargin: Kirigami.Units.largeSpacing
            Layout.leftMargin: root.sideInset
            Layout.rightMargin: root.sideInset
            columns: Math.max(1, Math.floor(width / (Kirigami.Units.gridUnit * 16)))
            columnSpacing: Kirigami.Units.largeSpacing
            rowSpacing: Kirigami.Units.smallSpacing

            Repeater {
                model: root.apps

                delegate: AppGridCard {
                    required property var modelData

                    Layout.fillWidth: true
                    Layout.preferredWidth: 1
                    Layout.minimumWidth: 0
                    selectable: true
                    selected: root.selection[modelData.id] === true
                    pkgId: modelData.id
                    appName: modelData.name
                    summary: modelData.summary
                    iconUrl: modelData.icon_url
                    installed: modelData.installed
                    onSelectionToggled: root.toggle(modelData.id)
                    onActivated: NavController.openApp(modelData.id, JSON.stringify({
                        name: modelData.name,
                        summary: modelData.summary,
                        iconUrl: modelData.icon_url,
                        installed: modelData.installed
                    }))
                }
            }
        }

        Item {
            Layout.preferredHeight: Kirigami.Units.gridUnit * 2
        }
    }
}
