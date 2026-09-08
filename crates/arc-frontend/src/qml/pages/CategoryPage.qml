import QtQuick
import org.blossomos.arc

ItemList {
    id: root

    property string categoryId: ""
    property string categoryLabel: ""
    property string categoryColor: ""
    property string categoryIcon: ""

    title: categoryLabel.length > 0 ? categoryLabel : i18n("Category")
    emptyText: i18n("No apps found in this category")
    headerColor: categoryColor
    headerIcon: categoryIcon

    PackageListModel {
        id: categoryListModel
    }

    packageListModel: categoryListModel

    function openCategory(id, label, color, icon) {
        root.categoryId = id;
        root.categoryLabel = label ?? "";
        root.categoryColor = color ?? "";
        root.categoryIcon = icon ?? "";
        categoryListModel.searchCategory(id);
        retryTimer.attempts = 0;
        retryTimer.restart();
    }

    Timer {
        id: retryTimer
        property int attempts: 0
        interval: 2000
        repeat: true
        onTriggered: {
            attempts += 1;
            categoryListModel.searchCategory(root.categoryId);
            if (attempts >= 5) {
                stop();
            }
        }
    }
}
