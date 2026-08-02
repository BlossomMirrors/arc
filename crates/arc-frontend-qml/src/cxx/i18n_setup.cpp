#include <QQmlApplicationEngine>
#include <QtGlobal>
#include <QByteArray>
#include <KLocalizedQmlContext>
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

extern "C" void arc_setup_i18n(void *engine_ptr, const char *domain) {
    auto *engine = reinterpret_cast<QQmlApplicationEngine *>(engine_ptr);
    auto *ctx = KLocalization::setupLocalizedContext(engine);
    if (ctx) {
        ctx->setTranslationDomain(QString::fromUtf8(domain));
    }
}

extern "C" bool arc_engine_has_root(void *engine_ptr) {
    auto *engine = reinterpret_cast<QQmlApplicationEngine *>(engine_ptr);
    return engine != nullptr && !engine->rootObjects().isEmpty();
}
