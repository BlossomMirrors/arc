#include <KDBusService>
#include <KWindowSystem>
#include <QByteArray>
#include <QCoreApplication>
#include <QGuiApplication>
#include <QStringList>
#include <QWindow>
#include <vector>

extern "C" void arc_single_instance(void (*on_activate)(const char *const *args, int count)) {
    QCoreApplication::setOrganizationDomain(QStringLiteral("blossomos.org"));
    QCoreApplication::setApplicationName(QStringLiteral("Arc"));
    auto *service = new KDBusService(KDBusService::Unique, QCoreApplication::instance());
    QObject::connect(service, &KDBusService::activateRequested, service, [on_activate](const QStringList &arguments, const QString &) {
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
        on_activate(pointers.data(), static_cast<int>(pointers.size()));
        for (QWindow *window : QGuiApplication::topLevelWindows()) {
            if (window->type() != Qt::Window && window->type() != Qt::Dialog) {
                continue;
            }
            if (window->windowState() & Qt::WindowMinimized) {
                window->setWindowStates(window->windowStates() & ~Qt::WindowMinimized);
            }
            window->show();
            window->raise();
            KWindowSystem::updateStartupId(window);
            KWindowSystem::activateWindow(window);
            break;
        }
    });
}
