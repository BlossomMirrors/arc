import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.blossomos.arc

Controls.ToolBar {
    id: root

    property alias searchText: searchField.text
    property string currentView: "home"
    readonly property bool onSearchPage: currentView === "search"

    signal homeRequested()
    signal searchRequested(string query)
    signal searchTextEdited(string query)
    signal installedRequested()
    signal downloadsRequested()
    signal settingsRequested()

    function focusSearch(prefill) {
        searchField.forceActiveFocus();
        if (prefill !== undefined) {
            searchField.text = prefill;
        }
        searchField.cursorPosition = searchField.text.length;
    }

    position: Controls.ToolBar.Header
    padding: Kirigami.Units.smallSpacing

    Shortcut {
        sequences: [StandardKey.Back, "Alt+Left"]
        onActivated: NavController.goBack()
    }

    Shortcut {
        sequences: [StandardKey.Forward, "Alt+Right"]
        onActivated: NavController.goForward()
    }

    Shortcut {
        sequence: StandardKey.Find
        onActivated: {
            searchField.forceActiveFocus();
            searchField.selectAll();
        }
    }

    component CountBadge: Rectangle {
        property int count: 0
        property color accent: Kirigami.Theme.highlightColor

        visible: count > 0
        width: Math.max(height, badgeLabel.implicitWidth + 8)
        height: badgeLabel.implicitHeight + 2
        radius: height / 2
        color: accent

        Controls.Label {
            id: badgeLabel
            anchors.centerIn: parent
            text: parent.count > 99 ? "99+" : parent.count
            font.pointSize: Kirigami.Theme.smallFont.pointSize - 1
            font.bold: true
            color: "white"
        }
    }

    contentItem: Item {
        implicitHeight: Math.max(navButtons.implicitHeight, viewTabs.implicitHeight, rightGroup.implicitHeight)

        RowLayout {
            id: navButtons
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            spacing: 0

            Controls.ToolButton {
                icon.name: "go-previous-symbolic"
                enabled: NavController.canGoBack
                onClicked: NavController.goBack()
                Controls.ToolTip.text: i18n("Back (Alt+Left)")
                Controls.ToolTip.visible: hovered
                Controls.ToolTip.delay: Kirigami.Units.toolTipDelay
            }

            Controls.ToolButton {
                icon.name: "go-next-symbolic"
                enabled: NavController.canGoForward
                onClicked: NavController.goForward()
                Controls.ToolTip.text: i18n("Forward (Alt+Right)")
                Controls.ToolTip.visible: hovered
                Controls.ToolTip.delay: Kirigami.Units.toolTipDelay
            }
        }

        Controls.TabBar {
            id: viewTabs

            // centered between the nav arrows and the search group
            x: Math.max(
                navButtons.width + Kirigami.Units.largeSpacing,
                Math.min(
                    (parent.width - width) / 2,
                    rightGroup.x - width - Kirigami.Units.largeSpacing))
            anchors.verticalCenter: parent.verticalCenter
            width: implicitWidth

            Binding on currentIndex {
                value: root.currentView === "home" ? 0
                    : root.currentView === "installed" ? 1
                    : root.currentView === "downloads" ? 2 : -1
            }

            Controls.TabButton {
                width: implicitWidth
                text: i18n("Home")
                onClicked: {
                    searchField.text = "";
                    root.homeRequested();
                }
            }

            Controls.TabButton {
                width: implicitWidth
                text: i18n("Installed")
                onClicked: root.installedRequested()
            }

            Controls.TabButton {
                width: implicitWidth
                text: i18n("Downloads")
                onClicked: root.downloadsRequested()

                CountBadge {
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.margins: 2
                    count: TransactionsModel.activeCount + TransactionsModel.updatesCount
                    accent: TransactionsModel.activeCount > 0
                        ? Kirigami.Theme.highlightColor
                        : Kirigami.Theme.negativeTextColor
                }
            }
        }

        RowLayout {
            id: rightGroup
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            spacing: Kirigami.Units.smallSpacing

            Kirigami.SearchField {
                id: searchField

                Layout.preferredWidth: activeFocus || text.length > 0
                    ? Kirigami.Units.gridUnit * 24
                    : Kirigami.Units.gridUnit * 16
                Layout.preferredHeight: Kirigami.Units.gridUnit * 1.8
                placeholderText: i18n("Search apps…")
                font.pointSize: Kirigami.Theme.defaultFont.pointSize + 1

                Behavior on Layout.preferredWidth {
                    NumberAnimation {
                        duration: Kirigami.Units.shortDuration
                        easing.type: Easing.InOutQuad
                    }
                }

                autoAccept: false

                property bool pendingFocusRestore: false

                onTextChanged: {
                    if (text.length === 0) {
                        liveSearchTimer.stop();
                        return;
                    }
                    if (!root.onSearchPage) {
                        liveSearchTimer.stop();
                        pendingFocusRestore = true;
                        root.searchRequested(text);
                    } else {
                        liveSearchTimer.restart();
                    }
                }

                onAccepted: {
                    liveSearchTimer.stop();
                    if (text.length === 0) {
                        return;
                    }
                    if (root.onSearchPage) {
                        root.searchTextEdited(text);
                    } else {
                        pendingFocusRestore = true;
                        root.searchRequested(text);
                    }
                }

                onActiveFocusChanged: {
                    if (!activeFocus && pendingFocusRestore) {
                        pendingFocusRestore = false;
                        focusRestoreTimer.start();
                    }
                }

                Keys.onEscapePressed: applicationWindow().pageStack.forceActiveFocus()

                Timer {
                    id: liveSearchTimer
                    interval: 220
                    onTriggered: root.searchTextEdited(searchField.text)
                }

                Timer {
                    id: focusRestoreTimer
                    interval: 0
                    onTriggered: searchField.forceActiveFocus()
                }
            }

            Controls.ToolButton {
                icon.name: "settings-configure-symbolic"
                onClicked: root.settingsRequested()
                Controls.ToolTip.text: i18n("Settings")
                Controls.ToolTip.visible: hovered
                Controls.ToolTip.delay: Kirigami.Units.toolTipDelay
            }
        }
    }

    Kirigami.Separator {
        parent: root
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
    }
}
