import QtQuick
import org.blossomos.arc
import org.kde.kirigami as Kirigami

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
