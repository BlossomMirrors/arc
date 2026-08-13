import QtQuick
import org.kde.kirigami as Kirigami

Rectangle {
    id: root

    radius: Kirigami.Units.cornerRadius
    color: Kirigami.Theme.alternateBackgroundColor
    clip: true

    Rectangle {
        width: root.width * 0.5
        height: root.height
        gradient: Gradient {
            orientation: Gradient.Horizontal
            GradientStop { position: 0.0; color: "transparent" }
            GradientStop { position: 0.5; color: Qt.rgba(1, 1, 1, 0.14) }
            GradientStop { position: 1.0; color: "transparent" }
        }

        SequentialAnimation on x {
            loops: Animation.Infinite
            NumberAnimation {
                from: -root.width * 0.5
                to: root.width
                duration: 1200
                easing.type: Easing.InOutQuad
            }
            PauseAnimation { duration: 400 }
        }
    }
}
