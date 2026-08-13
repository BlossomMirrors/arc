import QtQuick
import org.blossomos.arc

ItemList {
    id: root

    property string query: ""

    title: i18n("Search")
    emptyText: i18n("Search for apps to install")
    showFilters: true

    PackageListModel {
        id: searchListModel
    }

    packageListModel: searchListModel

    onQueryChanged: if (query.length > 0) searchListModel.search(query)

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
