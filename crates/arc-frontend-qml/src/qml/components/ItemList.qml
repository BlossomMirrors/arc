pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.blossomos.arc

Kirigami.ScrollablePage {
    id: root

    // Main.qml only allows the split view next to list pages
    readonly property bool isListPage: true

    property string emptyText: ""

    Kirigami.ColumnView.fillWidth: !root.Kirigami.ColumnView.view
        || root.Kirigami.ColumnView.index === root.Kirigami.ColumnView.view.count - 1

    ConveyorLoader {
        anchors.centerIn: parent
        visible: PackageListModel.loading
    }

    Kirigami.PlaceholderMessage {
        anchors.centerIn: parent
        width: parent.width - Kirigami.Units.gridUnit * 4
        visible: !PackageListModel.loading && listView.count === 0
        text: root.emptyText
    }

    Kirigami.CardsListView {
        id: listView
        visible: !PackageListModel.loading
        model: PackageListModel

        delegate: Kirigami.AbstractCard {
            id: delegate

            required property int index
            required property string pkgId
            required property string name
            required property string description
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

            contentItem: ColumnLayout {
                spacing: Kirigami.Units.largeSpacing

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.largeSpacing

                    Kirigami.Icon {
                        source: delegate.iconUrl.length > 0 ? delegate.iconUrl : "application-x-executable"
                        Layout.preferredWidth: Kirigami.Units.iconSizes.huge
                        Layout.preferredHeight: Kirigami.Units.iconSizes.huge
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
                            text: delegate.description
                            wrapMode: Text.WordWrap
                            elide: Text.ElideRight
                            maximumLineCount: 2
                            opacity: 0.7
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
