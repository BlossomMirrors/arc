import QtQuick
import org.kde.kirigami as Kirigami

Item {
    id: root

    property string source: ""

    readonly property bool isUrl: root.source.startsWith("file:")
        || root.source.startsWith("http")
        || root.source.startsWith("qrc:")

    Image {
        id: img
        anchors.fill: parent
        visible: root.isUrl
        asynchronous: true
        cache: true
        fillMode: Image.PreserveAspectFit
        sourceSize.width: Math.max(1, root.width)
        sourceSize.height: Math.max(1, root.height)
        source: root.isUrl ? root.source : ""
        opacity: status === Image.Ready ? 1 : 0
        Behavior on opacity {
            NumberAnimation { duration: Kirigami.Units.shortDuration }
        }
    }

    Kirigami.Icon {
        anchors.fill: parent
        visible: !root.isUrl || img.status === Image.Error
        source: root.source.length > 0 && !root.isUrl ? root.source : "application-x-executable"
    }
}
