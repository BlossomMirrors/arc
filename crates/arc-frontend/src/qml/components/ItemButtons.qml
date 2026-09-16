pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.blossomos.arc
import org.kde.ki18n

RowLayout {
    id: root

    property string pkgId: ""
    property string name: ""
    property string iconUrl: ""
    property bool installed: false
    property bool busy: false

    // "install" | "update"
    property string mode: "install"

    property bool allowRemove: true
    property bool fillWidth: false
    property bool flat: false
    property color textColor: Kirigami.Theme.textColor

    signal removeRequested()

    readonly property bool showUpdate: mode === "update"
    readonly property bool showRemove: !showUpdate && installed && allowRemove
    readonly property bool showInstall: !showUpdate && !installed
    readonly property bool showStart: !showUpdate && installed

    function triggerInstall() {
        root.showUpdate
            ? TransactionsModel.update(root.pkgId, root.name, root.iconUrl)
            : TransactionsModel.requestInstall(root.pkgId, root.name, root.iconUrl);
    }

    spacing: Kirigami.Units.smallSpacing

    Loader {
        Layout.fillWidth: root.fillWidth
        active: root.busy
        visible: active
        sourceComponent: Controls.Button {
            flat: root.flat
            palette.buttonText: root.textColor
            icon.color: root.textColor
            text: KI18n.i18n("Cancel")
            onClicked: TransactionsModel.cancelForPackage(root.pkgId)
        }
    }

    Loader {
        active: !root.busy && root.showRemove
        visible: active
        sourceComponent: Controls.Button {
            flat: root.flat
            palette.buttonText: root.textColor
            icon.name: "delete"
            icon.color: root.textColor
            display: Controls.Button.IconOnly
            text: KI18n.i18n("Remove")
            Controls.ToolTip.text: text
            Controls.ToolTip.visible: hovered
            Controls.ToolTip.delay: Kirigami.Units.toolTipDelay
            onClicked: root.removeRequested()
        }
    }

    Loader {
        active: !root.busy && root.showStart
        visible: active
        Layout.fillWidth: root.fillWidth
        sourceComponent: Controls.Button {
            flat: root.flat
            highlighted: !root.flat
            palette.buttonText: root.textColor
            icon.color: root.textColor
            text: KI18n.i18n("Start")
            onClicked: TransactionsModel.launch(root.pkgId)
        }
    }

    Loader {
        Layout.fillWidth: root.fillWidth
        active: !root.busy && (root.showInstall || root.showUpdate)
        visible: active
        sourceComponent: Controls.Button {
            flat: root.flat
            highlighted: !root.flat
            palette.buttonText: root.textColor
            icon.color: root.textColor
            text: root.showUpdate ? KI18n.i18n("Update") : KI18n.i18n("Install")
            onClicked: root.triggerInstall()
        }
    }
}
