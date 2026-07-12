pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls as Controls
import org.kde.kirigami as Kirigami
import org.kde.kirigamiaddons.formcard as FormCard
import org.blossomos.arc

FormCard.FormCardPage {
    id: root

    Kirigami.ColumnView.fillWidth: true

    title: i18n("Settings")

    // onValueChanged already fires during construction before the bindings apply
    property bool ready: false

    Component.onCompleted: {
        SettingsController.load();
        RemotesModel.load();
        ready = true;
    }

    FormCard.FormHeader {
        title: i18n("General")
    }

    FormCard.FormCard {
        FormCard.FormSwitchDelegate {
            id: autoUpdatesDelegate
            text: i18n("Automatic updates")
            description: i18n("Update installed apps in the background")
            checked: SettingsController.autoUpdates
            onToggled: {
                SettingsController.autoUpdates = checked;
                SettingsController.save();
            }
        }

        FormCard.FormDelegateSeparator {
            above: autoUpdatesDelegate
            below: securityWarningsDelegate
        }

        FormCard.FormSwitchDelegate {
            id: securityWarningsDelegate
            text: i18n("Security warnings")
            description: i18n("Warn before installing third-party software or adding repositories")
            checked: SettingsController.showSecurityWarnings
            onToggled: {
                SettingsController.showSecurityWarnings = checked;
                SettingsController.save();
            }
        }

        FormCard.FormDelegateSeparator {
            above: securityWarningsDelegate
            below: concurrentDelegate
        }

        FormCard.FormSpinBoxDelegate {
            id: concurrentDelegate
            label: i18n("Concurrent downloads")
            from: 1
            to: 16
            value: SettingsController.concurrentDownloads
            onValueChanged: {
                if (root.ready && value !== SettingsController.concurrentDownloads) {
                    SettingsController.concurrentDownloads = value;
                    SettingsController.save();
                }
            }
        }
    }

    FormCard.FormHeader {
        title: i18n("Repositories")
    }

    FormCard.FormCard {
        FormCard.FormTextDelegate {
            visible: RemotesModel.loading
            text: i18n("Loading…")
        }

        Repeater {
            model: RemotesModel

            delegate: FormCard.FormTextDelegate {
                id: remoteDelegate

                required property string name
                required property string url
                required property bool isProtected

                text: name
                description: url

                trailing: remoteDelegate.isProtected ? protectedMarker : removeButton

                Kirigami.Icon {
                    id: protectedMarker
                    visible: false
                    source: "object-locked-symbolic"
                    implicitWidth: Kirigami.Units.iconSizes.smallMedium
                    implicitHeight: Kirigami.Units.iconSizes.smallMedium

                    Controls.ToolTip.text: i18n("Protected system repository")
                    Controls.ToolTip.visible: protectedHover.hovered
                    Controls.ToolTip.delay: Kirigami.Units.toolTipDelay

                    HoverHandler {
                        id: protectedHover
                    }
                }

                Controls.ToolButton {
                    id: removeButton
                    visible: false
                    icon.name: "edit-delete-symbolic"
                    onClicked: RemotesModel.remove(remoteDelegate.name)

                    Controls.ToolTip.text: i18n("Remove repository")
                    Controls.ToolTip.visible: hovered
                    Controls.ToolTip.delay: Kirigami.Units.toolTipDelay
                }
            }
        }

        FormCard.FormDelegateSeparator {}

        FormCard.FormButtonDelegate {
            icon.name: "list-add-symbolic"
            text: i18n("Add Repository…")
            onClicked: addRepoDialog.open()
        }
    }

    FormCard.FormHeader {
        title: i18n("Danger Zone")
    }

    FormCard.FormCard {
        FormCard.FormButtonDelegate {
            id: forceUpdateDelegate
            icon.name: "update-none-symbolic"
            text: i18n("Force update")
            description: i18n("Runs flatpak update directly, bypassing the daemon")
            onClicked: SettingsController.forceUpdate()
        }

        FormCard.FormDelegateSeparator {
            above: forceUpdateDelegate
            below: restartDelegate
        }

        FormCard.FormButtonDelegate {
            id: restartDelegate
            icon.name: "system-reboot-symbolic"
            text: i18n("Restart daemon")
            description: i18n("Kills the running arc-daemon and starts a fresh one")
            onClicked: SettingsController.restartDaemon()
        }
    }

    FormCard.FormCardDialog {
        id: addRepoDialog

        parent: root.Controls.Overlay.overlay
        implicitWidth: Kirigami.Units.gridUnit * 24
        title: i18n("Add Repository")
        standardButtons: Controls.Dialog.Cancel | Controls.Dialog.Ok

        onAccepted: {
            RemotesModel.add(repoNameField.text, repoUrlField.text);
            repoNameField.text = "";
            repoUrlField.text = "";
        }
        onRejected: {
            repoNameField.text = "";
            repoUrlField.text = "";
        }

        FormCard.FormTextFieldDelegate {
            id: repoNameField
            label: i18n("Name")
            placeholderText: "my-repo"
        }

        FormCard.FormTextFieldDelegate {
            id: repoUrlField
            label: i18n("URL")
            placeholderText: "https://example.com/repo"
        }
    }
}
