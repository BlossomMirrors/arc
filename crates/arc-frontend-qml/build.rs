use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    println!("cargo::rustc-link-arg=-Wl,--no-as-needed");
    println!("cargo::rustc-link-arg=-lKF6I18nQml");
    println!("cargo::rustc-link-arg=-lKF6I18n");
    println!("cargo::rustc-link-arg=-Wl,--as-needed");

    let builder = CxxQtBuilder::new_qml_module(
        QmlModule::new("org.blossomos.arc").qml_files([
            "src/qml/Main.qml",
            "src/qml/pages/HomePage.qml",
            "src/qml/pages/StoryPage.qml",
            "src/qml/pages/SearchPage.qml",
            "src/qml/pages/InstalledPage.qml",
            "src/qml/pages/CategoryPage.qml",
            "src/qml/pages/DownloadsPage.qml",
            "src/qml/pages/DetailPage.qml",
            "src/qml/pages/SettingsPage.qml",
            "src/qml/pages/dialogs/AddRepoPage.qml",
            "src/qml/pages/dialogs/EulaPage.qml",
            "src/qml/pages/dialogs/InstallFlatpakrefPage.qml",
            "src/qml/pages/dialogs/InstallFilePage.qml",
            "src/qml/components/TopBar.qml",
            "src/qml/components/AppCard.qml",
            "src/qml/components/AppHeader.qml",
            "src/qml/components/AppIcon.qml",
            "src/qml/components/HeroCard.qml",
            "src/qml/components/HeroCarousel.qml",
            "src/qml/components/CardCarousel.qml",
            "src/qml/components/CategoryCard.qml",
            "src/qml/components/ConveyorLoader.qml",
            "src/qml/components/SkeletonBlock.qml",
            "src/qml/components/ItemList.qml",
            "src/qml/components/ItemButtons.qml",
            "src/qml/components/ItemProgressBar.qml",
            "src/qml/components/DialogPage.qml",
        ]),
    )
    .files([
        "src/bridge/models.rs",
        "src/bridge/detail_controller.rs",
        "src/bridge/home_model.rs",
        "src/bridge/settings_controller.rs",
        "src/bridge/deeplink_controller.rs",
        "src/bridge/nav_controller.rs",
    ])
    .cpp_file("src/cxx/i18n_setup.cpp");

    let builder = unsafe {
        builder.cc_builder(|cc| {
            cc.include("/usr/include/KF6/KI18n");
            cc.include("/usr/include/qt6/QtQmlIntegration");
        })
    };

    builder.build();
}
