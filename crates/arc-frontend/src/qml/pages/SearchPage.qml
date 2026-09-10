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

    Timer {
        property int attempts: 0
        interval: 2000
        running: true
        repeat: true
        onTriggered: {
            attempts += 1;
            if (root.query.length > 0) {
                searchListModel.search(root.query);
            }
            if (attempts >= 5) {
                stop();
            }
        }
    }
}
