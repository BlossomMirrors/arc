import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.blossomos.arc
import org.kde.ki18n

DialogPage {
    id: root

    signal closeRequested()

    property string pkgId: ""
    property string appName: ""
    property string iconUrl: ""
    property string eulaUrl: ""

    function open(newPkgId, newAppName, newIconUrl, newEulaUrl) {
        root.pkgId = newPkgId;
        root.appName = newAppName;
        root.iconUrl = newIconUrl;
        root.eulaUrl = newEulaUrl;
    }

    title: KI18n.i18n("License Agreement")
    dialogIcon: root.iconUrl.length > 0 ? root.iconUrl : "application-x-executable"
    dialogTitle: KI18n.i18n("License Agreement")
    dialogDescription: KI18n.i18n("%1 requires you to accept its End User License Agreement before installing.", root.appName)

    Item {
        Layout.fillWidth: true
        Layout.preferredHeight: readEulaButton.implicitHeight
        visible: root.eulaUrl.length > 0

        Controls.Button {
            id: readEulaButton
            anchors.horizontalCenter: parent.horizontalCenter
            flat: true
            icon.name: "link-symbolic"
            text: KI18n.i18n("Read License Agreement")
            onClicked: Qt.openUrlExternally(root.eulaUrl)
        }
    }

    Item {
        Layout.fillWidth: true
        Layout.preferredHeight: buttonRow.implicitHeight

        RowLayout {
            id: buttonRow
            anchors.horizontalCenter: parent.horizontalCenter
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
}
