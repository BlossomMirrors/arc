import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.blossomos.arc
import org.kde.ki18n

Controls.ToolBar {
    id: root

    property string currentView: "home"

    signal homeRequested()
    signal installedRequested()
    signal downloadsRequested()
    signal settingsRequested()

    signal searchFocusRequested(string prefill)

    position: Controls.ToolBar.Header
    padding: Kirigami.Units.smallSpacing

    Shortcut {
        sequences: [StandardKey.Back, "Alt+Left"]
        onActivated: NavController.goBack()
    }

    Shortcut {
        sequences: [StandardKey.Forward, "Alt+Right"]
        onActivated: NavController.goForward()
    }

    Shortcut {
        sequence: StandardKey.Find
        onActivated: root.searchFocusRequested("")
    }

    component CountBadge: Rectangle {
        property int count: 0
        property color accent: Kirigami.Theme.highlightColor

        visible: count > 0
        width: Math.max(height, badgeLabel.implicitWidth + 8)
        height: badgeLabel.implicitHeight + 2
        radius: height / 2
        color: accent

        Controls.Label {
            id: badgeLabel
            anchors.centerIn: parent
            text: parent.count > 99 ? "99+" : parent.count
            font.pointSize: Kirigami.Theme.smallFont.pointSize - 1
            font.bold: true
            color: "white"
        }
    }

    contentItem: Item {
        implicitHeight: Math.max(navButtons.implicitHeight, viewTabs.implicitHeight, rightGroup.implicitHeight)

        RowLayout {
            id: navButtons
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            spacing: 0

            Controls.ToolButton {
                icon.name: "go-previous-symbolic"
                enabled: NavController.canGoBack
                onClicked: NavController.goBack()
                Controls.ToolTip.text: KI18n.i18n("Back (Alt+Left)")
                Controls.ToolTip.visible: hovered
                Controls.ToolTip.delay: Kirigami.Units.toolTipDelay
            }

            Controls.ToolButton {
                icon.name: "go-next-symbolic"
                enabled: NavController.canGoForward
                onClicked: NavController.goForward()
                Controls.ToolTip.text: KI18n.i18n("Forward (Alt+Right)")
                Controls.ToolTip.visible: hovered
                Controls.ToolTip.delay: Kirigami.Units.toolTipDelay
            }
        }

        Controls.TabBar {
            id: viewTabs

            x: Math.max(
                navButtons.width + Kirigami.Units.largeSpacing,
                Math.min(
                    (parent.width - width) / 2,
                    rightGroup.x - width - Kirigami.Units.largeSpacing))
            anchors.verticalCenter: parent.verticalCenter
            width: implicitWidth

            WheelHandler {
                acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
                onWheel: event => event.accepted = true
            }

            Binding on currentIndex {
                value: root.currentView === "home" ? 0
                    : root.currentView === "installed" ? 1
                    : root.currentView === "downloads" ? 2 : -1
            }

            Controls.TabButton {
                width: implicitWidth
                text: KI18n.i18n("Home")
                icon.name: "go-home-symbolic"
                onClicked: root.homeRequested()
            }

            Controls.TabButton {
                width: implicitWidth
                text: KI18n.i18n("Installed")
                icon.name: "drive-harddisk-symbolic"
                onClicked: root.installedRequested()
            }

            Controls.TabButton {
                width: implicitWidth
                text: KI18n.i18n("Downloads")
                icon.name: "download-symbolic"
                onClicked: root.downloadsRequested()

                CountBadge {
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.margins: 2
                    count: TransactionsModel.activeCount + TransactionsModel.updatesCount
                    accent: TransactionsModel.activeCount > 0
                        ? Kirigami.Theme.highlightColor
                        : Kirigami.Theme.negativeTextColor
                }
            }
        }

        RowLayout {
            id: rightGroup
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            spacing: Kirigami.Units.smallSpacing

            Controls.ToolButton {
                icon.name: "settings-configure-symbolic"
                onClicked: root.settingsRequested()
                Controls.ToolTip.text: KI18n.i18n("Settings")
                Controls.ToolTip.visible: hovered
                Controls.ToolTip.delay: Kirigami.Units.toolTipDelay
            }
        }
    }

    Kirigami.Separator {
        parent: root
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
    }
}
