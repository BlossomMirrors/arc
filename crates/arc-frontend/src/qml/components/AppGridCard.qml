pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.blossomos.arc
import org.kde.ki18n

Kirigami.AbstractCard {
    id: root

    property string pkgId: ""
    property string appName: ""
    property string summary: ""
    property string iconUrl: ""
    property bool installed: false

    property bool selectable: false
    property bool selected: false

    readonly property var busyMap: JSON.parse(TransactionsModel.busyPackagesJson || "{}")
    readonly property bool busy: root.busyMap[root.pkgId] !== undefined
    readonly property real progress: root.busyMap[root.pkgId] ?? 0

    signal activated()
    signal selectionToggled()

    showClickFeedback: true
    onClicked: root.activated()

    topPadding: Kirigami.Units.smallSpacing
    bottomPadding: Kirigami.Units.smallSpacing

    background: Rectangle {
        radius: Kirigami.Units.cornerRadius
        color: Qt.alpha(Kirigami.Theme.textColor, hoverHandler.hovered ? 0.07 : 0)

        Behavior on color {
            ColorAnimation { duration: Kirigami.Units.shortDuration }
        }
    }

    HoverHandler {
        id: hoverHandler
    }

    Timer {
        interval: 200
        running: hoverHandler.hovered
        onTriggered: DetailController.prefetch(root.pkgId, Math.round(Kirigami.Units.gridUnit * 14 * 16 / 9 * 2))
    }

    contentItem: ColumnLayout {
        spacing: Kirigami.Units.smallSpacing

        RowLayout {
            Layout.fillWidth: true
            spacing: Kirigami.Units.largeSpacing

            Controls.CheckBox {
                visible: root.selectable
                enabled: !root.installed
                checked: root.selected
                Layout.alignment: Qt.AlignVCenter
                onToggled: {
                    checked = Qt.binding(() => root.selected);
                    root.selectionToggled();
                }
            }

            Item {
                Layout.alignment: Qt.AlignVCenter
                Layout.preferredWidth: Kirigami.Units.iconSizes.medium
                Layout.preferredHeight: Kirigami.Units.iconSizes.medium

                AppIcon {
                    anchors.fill: parent
                    source: root.iconUrl
                }

                Kirigami.Icon {
                    visible: root.installed
                    source: "emblem-checked"
                    width: Kirigami.Units.iconSizes.small
                    height: Kirigami.Units.iconSizes.small
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom
                    anchors.margins: -2
                    color: Kirigami.Theme.positiveTextColor
                }
            }

            ColumnLayout {
                Layout.fillWidth: true
                Layout.minimumWidth: 0
                Layout.alignment: Qt.AlignVCenter
                spacing: 0

                Controls.Label {
                    Layout.fillWidth: true
                    text: root.appName
                    elide: Text.ElideRight
                }

                Controls.Label {
                    Layout.fillWidth: true
                    visible: root.summary.length > 0
                    text: root.summary
                    elide: Text.ElideRight
                    wrapMode: Text.WordWrap
                    maximumLineCount: 2
                    opacity: 0.7
                    font.pointSize: Kirigami.Theme.smallFont.pointSize
                }
            }

            ItemButtons {
                Layout.alignment: Qt.AlignVCenter
                visible: !root.selectable
                pkgId: root.pkgId
                name: root.appName
                iconUrl: root.iconUrl
                installed: root.installed
                busy: root.busy
                mode: "install"
                allowRemove: false
            }

            Controls.Label {
                visible: root.selectable && root.installed
                text: KI18n.i18n("Installed")
                opacity: 0.7
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                color: Kirigami.Theme.positiveTextColor
            }
        }

        ItemProgressBar {
            Layout.fillWidth: true
            visible: root.busy
            progress: root.progress
        }
    }
}
