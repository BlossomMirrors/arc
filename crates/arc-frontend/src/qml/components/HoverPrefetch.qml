import QtQuick
import org.blossomos.arc

Timer {
    id: root

    property string pkgId: ""
    property string categoryId: ""
    property string listSlug: ""
    property bool hovered: false

    interval: 200
    running: root.hovered && (root.pkgId.length > 0 || root.categoryId.length > 0 || root.listSlug.length > 0)
    onTriggered: {
        if (root.pkgId.length > 0)
            DetailController.prefetch(root.pkgId);
        if (root.categoryId.length > 0)
            ListController.prefetchCategory(root.categoryId);
        if (root.listSlug.length > 0)
            ListController.prefetch(root.listSlug);
    }
}
