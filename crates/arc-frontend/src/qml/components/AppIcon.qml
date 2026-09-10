import QtQuick
import org.kde.kirigami as Kirigami

Item {
    id: root

    property string source: ""

    readonly property bool isUrl: root.source.startsWith("file:")
        || root.source.startsWith("http")
        || root.source.startsWith("qrc:")

    readonly property int decodeSize: Math.round(Math.max(root.width, root.height))

    property int attempt: 0
    readonly property bool retryable: root.source.startsWith("http")
    readonly property string requestUrl: root.attempt > 0
        ? root.source + (root.source.indexOf("?") >= 0 ? "&" : "?") + "retry=" + root.attempt
        : root.source

    onSourceChanged: root.attempt = 0

    Image {
        id: img
        anchors.fill: parent
        visible: root.isUrl
        asynchronous: true
        cache: true
        fillMode: Image.PreserveAspectFit
        sourceSize.width: root.decodeSize
        sourceSize.height: root.decodeSize

        source: root.isUrl && root.decodeSize > 0 ? root.requestUrl : ""
        opacity: status === Image.Ready ? 1 : 0
        Behavior on opacity {
            NumberAnimation { duration: Kirigami.Units.shortDuration }
        }
    }

    Timer {
        interval: 2000 * (root.attempt + 1)
        running: root.retryable && img.status === Image.Error && root.attempt < 1
        onTriggered: root.attempt += 1
    }

    Kirigami.Icon {
        anchors.fill: parent
        visible: !root.isUrl || img.status === Image.Error
        source: root.source.length > 0 && !root.isUrl ? root.source : "application-x-executable"
    }
}
