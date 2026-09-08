pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Effects
import org.kde.kirigami as Kirigami

Item {
    id: root

    property string bannerUrl: ""
    property string iconUrl: ""
    property string heroTitle: ""
    property string body: ""

    signal activated()

    anchors.fill: parent

    Item {
        anchors.fill: parent

        Image {
            anchors.fill: parent
            source: root.bannerUrl
            fillMode: Image.PreserveAspectCrop
            asynchronous: true
        }

        Column {
            anchors.fill: parent
            spacing: 3

            Controls.Label {
                width: parent.width
                text: root.heroTitle
                font.bold: true
                color: "white"
                elide: Text.ElideRight
            }

            Controls.Label {
                width: parent.width
                visible: root.body.length > 0
                text: root.body
                font.pointSize: Kirigami.Theme.smallFont.pointSize
                color: Qt.rgba(1, 1, 1, 0.67)
                elide: Text.ElideRight
            }
        }
    }


    Rectangle {
        anchors.fill: parent
        radius: 14
        color: "transparent"
        border.width: hoverHandler.hovered ? 2 : 1
        border.color: hoverHandler.hovered ? Qt.rgba(1, 1, 1, 0.31) : Qt.rgba(1, 1, 1, 0.09)
        Behavior on border.color {
            ColorAnimation { duration: 100 }
        }
    }

    HoverHandler {
        id: hoverHandler
        cursorShape: Qt.PointingHandCursor
    }

    TapHandler {
        onTapped: root.activated()
    }
}
