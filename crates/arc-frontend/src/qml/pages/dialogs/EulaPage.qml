import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.blossomos.arc
import org.kde.ki18n

DialogPage {
    id: root

    signal closeRequested()

    required property string pkgId
    required property string appName
    property string iconUrl: ""
    property string eulaUrl: ""

    title: KI18n.i18n("License Agreement")
    dialogIcon: root.iconUrl.length > 0 ? root.iconUrl : "application-x-executable"
    dialogTitle: KI18n.i18n("License Agreement")
    dialogDescription: KI18n.i18n("%1 requires you to accept its End User License Agreement before installing.", root.appName)

    Controls.Button {
        Layout.alignment: Qt.AlignHCenter
        visible: root.eulaUrl.length > 0
        flat: true
        icon.name: "link-symbolic"
        text: KI18n.i18n("Read License Agreement")
        onClicked: Qt.openUrlExternally(root.eulaUrl)
    }

    RowLayout {
        Layout.alignment: Qt.AlignHCenter
        spacing: Kirigami.Units.smallSpacing

        Controls.Button {
            text: KI18n.i18n("Cancel")
            onClicked: root.closeRequested()
        }

        Controls.Button {
            text: KI18n.i18n("Accept & Install")
            highlighted: true
            onClicked: {
                TransactionsModel.install(root.pkgId, root.appName, root.iconUrl);
                root.closeRequested();
            }
        }
    }
}
