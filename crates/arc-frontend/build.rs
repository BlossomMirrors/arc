use cxx_qt_build::{CxxQtBuilder, QmlModule};
use std::path::PathBuf;

fn build_locales() -> Option<PathBuf> {
    let locales = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("locales");
    println!("cargo::rerun-if-changed={}", locales.display());

    let out = PathBuf::from(std::env::var_os("OUT_DIR")?).join("locale");
    for entry in std::fs::read_dir(&locales).ok()?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("po") {
            continue;
        }
        let Some(lang) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let dir = out.join(lang).join("LC_MESSAGES");
        if std::fs::create_dir_all(&dir).is_err() {
            continue;
        }
        let status = std::process::Command::new("msgfmt")
            .arg("-o")
            .arg(dir.join("arc-frontend.mo"))
            .arg(&path)
            .status();
        match status {
            Ok(s) if s.success() => {}

            Ok(s) => println!("cargo::warning=msgfmt failed for {}: {s}", path.display()),
            Err(e) => {
                println!("cargo::warning=msgfmt unavailable, translations skipped: {e}");
                return None;
            }
        }
    }
    Some(out)
}

fn main() {
    let locale_dir = build_locales().unwrap_or_default();
    println!("cargo::rustc-env=ARC_LOCALE_DIR={}", locale_dir.display());

    println!("cargo::rustc-link-arg=-Wl,--no-as-needed");
    println!("cargo::rustc-link-arg=-lKF6I18nQml");
    println!("cargo::rustc-link-arg=-lKF6I18n");
    println!("cargo::rustc-link-arg=-Wl,--as-needed");

    let builder = CxxQtBuilder::new_qml_module(
        QmlModule::new("org.blossomos.arc").depend("QtQml.Models").qml_files([
            "src/qml/Main.qml",
            "src/qml/pages/HomePage.qml",
            "src/qml/pages/StoryPage.qml",
            "src/qml/pages/SearchPage.qml",
            "src/qml/pages/InstalledPage.qml",
            "src/qml/pages/CategoryPage.qml",
            "src/qml/pages/ListPage.qml",
            "src/qml/pages/DownloadsPage.qml",
            "src/qml/pages/DetailPage.qml",
            "src/qml/pages/SettingsPage.qml",
            "src/qml/pages/dialogs/AddRepoPage.qml",
            "src/qml/pages/dialogs/EulaPage.qml",
            "src/qml/pages/dialogs/InstallFlatpakrefPage.qml",
            "src/qml/pages/dialogs/InstallFilePage.qml",
            "src/qml/components/TopBar.qml",
            "src/qml/components/AppCard.qml",
            "src/qml/components/AppGridCard.qml",
            "src/qml/components/ListBadgeBar.qml",
            "src/qml/components/ListLabels.qml",
            "src/qml/components/AppHeader.qml",
            "src/qml/components/AppIcon.qml",
            "src/qml/components/HeroCard.qml",
            "src/qml/components/HeroCarousel.qml",
            "src/qml/components/CardCarousel.qml",
            "src/qml/components/CategoryCard.qml",
            "src/qml/components/ConveyorLoader.qml",
            "src/qml/components/LoadingOverlay.qml",
            "src/qml/components/RowLoadingPlaceholder.qml",
            "src/qml/components/SkeletonBlock.qml",
            "src/qml/components/ScreenshotCard.qml",
            "src/qml/components/ItemList.qml",
            "src/qml/components/ItemButtons.qml",
            "src/qml/components/ItemProgressBar.qml",
            "src/qml/components/DialogPage.qml",
            "src/qml/components/ProjectLinksGrid.qml",
        ]),
    )
    .files([
        "src/bridge/models.rs",
        "src/bridge/detail_controller.rs",
        "src/bridge/home_model.rs",
        "src/bridge/leftover_data_controller.rs",
        "src/bridge/list_controller.rs",
        "src/bridge/settings_controller.rs",
        "src/bridge/deeplink_controller.rs",
        "src/bridge/nav_controller.rs",
    ])
    .cpp_file("src/cxx/i18n_setup.cpp");

    let builder = unsafe {
        builder.cc_builder(|cc| {
            cc.include("/usr/include/KF6/KI18n");
            cc.include("/usr/include/qt6/QtQmlIntegration");
            cc.flag_if_supported("-Wno-sfinae-incomplete");
        })
    };

    builder.build();
}
