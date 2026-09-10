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

    property bool markInstalled: true
    property string headerColor: ""
    property string headerIcon: ""

    readonly property alias rowCount: listView.count

    readonly property real maxContentWidth: Kirigami.Units.gridUnit * 70

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

    component MetaPill: Rectangle {
        id: pill

        property alias text: pillLabel.text
        property string pillIcon: ""
        property color textColor: Kirigami.Theme.textColor

        radius: height / 2
        color: Kirigami.Theme.alternateBackgroundColor
        implicitHeight: pillRow.implicitHeight + Kirigami.Units.smallSpacing
        implicitWidth: pillRow.implicitWidth + Kirigami.Units.largeSpacing

        Row {
            id: pillRow
            anchors.centerIn: parent
            spacing: Kirigami.Units.smallSpacing / 2

            Kirigami.Icon {
                source: pill.pillIcon
                visible: pill.pillIcon.length > 0
                width: Kirigami.Units.iconSizes.small
                height: Kirigami.Units.iconSizes.small
                anchors.verticalCenter: parent.verticalCenter
                color: pill.textColor
            }

            Controls.Label {
                id: pillLabel
                anchors.verticalCenter: parent.verticalCenter
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                color: pill.textColor
                opacity: 0.85
            }
        }
    }

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
                text: KI18n.i18np("%1 app", "%1 apps", listView.count)
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

    Kirigami.CardsListView {
        id: listView
        model: root.packageListModel

        reuseItems: true

        headerPositioning: ListView.InlineHeader

        LoadingOverlay {
            visible: root.packageListModel.loading
        }

        Kirigami.PlaceholderMessage {
            anchors.centerIn: parent
            width: parent.width - Kirigami.Units.gridUnit * 4
            visible: !root.packageListModel.loading && listView.count === 0
            text: root.emptyText
        }

        header: Item {
            width: listView.width
            height: root.headerColor.length > 0 ? Kirigami.Units.gridUnit * 7 : 0
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

        delegate: Kirigami.AbstractCard {
            id: delegate

            width: Math.min(listView.width - Kirigami.Units.largeSpacing * 2,
                root.maxContentWidth)
            x: Math.round((listView.width - width) / 2)

            required property int index
            required property string pkgId
            required property string name
            required property string version
            required property string description
            required property string provider
            required property string iconUrl
            required property bool installed
            required property bool busy
            required property real progress

            showClickFeedback: true
            onClicked: NavController.openApp(delegate.pkgId, JSON.stringify({
                name: delegate.name,
                summary: delegate.description,
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

            contentItem: ColumnLayout {
                spacing: Kirigami.Units.largeSpacing

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.largeSpacing

                    AppIcon {
                        source: delegate.iconUrl
                        Layout.preferredWidth: Kirigami.Units.iconSizes.huge
                        Layout.preferredHeight: Kirigami.Units.iconSizes.huge
                        Layout.alignment: Qt.AlignTop
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: Kirigami.Units.smallSpacing / 2

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: Kirigami.Units.smallSpacing

                            Kirigami.Heading {
                                Layout.fillWidth: true
                                level: 3
                                text: delegate.name
                                elide: Text.ElideRight
                            }

                            Controls.Label {
                                visible: delegate.version.length > 0
                                text: delegate.version
                                elide: Text.ElideRight
                                opacity: 0.6
                                font.pointSize: Kirigami.Theme.smallFont.pointSize
                            }
                        }

                        Controls.Label {
                            id: descriptionLabel

                            Layout.fillWidth: true
                            Layout.minimumWidth: 0
                            Layout.preferredHeight: descriptionMetrics.height * 2
                            text: delegate.description
                            wrapMode: Text.WordWrap
                            elide: Text.ElideRight
                            maximumLineCount: 2
                            verticalAlignment: Text.AlignTop
                            opacity: 0.7

                            FontMetrics {
                                id: descriptionMetrics
                                font: descriptionLabel.font
                            }
                        }

                        Row {
                            Layout.topMargin: Kirigami.Units.smallSpacing
                            spacing: Kirigami.Units.smallSpacing

                            MetaPill {
                                text: delegate.provider
                            }

                            MetaPill {
                                visible: root.markInstalled && delegate.installed
                                pillIcon: "checkmark-symbolic"
                                text: KI18n.i18n("Installed")
                                textColor: Kirigami.Theme.positiveTextColor
                            }
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

                ItemProgressBar {
                    Layout.fillWidth: true
                    visible: delegate.busy
                    progress: delegate.progress
                }
            }
        }
    }
}
