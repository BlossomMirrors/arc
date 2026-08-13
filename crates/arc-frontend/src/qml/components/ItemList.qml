pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.blossomos.arc

Kirigami.ScrollablePage {
    id: root

    property PackageListModel packageListModel
    property string emptyText: ""
    property bool showFilters: false
    property bool markInstalled: true
    property string headerColor: ""
    property string headerIcon: ""

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

    header: Loader {
        active: root.showFilters
        visible: active

        sourceComponent: Controls.ToolBar {
            position: Controls.ToolBar.Header

            contentItem: RowLayout {
                spacing: Kirigami.Units.smallSpacing

                function applyFilters() {
                    packageListModel.setFilters(
                        sourceCombo.currentIndex === 0 ? "" : sourceCombo.currentText,
                        stateCombo.currentIndex,
                        sortCombo.currentIndex === 1);
                }

                Controls.ComboBox {
                    id: sourceCombo
                    visible: packageListModel.providers.length > 1
                    model: [i18n("All sources")].concat(packageListModel.providers)
                    onActivated: parent.applyFilters()
                }

                Controls.ComboBox {
                    id: stateCombo
                    model: [i18n("Everything"), i18n("Installed"), i18n("Not installed")]
                    onActivated: parent.applyFilters()
                }

                Controls.ComboBox {
                    id: sortCombo
                    model: [i18n("Relevance"), i18n("Name A-Z")]
                    onActivated: parent.applyFilters()
                }

                Item { Layout.fillWidth: true }

                Controls.Label {
                    visible: !packageListModel.loading
                    text: i18np("%1 app", "%1 apps", listView.count)
                    opacity: 0.7
                }
            }
        }
    }

    Kirigami.CardsListView {
        id: listView
        model: packageListModel

        reuseItems: true

        headerPositioning: ListView.InlineHeader

        LoadingOverlay {
            visible: packageListModel.loading
        }

        Kirigami.PlaceholderMessage {
            anchors.centerIn: parent
            width: parent.width - Kirigami.Units.gridUnit * 4
            visible: !packageListModel.loading && listView.count === 0
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
            onClicked: applicationWindow().openApp(delegate.pkgId, {
                name: delegate.name,
                summary: delegate.description,
                iconUrl: delegate.iconUrl,
                installed: delegate.installed
            })

            HoverHandler {
                id: rowHover
            }

            Timer {
                interval: 200
                running: rowHover.hovered
                onTriggered: DetailController.prefetch(delegate.pkgId)
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
                            Layout.fillWidth: true
                            text: delegate.description
                            wrapMode: Text.WordWrap
                            elide: Text.ElideRight
                            maximumLineCount: 2
                            opacity: 0.7
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
                                text: i18n("Installed")
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
                        onRemoveRequested: TransactionsModel.removePackage(delegate.pkgId)
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
