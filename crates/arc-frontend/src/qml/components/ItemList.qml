pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.blossomos.arc
import org.kde.ki18n

Kirigami.ScrollablePage {
    id: root

    property PackageListModel packageListModel
    property string emptyText: ""
    property bool showFilters: false
    property bool showSearch: false
    property string searchQuery: ""

    property string headerColor: ""
    property string headerIcon: ""

    readonly property alias rowCount: repeater.count

    signal searchEdited(string query)

    function focusSearch(prefill) {
        if (!root.showSearch) {
            return;
        }
        barSearchField.forceActiveFocus();
        if (prefill !== undefined && prefill.length > 0) {
            barSearchField.text = prefill;
        }
        barSearchField.cursorPosition = barSearchField.text.length;
    }

    Kirigami.ColumnView.fillWidth: true

    header: Controls.ToolBar {
        id: filterBar
        position: Controls.ToolBar.Header
        visible: root.showFilters || root.showSearch
        height: visible ? implicitHeight : 0
        leftPadding: Kirigami.Units.largeSpacing
        rightPadding: Kirigami.Units.largeSpacing

        contentItem: RowLayout {
            spacing: Kirigami.Units.smallSpacing

            function applyFilters() {
                root.packageListModel.setFilters(
                    sourceCombo.currentIndex === 0 ? "" : sourceCombo.currentText,
                    stateCombo.currentIndex,
                    sortCombo.currentIndex === 1);
            }

            Controls.ComboBox {
                id: sourceCombo
                visible: root.showFilters && root.packageListModel.providers.length > 1
                model: [KI18n.i18n("All sources")].concat(root.packageListModel.providers)
                onActivated: parent.applyFilters()
            }

            Controls.ComboBox {
                id: stateCombo
                visible: root.showFilters
                model: [KI18n.i18n("Everything"), KI18n.i18n("Installed"), KI18n.i18n("Not installed")]
                onActivated: parent.applyFilters()
            }

            Controls.ComboBox {
                id: sortCombo
                visible: root.showFilters
                model: [KI18n.i18n("Relevance"), KI18n.i18n("Name A-Z")]
                onActivated: parent.applyFilters()
            }

            Item { Layout.fillWidth: true }

            Controls.Label {
                visible: !root.packageListModel.loading
                text: KI18n.i18np("%1 app", "%1 apps", repeater.count)
                opacity: 0.7
            }

            Kirigami.SearchField {
                id: barSearchField
                visible: root.showSearch
                Layout.preferredWidth: Kirigami.Units.gridUnit * 24
                Layout.preferredHeight: Kirigami.Units.gridUnit * 1.8
                placeholderText: KI18n.i18n("Search apps...")
                font.pointSize: Kirigami.Theme.defaultFont.pointSize + 1
                autoAccept: false
                text: root.searchQuery

                onTextChanged: if (text !== root.searchQuery) liveSearchTimer.restart()
                onAccepted: {
                    liveSearchTimer.stop();
                    root.searchEdited(text);
                }

                Timer {
                    id: liveSearchTimer
                    interval: 220
                    onTriggered: root.searchEdited(barSearchField.text)
                }
            }
        }
    }

    LoadingOverlay {
        visible: root.packageListModel.loading
    }

    Kirigami.PlaceholderMessage {
        anchors.centerIn: parent
        width: parent.width - Kirigami.Units.gridUnit * 4
        visible: !root.packageListModel.loading && repeater.count === 0
        text: root.emptyText
    }

    ColumnLayout {
        width: root.width
        spacing: 0

        Item {
            Layout.fillWidth: true
            Layout.preferredHeight: root.headerColor.length > 0 ? Kirigami.Units.gridUnit * 7 : 0
            visible: root.headerColor.length > 0

            Rectangle {
                anchors.fill: parent
                gradient: Gradient {
                    GradientStop { position: 0.0; color: root.headerColor }
                    GradientStop { position: 1.0; color: Kirigami.Theme.backgroundColor }
                }
            }

            ColumnLayout {
                anchors.centerIn: parent
                spacing: Kirigami.Units.smallSpacing

                Kirigami.Icon {
                    Layout.alignment: Qt.AlignHCenter
                    source: root.headerIcon
                    visible: root.headerIcon.length > 0
                    color: "white"
                    Layout.preferredWidth: Kirigami.Units.iconSizes.huge
                    Layout.preferredHeight: Kirigami.Units.iconSizes.huge
                }

                Kirigami.Heading {
                    Layout.alignment: Qt.AlignHCenter
                    level: 1
                    text: root.title
                    color: "white"
                }
            }
        }

        ColumnLayout {
            Layout.alignment: Qt.AlignHCenter
            Layout.fillWidth: true
            Layout.maximumWidth: Kirigami.Units.gridUnit * 44
            spacing: Kirigami.Units.largeSpacing

            Repeater {
                id: repeater
                model: root.packageListModel

                delegate: Kirigami.AbstractCard {
                    id: delegate

                    required property int index
                    required property string pkgId
                    required property string name
                    required property string version
                    required property string iconUrl
                    required property bool installed
                    required property bool busy

                    Layout.fillWidth: true

                    showClickFeedback: true
                    onClicked: NavController.openApp(delegate.pkgId, JSON.stringify({
                        name: delegate.name,
                        iconUrl: delegate.iconUrl,
                        installed: delegate.installed
                    }))

                    HoverHandler {
                        id: rowHover
                    }

                    Timer {
                        interval: 200
                        running: rowHover.hovered
                        onTriggered: DetailController.prefetch(delegate.pkgId, Math.round(Kirigami.Units.gridUnit * 14 * 16 / 9 * 2))
                    }

                    contentItem: RowLayout {
                        spacing: Kirigami.Units.largeSpacing

                        AppIcon {
                            source: delegate.iconUrl
                            Layout.preferredWidth: Kirigami.Units.iconSizes.large
                            Layout.preferredHeight: Kirigami.Units.iconSizes.large
                        }

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: Kirigami.Units.smallSpacing / 2

                            Kirigami.Heading {
                                Layout.fillWidth: true
                                level: 3
                                text: delegate.name
                                elide: Text.ElideRight
                            }

                            Controls.Label {
                                Layout.fillWidth: true
                                visible: delegate.version.length > 0
                                text: delegate.version
                                opacity: 0.7
                                elide: Text.ElideRight
                            }
                        }

                        ItemButtons {
                            Layout.alignment: Qt.AlignVCenter
                            pkgId: delegate.pkgId
                            name: delegate.name
                            iconUrl: delegate.iconUrl
                            installed: delegate.installed
                            busy: delegate.busy
                            mode: "install"
                            onRemoveRequested: TransactionsModel.removePackage(delegate.pkgId, delegate.name, delegate.iconUrl)
                        }
                    }
                }
            }

            Item { Layout.preferredHeight: Kirigami.Units.gridUnit * 2 }
        }
    }
}
