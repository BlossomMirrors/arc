import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami

Kirigami.AbstractCard {
    id: root

    property string pkgId: ""
    property string appName: ""
    property string summary: ""
    property string iconUrl: ""
    property bool installed: false

    signal activated()

    showClickFeedback: true
    onClicked: root.activated()

    contentItem: ColumnLayout {
        spacing: Kirigami.Units.smallSpacing

        Item {
            Layout.alignment: Qt.AlignHCenter
            Layout.preferredWidth: Kirigami.Units.iconSizes.large
            Layout.preferredHeight: Kirigami.Units.iconSizes.large

            Kirigami.Icon {
                anchors.fill: parent
                source: root.iconUrl.length > 0 ? root.iconUrl : "application-x-executable"
            }

            Kirigami.Icon {
                visible: root.installed
                source: "emblem-checked"
                width: Kirigami.Units.iconSizes.small
                height: Kirigami.Units.iconSizes.small
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                anchors.margins: -2
                color: Kirigami.Theme.positiveTextColor
            }
        }

        Controls.Label {
            Layout.fillWidth: true
            text: root.appName
            horizontalAlignment: Text.AlignHCenter
            elide: Text.ElideRight
            font.bold: true
        }

        Controls.Label {
            Layout.fillWidth: true
            Layout.fillHeight: true
            text: root.summary
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignTop
            elide: Text.ElideRight
            wrapMode: Text.WordWrap
            maximumLineCount: 2
            opacity: 0.7
            font.pointSize: Kirigami.Theme.smallFont.pointSize
        }
    }
}
