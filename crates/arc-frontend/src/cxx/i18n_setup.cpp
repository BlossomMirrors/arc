#include <QQmlApplicationEngine>
#include <QtGlobal>
#include <QByteArray>
#include <QIcon>
#include <KLocalizedQmlContext>
#include <KLocalizedString>
#include <cstdio>
#include <cstdlib>

static void arc_message_handler(QtMsgType type, const QMessageLogContext &context, const QString &msg) {
    const char *level = "INFO";
    switch (type) {
    case QtDebugMsg: level = "DEBUG"; break;
    case QtInfoMsg: level = "INFO"; break;
    case QtWarningMsg: level = "WARN"; break;
    case QtCriticalMsg: level = "ERROR"; break;
    case QtFatalMsg: level = "FATAL"; break;
    }
    const QByteArray text = msg.toLocal8Bit();
    if (context.file != nullptr && context.line > 0) {
        fprintf(stderr, "[qml %s] %s (%s:%d)\n", level, text.constData(), context.file, context.line);
    } else {
        fprintf(stderr, "[qml %s] %s\n", level, text.constData());
    }
    fflush(stderr);
    if (type == QtFatalMsg) {
        abort();
    }
}

extern "C" void arc_install_message_handler() {
    qInstallMessageHandler(arc_message_handler);
}

extern "C" void arc_add_locale_dir(const char *domain, const char *dir) {
    const QString path = QString::fromUtf8(dir);
    if (!path.isEmpty()) {
        KLocalizedString::addDomainLocaleDir(QByteArray(domain), path);
    }
}

extern "C" void arc_setup_i18n(void *engine_ptr, const char *domain) {
    auto *engine = reinterpret_cast<QQmlApplicationEngine *>(engine_ptr);
    const QString domainName = QString::fromUtf8(domain);

    auto *ctx = KLocalization::setupLocalizedContext(engine);
    if (ctx) {
        ctx->setTranslationDomain(domainName);
    }

    auto *singleton = engine->singletonInstance<KLocalizedQmlContext *>(
        QStringLiteral("org.kde.ki18n"), QStringLiteral("KI18n"));
    if (singleton) {
        singleton->setTranslationDomain(domainName);
    }
}

extern "C" bool arc_engine_has_root(void *engine_ptr) {
    auto *engine = reinterpret_cast<QQmlApplicationEngine *>(engine_ptr);
    return engine != nullptr && !engine->rootObjects().isEmpty();
}

extern "C" void arc_set_icon_theme(const char *name) {
    QIcon::setThemeName(QString::fromUtf8(name));
}
