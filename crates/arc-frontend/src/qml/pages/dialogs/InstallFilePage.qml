import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.blossomos.arc
import org.kde.ki18n

DialogPage {
    id: root

    title: KI18n.i18n("Install File")
    dialogIcon: "package-x-generic"
    dialogTitle: DeepLinkController.fileName
    dialogDescription: DeepLinkController.fileHasFlatpakAlt
        ? KI18n.i18n("A Flatpak version of this app is also available: %1", DeepLinkController.fileFlatpakAltName)
        : ""

    Controls.Button {
        Layout.fillWidth: true
        visible: DeepLinkController.fileIsAppimage
        text: KI18n.i18n("Install AppImage")
        highlighted: true
        onClicked: {
            TransactionsModel.install(DeepLinkController.filePath, DeepLinkController.fileName, "");
            NavController.goDownloads();
        }
    }

    Controls.Button {
        Layout.fillWidth: true
        visible: DeepLinkController.fileIsBundle
        text: KI18n.i18n("Install Flatpak Bundle")
        highlighted: true
        onClicked: {
            TransactionsModel.installBundle(DeepLinkController.filePath, DeepLinkController.fileName);
            NavController.goDownloads();
        }
    }

    Controls.Button {
        Layout.fillWidth: true
        visible: !DeepLinkController.fileIsAppimage && !DeepLinkController.fileIsBundle
        text: KI18n.i18n("Install via Distrobox")
        highlighted: !DeepLinkController.fileHasFlatpakAlt
        onClicked: {
            TransactionsModel.install(DeepLinkController.filePath, DeepLinkController.filePkgName, "");
            NavController.goDownloads();
        }
    }

    Controls.Button {
        Layout.fillWidth: true
        visible: DeepLinkController.fileHasFlatpakAlt
        text: KI18n.i18n("Install Flatpak Instead")
        highlighted: true
        onClicked: NavController.openApp(DeepLinkController.fileFlatpakAltId, "")
    }

    Controls.Button {
        Layout.fillWidth: true
        text: KI18n.i18n("Cancel")
        onClicked: NavController.goHome()
    }
}
