#include <QQmlApplicationEngine>
#include <KLocalizedQmlContext>

extern "C" void arc_setup_i18n(void *engine_ptr, const char *domain) {
    auto *engine = reinterpret_cast<QQmlApplicationEngine *>(engine_ptr);
    auto *ctx = KLocalization::setupLocalizedContext(engine);
    if (ctx) {
        ctx->setTranslationDomain(QString::fromUtf8(domain));
    }
}
