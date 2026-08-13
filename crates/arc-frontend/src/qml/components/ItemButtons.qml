import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.blossomos.arc

RowLayout {
    id: root

    property string pkgId: ""
    property string name: ""
    property string iconUrl: ""
    property bool installed: false
    property bool busy: false

    // "install" | "update"
    property string mode: "install"

    property bool tonal: false
    property bool allowRemove: true
    property bool fillWidth: false

    property bool highlightStart: true

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
            text: i18n("Cancel")
            onClicked: TransactionsModel.cancelForPackage(root.pkgId)
        }
    }

    Loader {
        active: !root.busy && root.showRemove
        visible: active
        sourceComponent: Controls.Button {
            icon.name: "delete"
            display: Controls.Button.IconOnly
            text: i18n("Remove")
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
            highlighted: root.highlightStart
            text: i18n("Start")
            onClicked: TransactionsModel.launch(root.pkgId)
        }
    }

    Loader {
        Layout.fillWidth: root.fillWidth
        active: !root.busy && (root.showInstall || root.showUpdate)
        visible: active
        sourceComponent: Controls.Button {
            highlighted: !root.tonal
            text: root.showUpdate ? i18n("Update") : i18n("Install")
            onClicked: root.triggerInstall()
        }
    }
}
