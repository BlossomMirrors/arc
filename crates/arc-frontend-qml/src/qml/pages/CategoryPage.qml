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

    Component.onCompleted: PackageListModel.searchCategory(categoryId)
}
