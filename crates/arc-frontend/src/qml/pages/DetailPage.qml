pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Effects
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.blossomos.arc
import org.kde.ki18n

Kirigami.ScrollablePage {
    id: root

    Kirigami.ColumnView.fillWidth: true

    property string pkgId: ""
    property var failedShots: ({})
    property bool descriptionExpanded: false

    function openEntry(newPkgId, seed) {
        root.pkgId = newPkgId;
        root.failedShots = {};
        root.descriptionExpanded = false;
        if (seed) {
            DetailController.loadWithSeed(newPkgId, seed.name ?? "", seed.summary ?? "", seed.iconUrl ?? "", seed.installed ?? false);
        } else {
            DetailController.load(newPkgId);
        }
        LeftoverDataController.check();
    }

    readonly property var leftoverEntries: JSON.parse(LeftoverDataController.leftoverJson || "[]")
    readonly property var leftoverEntryForThisApp: root.leftoverEntries.find(e => e.id === DetailController.id) ?? null

    readonly property var extensions: JSON.parse(DetailController.extensionsJson.length > 0 ? DetailController.extensionsJson : "[]")

    readonly property var busyMap: JSON.parse(TransactionsModel.busyPackagesJson || "{}")
    readonly property bool liveBusy: root.busyMap[DetailController.id] !== undefined
    readonly property real liveProgress: root.busyMap[DetailController.id] ?? 0

    title: DetailController.name

    ColumnLayout {
        visible: DetailController.loading
        width: root.width
        spacing: 0

        ColumnLayout {
            Layout.alignment: Qt.AlignHCenter
            Layout.fillWidth: true
            Layout.maximumWidth: Kirigami.Units.gridUnit * 48
            spacing: Kirigami.Units.largeSpacing

            RowLayout {
                Layout.fillWidth: true
                spacing: Kirigami.Units.largeSpacing

                SkeletonBlock {
                    Layout.preferredWidth: 96
                    Layout.preferredHeight: 96
                    Layout.alignment: Qt.AlignTop
                    radius: 48
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: Kirigami.Units.smallSpacing

                    SkeletonBlock { Layout.preferredWidth: 220; Layout.preferredHeight: 28 }
                    SkeletonBlock { Layout.preferredWidth: 140; Layout.preferredHeight: 16 }

                    RowLayout {
                        spacing: Kirigami.Units.smallSpacing
                        SkeletonBlock { Layout.preferredWidth: 60; Layout.preferredHeight: 22; radius: 11 }
                        SkeletonBlock { Layout.preferredWidth: 80; Layout.preferredHeight: 22; radius: 11 }
                    }
                }

                SkeletonBlock { Layout.preferredWidth: 110; Layout.preferredHeight: 36 }
            }

            Kirigami.Separator { Layout.fillWidth: true }

            SkeletonBlock { Layout.fillWidth: true; Layout.preferredHeight: 16 }
            SkeletonBlock { Layout.fillWidth: true; Layout.preferredHeight: 16 }
            SkeletonBlock { Layout.preferredWidth: parent.width * 0.6; Layout.preferredHeight: 16 }

            SkeletonBlock { Layout.fillWidth: true; Layout.preferredHeight: Kirigami.Units.gridUnit * 14 }

            Item { Layout.preferredHeight: Kirigami.Units.gridUnit * 2 }
        }
    }

    ColumnLayout {
        visible: !DetailController.loading
        width: root.width
        spacing: 0

        ColumnLayout {
            Layout.alignment: Qt.AlignHCenter
            Layout.fillWidth: true
            Layout.maximumWidth: Kirigami.Units.gridUnit * 48
            spacing: Kirigami.Units.largeSpacing

            AppHeader {
                Layout.fillWidth: true
                hasExtensions: root.extensions.length > 0

                onRemoveClicked: TransactionsModel.removePackage(DetailController.id, DetailController.name, DetailController.iconUrl)
                onAddonsClicked: extensionsSheet.open()
            }

            Kirigami.InlineMessage {
                Layout.fillWidth: true
                visible: root.leftoverEntryForThisApp !== null
                type: Kirigami.MessageType.Information
                text: KI18n.i18n("This app left data behind on your system.")
                showCloseButton: false

                actions: [
                    Kirigami.Action {
                        text: KI18n.i18n("Delete Data")
                        icon.name: "delete"
                        enabled: !LeftoverDataController.busy
                        onTriggered: LeftoverDataController.cleanupOne(DetailController.id)
                    }
                ]
            }

            ItemProgressBar {
                Layout.fillWidth: true
                visible: root.liveBusy
                progress: root.liveProgress
            }

            Kirigami.Separator {
                Layout.fillWidth: true
            }

            Controls.Label {
                Layout.fillWidth: true
                visible: DetailController.summary.length > 0
                text: DetailController.summary
                wrapMode: Text.WordWrap
                font.pointSize: Kirigami.Theme.defaultFont.pointSize + 2
            }

            CardCarousel {
                id: screenshotStrip
                Layout.fillWidth: true
                Layout.preferredHeight: Kirigami.Units.gridUnit * 14
                visible: DetailController.screenshots.length > 0
                model: DetailController.screenshots
                cardWidth: screenshotStrip.height * 16 / 9

                Item {
                    id: sharedShotMask
                    width: screenshotStrip.height * 16 / 9
                    height: screenshotStrip.height
                    visible: false
                    layer.enabled: true
                    Rectangle {
                        anchors.fill: parent
                        radius: 10
                    }
                }

                delegate: ScreenshotCard {
                    width: height * 16 / 9
                    height: screenshotStrip.height
                    maskSource: sharedShotMask
                    failedShots: root.failedShots
                    onLoadFailed: url => {
                        var failed = Object.assign({}, root.failedShots);
                        failed[url] = true;
                        root.failedShots = failed;
                    }
                    onActivated: shotIndex => {
                        lightbox.currentIndex = shotIndex;
                        lightbox.open();
                    }
                }
            }

            ColumnLayout {
                Layout.fillWidth: true
                visible: DetailController.refining
                spacing: Kirigami.Units.smallSpacing

                SkeletonBlock { Layout.fillWidth: true; Layout.preferredHeight: 14 }
                SkeletonBlock { Layout.fillWidth: true; Layout.preferredHeight: 14 }
                SkeletonBlock { Layout.preferredWidth: parent.width * 0.7; Layout.preferredHeight: 14 }
                SkeletonBlock {
                    Layout.fillWidth: true
                    Layout.topMargin: Kirigami.Units.smallSpacing
                    Layout.preferredHeight: Kirigami.Units.gridUnit * 12
                }
            }

            Controls.Label {
                id: descriptionLabel
                Layout.fillWidth: true
                visible: DetailController.description.length > 0
                text: DetailController.description
                textFormat: Text.StyledText
                wrapMode: Text.WordWrap
                maximumLineCount: root.descriptionExpanded ? -1 : 6
                elide: root.descriptionExpanded ? Text.ElideNone : Text.ElideRight
                onLinkActivated: link => Qt.openUrlExternally(link)
            }

            Controls.Button {
                Layout.alignment: Qt.AlignHCenter
                visible: !root.descriptionExpanded && descriptionLabel.truncated
                flat: true
                text: KI18n.i18n("Read More")
                onClicked: root.descriptionExpanded = true
            }

            Kirigami.Separator {
                Layout.fillWidth: true
                visible: projectLinks.visible
            }

            ProjectLinksGrid {
                id: projectLinks
                Layout.fillWidth: true
                pkgId: DetailController.id
                remote: DetailController.remote
                links: JSON.parse(DetailController.projectUrlsJson || "{}")
            }

            Item { Layout.preferredHeight: Kirigami.Units.gridUnit * 2 }
        }
    }

    Kirigami.OverlaySheet {
        id: extensionsSheet
        title: KI18n.i18n("Add-ons")

        Column {
            width: Kirigami.Units.gridUnit * 26
            spacing: Kirigami.Units.smallSpacing

            Repeater {
                model: root.extensions

                delegate: RowLayout {
                    id: extDelegate

                    required property var modelData

                    width: parent.width
                    spacing: Kirigami.Units.largeSpacing

                    Controls.Label {
                        Layout.fillWidth: true
                        text: extDelegate.modelData.name
                        elide: Text.ElideRight
                    }

                    ItemButtons {
                        pkgId: extDelegate.modelData.id
                        name: extDelegate.modelData.name
                        installed: extDelegate.modelData.installed
                        onRemoveRequested: TransactionsModel.removePackage(extDelegate.modelData.id, extDelegate.modelData.name, "")
                    }
                }
            }
        }
    }

    Controls.Popup {
        id: lightbox

        property int currentIndex: 0
        readonly property string currentScreenshot: lightbox.visible && lightbox.currentIndex < DetailController.screenshots.length
            ? DetailController.screenshots[lightbox.currentIndex]
            : ""

        parent: Controls.Overlay.overlay
        anchors.centerIn: parent
        width: parent.width
        height: parent.height
        modal: true
        padding: 0
        background: Rectangle {
            color: Qt.rgba(0, 0, 0, 0.9)
        }

        Image {
            anchors.fill: parent
            anchors.margins: Kirigami.Units.gridUnit * 2
            source: lightbox.currentScreenshot
            fillMode: Image.PreserveAspectFit
            asynchronous: true
        }

        TapHandler {
            onTapped: lightbox.close()
        }

        Controls.RoundButton {
            visible: lightbox.currentIndex > 0
            anchors.left: parent.left
            anchors.leftMargin: Kirigami.Units.largeSpacing
            anchors.verticalCenter: parent.verticalCenter
            icon.name: "go-previous-symbolic"
            onClicked: lightbox.currentIndex -= 1
        }

        Controls.RoundButton {
            visible: lightbox.currentIndex < DetailController.screenshots.length - 1
            anchors.right: parent.right
            anchors.rightMargin: Kirigami.Units.largeSpacing
            anchors.verticalCenter: parent.verticalCenter
            icon.name: "go-next-symbolic"
            onClicked: lightbox.currentIndex += 1
        }

        Controls.RoundButton {
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.margins: Kirigami.Units.largeSpacing
            icon.name: "window-close-symbolic"
            onClicked: lightbox.close()
        }
    }
}
