import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.blossomos.arc

RowLayout {
    id: root

    signal removeClicked()
    signal addonsClicked()

    property bool hasExtensions: false

    readonly property var busyMap: JSON.parse(TransactionsModel.busyPackagesJson || "{}")
    readonly property bool liveBusy: root.busyMap[DetailController.id] !== undefined
    readonly property real liveProgress: root.busyMap[DetailController.id] ?? 0

    spacing: Kirigami.Units.largeSpacing

    component Pill: Rectangle {
        property alias text: pillLabel.text
        property color accent: Kirigami.Theme.textColor

        implicitWidth: pillLabel.implicitWidth + Kirigami.Units.largeSpacing
        implicitHeight: pillLabel.implicitHeight + Kirigami.Units.smallSpacing
        radius: height / 2
        color: Qt.alpha(accent, 0.12)
        border.width: 1
        border.color: Qt.alpha(accent, 0.3)

        Controls.Label {
            id: pillLabel
            anchors.centerIn: parent
            font.pointSize: Kirigami.Theme.smallFont.pointSize
            color: parent.accent
        }
    }

    AppIcon {
        source: DetailController.iconUrl
        Layout.preferredWidth: 96
        Layout.preferredHeight: 96
        Layout.alignment: Qt.AlignTop
    }

    ColumnLayout {
        Layout.fillWidth: true
        spacing: Kirigami.Units.smallSpacing

        Kirigami.Heading {
            level: 1
            Layout.fillWidth: true
            text: DetailController.name
            elide: Text.ElideRight
        }

        Controls.Label {
            Layout.fillWidth: true
            visible: DetailController.developerName.length > 0
            text: DetailController.developerName
            opacity: 0.65
            elide: Text.ElideRight
        }

        RowLayout {
            spacing: Kirigami.Units.smallSpacing

            Pill {
                visible: DetailController.version.length > 0
                text: DetailController.version
            }

            Pill {
                visible: DetailController.license.length > 0
                text: DetailController.license === "Proprietary" ? i18n("Proprietary") : DetailController.license
                accent: DetailController.license === "Proprietary"
                    ? Kirigami.Theme.negativeTextColor
                    : Kirigami.Theme.highlightColor
            }

            Pill {
                visible: DetailController.contentRating.length > 0
                text: DetailController.contentRating === "All ages" ? i18n("All ages") : DetailController.contentRating
                accent: DetailController.contentRating === "18+"
                    ? Kirigami.Theme.negativeTextColor
                    : DetailController.contentRating === "12+" || DetailController.contentRating === "7+"
                        ? Kirigami.Theme.neutralTextColor
                        : Kirigami.Theme.textColor
            }
        }
    }

    RowLayout {
        Layout.alignment: Qt.AlignVCenter
        spacing: Kirigami.Units.smallSpacing

        Controls.Button {
            visible: !root.liveBusy && DetailController.installed && root.hasExtensions
            icon.name: "list-add-symbolic"
            display: Controls.Button.IconOnly
            text: i18n("Add-ons")
            Controls.ToolTip.text: i18n("Add-ons")
            Controls.ToolTip.visible: hovered
            Controls.ToolTip.delay: Kirigami.Units.toolTipDelay
            onClicked: root.addonsClicked()
        }

        ItemButtons {
            pkgId: DetailController.id
            name: DetailController.name
            iconUrl: DetailController.iconUrl
            installed: DetailController.installed
            busy: root.liveBusy
            highlightStart: false
            onRemoveRequested: root.removeClicked()
        }
    }
}
