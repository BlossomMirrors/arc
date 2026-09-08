pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Effects
import org.kde.kirigami as Kirigami

Item {
    id: root

    implicitHeight: Math.max(300, Window.height * 0.5)

    property var items: []

    signal storyActivated(int storyIndex)
    signal appActivated(string pkgId)

    property int currentIndex: 0

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
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.bottom: indicatorBackground.top
        anchors.bottomMargin: Kirigami.Units.largeSpacing

        Repeater {
            model: root.items

            delegate: Item {
                id: slide

                required property var modelData
                required property int index

                anchors.fill: parent
                opacity: index === root.currentIndex ? 1 : 0
                Behavior on opacity {
                    NumberAnimation {
                        duration: 400
                        easing.type: Easing.InOutQuad
                    }
                }

                Image {
                    anchors.fill: parent
                    source: slide.modelData.is_story ? slide.modelData.banner_url : ""
                    fillMode: Image.PreserveAspectCrop
                    asynchronous: true
                    visible: slide.modelData.is_story && slide.modelData.banner_url.length > 0
                }

                Kirigami.Icon {
                    visible: !slide.modelData.is_story && slide.modelData.icon_url.length > 0
                    source: slide.modelData.icon_url
                    width: 120
                    height: 120
                    anchors.horizontalCenter: parent.horizontalCenter
                    y: 60
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
                    onTapped: slide.modelData.is_story ? root.storyActivated(slide.modelData.story_index) : root.appActivated(slide.modelData.id)
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
    Rectangle {
        id: indicatorBackground
        Kirigami.Theme.colorSet: Kirigami.Theme.Complementary
        Kirigami.Theme.inherit: false

        implicitWidth: dots.implicitWidth + Kirigami.Units.largeSpacing
        implicitHeight: dots.implicitHeight + Kirigami.Units.smallSpacing * 2
        radius: height / 2

        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom

        color: Qt.alpha(Kirigami.Theme.alternateBackgroundColor, 0.6)

        Row {
            id: dots
            anchors.centerIn: parent
            spacing: Kirigami.Units.smallSpacing

            Repeater {
                model: root.items.length

                delegate: Rectangle {
                    id: dot
                    required property int index
                    width: index === root.currentIndex ? 30 : 8
                    height: 8
                    radius: 5
                    color: index === root.currentIndex ? "white" : Qt.rgba(1, 1, 1, 0.33)
                    Behavior on width {
                        NumberAnimation {
                            duration: 200
                            easing.type: Easing.InOutQuad
                        }
                    }
                    TapHandler {
                        enabled: dot.index !== root.currentIndex
                        margin: Kirigami.Units.smallSpacing * 2
                        onTapped: root.currentIndex = dot.index
                    }
                    HoverHandler {
                        enabled: dot.index !== root.currentIndex
                        margin: Kirigami.Units.smallSpacing * 2
                        cursorShape: Qt.PointingHandCursor
                    }
                }
            }
        }
    }
}
