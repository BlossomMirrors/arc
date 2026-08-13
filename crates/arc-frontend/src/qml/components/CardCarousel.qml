import QtQuick
import QtQuick.Controls as Controls
import org.kde.kirigami as Kirigami

// Horizontally-scrolling row of cards. Built on Repeater + Flickable rather
// than ListView: ListView bound directly to a plain JS array (JSON.parse'd
// model data, as opposed to a real QAbstractListModel) rendered nothing here
// despite the underlying data being correct, while this Repeater-based
// approach (the same pattern HeroCarousel already uses successfully) does.
Item {
    id: root

    property alias model: repeater.model
    property alias delegate: repeater.delegate
    property real spacing: Kirigami.Units.largeSpacing * 2
    readonly property int count: repeater.count

    implicitHeight: row.implicitHeight

    HoverHandler {
        id: hover
    }

    Flickable {
        id: flick
        anchors.fill: parent
        clip: true
        contentWidth: row.implicitWidth
        contentHeight: height
        boundsBehavior: Flickable.StopAtBounds
        flickDeceleration: 4000

        Row {
            id: row
            height: flick.height
            spacing: root.spacing

            Repeater {
                id: repeater
            }
        }
    }

    NumberAnimation {
        id: scrollAnim
        target: flick
        property: "contentX"
        duration: 300
        easing.type: Easing.InOutQuad
    }

    function stepBy(delta) {
        flick.cancelFlick();
        scrollAnim.stop();
        var target = Math.max(0, Math.min(flick.contentX + delta, flick.contentWidth - flick.width));
        scrollAnim.from = flick.contentX;
        scrollAnim.to = target;
        scrollAnim.start();
    }

    Controls.RoundButton {
        visible: hover.hovered && flick.contentX > 0
        anchors.left: parent.left
        anchors.leftMargin: Kirigami.Units.smallSpacing
        anchors.verticalCenter: parent.verticalCenter
        icon.name: "go-previous-symbolic"
        onClicked: root.stepBy(-root.width * 0.8)
    }

    Controls.RoundButton {
        visible: hover.hovered && flick.contentX < flick.contentWidth - flick.width - 1
        anchors.right: parent.right
        anchors.rightMargin: Kirigami.Units.smallSpacing
        anchors.verticalCenter: parent.verticalCenter
        icon.name: "go-next-symbolic"
        onClicked: root.stepBy(root.width * 0.8)
    }
}
