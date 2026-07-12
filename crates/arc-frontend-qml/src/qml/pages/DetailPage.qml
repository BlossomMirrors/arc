pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Effects
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.blossomos.arc

Kirigami.ScrollablePage {
    id: root

    Kirigami.ColumnView.fillWidth: true

    required property string pkgId
    // optional row data from wherever the app was clicked
    property var seed: null

    readonly property var extensions: JSON.parse(DetailController.extensionsJson.length > 0 ? DetailController.extensionsJson : "[]")

    title: DetailController.name

    Component.onCompleted: {
        if (seed) {
            DetailController.loadWithSeed(pkgId, seed.name ?? "", seed.summary ?? "", seed.iconUrl ?? "", seed.installed ?? false);
        } else {
            DetailController.load(pkgId);
        }
    }

    Component.onDestruction: DetailController.pageClosed(pkgId)

    ConveyorLoader {
        anchors.centerIn: parent
        visible: DetailController.loading
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

                onRemoveClicked: removeDialog.open()
                onStartClicked: DetailController.launch()
                onAddonsClicked: extensionsSheet.open()
            }

            ItemProgressBar {
                Layout.fillWidth: true
                visible: DetailController.busy
                progress: DetailController.progress
            }

            Kirigami.Separator {
                Layout.fillWidth: true
            }

            Controls.Label {
                Layout.fillWidth: true
                visible: DetailController.summary.length > 0
                text: DetailController.summary
                wrapMode: Text.WordWrap
            }

            CardCarousel {
                id: screenshotStrip
                Layout.fillWidth: true
                Layout.preferredHeight: Kirigami.Units.gridUnit * 14
                visible: DetailController.screenshots.length > 0
                model: DetailController.screenshots

                delegate: Item {
                    id: shotDelegate

                    required property string modelData
                    required property int index

                    width: height * 16 / 9
                    height: screenshotStrip.height

                    Image {
                        id: shotImage
                        anchors.fill: parent
                        source: shotDelegate.modelData
                        fillMode: Image.PreserveAspectCrop
                        asynchronous: true

                        layer.enabled: true
                        layer.effect: MultiEffect {
                            maskEnabled: true
                            maskSource: shotMask
                            maskThresholdMin: 0.5
                            maskSpreadAtMin: 1.0
                        }
                    }

                    Item {
                        id: shotMask
                        anchors.fill: parent
                        visible: false
                        layer.enabled: true
                        Rectangle {
                            anchors.fill: parent
                            radius: 10
                        }
                    }

                    HoverHandler {
                        cursorShape: Qt.PointingHandCursor
                    }

                    TapHandler {
                        onTapped: {
                            lightbox.currentIndex = shotDelegate.index;
                            lightbox.open();
                        }
                    }
                }
            }

            Controls.Label {
                Layout.fillWidth: true
                visible: DetailController.description.length > 0
                text: DetailController.description
                textFormat: Text.RichText
                wrapMode: Text.WordWrap
                onLinkActivated: link => Qt.openUrlExternally(link)
            }

            Kirigami.Separator {
                Layout.fillWidth: true
                visible: DetailController.homepageUrl.length > 0
            }

            RowLayout {
                Layout.fillWidth: true
                visible: DetailController.homepageUrl.length > 0
                spacing: Kirigami.Units.largeSpacing

                Controls.Label {
                    text: i18n("Website")
                    font.bold: true
                    opacity: 0.7
                }

                Controls.Label {
                    Layout.fillWidth: true
                    text: "<a href=\"" + DetailController.homepageUrl + "\">" + DetailController.homepageUrl + "</a>"
                    textFormat: Text.RichText
                    elide: Text.ElideRight
                    onLinkActivated: link => Qt.openUrlExternally(link)
                }
            }

            Item { Layout.preferredHeight: Kirigami.Units.gridUnit * 2 }
        }
    }

    Kirigami.PromptDialog {
        id: removeDialog
        title: i18n("Remove %1?", DetailController.name)
        subtitle: i18n("The application will be uninstalled from your system.")
        standardButtons: Kirigami.Dialog.Cancel
        showCloseButton: false

        customFooterActions: [
            Kirigami.Action {
                text: i18n("Remove")
                icon.name: "edit-delete-symbolic"
                onTriggered: {
                    TransactionsModel.removePackage(DetailController.id);
                    removeDialog.close();
                }
            }
        ]
    }

    Kirigami.OverlaySheet {
        id: extensionsSheet
        title: i18n("Add-ons")

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
                        onRemoveRequested: TransactionsModel.removePackage(extDelegate.modelData.id)
                    }
                }
            }
        }
    }

    Controls.Popup {
        id: lightbox

        property int currentIndex: 0

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
            source: lightbox.visible && lightbox.currentIndex < DetailController.screenshots.length
                ? DetailController.screenshots[lightbox.currentIndex]
                : ""
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
