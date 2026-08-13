import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.blossomos.arc

DialogPage {
    id: root

    required property string pkgId
    required property string appName
    property string iconUrl: ""
    property string eulaUrl: ""

    title: i18n("License Agreement")
    dialogIcon: root.iconUrl.length > 0 ? root.iconUrl : "application-x-executable"
    dialogTitle: i18n("License Agreement")
    dialogDescription: i18n("%1 requires you to accept its End User License Agreement before installing.", root.appName)

    Controls.Button {
        Layout.alignment: Qt.AlignHCenter
        visible: root.eulaUrl.length > 0
        flat: true
        icon.name: "link-symbolic"
        text: i18n("Read License Agreement")
        onClicked: Qt.openUrlExternally(root.eulaUrl)
    }

    RowLayout {
        Layout.alignment: Qt.AlignHCenter
        spacing: Kirigami.Units.smallSpacing

        Controls.Button {
            text: i18n("Cancel")
            onClicked: applicationWindow().pageStack.pop()
        }

        Controls.Button {
            text: i18n("Accept & Install")
            highlighted: true
            onClicked: {
                TransactionsModel.install(root.pkgId, root.appName, root.iconUrl);
                applicationWindow().pageStack.pop();
            }
        }
    }
}
