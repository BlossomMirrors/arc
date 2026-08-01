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

    Component.onCompleted: PackageListModel.loadUpdates()

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

    ConveyorLoader {
        anchors.centerIn: parent
        visible: PackageListModel.loading && transactionsRepeater.count === 0
    }

    Kirigami.PlaceholderMessage {
        anchors.centerIn: parent
        width: parent.width - Kirigami.Units.gridUnit * 4
        visible: !PackageListModel.loading && transactionsRepeater.count === 0 && updatesRepeater.count === 0
        icon.name: "download"
        text: i18n("Nothing to download")
        explanation: i18n("Installs, removals and updates show up here.")
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
                id: transactionsRepeater
                model: TransactionsModel

                delegate: Kirigami.AbstractCard {
                    id: delegate

                    required property int index
                    required property string txId
                    required property string pkgId
                    required property string name
                    required property string iconUrl
                    required property real progress
                    required property string status
                    required property string txType
                    required property string error
                    required property real bytesDone
                    required property real bytesTotal
                    required property real speedBps
                    required property int etaSecs

                    readonly property bool running: status === "running"
                    readonly property bool queued: status === "pending"

                    Layout.fillWidth: true

                    contentItem: ColumnLayout {
                        spacing: Kirigami.Units.smallSpacing

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: Kirigami.Units.largeSpacing

                            Kirigami.Icon {
                                source: delegate.iconUrl.length > 0 ? delegate.iconUrl : "application-x-executable"
                                Layout.preferredWidth: delegate.running ? Kirigami.Units.iconSizes.huge : Kirigami.Units.iconSizes.large
                                Layout.preferredHeight: Layout.preferredWidth
                            }

                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: Kirigami.Units.smallSpacing / 2

                                Kirigami.Heading {
                                    Layout.fillWidth: true
                                    level: delegate.running ? 2 : 3
                                    text: delegate.name
                                    elide: Text.ElideRight
                                }

                                Controls.Label {
                                    Layout.fillWidth: true
                                    text: delegate.status === "failed"
                                        ? (delegate.error.length > 0 ? delegate.error : i18n("Failed"))
                                        : delegate.queued ? i18n("Queued")
                                        : delegate.status === "completed" ? i18n("Done")
                                        : delegate.txType === "remove" ? i18n("Removing")
                                        : delegate.txType === "update" ? i18n("Updating")
                                        : i18n("Installing")
                                    color: delegate.status === "failed" ? Kirigami.Theme.negativeTextColor : Kirigami.Theme.textColor
                                    opacity: delegate.status === "failed" ? 1 : 0.7
                                    elide: Text.ElideRight
                                }
                            }

                            Controls.Label {
                                visible: delegate.running && delegate.speedBps > 0
                                text: root.formatBytes(delegate.speedBps) + "/s"
                                font.bold: true
                                color: Kirigami.Theme.highlightColor
                            }

                            Controls.Button {
                                Layout.alignment: Qt.AlignVCenter
                                visible: delegate.running || delegate.queued
                                icon.name: "process-stop-symbolic"
                                display: Controls.Button.IconOnly
                                text: i18n("Cancel")
                                Controls.ToolTip.text: i18n("Cancel")
                                Controls.ToolTip.visible: hovered
                                Controls.ToolTip.delay: Kirigami.Units.toolTipDelay
                                onClicked: TransactionsModel.cancel(delegate.txId)
                            }
                        }

                        ItemProgressBar {
                            Layout.fillWidth: true
                            visible: delegate.running || delegate.queued
                            progress: delegate.queued ? 0 : delegate.progress
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            visible: delegate.running && delegate.bytesTotal > 0
                            spacing: Kirigami.Units.largeSpacing

                            Controls.Label {
                                text: i18n("%1 of %2", root.formatBytes(delegate.bytesDone), root.formatBytes(delegate.bytesTotal))
                                opacity: 0.7
                                font.pointSize: Kirigami.Theme.smallFont.pointSize
                            }

                            Item { Layout.fillWidth: true }

                            Controls.Label {
                                visible: delegate.etaSecs > 0
                                text: root.formatEta(delegate.etaSecs)
                                opacity: 0.7
                                font.pointSize: Kirigami.Theme.smallFont.pointSize
                            }
                        }
                    }
                }
            }

            RowLayout {
                Layout.fillWidth: true
                Layout.topMargin: transactionsRepeater.count > 0 ? Kirigami.Units.gridUnit : 0
                visible: updatesRepeater.count > 0

                Kirigami.Heading {
                    Layout.fillWidth: true
                    level: 2
                    text: i18n("Available Updates")
                }

                Controls.Button {
                    icon.name: "update-none-symbolic"
                    text: i18n("Update All")
                    highlighted: true
                    onClicked: TransactionsModel.updateAll()
                }
            }

            Repeater {
                id: updatesRepeater
                model: PackageListModel

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

                    contentItem: RowLayout {
                        spacing: Kirigami.Units.largeSpacing

                        Kirigami.Icon {
                            source: updateDelegate.iconUrl.length > 0 ? updateDelegate.iconUrl : "application-x-executable"
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

            Item { Layout.preferredHeight: Kirigami.Units.gridUnit * 2 }
        }
    }
}
