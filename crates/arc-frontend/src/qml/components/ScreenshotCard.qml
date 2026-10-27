pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Effects
import org.blossomos.arc

Item {
    id: root

    property string modelData: ""
    property int index

    property Item maskSource: null
    property real visibleWidth: 0

    property var failedShots: ({})

    signal activated(int index)
    signal loadFailed(string url)

    readonly property string requestUrl: root.modelData

    readonly property bool failed: root.failedShots[root.requestUrl] === true

    SkeletonBlock {
        anchors.fill: parent
        visible: !root.failed
            && shotImage.status !== Image.Ready
            && shotImage.status !== Image.Error
    }

    CachedImage {
        id: shotImage
        anchors.fill: parent
        source: root.failedShots[root.requestUrl] || root.width <= 0 ? "" : root.requestUrl
        asynchronous: !shotImage.local || root.x >= root.visibleWidth
        sourceSize.width: root.width * 2
        sourceSize.height: root.height * 2
        fillMode: Image.PreserveAspectCrop

        layer.enabled: true
        layer.effect: MultiEffect {
            maskEnabled: true
            maskSource: root.maskSource
            maskThresholdMin: 0.5
            maskSpreadAtMin: 0.0
        }

        onStatusChanged: {
            if (status === Image.Error) {
                root.loadFailed(root.requestUrl);
            }
        }
    }

    HoverHandler {
        cursorShape: Qt.PointingHandCursor
    }

    TapHandler {
        enabled: shotImage.status !== Image.Error
        onTapped: root.activated(root.index)
    }
}
