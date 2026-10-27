import QtQuick
import org.blossomos.arc
import org.kde.ki18n

ItemList {
    id: root

    property string query: ""

    title: KI18n.i18n("Search")
    emptyText: KI18n.i18n("Search for apps to install")
    showFilters: true
    showSearch: true
    searchQuery: root.query

    PackageListModel {
        id: searchListModel
    }

    packageListModel: searchListModel

    onQueryChanged: if (query.length > 0) searchListModel.search(query)

    onSearchEdited: edited => {
        if (edited === root.query) {
            return;
        }
        root.query = edited;
        NavController.updateQuery(edited);
    }

    function release() { searchListModel.release() }
    function restore() { searchListModel.restore() }
}
