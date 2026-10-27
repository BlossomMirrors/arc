import QtQuick
import org.kde.kirigami as Kirigami
import org.blossomos.arc

Image {
    id: root

    readonly property bool local: root.source.toString().startsWith("file:")

    asynchronous: !root.local
    cache: true

    opacity: root.local || root.status === Image.Ready ? 1 : 0
    Behavior on opacity {
        enabled: !root.local
        NumberAnimation { duration: Kirigami.Units.shortDuration }
    }
}
