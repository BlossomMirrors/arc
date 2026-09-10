pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Effects
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.blossomos.arc
import org.kde.ki18n

Item {
    id: root

    property Flickable backdrop: null

    property real inset: Kirigami.Units.largeSpacing

    property alias searchText: searchField.text

    signal listActivated(string slug, string label)
    signal searchRequested(string query)
    signal searchDismissed()

    readonly property var listDefs: JSON.parse(ListController.defsJson || "[]")

    ListLabels {
        id: listLabels
    }

    function focusSearch(prefill) {
        searchField.forceActiveFocus();
        if (prefill !== undefined) {
            searchField.text = prefill;
        }
        searchField.cursorPosition = searchField.text.length;
    }

    implicitHeight: barContent.implicitHeight + Kirigami.Units.smallSpacing * 2

    ShaderEffectSource {
        id: backdropSource
        anchors.fill: parent
        visible: false
        live: true
        hideSource: false
        sourceItem: root.backdrop ? root.backdrop.contentItem : null

        sourceRect: root.backdrop
            ? Qt.rect(0, root.backdrop.contentY, root.width, root.height)
            : Qt.rect(0, 0, 0, 0)
    }

    MultiEffect {
        anchors.fill: parent
        source: backdropSource
        visible: root.backdrop !== null
        blurEnabled: true
        blur: 1.0
        blurMax: 48
        autoPaddingEnabled: false
    }

    Rectangle {
        anchors.fill: parent
        color: Qt.alpha(Kirigami.Theme.backgroundColor, 0.7)
    }

    RowLayout {
        id: barContent
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        anchors.leftMargin: root.inset
        anchors.rightMargin: root.inset
        spacing: Kirigami.Units.largeSpacing

        Flickable {
            id: badgeStrip
            Layout.fillWidth: true
            Layout.preferredHeight: badgeRow.implicitHeight
            contentWidth: badgeRow.implicitWidth
            contentHeight: badgeRow.implicitHeight
            flickableDirection: Flickable.HorizontalFlick
            boundsBehavior: Flickable.StopAtBounds
            clip: true

            Row {
                id: badgeRow
                spacing: Kirigami.Units.smallSpacing

                Repeater {
                    model: root.listDefs

                    delegate: Kirigami.Chip {
                        id: badge

                        required property var modelData

                        height: Math.round(Kirigami.Units.gridUnit * 1.6)
                        closable: false
                        checkable: false
                        checked: false

                        iconMask: true
                        icon.name: badge.modelData.iconName
                        icon.color: badge.modelData.color
                        icon.width: Kirigami.Units.iconSizes.small
                        icon.height: Kirigami.Units.iconSizes.small
                        text: listLabels.labelFor(badge.modelData.slug)

                        background: Rectangle {
                            radius: Kirigami.Units.cornerRadius
                            color: Qt.alpha(Kirigami.Theme.textColor,
                                badge.hovered || badge.down ? 0.14 : 0.07)

                            Behavior on color {
                                ColorAnimation { duration: Kirigami.Units.shortDuration }
                            }
                        }

                        onClicked: root.listActivated(badge.modelData.slug, badge.text)

                        HoverHandler {
                            cursorShape: Qt.PointingHandCursor
                        }
                    }
                }
            }
        }

        Kirigami.SearchField {
            id: searchField

            Layout.preferredWidth: Kirigami.Units.gridUnit * 22
            placeholderText: KI18n.i18n("Search apps...")
            font.pointSize: Kirigami.Theme.defaultFont.pointSize + 1

            autoAccept: false

            onTextChanged: if (text.length > 0) root.searchRequested(text)
            onAccepted: if (text.length > 0) root.searchRequested(text)

            Keys.onEscapePressed: root.searchDismissed()
        }
    }
}
