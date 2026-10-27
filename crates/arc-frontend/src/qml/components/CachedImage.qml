import QtQuick

Image {
    id: root

    readonly property bool local: root.source.toString().startsWith("file:")

    asynchronous: !root.local
    cache: true
}
