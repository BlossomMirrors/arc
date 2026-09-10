import QtQuick
import org.kde.ki18n

QtObject {
    id: root

    readonly property var labels: ({
            "office": KI18n.i18n("Office"),
            "creative": KI18n.i18n("Creative"),
            "chat": KI18n.i18n("Chat"),
            "gaming": KI18n.i18n("Gaming"),
            "browser": KI18n.i18n("Browser"),
            "music": KI18n.i18n("Music"),
            "code": KI18n.i18n("Code")
        })

    function labelFor(slug) {
        return root.labels[slug] ?? slug;
    }
}
