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

    implicitWidth: 220
    implicitHeight: 150

    Item {
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
            visible: root.bannerUrl.length === 0
            gradient: Gradient {
                GradientStop { position: 0.0; color: Qt.darker(Kirigami.Theme.highlightColor, 1.8) }
                GradientStop { position: 1.0; color: Qt.darker(Kirigami.Theme.highlightColor, 3.2) }
            }
        }

        Image {
            anchors.fill: parent
            visible: root.bannerUrl.length > 0
            source: root.bannerUrl
            fillMode: Image.PreserveAspectCrop
            asynchronous: true
        }

        Kirigami.Icon {
            visible: root.bannerUrl.length === 0 && root.iconUrl.length > 0
            source: root.iconUrl
            width: 56
            height: 56
            anchors.horizontalCenter: parent.horizontalCenter
            y: 20
        }

        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            height: 80
            gradient: Gradient {
                GradientStop { position: 0.0; color: "transparent" }
                GradientStop { position: 1.0; color: Qt.rgba(0, 0, 0, 0.82) }
            }
        }

        Column {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            anchors.margins: 12
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

    Item {
        id: maskShape
        anchors.fill: parent
        visible: false
        layer.enabled: true
        Rectangle {
            anchors.fill: parent
            radius: 14
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
