import QtQuick
import org.blossomos.arc
import org.kde.kirigami as Kirigami
import org.kde.ki18n

ItemList {
    id: root

    title: KI18n.i18n("Installed")

    emptyText: KI18n.i18n("Nothing installed yet")
    showSearch: true

    onSearchEdited: query => installListModel.setSearchText(query)

    PackageListModel {
        id: installListModel
    }

    packageListModel: installListModel

    function load() { installListModel.loadInstalled() }
}
