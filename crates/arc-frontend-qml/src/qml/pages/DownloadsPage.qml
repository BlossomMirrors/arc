pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.blossomos.arc

Kirigami.ScrollablePage {
    id: root

    Kirigami.ColumnView.fillWidth: true

    title: i18n("Downloads")

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
        if (secs >= 3600) return i18n("%1 h %2 min remaining", Math.floor(secs / 3600), Math.floor((secs % 3600) / 60));
        if (secs >= 60) return i18n("%1 min remaining", Math.floor(secs / 60));
        return i18n("%1 s remaining", secs);
    }

    function txTypeLabel(txType) {
        if (txType === "remove") return i18n("Removing");
        if (txType === "update") return i18n("Updating");
        return i18n("Installing");
    }

    Kirigami.PlaceholderMessage {
        anchors.centerIn: parent
        width: parent.width - Kirigami.Units.gridUnit * 4
        visible: !downloadListModel.loading
            && TransactionsModel.runningCount === 0
            && TransactionsModel.queuedCount === 0
            && TransactionsModel.doneCount === 0
            && updatesRepeater.count === 0
        icon.name: "checkmark"
        text: i18n("Everything is up to date")
        explanation: i18n("No updates available. Installs, removals and updates show up here.")

        helpfulAction: Kirigami.Action {
            icon.name: "view-refresh-symbolic"
            text: i18n("Check for Updates")
            onTriggered: downloadListModel.loadUpdates()
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

                    contentItem: ColumnLayout {
                        spacing: Kirigami.Units.largeSpacing

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: Kirigami.Units.largeSpacing

                            Kirigami.Icon {
                                source: heroDelegate.iconUrl.length > 0 ? heroDelegate.iconUrl : "application-x-executable"
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
                                text: i18n("Cancel")
                                Controls.ToolTip.text: i18n("Cancel")
                                Controls.ToolTip.visible: hovered
                                Controls.ToolTip.delay: Kirigami.Units.toolTipDelay
                                onClicked: TransactionsModel.cancel(heroDelegate.txId)
                            }
                        }

                        Rectangle {
                            Layout.fillWidth: true
                            Layout.preferredHeight: Kirigami.Units.gridUnit * 0.7
                            visible: heroDelegate.progress > 0
                            radius: height / 2
                            color: Kirigami.Theme.alternateBackgroundColor

                            Rectangle {
                                width: parent.width * Math.min(1, heroDelegate.progress)
                                height: parent.height
                                radius: parent.radius
                                color: Kirigami.Theme.highlightColor

                                Behavior on width {
                                    NumberAnimation { duration: 250; easing.type: Easing.OutQuad }
                                }
                            }
                        }

                        ItemProgressBar {
                            Layout.fillWidth: true
                            visible: heroDelegate.progress <= 0
                            progress: 0
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: Kirigami.Units.largeSpacing

                            Controls.Label {
                                visible: heroDelegate.bytesTotal > 0
                                text: i18n("%1 of %2", root.formatBytes(heroDelegate.bytesDone), root.formatBytes(heroDelegate.bytesTotal))
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
                text: i18n("Up Next")
            }

            Repeater {
                model: TransactionsModel

                delegate: Kirigami.AbstractCard {
                    id: queuedDelegate

                    required property int index
                    required property string txId
                    required property string name
                    required property string iconUrl
                    required property string status
                    required property string txType

                    visible: status === "pending"
                    Layout.fillWidth: true

                    contentItem: RowLayout {
                        spacing: Kirigami.Units.largeSpacing

                        Kirigami.Icon {
                            source: queuedDelegate.iconUrl.length > 0 ? queuedDelegate.iconUrl : "application-x-executable"
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
                            text: i18n("Queued")
                            opacity: 0.7
                        }

                        Controls.Button {
                            icon.name: "process-stop-symbolic"
                            display: Controls.Button.IconOnly
                            text: i18n("Cancel")
                            Controls.ToolTip.text: i18n("Cancel")
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
                visible: !downloadListModel.loading
                    && (updatesRepeater.count > 0
                        || TransactionsModel.runningCount > 0
                        || TransactionsModel.queuedCount > 0
                        || TransactionsModel.doneCount > 0)

                Kirigami.Heading {
                    Layout.fillWidth: true
                    level: 2
                    text: i18n("Available Updates")
                }

                Controls.ToolButton {
                    icon.name: "view-refresh-symbolic"
                    text: i18n("Check")
                    onClicked: downloadListModel.loadUpdates()
                }

                Controls.Button {
                    visible: updatesRepeater.count > 0
                    icon.name: "update-none-symbolic"
                    text: i18n("Update All")
                    highlighted: true
                    onClicked: TransactionsModel.updateAll()
                }
            }

            Controls.Label {
                Layout.fillWidth: true
                visible: !downloadListModel.loading && updatesRepeater.count === 0
                    && (TransactionsModel.runningCount > 0
                        || TransactionsModel.queuedCount > 0
                        || TransactionsModel.doneCount > 0)
                text: i18n("Everything is up to date.")
                opacity: 0.7
            }

            ColumnLayout {
                Layout.fillWidth: true
                Layout.topMargin: Kirigami.Units.largeSpacing
                visible: downloadListModel.loading
                spacing: Kirigami.Units.largeSpacing

                Repeater {
                    model: 3

                    delegate: Kirigami.AbstractCard {
                        Layout.fillWidth: true

                        contentItem: RowLayout {
                            spacing: Kirigami.Units.largeSpacing

                            SkeletonBlock {
                                Layout.preferredWidth: Kirigami.Units.iconSizes.large
                                Layout.preferredHeight: Kirigami.Units.iconSizes.large
                            }

                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: Kirigami.Units.smallSpacing

                                SkeletonBlock { Layout.preferredWidth: 160; Layout.preferredHeight: 16 }
                                SkeletonBlock { Layout.preferredWidth: 80; Layout.preferredHeight: 12 }
                            }

                            SkeletonBlock { Layout.preferredWidth: 90; Layout.preferredHeight: 32 }
                        }
                    }
                }
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
                    onClicked: applicationWindow().openApp(updateDelegate.pkgId, {
                        name: updateDelegate.name,
                        iconUrl: updateDelegate.iconUrl,
                        installed: true
                    })

                    HoverHandler {
                        id: updateRowHover
                    }

                    Timer {
                        interval: 200
                        running: updateRowHover.hovered
                        onTriggered: DetailController.prefetch(updateDelegate.pkgId)
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
                    text: i18n("Completed")
                }

                Controls.ToolButton {
                    icon.name: "edit-clear-history-symbolic"
                    text: i18n("Clear")
                    onClicked: TransactionsModel.clearFinished()
                }
            }

            Repeater {
                model: TransactionsModel

                delegate: Kirigami.AbstractCard {
                    id: doneDelegate

                    required property int index
                    required property string name
                    required property string iconUrl
                    required property string status
                    required property string txType
                    required property string error

                    readonly property bool failed: status === "failed"

                    visible: status === "completed" || failed
                    Layout.fillWidth: true
                    opacity: failed ? 1 : 0.7

                    contentItem: RowLayout {
                        spacing: Kirigami.Units.largeSpacing

                        Kirigami.Icon {
                            source: doneDelegate.iconUrl.length > 0 ? doneDelegate.iconUrl : "application-x-executable"
                            Layout.preferredWidth: Kirigami.Units.iconSizes.medium
                            Layout.preferredHeight: Kirigami.Units.iconSizes.medium
                        }

                        Kirigami.Heading {
                            level: 3
                            text: doneDelegate.name
                            elide: Text.ElideRight
                        }

                        Controls.Label {
                            Layout.fillWidth: true
                            text: doneDelegate.failed
                                ? (doneDelegate.error.length > 0 ? doneDelegate.error : i18n("Failed"))
                                : ""
                            color: Kirigami.Theme.negativeTextColor
                            elide: Text.ElideRight
                        }

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
