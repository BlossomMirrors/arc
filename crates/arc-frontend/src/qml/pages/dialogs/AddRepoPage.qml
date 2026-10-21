import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.blossomos.arc
import org.kde.ki18n

DialogPage {
    id: root

    title: KI18n.i18n("Add Repository")
    dialogIcon: "folder-remote"
    dialogTitle: DeepLinkController.repoTitle
    dialogDescription: DeepLinkController.repoUrl

    Kirigami.InlineMessage {
        Layout.fillWidth: true
        visible: SettingsController.showSecurityWarnings
        type: Kirigami.MessageType.Warning
        text: KI18n.i18n("Adding a third-party repository gives it the ability to distribute software to your system. Only add repositories from sources you trust.")
        showCloseButton: false
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
                onClicked: NavController.goSettings()
            }

            Controls.Button {
                text: KI18n.i18n("Add Repository")
                highlighted: true
                onClicked: {
                    RemotesModel.addFlatpakrepo(DeepLinkController.repoContent);
                    NavController.goSettings();
                }
            }
        }
    }
}
