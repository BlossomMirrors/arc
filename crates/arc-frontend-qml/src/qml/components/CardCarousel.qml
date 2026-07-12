import QtQuick
import QtQuick.Controls as Controls

Item {
    id: root

    property alias model: list.model
    property alias delegate: list.delegate
    property alias spacing: list.spacing
    property alias count: list.count

    implicitHeight: list.implicitHeight

    HoverHandler {
        id: hover
    }

    ListView {
        id: list
        anchors.fill: parent
        orientation: ListView.Horizontal
        clip: true
        spacing: 12
        snapMode: ListView.SnapToItem
        flickDeceleration: 4000
    }

    NumberAnimation {
        id: scrollAnim
        target: list
        property: "contentX"
        duration: 300
        easing.type: Easing.InOutQuad
    }

    function stepBy(delta) {
        list.cancelFlick();
        scrollAnim.stop();
        var target = Math.max(0, Math.min(list.contentX + delta, list.contentWidth - list.width));
        scrollAnim.from = list.contentX;
        scrollAnim.to = target;
        scrollAnim.start();
    }

    Controls.RoundButton {
        visible: hover.hovered && !list.atXBeginning
        anchors.left: parent.left
        anchors.leftMargin: 8
        anchors.verticalCenter: parent.verticalCenter
        icon.name: "go-previous-symbolic"
        onClicked: root.stepBy(-root.width * 0.8)
    }

    Controls.RoundButton {
        visible: hover.hovered && !list.atXEnd
        anchors.right: parent.right
        anchors.rightMargin: 8
        anchors.verticalCenter: parent.verticalCenter
        icon.name: "go-next-symbolic"
        onClicked: root.stepBy(root.width * 0.8)
    }
}
