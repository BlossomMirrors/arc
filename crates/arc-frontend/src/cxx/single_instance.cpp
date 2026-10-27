#include <KDBusService>
#include <KWindowSystem>
#include <QByteArray>
#include <QCoreApplication>
#include <QDBusArgument>
#include <QDBusConnection>
#include <QDBusMessage>
#include <QDBusMetaType>
#include <QDBusVirtualObject>
#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <QQuickItem>
#include <QQuickWindow>
#include <QStringList>
#include <QTimer>
#include <QUrl>
#include <QWindow>
#include <malloc.h>
#include <vector>

using ActivateFn = bool (*)(const char *const *args, int count);

static QWindow *main_window() {
    for (QWindow *window : QGuiApplication::topLevelWindows()) {
        if (window->type() == Qt::Window || window->type() == Qt::Dialog) {
            return window;
        }
    }
    return nullptr;
}

static int polish_tree(QQuickItem *item) {
    item->ensurePolished();
    int count = 1;
    const auto children = item->childItems();
    for (QQuickItem *child : children) {
        count += polish_tree(child);
    }
    return count;
}

static void lay_out(QQuickWindow *window) {
    int previous = -1;
    for (int pass = 0; pass < 10; ++pass) {
        QCoreApplication::processEvents();
        const int count = polish_tree(window->contentItem());
        if (count == previous) {
            break;
        }
        previous = count;
    }
}

extern "C" void arc_show_window() {
    QWindow *window = main_window();
    if (!window) {
        return;
    }
    if (auto *quick = qobject_cast<QQuickWindow *>(window); quick && !quick->isVisible()) {
        lay_out(quick);
    }
    if (window->windowState() & Qt::WindowMinimized) {
        window->setWindowStates(window->windowStates() & ~Qt::WindowMinimized);
    }
    window->show();
    window->raise();
    KWindowSystem::updateStartupId(window);
    KWindowSystem::activateWindow(window);
}

extern "C" void arc_show_window_later(int delay_ms) {
    QTimer::singleShot(delay_ms, QCoreApplication::instance(), [] {
        QWindow *window = main_window();
        if (window && !window->isVisible()) {
            arc_show_window();
        }
    });
}

static void activate(ActivateFn on_activate, const QStringList &arguments) {
    // a standby start at login must not open the window
    if (arguments.contains(QStringLiteral("--standby"))) {
        return;
    }
    std::vector<QByteArray> utf8;
    utf8.reserve(arguments.size());
    for (const QString &argument : arguments) {
        utf8.push_back(argument.toUtf8());
    }
    std::vector<const char *> pointers;
    pointers.reserve(utf8.size());
    for (const QByteArray &bytes : utf8) {
        pointers.push_back(bytes.constData());
    }
    // a linked page shows the window itself once it is ready
    const bool page_shows_window = on_activate(pointers.data(), static_cast<int>(pointers.size()));
    QWindow *window = main_window();
    if (!page_shows_window || (window && window->isVisible())) {
        arc_show_window();
    }
}

// the link handler desktop file needs its own bus name to be dbus activatable so the running window answers it too
class HandlerApplication : public QDBusVirtualObject {
public:
    explicit HandlerApplication(ActivateFn on_activate, QObject *parent)
        : QDBusVirtualObject(parent), m_on_activate(on_activate) {}

    QString introspect(const QString &) const override {
        // that long xml inline string is fine as its weird cpp code anyway
        return QStringLiteral(
            "<interface name=\"org.freedesktop.Application\">"
            "<method name=\"Activate\"><arg type=\"a{sv}\" direction=\"in\"/></method>"
            "<method name=\"Open\"><arg type=\"as\" direction=\"in\"/><arg type=\"a{sv}\" direction=\"in\"/></method>"
            "<method name=\"ActivateAction\"><arg type=\"s\" direction=\"in\"/><arg type=\"av\" direction=\"in\"/><arg type=\"a{sv}\" direction=\"in\"/></method>"
            "</interface>");
    }

    bool handleMessage(const QDBusMessage &message, const QDBusConnection &connection) override {
        if (message.interface() != QStringLiteral("org.freedesktop.Application")) {
            return false;
        }
        const QList<QVariant> args = message.arguments();
        QStringList arguments;
        if (message.member() == QStringLiteral("Open") && !args.isEmpty()) {
            for (const QString &uri : args.at(0).toStringList()) {
                const QUrl url(uri);
                arguments << (url.isLocalFile() ? url.toLocalFile() : uri);
            }
        } else if (message.member() != QStringLiteral("Activate") && message.member() != QStringLiteral("ActivateAction")) {
            return false;
        }
        if (!args.isEmpty()) {
            const QVariantMap platform = qdbus_cast<QVariantMap>(args.last());
            const QString token = platform.value(QStringLiteral("activation-token")).toString();
            if (!token.isEmpty()) {
                KWindowSystem::setCurrentXdgActivationToken(token);
            }
        }
        connection.send(message.createReply());
        activate(m_on_activate, arguments);
        return true;
    }

private:
    ActivateFn m_on_activate;
};

extern "C" void arc_single_instance(ActivateFn on_activate) {
    QCoreApplication::setOrganizationDomain(QStringLiteral("blossomos.org"));
    QCoreApplication::setApplicationName(QStringLiteral("Arc"));
    auto *service = new KDBusService(KDBusService::Unique, QCoreApplication::instance());
    QObject::connect(service, &KDBusService::activateRequested, service, [on_activate](const QStringList &arguments, const QString &) {
        activate(on_activate, arguments);
    });
    // the launcher hands over files and links directly when the desktop file is dbus activatable
    QObject::connect(service, &KDBusService::openRequested, service, [on_activate](const QList<QUrl> &urls) {
        QStringList arguments;
        for (const QUrl &url : urls) {
            arguments << (url.isLocalFile() ? url.toLocalFile() : url.toString());
        }
        activate(on_activate, arguments);
    });
    QObject::connect(service, &KDBusService::activateActionRequested, service, [on_activate](const QString &, const QVariant &) {
        activate(on_activate, {});
    });
    QDBusConnection bus = QDBusConnection::sessionBus();
    bus.registerVirtualObject(QStringLiteral("/org/blossomos/Arc/Handler"), new HandlerApplication(on_activate, service));
    bus.registerService(QStringLiteral("org.blossomos.Arc.Handler"));
}

extern "C" void arc_prepare_hidden(void *engine_ptr) {
    auto *engine = static_cast<QQmlApplicationEngine *>(engine_ptr);
    // runs once the event loop is up so queued work from loading has happened
    QTimer::singleShot(0, engine, [engine] {
        const auto roots = engine->rootObjects();
        for (QObject *root : roots) {
            auto *window = qobject_cast<QQuickWindow *>(root);
            if (!window) {
                continue;
            }
            window->create();
            lay_out(window);
        }
        engine->collectGarbage();
        malloc_trim(0);
    });
}
