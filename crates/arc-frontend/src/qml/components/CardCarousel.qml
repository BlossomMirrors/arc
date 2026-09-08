pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls as Controls
import org.kde.kirigami as Kirigami

Item {
    id: root

    property alias model: repeater.model
    property Component delegate
    property real cardWidth: Kirigami.Units.gridUnit * 10
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

                delegate: Item {
                    id: slot

                    required property var modelData
                    required property int index

                    width: root.cardWidth
                    height: row.height

                    readonly property real slotX: index * (root.cardWidth + root.spacing)
                    readonly property bool inView: slot.slotX + root.cardWidth > flick.contentX - root.cardWidth
                        && slot.slotX < flick.contentX + flick.width + root.cardWidth

                    Loader {
                        anchors.fill: parent
                        active: slot.inView
                        sourceComponent: root.delegate
                        onLoaded: {
                            item.modelData = slot.modelData;
                            item.index = slot.index;
                        }
                    }
                }
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
