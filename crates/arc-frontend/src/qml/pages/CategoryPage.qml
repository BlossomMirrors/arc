import QtQuick
import org.blossomos.arc

ItemList {
    id: root

    required property string categoryId
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

    Component.onCompleted: categoryListModel.searchCategory(categoryId)

    Timer {
        property int attempts: 0
        interval: 2000
        running: true
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
