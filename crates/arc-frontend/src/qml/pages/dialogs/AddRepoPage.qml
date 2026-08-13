import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.blossomos.arc

DialogPage {
    id: root

    title: i18n("Add Repository")
    dialogIcon: "folder-remote"
    dialogTitle: DeepLinkController.repoTitle
    dialogDescription: DeepLinkController.repoUrl

    RowLayout {
        Layout.alignment: Qt.AlignHCenter
        spacing: Kirigami.Units.smallSpacing

        Controls.Button {
            text: i18n("Cancel")
            onClicked: applicationWindow().goSettings()
        }

        Controls.Button {
            text: i18n("Add Repository")
            highlighted: true
            onClicked: {
                RemotesModel.addFlatpakrepo(DeepLinkController.repoContent);
                applicationWindow().goSettings();
            }
        }
    }
}
