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

    // "install" or "update"
    property string mode: "install"

    property bool showStart: false
    property bool compactRemove: false

    signal removeRequested()
    signal startRequested()

    readonly property bool showUpdate: mode === "update"
    readonly property bool showRemove: !showUpdate && installed
    readonly property bool showInstall: !showUpdate && !installed

    spacing: Kirigami.Units.smallSpacing

    Controls.Button {
        visible: root.busy
        text: i18n("Cancel")
        onClicked: TransactionsModel.cancelForPackage(root.pkgId)
    }

    Controls.Button {
        visible: !root.busy && root.showRemove
        icon.name: root.compactRemove ? "edit-delete-symbolic" : ""
        display: root.compactRemove ? Controls.Button.IconOnly : Controls.Button.TextOnly
        text: i18n("Remove")
        Controls.ToolTip.text: text
        Controls.ToolTip.visible: root.compactRemove && hovered
        Controls.ToolTip.delay: Kirigami.Units.toolTipDelay
        onClicked: root.removeRequested()
    }

    Controls.Button {
        visible: !root.busy && root.showRemove && root.showStart
        highlighted: true
        text: i18n("Start")
        onClicked: root.startRequested()
    }

    Controls.Button {
        visible: !root.busy && (root.showInstall || root.showUpdate)
        highlighted: true
        text: root.showUpdate ? i18n("Update") : i18n("Install")
        onClicked: root.showUpdate
            ? TransactionsModel.update(root.pkgId, root.name, root.iconUrl)
            : TransactionsModel.requestInstall(root.pkgId, root.name, root.iconUrl)
    }
}
