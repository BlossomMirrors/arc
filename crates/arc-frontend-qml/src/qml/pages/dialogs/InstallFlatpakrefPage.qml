import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.blossomos.arc

DialogPage {
    id: root

    title: i18n("Install App")
    dialogIcon: "download"
    dialogTitle: DeepLinkController.refTitle
    dialogDescription: i18n("This app comes from a third-party source outside your configured repositories.")

    RowLayout {
        Layout.alignment: Qt.AlignHCenter
        spacing: Kirigami.Units.smallSpacing

        Controls.Button {
            text: i18n("Cancel")
            onClicked: applicationWindow().goHome()
        }

        Controls.Button {
            text: i18n("Install")
            highlighted: true
            onClicked: {
                TransactionsModel.installFlatpakref(DeepLinkController.refSource, DeepLinkController.refTitle);
                applicationWindow().goDownloads();
            }
        }
    }
}
