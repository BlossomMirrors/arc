pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Effects
import org.kde.kirigami as Kirigami

Item {
    id: root

    property var items: []

    signal storyActivated(int storyIndex)
    signal appActivated(string pkgId)

    property int currentIndex: 0

    implicitHeight: 400

    onItemsChanged: currentIndex = 0

    HoverHandler {
        id: hover
    }

    Timer {
        interval: 5000
        repeat: true
        running: root.items.length > 1 && !hover.hovered
        onTriggered: root.currentIndex = (root.currentIndex + 1) % root.items.length
    }

    Item {
        id: content
        anchors.fill: parent

        layer.enabled: true
        layer.effect: MultiEffect {
            maskEnabled: true
            maskSource: maskShape
            maskThresholdMin: 0.5
            maskSpreadAtMin: 1.0
        }

        Rectangle {
            anchors.fill: parent
            color: Kirigami.Theme.alternateBackgroundColor
        }

        Repeater {
            model: root.items

            delegate: Item {
                id: slide

                required property var modelData
                required property int index

                anchors.fill: parent
                opacity: index === root.currentIndex ? 1 : 0
                Behavior on opacity {
                    NumberAnimation { duration: 400; easing.type: Easing.InOutQuad }
                }

                Image {
                    anchors.fill: parent
                    source: slide.modelData.is_story ? slide.modelData.banner_url : ""
                    fillMode: Image.PreserveAspectCrop
                    asynchronous: true
                    visible: slide.modelData.is_story && slide.modelData.banner_url.length > 0
                }

                Rectangle {
                    anchors.fill: parent
                    visible: !slide.modelData.is_story || slide.modelData.banner_url.length === 0
                    gradient: Gradient {
                        GradientStop { position: 0.0; color: Qt.darker(Kirigami.Theme.highlightColor, 1.8) }
                        GradientStop { position: 1.0; color: Qt.darker(Kirigami.Theme.highlightColor, 3.2) }
                    }
                }

                Kirigami.Icon {
                    visible: !slide.modelData.is_story && slide.modelData.icon_url.length > 0
                    source: slide.modelData.icon_url
                    width: 120
                    height: 120
                    anchors.horizontalCenter: parent.horizontalCenter
                    y: 60
                }

                Rectangle {
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom
                    height: 180
                    gradient: Gradient {
                        GradientStop { position: 0.0; color: "transparent" }
                        GradientStop { position: 1.0; color: Qt.rgba(0, 0, 0, 0.88) }
                    }
                }

                Column {
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom
                    anchors.leftMargin: 24
                    anchors.rightMargin: 96
                    anchors.bottomMargin: 36
                    spacing: 6

                    Kirigami.Heading {
                        width: parent.width
                        level: 1
                        text: slide.modelData.title
                        color: "white"
                        elide: Text.ElideRight
                        maximumLineCount: 2
                        wrapMode: Text.WordWrap
                    }

                    Controls.Label {
                        width: parent.width
                        visible: slide.modelData.body.length > 0
                        text: slide.modelData.body
                        color: Qt.rgba(1, 1, 1, 0.8)
                        elide: Text.ElideRight
                        maximumLineCount: 2
                        wrapMode: Text.WordWrap
                    }
                }

                TapHandler {
                    enabled: slide.index === root.currentIndex
                    onTapped: slide.modelData.is_story
                        ? root.storyActivated(slide.modelData.story_index)
                        : root.appActivated(slide.modelData.id)
                }
            }
        }
    }

    Item {
        id: maskShape
        anchors.fill: parent
        visible: false
        layer.enabled: true
        Rectangle {
            anchors.fill: parent
            radius: 16
        }
    }

    Controls.RoundButton {
        visible: root.items.length > 1
        enabled: root.currentIndex > 0
        anchors.left: parent.left
        anchors.leftMargin: 16
        anchors.verticalCenter: parent.verticalCenter
        icon.name: "go-previous-symbolic"
        onClicked: root.currentIndex -= 1
    }

    Controls.RoundButton {
        visible: root.items.length > 1
        enabled: root.currentIndex < root.items.length - 1
        anchors.right: parent.right
        anchors.rightMargin: 16
        anchors.verticalCenter: parent.verticalCenter
        icon.name: "go-next-symbolic"
        onClicked: root.currentIndex += 1
    }

    Row {
        visible: root.items.length > 1
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: 12
        spacing: 6

        Repeater {
            model: root.items.length

            delegate: Rectangle {
                id: dot
                required property int index
                width: index === root.currentIndex ? 20 : 6
                height: 6
                radius: 3
                color: index === root.currentIndex ? "white" : Qt.rgba(1, 1, 1, 0.33)
                Behavior on width {
                    NumberAnimation { duration: 200; easing.type: Easing.InOutQuad }
                }
                TapHandler {
                    onTapped: root.currentIndex = dot.index
                }
            }
        }
    }
}
