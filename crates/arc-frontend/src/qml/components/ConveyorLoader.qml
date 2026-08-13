import QtQuick
import org.kde.kirigami as Kirigami

Item {
    id: root

    implicitWidth: 560
    implicitHeight: 170
    width: implicitWidth
    height: implicitHeight
    clip: true

    property real phase: 0

    NumberAnimation on phase {
        from: 0
        to: 1
        duration: 6000
        loops: Animation.Infinite
        running: root.visible
    }

    component RollingBlock: Rectangle {
        id: block

        property real offset: 0
        readonly property real blockPhase: (root.phase + offset) % 1
        readonly property real bobT: (blockPhase * 12) % 1
        property color highlightColor
        property color darkColor

        width: 56
        height: 56
        radius: 20

        x: -110 + blockPhase * (root.width + 112)
        y: 57 - 9 * (1 - 2 * Math.abs(bobT - 0.5))
        rotation: blockPhase * 1080

        gradient: Gradient {
            GradientStop { position: 0.0; color: block.highlightColor }
            GradientStop { position: 1.0; color: block.darkColor }
        }

        Rectangle {
            anchors.fill: parent
            radius: parent.radius
            gradient: Gradient {
                GradientStop { position: 0.0; color: Qt.rgba(1, 1, 1, 0.2) }
                GradientStop { position: 0.58; color: "transparent" }
            }
        }
    }

    RollingBlock {
        offset: 0
        highlightColor: Qt.lighter(Kirigami.Theme.highlightColor, 1.35)
        darkColor: Qt.darker(Kirigami.Theme.highlightColor, 1.7)
    }
    RollingBlock {
        offset: 0.2
        highlightColor: Qt.lighter(Kirigami.Theme.highlightColor, 1.25)
        darkColor: Qt.darker(Kirigami.Theme.highlightColor, 2.0)
    }
    RollingBlock {
        offset: 0.4
        highlightColor: Qt.lighter(Kirigami.Theme.highlightColor, 1.15)
        darkColor: Qt.darker(Kirigami.Theme.highlightColor, 2.4)
    }
    RollingBlock {
        offset: 0.6
        highlightColor: Qt.lighter(Kirigami.Theme.highlightColor, 1.05)
        darkColor: Qt.darker(Kirigami.Theme.highlightColor, 2.9)
    }
    RollingBlock {
        offset: 0.8
        highlightColor: Kirigami.Theme.highlightColor
        darkColor: Qt.darker(Kirigami.Theme.highlightColor, 3.6)
    }

    Rectangle {
        width: 120
        height: parent.height
        gradient: Gradient {
            orientation: Gradient.Horizontal
            GradientStop { position: 0.0; color: Kirigami.Theme.backgroundColor }
            GradientStop { position: 1.0; color: "transparent" }
        }
    }

    Rectangle {
        x: parent.width - width
        width: 120
        height: parent.height
        gradient: Gradient {
            orientation: Gradient.Horizontal
            GradientStop { position: 0.0; color: "transparent" }
            GradientStop { position: 1.0; color: Kirigami.Theme.backgroundColor }
        }
    }
}
