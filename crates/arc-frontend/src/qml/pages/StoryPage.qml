pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.blossomos.arc

Kirigami.ScrollablePage {
    id: root

    Kirigami.ColumnView.fillWidth: true

    leftPadding: 0
    rightPadding: 0
    topPadding: 0

    required property string storyId

    title: StoryController.title

    Component.onCompleted: StoryController.load(storyId)

    ConveyorLoader {
        anchors.centerIn: parent
        visible: StoryController.loading
    }

    ColumnLayout {
        visible: !StoryController.loading
        width: root.width
        spacing: 0

        Item {
            Layout.fillWidth: true
            Layout.preferredHeight: 280
            visible: StoryController.bannerUrl.length > 0

            Image {
                anchors.fill: parent
                source: StoryController.bannerUrl
                fillMode: Image.PreserveAspectCrop
                asynchronous: true
            }

            Rectangle {
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                height: 100
                gradient: Gradient {
                    GradientStop { position: 0.0; color: "transparent" }
                    GradientStop { position: 1.0; color: Kirigami.Theme.backgroundColor }
                }
            }
        }

        ColumnLayout {
            Layout.alignment: Qt.AlignHCenter
            Layout.fillWidth: true
            Layout.maximumWidth: Kirigami.Units.gridUnit * 40
            Layout.margins: Kirigami.Units.largeSpacing
            spacing: Kirigami.Units.smallSpacing

            Kirigami.Heading {
                level: 1
                Layout.fillWidth: true
                text: StoryController.title
                wrapMode: Text.WordWrap
            }

            Repeater {
                model: JSON.parse(StoryController.blocksJson)

                delegate: Loader {
                    id: blockLoader

                    required property var modelData

                    Layout.fillWidth: true
                    Layout.topMargin: modelData.is_heading ? Kirigami.Units.largeSpacing : 0

                    sourceComponent: modelData.is_image ? imageBlock
                        : modelData.is_app ? appBlock
                        : modelData.is_list_item ? listBlock
                        : modelData.is_heading ? headingBlock
                        : textBlock

                    Component {
                        id: headingBlock
                        Kirigami.Heading {
                            level: 3
                            text: blockLoader.modelData.text
                            wrapMode: Text.WordWrap
                        }
                    }

                    Component {
                        id: textBlock
                        Controls.Label {
                            text: blockLoader.modelData.text
                            font.bold: blockLoader.modelData.is_bold
                            opacity: blockLoader.modelData.is_bold ? 1.0 : 0.85
                            wrapMode: Text.WordWrap
                        }
                    }

                    Component {
                        id: listBlock
                        RowLayout {
                            spacing: Kirigami.Units.smallSpacing

                            Controls.Label {
                                Layout.alignment: Qt.AlignTop
                                Layout.leftMargin: Kirigami.Units.smallSpacing
                                text: "•"
                            }

                            Controls.Label {
                                Layout.fillWidth: true
                                text: blockLoader.modelData.text
                                font.bold: blockLoader.modelData.is_bold
                                opacity: 0.85
                                wrapMode: Text.WordWrap
                            }
                        }
                    }

                    Component {
                        id: imageBlock
                        ColumnLayout {
                            spacing: Kirigami.Units.smallSpacing

                            Image {
                                id: blockImage
                                Layout.fillWidth: true
                                Layout.preferredHeight: width * 9 / 16
                                Layout.topMargin: Kirigami.Units.smallSpacing
                                source: blockLoader.modelData.image_url
                                fillMode: Image.PreserveAspectCrop
                                asynchronous: true
                            }

                            Controls.Label {
                                Layout.fillWidth: true
                                visible: blockLoader.modelData.text.length > 0
                                text: blockLoader.modelData.text
                                horizontalAlignment: Text.AlignHCenter
                                font.pointSize: Kirigami.Theme.smallFont.pointSize
                                opacity: 0.6
                            }
                        }
                    }

                    Component {
                        id: appBlock
                        Kirigami.AbstractCard {
                            visible: blockLoader.modelData.app_name.length > 0
                            showClickFeedback: true
                            onClicked: applicationWindow().openApp(blockLoader.modelData.app_id)

                            contentItem: RowLayout {
                                spacing: Kirigami.Units.largeSpacing

                                Kirigami.Icon {
                                    source: blockLoader.modelData.app_icon_url.length > 0
                                        ? blockLoader.modelData.app_icon_url
                                        : "application-x-executable"
                                    Layout.preferredWidth: Kirigami.Units.iconSizes.large
                                    Layout.preferredHeight: Kirigami.Units.iconSizes.large
                                }

                                ColumnLayout {
                                    Layout.fillWidth: true
                                    spacing: Kirigami.Units.smallSpacing / 2

                                    Controls.Label {
                                        Layout.fillWidth: true
                                        text: blockLoader.modelData.app_name
                                        font.bold: true
                                        elide: Text.ElideRight
                                    }

                                    Controls.Label {
                                        Layout.fillWidth: true
                                        text: blockLoader.modelData.app_summary
                                        opacity: 0.7
                                        elide: Text.ElideRight
                                    }
                                }
                            }
                        }
                    }
                }
            }

            Item { Layout.preferredHeight: Kirigami.Units.gridUnit * 2 }
        }
    }
}
