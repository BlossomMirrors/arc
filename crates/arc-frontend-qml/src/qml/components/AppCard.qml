import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.blossomos.arc

Kirigami.AbstractCard {
    id: root

    property string pkgId: ""
    property string appName: ""
    property string summary: ""
    property string iconUrl: ""
    property bool installed: false

    readonly property var busyMap: JSON.parse(TransactionsModel.busyPackagesJson || "{}")
    readonly property bool busy: root.busyMap[root.pkgId] !== undefined
    readonly property real progress: root.busyMap[root.pkgId] ?? 0

    readonly property bool cardHovered: hoverHandler.hovered

    signal activated()

    showClickFeedback: true
    onClicked: root.activated()

    HoverHandler {
        id: hoverHandler
    }

    Timer {
        interval: 200
        running: root.cardHovered
        onTriggered: DetailController.prefetch(root.pkgId)
    }

    background: Kirigami.ShadowedRectangle {
        color: Kirigami.Theme.alternateBackgroundColor
        radius: Kirigami.Units.cornerRadius * 4
        border.width: 1
        border.color: Qt.rgba(Kirigami.Theme.textColor.r, Kirigami.Theme.textColor.g, Kirigami.Theme.textColor.b, root.cardHovered ? 0.2 : 0.1)

        shadow.size: root.cardHovered ? Kirigami.Units.largeSpacing * 2 : Kirigami.Units.largeSpacing
        shadow.yOffset: root.cardHovered ? 6 : 3
        shadow.color: Qt.rgba(0, 0, 0, root.cardHovered ? 0.4 : 0.25)

        Behavior on shadow.size { NumberAnimation { duration: Kirigami.Units.shortDuration } }
        Behavior on shadow.yOffset { NumberAnimation { duration: Kirigami.Units.shortDuration } }
        Behavior on color { ColorAnimation { duration: Kirigami.Units.shortDuration } }
        Behavior on border.color { ColorAnimation { duration: Kirigami.Units.shortDuration } }
    }

    contentItem: ColumnLayout {
        spacing: 0

        Item {
            Layout.alignment: Qt.AlignHCenter
            Layout.topMargin: Kirigami.Units.smallSpacing
            Layout.preferredWidth: Kirigami.Units.iconSizes.huge
            Layout.preferredHeight: Kirigami.Units.iconSizes.huge

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

        Controls.Label {
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing / 2
            text: root.appName
            horizontalAlignment: Text.AlignHCenter
            elide: Text.ElideRight
            font.bold: true
            font.pointSize: Kirigami.Theme.defaultFont.pointSize + 1
        }

        Controls.Label {
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.smallSpacing / 2
            text: root.summary
            horizontalAlignment: Text.AlignHCenter
            elide: Text.ElideRight
            wrapMode: Text.WordWrap
            maximumLineCount: 1
            opacity: 0.7
            font.pointSize: Kirigami.Theme.smallFont.pointSize
        }

        ItemButtons {
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.largeSpacing
            pkgId: root.pkgId
            name: root.appName
            iconUrl: root.iconUrl
            installed: root.installed
            busy: root.busy
            mode: "install"
            tonal: true
            fillWidth: true
            allowRemove: false
        }

        ItemProgressBar {
            Layout.fillWidth: true
            Layout.topMargin: Kirigami.Units.smallSpacing
            visible: root.busy
            progress: root.progress
        }

        Item { Layout.fillHeight: true }
    }
}
