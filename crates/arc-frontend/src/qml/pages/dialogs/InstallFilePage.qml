import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.blossomos.arc

DialogPage {
    id: root

    title: i18n("Install File")
    dialogIcon: "package-x-generic"
    dialogTitle: DeepLinkController.fileName
    dialogDescription: DeepLinkController.fileHasFlatpakAlt
        ? i18n("A Flatpak version of this app is also available: %1", DeepLinkController.fileFlatpakAltName)
        : ""

    Controls.Button {
        Layout.fillWidth: true
        visible: DeepLinkController.fileIsAppimage
        text: i18n("Install AppImage")
        highlighted: true
        onClicked: {
            TransactionsModel.install(DeepLinkController.filePath, DeepLinkController.fileName, "");
            applicationWindow().goDownloads();
        }
    }

    Controls.Button {
        Layout.fillWidth: true
        visible: DeepLinkController.fileIsBundle
        text: i18n("Install Flatpak Bundle")
        highlighted: true
        onClicked: {
            TransactionsModel.installBundle(DeepLinkController.filePath, DeepLinkController.fileName);
            applicationWindow().goDownloads();
        }
    }

    Controls.Button {
        Layout.fillWidth: true
        visible: !DeepLinkController.fileIsAppimage && !DeepLinkController.fileIsBundle
        text: i18n("Install via Distrobox")
        highlighted: !DeepLinkController.fileHasFlatpakAlt
        onClicked: {
            TransactionsModel.install(DeepLinkController.filePath, DeepLinkController.filePkgName, "");
            applicationWindow().goDownloads();
        }
    }

    Controls.Button {
        Layout.fillWidth: true
        visible: DeepLinkController.fileHasFlatpakAlt
        text: i18n("Install Flatpak Instead")
        highlighted: true
        onClicked: applicationWindow().openApp(DeepLinkController.fileFlatpakAltId)
    }

    Controls.Button {
        Layout.fillWidth: true
        text: i18n("Cancel")
        onClicked: applicationWindow().goHome()
    }
}
