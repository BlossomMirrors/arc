import QtQuick
import org.blossomos.arc

ItemList {
    id: root

    title: i18n("Installed")
    emptyText: i18n("Nothing installed yet")
    markInstalled: false

    PackageListModel {
        id: installListModel
    }

    packageListModel: installListModel

    function load() { installListModel.loadInstalled() }
}
