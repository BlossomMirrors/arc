pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.blossomos.arc
import org.kde.ki18n

Kirigami.ScrollablePage {
    id: root

    Kirigami.ColumnView.fillWidth: true

    title: KI18n.i18n("Downloads")

    PackageListModel {
        id: downloadListModel
    }

    function load() { downloadListModel.loadUpdates() }

    function formatBytes(bytes) {
        if (bytes >= 1e9) return (bytes / 1e9).toFixed(1) + " GB";
        if (bytes >= 1e6) return (bytes / 1e6).toFixed(1) + " MB";
        if (bytes >= 1e3) return (bytes / 1e3).toFixed(0) + " KB";
        return bytes.toFixed(0) + " B";
    }

    function formatEta(secs) {
        if (secs >= 3600) return KI18n.i18n("%1 h %2 min remaining", Math.floor(secs / 3600), Math.floor((secs % 3600) / 60));
        if (secs >= 60) return KI18n.i18n("%1 min remaining", Math.floor(secs / 60));
        return KI18n.i18n("%1 s remaining", secs);
    }

    function txTypeLabel(txType) {
        if (txType === "remove") return KI18n.i18n("Removing");
        if (txType === "update") return KI18n.i18n("Updating");
        return KI18n.i18n("Installing");
    }

    readonly property bool contentReady: !downloadListModel.loading && TransactionsModel.historyLoaded

    LoadingOverlay {
        visible: !root.contentReady
    }

    Kirigami.PlaceholderMessage {
        anchors.centerIn: parent
        width: parent.width - Kirigami.Units.gridUnit * 4
        visible: root.contentReady
            && TransactionsModel.runningCount === 0
            && TransactionsModel.queuedCount === 0
            && TransactionsModel.doneCount === 0
            && updatesRepeater.count === 0
        icon.name: "checkmark"
        text: KI18n.i18n("Everything is up to date")
        explanation: KI18n.i18n("No updates available. Installs, removals and updates show up here.")

        helpfulAction: Kirigami.Action {
            icon.name: "view-refresh-symbolic"
            text: KI18n.i18n("Check for Updates")
            onTriggered: downloadListModel.checkForUpdates()
        }
    }

    ColumnLayout {
        width: root.width
        spacing: 0

        ColumnLayout {
            Layout.alignment: Qt.AlignHCenter
            Layout.fillWidth: true
            Layout.maximumWidth: Kirigami.Units.gridUnit * 44
            spacing: Kirigami.Units.largeSpacing

            Repeater {
                model: TransactionsModel

                delegate: Kirigami.AbstractCard {
                    id: heroDelegate

                    required property int index
                    required property string txId
                    required property string pkgId
                    required property string name
                    required property string iconUrl
                    required property real progress
                    required property string status
                    required property string txType
                    required property real bytesDone
                    required property real bytesTotal
                    required property real speedBps
                    required property int etaSecs

                    visible: status === "running"
                    Layout.fillWidth: true

                    showClickFeedback: true
                    onClicked: NavController.openApp(heroDelegate.pkgId, JSON.stringify({
                        name: heroDelegate.name,
                        iconUrl: heroDelegate.iconUrl
                    }))

                    contentItem: ColumnLayout {
                        spacing: Kirigami.Units.largeSpacing

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: Kirigami.Units.largeSpacing

                            AppIcon {
                                source: heroDelegate.iconUrl
                                Layout.preferredWidth: Kirigami.Units.iconSizes.huge
                                Layout.preferredHeight: Kirigami.Units.iconSizes.huge
                            }

                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: Kirigami.Units.smallSpacing / 2

                                Kirigami.Heading {
                                    Layout.fillWidth: true
                                    level: 1
                                    text: heroDelegate.name
                                    elide: Text.ElideRight
                                }

                                Controls.Label {
                                    text: root.txTypeLabel(heroDelegate.txType)
                                    opacity: 0.7
                                }
                            }

                            Kirigami.Heading {
                                visible: heroDelegate.progress > 0
                                level: 1
                                text: Math.round(heroDelegate.progress * 100) + "%"
                                color: Kirigami.Theme.highlightColor
                            }

                            Controls.Button {
                                Layout.alignment: Qt.AlignVCenter
                                icon.name: "process-stop-symbolic"
                                display: Controls.Button.IconOnly
                                text: KI18n.i18n("Cancel")
                                Controls.ToolTip.text: KI18n.i18n("Cancel")
                                Controls.ToolTip.visible: hovered
                                Controls.ToolTip.delay: Kirigami.Units.toolTipDelay
                                onClicked: TransactionsModel.cancel(heroDelegate.txId)
                            }
                        }


                        ItemProgressBar {
                            Layout.fillWidth: true
                            progress: heroDelegate.progress
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: Kirigami.Units.largeSpacing

                            Controls.Label {
                                visible: heroDelegate.bytesTotal > 0
                                text: KI18n.i18n("%1 of %2", root.formatBytes(heroDelegate.bytesDone), root.formatBytes(heroDelegate.bytesTotal))
                                opacity: 0.7
                            }

                            Controls.Label {
                                visible: heroDelegate.speedBps > 0
                                text: root.formatBytes(heroDelegate.speedBps) + "/s"
                                font.bold: true
                                color: Kirigami.Theme.highlightColor
                            }

                            Item { Layout.fillWidth: true }

                            Controls.Label {
                                visible: heroDelegate.etaSecs > 0
                                text: root.formatEta(heroDelegate.etaSecs)
                                opacity: 0.7
                            }
                        }
                    }
                }
            }

            Kirigami.Heading {
                Layout.fillWidth: true
                Layout.topMargin: Kirigami.Units.largeSpacing
                level: 2
                visible: TransactionsModel.queuedCount > 0
                text: KI18n.i18n("Up Next")
            }

            Repeater {
                model: TransactionsModel

                delegate: Kirigami.AbstractCard {
                    id: queuedDelegate

                    required property int index
                    required property string txId
                    required property string pkgId
                    required property string name
                    required property string iconUrl
                    required property string status
                    required property string txType

                    visible: status === "pending"
                    Layout.fillWidth: true

                    showClickFeedback: true
                    onClicked: NavController.openApp(queuedDelegate.pkgId, JSON.stringify({
                        name: queuedDelegate.name,
                        iconUrl: queuedDelegate.iconUrl
                    }))

                    contentItem: RowLayout {
                        spacing: Kirigami.Units.largeSpacing

                        AppIcon {
                            source: queuedDelegate.iconUrl
                            Layout.preferredWidth: Kirigami.Units.iconSizes.medium
                            Layout.preferredHeight: Kirigami.Units.iconSizes.medium
                        }

                        Kirigami.Heading {
                            Layout.fillWidth: true
                            level: 3
                            text: queuedDelegate.name
                            elide: Text.ElideRight
                        }

                        Controls.Label {
                            text: KI18n.i18n("Queued")
                            opacity: 0.7
                        }

                        Controls.Button {
                            icon.name: "process-stop-symbolic"
                            display: Controls.Button.IconOnly
                            text: KI18n.i18n("Cancel")
                            Controls.ToolTip.text: KI18n.i18n("Cancel")
                            Controls.ToolTip.visible: hovered
                            Controls.ToolTip.delay: Kirigami.Units.toolTipDelay
                            onClicked: TransactionsModel.cancel(queuedDelegate.txId)
                        }
                    }
                }
            }

            RowLayout {
                Layout.fillWidth: true
                Layout.topMargin: Kirigami.Units.largeSpacing
                visible: root.contentReady
                    && (updatesRepeater.count > 0
                        || TransactionsModel.runningCount > 0
                        || TransactionsModel.queuedCount > 0
                        || TransactionsModel.doneCount > 0)

                Kirigami.Heading {
                    Layout.fillWidth: true
                    level: 2
                    text: KI18n.i18n("Available Updates")
                }

                Controls.ToolButton {
                    icon.name: "view-refresh-symbolic"
                    text: KI18n.i18n("Check")
                    onClicked: downloadListModel.checkForUpdates()
                }

                Controls.Button {
                    visible: updatesRepeater.count > 0
                    icon.name: "update-none-symbolic"
                    text: KI18n.i18n("Update All")
                    highlighted: true
                    onClicked: TransactionsModel.updateAll()
                }
            }

            Controls.Label {
                Layout.fillWidth: true
                visible: root.contentReady && updatesRepeater.count === 0
                    && (TransactionsModel.runningCount > 0
                        || TransactionsModel.queuedCount > 0
                        || TransactionsModel.doneCount > 0)
                text: KI18n.i18n("Everything is up to date.")
                opacity: 0.7
            }

            Repeater {
                id: updatesRepeater
                model: downloadListModel

                delegate: Kirigami.AbstractCard {
                    id: updateDelegate

                    required property int index
                    required property string pkgId
                    required property string name
                    required property string version
                    required property string iconUrl
                    required property bool busy

                    // hide rows that already turned into a running transaction
                    visible: !busy
                    Layout.fillWidth: true

                    showClickFeedback: true
                    onClicked: NavController.openApp(updateDelegate.pkgId, JSON.stringify({
                        name: updateDelegate.name,
                        iconUrl: updateDelegate.iconUrl,
                        installed: true
                    }))

                    HoverHandler {
                        id: updateRowHover
                    }

                    Timer {
                        interval: 200
                        running: updateRowHover.hovered
                        onTriggered: DetailController.prefetch(updateDelegate.pkgId, Math.round(Kirigami.Units.gridUnit * 14 * 16 / 9 * 2))
                    }

                    contentItem: RowLayout {
                        spacing: Kirigami.Units.largeSpacing

                        AppIcon {
                            source: updateDelegate.iconUrl
                            Layout.preferredWidth: Kirigami.Units.iconSizes.large
                            Layout.preferredHeight: Kirigami.Units.iconSizes.large
                        }

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: Kirigami.Units.smallSpacing / 2

                            Kirigami.Heading {
                                Layout.fillWidth: true
                                level: 3
                                text: updateDelegate.name
                                elide: Text.ElideRight
                            }

                            Controls.Label {
                                Layout.fillWidth: true
                                visible: updateDelegate.version.length > 0
                                text: updateDelegate.version
                                opacity: 0.7
                                elide: Text.ElideRight
                            }
                        }

                        ItemButtons {
                            Layout.alignment: Qt.AlignVCenter
                            pkgId: updateDelegate.pkgId
                            name: updateDelegate.name
                            iconUrl: updateDelegate.iconUrl
                            mode: "update"
                        }
                    }
                }
            }

            RowLayout {
                Layout.fillWidth: true
                Layout.topMargin: Kirigami.Units.largeSpacing
                visible: TransactionsModel.doneCount > 0

                Kirigami.Heading {
                    Layout.fillWidth: true
                    level: 2
                    text: KI18n.i18n("Completed")
                }

                Controls.ToolButton {
                    icon.name: "edit-clear-history-symbolic"
                    text: KI18n.i18n("Clear")
                    onClicked: TransactionsModel.clearFinished()
                }
            }

            Repeater {
                model: TransactionsModel

                delegate: Kirigami.AbstractCard {
                    id: doneDelegate

                    required property int index
                    required property string pkgId
                    required property string name
                    required property string iconUrl
                    required property string status
                    required property string txType
                    required property string error
                    required property real finishedAt
                    required property bool automatic

                    readonly property bool failed: status === "failed"

                    readonly property string actionText: {
                        const removal = txType === "remove" || txType === "remove_with_data";
                        if (failed) {
                            return removal ? KI18n.i18n("Removal failed")
                                : txType === "update" ? KI18n.i18n("Update failed")
                                : KI18n.i18n("Installation failed");
                        }
                        if (removal) {
                            return KI18n.i18n("Removed");
                        }
                        if (txType === "update") {
                            return automatic ? KI18n.i18n("Updated in the background") : KI18n.i18n("Updated");
                        }
                        return KI18n.i18n("Installed");
                    }

                    readonly property string finishedText: finishedAt > 0
                        ? new Date(finishedAt * 1000).toLocaleString(Qt.locale(), Locale.ShortFormat)
                        : ""

                    visible: status === "completed" || failed
                    Layout.fillWidth: true
                    opacity: failed ? 1 : 0.7

                    showClickFeedback: true
                    onClicked: NavController.openApp(doneDelegate.pkgId, JSON.stringify({
                        name: doneDelegate.name,
                        iconUrl: doneDelegate.iconUrl
                    }))

                    contentItem: RowLayout {
                        spacing: Kirigami.Units.largeSpacing

                        AppIcon {
                            source: doneDelegate.iconUrl
                            Layout.preferredWidth: Kirigami.Units.iconSizes.medium
                            Layout.preferredHeight: Kirigami.Units.iconSizes.medium
                        }

                        ColumnLayout {
                            spacing: 0

                            Kirigami.Heading {
                                level: 3
                                Layout.fillWidth: true
                                text: doneDelegate.name
                                elide: Text.ElideRight
                            }

                            Controls.Label {
                                Layout.fillWidth: true
                                text: doneDelegate.finishedText.length > 0
                                    ? KI18n.i18nc("what happened to an app and when", "%1 · %2", doneDelegate.actionText, doneDelegate.finishedText)
                                    : doneDelegate.actionText
                                elide: Text.ElideRight
                                opacity: 0.7
                                font.pointSize: Kirigami.Theme.smallFont.pointSize
                            }
                        }

                        TextEdit {
                            Layout.fillWidth: true
                            visible: doneDelegate.failed && doneDelegate.error.length > 0
                            text: doneDelegate.error
                            color: Kirigami.Theme.negativeTextColor
                            font: Kirigami.Theme.defaultFont
                            wrapMode: Text.Wrap
                            readOnly: true
                            selectByMouse: true
                            selectionColor: Kirigami.Theme.highlightColor
                            persistentSelection: true
                        }

                        Item { Layout.fillWidth: true }

                        Kirigami.Icon {
                            source: doneDelegate.failed ? "dialog-error-symbolic" : "checkmark-symbolic"
                            color: doneDelegate.failed ? Kirigami.Theme.negativeTextColor : Kirigami.Theme.positiveTextColor
                            Layout.preferredWidth: Kirigami.Units.iconSizes.smallMedium
                            Layout.preferredHeight: Kirigami.Units.iconSizes.smallMedium
                        }
                    }
                }
            }

            Item { Layout.preferredHeight: Kirigami.Units.gridUnit * 2 }
        }
    }
}
