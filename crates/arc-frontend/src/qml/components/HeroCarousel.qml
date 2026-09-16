pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls as Controls
import QtQuick.Effects
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import org.blossomos.arc

Item {
    id: root

    implicitHeight: Math.max(350, Window.height * 0.62)

    property var items: []

    signal storyActivated(int storyIndex)
    signal appActivated(string pkgId)

    property alias currentIndex: view.currentIndex

    property real slideProgress: 0
    property bool advancing: false

    function restartAutoplay() {
        autoplay.stop();
        root.slideProgress = 0;
        if (root.items.length > 1) {
            autoplay.start();
        }
    }

    onItemsChanged: {
        view.positionViewAtBeginning();
        root.restartAutoplay();
    }

    Component.onCompleted: root.restartAutoplay()

    HoverHandler {
        id: hover
    }

    SequentialAnimation {
        id: autoplay
        loops: Animation.Infinite
        paused: hover.hovered || view.moving || view.dragging

        NumberAnimation {
            target: root
            property: "slideProgress"
            from: 0
            to: 1
            duration: 5000
        }

        ScriptAction {
            script: {
                root.advancing = true;
                view.currentIndex = (view.currentIndex + 1) % root.items.length;
                root.advancing = false;
            }
        }
    }

    Connections {
        target: view
        function onCurrentIndexChanged() {
            if (!root.advancing) {
                root.restartAutoplay();
            }
        }
    }

    ListView {
        id: view
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.bottom: indicatorBackground.top
        anchors.bottomMargin: Kirigami.Units.largeSpacing

        model: root.items

        orientation: ListView.Horizontal
        snapMode: ListView.SnapOneItem
        highlightRangeMode: ListView.StrictlyEnforceRange
        preferredHighlightBegin: 0
        preferredHighlightEnd: width
        highlightMoveDuration: 400
        highlightMoveVelocity: -1
        boundsBehavior: Flickable.StopAtBounds
        cacheBuffer: width * 2
        clip: true

        delegate: Item {
            id: slide

            required property var modelData
            required property int index

            readonly property var apps: slide.modelData.apps ?? []

            width: view.width
            height: view.height

            property real bannerLuminance: -1
            readonly property bool darkBanner: slide.bannerLuminance < 140
            readonly property color fgColor: slide.darkBanner ? "white" : "black"
            readonly property color fgColorMuted: slide.darkBanner ? Qt.rgba(1, 1, 1, 0.85) : Qt.rgba(0, 0, 0, 0.75)

            Image {
                id: bannerImage
                anchors.fill: parent
                source: slide.modelData.is_story ? slide.modelData.banner_url : ""
                fillMode: Image.PreserveAspectCrop
                asynchronous: true
                visible: slide.modelData.is_story && slide.modelData.banner_url.length > 0
            }

            Timer {
                interval: 400
                repeat: true
                running: slide.modelData.is_story && slide.bannerLuminance < 0
                triggeredOnStart: true
                onTriggered: brightnessSampler.requestPaint()
            }

            Canvas {
                id: brightnessSampler
                width: 8
                height: 8
                opacity: 0
                z: -1

                onPaint: {
                    var ctx = getContext("2d");
                    ctx.clearRect(0, 0, width, height);
                    ctx.drawImage(bannerImage, 0, 0, width, height);
                    var data = ctx.getImageData(0, 0, width, height).data;
                    var alphaSum = 0;
                    var total = 0;
                    for (var i = 0; i < data.length; i += 4) {
                        alphaSum += data[i + 3];
                        total += 0.299 * data[i] + 0.587 * data[i + 1] + 0.114 * data[i + 2];
                    }
                    if (alphaSum === 0) {
                        return;
                    }
                    slide.bannerLuminance = total / (data.length / 4);
                }
            }

            readonly property Image banner: bannerImage

            Kirigami.Icon {
                visible: !slide.modelData.is_story && slide.modelData.icon_url.length > 0
                source: slide.modelData.icon_url
                width: 120
                height: 120
                anchors.horizontalCenter: parent.horizontalCenter
                y: 60
            }

            MouseArea {
                anchors.fill: parent
                enabled: slide.index === view.currentIndex
                cursorShape: Qt.PointingHandCursor
                onClicked: {
                    if (view.moving) {
                        return;
                    }
                    if (slide.modelData.is_story) {
                        root.storyActivated(slide.modelData.story_index);
                    } else {
                        root.appActivated(slide.modelData.id);
                    }
                }
            }

            ColumnLayout {
                id: leftColumn

                anchors.left: parent.left
                anchors.bottom: parent.bottom
                anchors.leftMargin: 24
                anchors.bottomMargin: slide.apps.length > 0 ? 0 : 36
                width: Math.min(slide.width * 0.52, Kirigami.Units.gridUnit * 34)
                spacing: 6

                Kirigami.Heading {
                    Layout.fillWidth: true
                    level: 1
                    text: slide.modelData.title
                    color: slide.fgColor
                    maximumLineCount: 2
                    wrapMode: Text.WordWrap
                }

                Controls.Label {
                    Layout.fillWidth: true
                    visible: slide.modelData.body.length > 0
                    text: slide.modelData.body
                    color: slide.fgColorMuted
                    maximumLineCount: 2
                    wrapMode: Text.WordWrap
                }

                Rectangle {
                    Layout.topMargin: Kirigami.Units.smallSpacing
                    Layout.preferredWidth: parent.width * 0.75
                    Layout.preferredHeight: 1
                    color: slide.darkBanner ? Qt.rgba(1, 1, 1, 0.35) : Qt.rgba(0, 0, 0, 0.25)
                    visible: slide.apps.length > 0
                }

                Item {
                    id: appPanel

                    readonly property real rowHeight: Kirigami.Units.gridUnit * 2.8
                    readonly property int visibleRows: 3
                    readonly property real radius: 12

                    Layout.topMargin: Kirigami.Units.smallSpacing
                    Layout.fillWidth: true
                    Layout.preferredHeight: appPanel.visibleRows * appPanel.rowHeight
                        + appPanel.visibleRows * Kirigami.Units.smallSpacing
                        + Kirigami.Units.smallSpacing
                    visible: slide.apps.length > 0

                    Item {
                        anchors.fill: parent
                        layer.enabled: true
                        layer.effect: MultiEffect {
                            maskEnabled: true
                            maskSource: panelMask
                            maskThresholdMin: 0.5
                            maskSpreadAtMin: 0.0
                        }

                        ShaderEffectSource {
                            id: panelBackdrop
                            anchors.fill: parent
                            visible: false
                            live: true
                            hideSource: false
                            sourceItem: slide.banner
                            sourceRect: Qt.rect(leftColumn.x + appPanel.x,
                                leftColumn.y + appPanel.y,
                                appPanel.width,
                                appPanel.height)
                        }

                        MultiEffect {
                            anchors.fill: parent
                            source: panelBackdrop
                            visible: slide.banner.status === Image.Ready
                            blurEnabled: true
                            blur: 1.0
                            blurMax: 64
                            autoPaddingEnabled: false
                        }

                        Rectangle {
                            anchors.fill: parent
                            color: Qt.rgba(1, 1, 1, 0.18)
                        }
                    }

                    Item {
                        id: panelMask
                        anchors.fill: parent
                        visible: false
                        layer.enabled: true

                        Rectangle {
                            width: parent.width
                            height: parent.height + radius
                            radius: appPanel.radius
                        }
                    }

                    ListView {
                        anchors.fill: parent
                        anchors.margins: Kirigami.Units.smallSpacing
                        clip: true
                        model: slide.apps
                        spacing: Kirigami.Units.smallSpacing
                        boundsBehavior: Flickable.StopAtBounds

                        Controls.ScrollBar.vertical: Controls.ScrollBar {
                            Kirigami.Theme.inherit: false
                            Kirigami.Theme.colorSet: Kirigami.Theme.Complementary
                            policy: Controls.ScrollBar.AsNeeded
                        }

                        delegate: Item {
                            id: appRow

                            required property var modelData

                            width: ListView.view.width
                            height: appPanel.rowHeight

                            Rectangle {
                                anchors.fill: parent
                                radius: Kirigami.Units.cornerRadius
                                color: slide.darkBanner
                                    ? Qt.rgba(1, 1, 1, rowHover.hovered ? 0.18 : 0.08)
                                    : Qt.rgba(0, 0, 0, rowHover.hovered ? 0.14 : 0.06)

                                Behavior on color {
                                    ColorAnimation { duration: Kirigami.Units.shortDuration }
                                }
                            }

                            HoverHandler {
                                id: rowHover
                                cursorShape: Qt.PointingHandCursor
                            }

                            TapHandler {
                                onTapped: root.appActivated(appRow.modelData.id)
                            }

                            RowLayout {
                                anchors.fill: parent
                                anchors.leftMargin: Kirigami.Units.largeSpacing * 2
                                anchors.rightMargin: Kirigami.Units.largeSpacing * 2
                                spacing: Kirigami.Units.smallSpacing

                                AppIcon {
                                    Layout.preferredWidth: Kirigami.Units.iconSizes.medium
                                    Layout.preferredHeight: Kirigami.Units.iconSizes.medium
                                    Layout.alignment: Qt.AlignVCenter
                                    source: appRow.modelData.icon_url
                                }

                                ColumnLayout {
                                    Layout.fillWidth: true
                                    Layout.minimumWidth: 0
                                    Layout.alignment: Qt.AlignVCenter
                                    spacing: 0

                                    Controls.Label {
                                        Layout.fillWidth: true
                                        Layout.minimumWidth: 0
                                        text: appRow.modelData.name
                                        color: slide.fgColor
                                        font.bold: true
                                        maximumLineCount: 1
                                        elide: Text.ElideRight
                                    }

                                    Controls.Label {
                                        Layout.fillWidth: true
                                        Layout.minimumWidth: 0
                                        visible: appRow.modelData.summary.length > 0
                                        text: appRow.modelData.summary
                                        color: slide.fgColorMuted
                                        font.pointSize: Kirigami.Theme.smallFont.pointSize
                                        maximumLineCount: 1
                                        elide: Text.ElideRight
                                    }
                                }

                                ItemButtons {
                                    Layout.alignment: Qt.AlignVCenter
                                    Kirigami.Theme.inherit: false
                                    Kirigami.Theme.colorSet: Kirigami.Theme.Complementary
                                    pkgId: appRow.modelData.id
                                    name: appRow.modelData.name
                                    iconUrl: appRow.modelData.icon_url
                                    installed: appRow.modelData.installed
                                    mode: "install"
                                    allowRemove: false
                                    flat: true
                                    textColor: slide.fgColor
                                    backgroundColor: Qt.rgba(1, 1, 1, 0.12)
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Rectangle {
        id: indicatorBackground
        Kirigami.Theme.colorSet: Kirigami.Theme.View
        Kirigami.Theme.inherit: false

        implicitWidth: dots.implicitWidth + Kirigami.Units.largeSpacing
        implicitHeight: dots.implicitHeight + Kirigami.Units.smallSpacing * 2
        radius: height / 2

        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom

        color: Qt.alpha(Kirigami.Theme.backgroundColor, 0.75)

        Row {
            id: dots
            anchors.centerIn: parent
            spacing: Kirigami.Units.smallSpacing

            Repeater {
                model: root.items.length

                delegate: Rectangle {
                    id: dot
                    required property int index
                    readonly property bool active: dot.index === view.currentIndex

                    width: dot.active ? 30 : 8
                    height: 8
                    radius: height / 2
                    color: Kirigami.Theme.backgroundColor
                    border.width: 1
                    border.color: Qt.alpha(Kirigami.Theme.textColor, 0.08)
                    Behavior on width {
                        NumberAnimation {
                            duration: 200
                            easing.type: Easing.InOutQuad
                        }
                    }

                    Rectangle {
                        anchors.left: parent.left
                        anchors.top: parent.top
                        anchors.bottom: parent.bottom
                        radius: parent.radius
                        color: Kirigami.Theme.highlightColor
                        width: dot.active
                            ? dot.height + (dot.width - dot.height)
                                * (autoplay.running ? root.slideProgress : 1)
                            : 0
                    }

                    TapHandler {
                        enabled: !dot.active
                        margin: Kirigami.Units.smallSpacing * 2
                        onTapped: view.currentIndex = dot.index
                    }
                    HoverHandler {
                        enabled: !dot.active
                        margin: Kirigami.Units.smallSpacing * 2
                        cursorShape: Qt.PointingHandCursor
                    }
                }
            }
        }
    }
}
