import QtQuick
import org.blossomos.arc

ItemList {
    id: root

    title: i18n("Installed")
    emptyText: i18n("Nothing installed yet")
    markInstalled: false

    function load() { PackageListModel.loadInstalled() }
}
