pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.kde.ki18n

Item {
    id: root

    property string pkgId: ""
    property string remote: ""
    property var links: ({})

    readonly property var allItems: [
        { label: KI18n.i18n("Flathub Page"), icon: "package-x-generic-symbolic", url: root.remote === "flathub" ? ("https://flathub.org/apps/" + root.pkgId) : "" },
        { label: KI18n.i18n("Project Website"), icon: "go-home-symbolic", url: root.links.homepage ?? "" },
        { label: KI18n.i18n("Bug Tracker"), icon: "tools-report-bug-symbolic", url: root.links.bugtracker ?? "" },
        { label: KI18n.i18n("FAQ"), icon: "system-help-symbolic", url: root.links.faq ?? "" },
        { label: KI18n.i18n("Help"), icon: "help-contents-symbolic", url: root.links.help ?? "" },
        { label: KI18n.i18n("Donate"), icon: "emblem-favorite-symbolic", url: root.links.donation ?? "" },
        { label: KI18n.i18n("Translate"), icon: "preferences-desktop-locale-symbolic", url: root.links.translate ?? "" },
        { label: KI18n.i18n("Contact"), icon: "mail-message-new-symbolic", url: root.links.contact ?? "" },
        { label: KI18n.i18n("Source Code"), icon: "applications-development-symbolic", url: root.links["vcs-browser"] ?? "" },
        { label: KI18n.i18n("Contribute"), icon: "system-users-symbolic", url: root.links.contribute ?? "" }
    ].filter(item => item.url.length > 0)

    readonly property int leftCount: Math.ceil(root.allItems.length / 2)
    readonly property var leftItems: root.allItems.slice(0, root.leftCount)
    readonly property var rightItems: root.allItems.slice(root.leftCount)

    visible: root.leftItems.length > 0 || root.rightItems.length > 0
    implicitHeight: grid.implicitHeight

    TextEdit {
        id: clipboardHelper
        visible: false

        function copyText(text) {
            clipboardHelper.text = text;
            clipboardHelper.selectAll();
            clipboardHelper.copy();
        }
    }

    component LinkCard: Kirigami.AbstractCard {
        id: linkCard

        required property var modelData
        required property int index
        required property int gridColumn

        Layout.row: linkCard.index
        Layout.column: linkCard.gridColumn
        Layout.fillWidth: true
        Layout.preferredWidth: 0

        showClickFeedback: true
        onClicked: Qt.openUrlExternally(linkCard.modelData.url)

        contentItem: RowLayout {
            spacing: Kirigami.Units.largeSpacing

            Kirigami.Icon {
                source: linkCard.modelData.icon
                Layout.preferredWidth: Kirigami.Units.iconSizes.medium
                Layout.preferredHeight: Kirigami.Units.iconSizes.medium
            }

            ColumnLayout {
                Layout.fillWidth: true
                Layout.minimumWidth: 0
                spacing: 0

                Controls.Label {
                    Layout.fillWidth: true
                    text: linkCard.modelData.label
                    font.bold: true
                    elide: Text.ElideRight
                }

                Controls.Label {
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0
                    text: linkCard.modelData.url.replace(/^https?:\/\//, "")
                    opacity: 0.7
                    elide: Text.ElideRight
                    font.pointSize: Kirigami.Theme.smallFont.pointSize
                }
            }

            Controls.ToolButton {
                icon.name: "edit-copy-symbolic"
                display: Controls.ToolButton.IconOnly
                text: KI18n.i18n("Copy Link")
                Controls.ToolTip.text: text
                Controls.ToolTip.visible: hovered
                Controls.ToolTip.delay: Kirigami.Units.toolTipDelay
                onClicked: clipboardHelper.copyText(linkCard.modelData.url)
            }

            Controls.ToolButton {
                icon.name: "link-symbolic"
                display: Controls.ToolButton.IconOnly
                text: KI18n.i18n("Open Link")
                Controls.ToolTip.text: text
                Controls.ToolTip.visible: hovered
                Controls.ToolTip.delay: Kirigami.Units.toolTipDelay
                onClicked: Qt.openUrlExternally(linkCard.modelData.url)
            }
        }
    }

    GridLayout {
        id: grid
        anchors.fill: parent
        columns: 2
        columnSpacing: Kirigami.Units.largeSpacing
        rowSpacing: Kirigami.Units.smallSpacing

        Repeater {
            model: root.leftItems
            delegate: LinkCard { gridColumn: 0 }
        }

        Repeater {
            model: root.rightItems
            delegate: LinkCard { gridColumn: 1 }
        }
    }
}
