import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.blossomos.arc

Kirigami.Page {
    id: root

    Kirigami.ColumnView.fillWidth: true

    property string dialogIcon: "dialog-question-symbolic"
    property string dialogTitle: ""
    property string dialogDescription: ""

    default property alias content: contentColumn.data

    ColumnLayout {
        anchors.centerIn: parent
        width: Math.min(parent.width - Kirigami.Units.gridUnit * 4, Kirigami.Units.gridUnit * 30)
        spacing: Kirigami.Units.largeSpacing

        Kirigami.Icon {
            source: root.dialogIcon
            Layout.alignment: Qt.AlignHCenter
            Layout.preferredWidth: Kirigami.Units.iconSizes.huge
            Layout.preferredHeight: Kirigami.Units.iconSizes.huge
        }

        Kirigami.Heading {
            level: 2
            text: root.dialogTitle
            Layout.fillWidth: true
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
        }

        Controls.Label {
            visible: root.dialogDescription.length > 0
            text: root.dialogDescription
            wrapMode: Text.WordWrap
            Layout.fillWidth: true
            horizontalAlignment: Text.AlignHCenter
            opacity: 0.7
        }

        ColumnLayout {
            id: contentColumn
            Layout.fillWidth: true
            spacing: Kirigami.Units.largeSpacing
        }
    }
}
