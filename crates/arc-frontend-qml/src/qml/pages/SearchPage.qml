import QtQuick
import org.blossomos.arc

ItemList {
    id: root

    property string query: ""

    title: i18n("Search")
    emptyText: i18n("Search for apps to install")

    Component.onCompleted: if (query.length > 0) PackageListModel.search(query)
}
