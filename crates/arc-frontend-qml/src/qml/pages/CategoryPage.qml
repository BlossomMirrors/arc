import QtQuick
import org.blossomos.arc

ItemList {
    id: root

    required property string categoryId
    property string categoryLabel: ""

    title: categoryLabel.length > 0 ? categoryLabel : i18n("Category")
    emptyText: i18n("No apps found in this category")

    Component.onCompleted: PackageListModel.searchCategory(categoryId)
}
