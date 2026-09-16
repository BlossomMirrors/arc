import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.blossomos.arc
import org.kde.ki18n

DialogPage {
    id: root

    title: KI18n.i18n("Install App")
    dialogIcon: "download"
    dialogTitle: DeepLinkController.refTitle
    dialogDescription: KI18n.i18n("This app comes from a third-party source outside your configured repositories.")

    Item {
        Layout.fillWidth: true
        Layout.preferredHeight: buttonRow.implicitHeight

        RowLayout {
            id: buttonRow
            anchors.horizontalCenter: parent.horizontalCenter
            spacing: Kirigami.Units.smallSpacing

            Controls.Button {
                text: KI18n.i18n("Cancel")
                onClicked: NavController.goHome()
            }

            Controls.Button {
                text: KI18n.i18n("Install")
                highlighted: true
                onClicked: {
                    TransactionsModel.installFlatpakref(DeepLinkController.refSource, DeepLinkController.refTitle);
                    NavController.goDownloads();
                }
            }
        }
    }
}
