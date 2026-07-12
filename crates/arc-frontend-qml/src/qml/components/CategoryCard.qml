import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Kirigami.AbstractCard {
    id: root

    property string categoryId: ""
    property string label: ""
    property string iconName: ""
    property color bgColor: Kirigami.Theme.backgroundColor

    signal activated()

    showClickFeedback: true
    onClicked: root.activated()

    background: Rectangle {
        color: root.bgColor
        radius: Kirigami.Units.cornerRadius
    }

    contentItem: RowLayout {
        spacing: Kirigami.Units.smallSpacing

        Kirigami.Icon {
            source: root.iconName
            color: "white"
            Layout.preferredWidth: Kirigami.Units.iconSizes.medium
            Layout.preferredHeight: Kirigami.Units.iconSizes.medium
        }

        Controls.Label {
            text: root.label
            color: "white"
            font.bold: true
            elide: Text.ElideRight
            Layout.fillWidth: true
        }
    }
}
