import QtQuick
import org.blossomos.arc

Loader {
    id: root

    property int order: 0

    active: false

    Timer {
        interval: 60 + root.order * 60
        running: StartupController.firstFrameShown && !root.active
        onTriggered: root.active = true
    }

    function page() {
        root.active = true;
        return root.item;
    }
}
