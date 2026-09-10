pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls as Controls
import org.kde.kirigami as Kirigami
import org.kde.kirigamiaddons.formcard as FormCard
import org.blossomos.arc
import org.kde.ki18n

FormCard.FormCardPage {
    id: root

    Kirigami.ColumnView.fillWidth: true

    title: KI18n.i18n("Settings")

    // onValueChanged already fires during construction before the bindings apply
    property bool ready: false

    signal notificationRequested(string message)

    Component.onCompleted: {
        SettingsController.load();
        RemotesModel.load();
        ready = true;
    }

    Connections {
        target: RemotesModel
        function onActionFailed(message) { root.notificationRequested(message); }
        function onActionSucceeded(message) { root.notificationRequested(message); }
    }

    Connections {
        target: SettingsController
        function onActionFailed(message) { root.notificationRequested(message); }
        function onActionSucceeded(message) { root.notificationRequested(message); }
    }

    FormCard.FormHeader {
        title: KI18n.i18n("General")
    }

    FormCard.FormCard {
        FormCard.FormSwitchDelegate {
            id: autoUpdatesDelegate
            text: KI18n.i18n("Automatic updates")
            description: KI18n.i18n("Update installed apps in the background")
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
            text: KI18n.i18n("Security warnings")
            description: KI18n.i18n("Warn before installing third-party software or adding repositories")
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
            label: KI18n.i18n("Concurrent downloads")
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
        title: KI18n.i18n("Repositories")
    }

    FormCard.FormCard {
        FormCard.FormTextDelegate {
            visible: RemotesModel.loading
            text: KI18n.i18n("Loading...")
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

                trailing: Controls.ToolButton {
                    icon.name: remoteDelegate.isProtected ? "object-locked-symbolic" : "delete"
                    enabled: !remoteDelegate.isProtected
                    onClicked: {
                        removeRepoDialog.repoName = remoteDelegate.name;
                        removeRepoDialog.open();
                    }
                    Controls.ToolTip.text: remoteDelegate.isProtected
                        ? KI18n.i18n("Protected system repository")
                        : KI18n.i18n("Remove repository")
                    Controls.ToolTip.visible: hovered
                    Controls.ToolTip.delay: Kirigami.Units.toolTipDelay
                }
            }
        }

        FormCard.FormDelegateSeparator {}

        FormCard.FormButtonDelegate {
            icon.name: "list-add-symbolic"
            text: KI18n.i18n("Add Repository...")
            onClicked: addRepoDialog.open()
        }
    }

    FormCard.FormHeader {
        title: KI18n.i18n("Danger Zone")
    }

    FormCard.FormCard {
        FormCard.FormButtonDelegate {
            id: forceUpdateDelegate
            icon.name: "update-none-symbolic"
            icon.color: Kirigami.Theme.negativeTextColor
            text: KI18n.i18n("Force update")
            description: KI18n.i18n("Runs flatpak update directly, bypassing the daemon")
            onClicked: forceUpdateDialog.open()
        }

        FormCard.FormDelegateSeparator {
            above: forceUpdateDelegate
            below: restartDelegate
        }

        FormCard.FormButtonDelegate {
            id: restartDelegate
            icon.name: "system-reboot-symbolic"
            icon.color: Kirigami.Theme.negativeTextColor
            text: KI18n.i18n("Restart daemon")
            description: KI18n.i18n("Kills the running arc-daemon and starts a fresh one")
            onClicked: restartDaemonDialog.open()
        }

        FormCard.FormDelegateSeparator {
            above: restartDelegate
            below: clearIconCacheDelegate
        }

        FormCard.FormButtonDelegate {
            id: clearIconCacheDelegate
            icon.name: "edit-clear-symbolic"
            icon.color: Kirigami.Theme.negativeTextColor
            text: KI18n.i18n("Clear icon cache")
            description: KI18n.i18n("Reprocesses app icons the next time they're shown")
            onClicked: SettingsController.clearIconCache()
        }
    }

    FormCard.FormCardDialog {
        id: addRepoDialog

        parent: root.Controls.Overlay.overlay
        implicitWidth: Kirigami.Units.gridUnit * 24
        title: KI18n.i18n("Add Repository")
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
            label: KI18n.i18n("Name")
            placeholderText: "my-repo"
        }

        FormCard.FormTextFieldDelegate {
            id: repoUrlField
            label: KI18n.i18n("URL")
            placeholderText: "https://example.com/repo"
        }
    }

    Kirigami.PromptDialog {
        id: removeRepoDialog

        property string repoName: ""

        title: KI18n.i18n("Remove %1?", removeRepoDialog.repoName)
        subtitle: KI18n.i18n("Apps installed from this repository keep working, but it will no longer offer updates or new installs.")
        standardButtons: Kirigami.Dialog.Cancel
        showCloseButton: false

        customFooterActions: [
            Kirigami.Action {
                text: KI18n.i18n("Remove")
                icon.name: "delete"
                onTriggered: {
                    RemotesModel.remove(removeRepoDialog.repoName);
                    removeRepoDialog.close();
                }
            }
        ]
    }

    Kirigami.PromptDialog {
        id: forceUpdateDialog

        title: KI18n.i18n("Force update now?")
        subtitle: KI18n.i18n("Runs flatpak update directly, bypassing the daemon and its progress tracking.")
        standardButtons: Kirigami.Dialog.Cancel
        showCloseButton: false

        customFooterActions: [
            Kirigami.Action {
                text: KI18n.i18n("Update")
                icon.name: "update-none-symbolic"
                onTriggered: {
                    SettingsController.forceUpdate();
                    forceUpdateDialog.close();
                }
            }
        ]
    }

    Kirigami.PromptDialog {
        id: restartDaemonDialog

        title: KI18n.i18n("Restart the daemon?")
        subtitle: KI18n.i18n("Cancels every running install, remove and update right now.")
        standardButtons: Kirigami.Dialog.Cancel
        showCloseButton: false

        customFooterActions: [
            Kirigami.Action {
                text: KI18n.i18n("Restart")
                icon.name: "system-reboot-symbolic"
                onTriggered: {
                    SettingsController.restartDaemon();
                    restartDaemonDialog.close();
                }
            }
        ]
    }
}
