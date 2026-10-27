import QtQuick
import org.blossomos.arc
import org.kde.ki18n

ItemList {
    id: root

    property string categoryId: ""
    property string categoryLabel: ""
    property string categoryColor: ""
    property string categoryIcon: ""

    title: categoryLabel.length > 0 ? categoryLabel : KI18n.i18n("Category")
    emptyText: KI18n.i18n("No apps found in this category")
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
    }

    onVisibleChanged: if (!visible) categoryListModel.release()
}
