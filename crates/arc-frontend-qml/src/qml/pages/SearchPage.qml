import QtQuick
import org.blossomos.arc

ItemList {
    id: root

    property string query: ""

    title: i18n("Search")
    emptyText: i18n("Search for apps to install")
    showFilters: true

    onQueryChanged: if (query.length > 0) PackageListModel.search(query)
}
