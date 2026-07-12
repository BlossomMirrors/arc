#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        #[qproperty(QString, kind)]
        #[qproperty(QString, pkg_id, cxx_name = "pkgId")]
        #[qproperty(QString, ref_title, cxx_name = "refTitle")]
        #[qproperty(QString, ref_source, cxx_name = "refSource")]
        #[qproperty(QString, repo_title, cxx_name = "repoTitle")]
        #[qproperty(QString, repo_url, cxx_name = "repoUrl")]
        #[qproperty(QString, repo_content, cxx_name = "repoContent")]
        #[qproperty(QString, file_path, cxx_name = "filePath")]
        #[qproperty(QString, file_name, cxx_name = "fileName")]
        #[qproperty(QString, file_pkg_name, cxx_name = "filePkgName")]
        #[qproperty(bool, file_is_appimage, cxx_name = "fileIsAppimage")]
        #[qproperty(bool, file_is_bundle, cxx_name = "fileIsBundle")]
        #[qproperty(bool, file_has_flatpak_alt, cxx_name = "fileHasFlatpakAlt")]
        #[qproperty(QString, file_flatpak_alt_id, cxx_name = "fileFlatpakAltId")]
        #[qproperty(QString, file_flatpak_alt_name, cxx_name = "fileFlatpakAltName")]
        type DeepLinkController = super::DeepLinkControllerRust;

        #[qinvokable]
        fn resolve(self: Pin<&mut DeepLinkController>);
    }

    impl cxx_qt::Threading for DeepLinkController {}
}

use crate::runtime;
use crate::services::deeplink::{self, LaunchIntent};
use cxx_qt::Threading;
use cxx_qt_lib::QString;
use std::pin::Pin;

#[derive(Default)]
pub struct DeepLinkControllerRust {
    kind: QString,
    pkg_id: QString,
    ref_title: QString,
    ref_source: QString,
    repo_title: QString,
    repo_url: QString,
    repo_content: QString,
    file_path: QString,
    file_name: QString,
    file_pkg_name: QString,
    file_is_appimage: bool,
    file_is_bundle: bool,
    file_has_flatpak_alt: bool,
    file_flatpak_alt_id: QString,
    file_flatpak_alt_name: QString,
}

impl qobject::DeepLinkController {
    pub fn resolve(self: Pin<&mut Self>) {
        let Some(intent) = deeplink::take_intent() else {
            return;
        };

        let qt_thread = self.qt_thread();
        runtime::spawn(async move {
            match intent {
                LaunchIntent::Detail { pkg_id } => {
                    qt_thread
                        .queue(move |mut this| {
                            this.as_mut().set_pkg_id(QString::from(&pkg_id));
                            this.as_mut().set_kind(QString::from("detail"));
                        })
                        .ok();
                }
                LaunchIntent::InstallFlatpakref { source, is_local_file } => {
                    let content = if is_local_file {
                        std::fs::read_to_string(&source).unwrap_or_default()
                    } else {
                        match reqwest::get(&source).await {
                            Ok(r) => r.text().await.unwrap_or_default(),
                            Err(_) => String::new(),
                        }
                    };
                    let (title, app_id, repo_url) = deeplink::parse_flatpakref(&content);

                    // flathub refs are known apps so skip the confirmation
                    let is_flathub = is_local_file
                        && (repo_url.contains("dl.flathub.org") || repo_url.contains("flathub.org"));

                    let install_source = if is_local_file { format!("file://{source}") } else { source };

                    qt_thread
                        .queue(move |mut this| {
                            if is_flathub {
                                this.as_mut().set_pkg_id(QString::from(&app_id));
                                this.as_mut().set_kind(QString::from("detail"));
                            } else {
                                this.as_mut().set_ref_title(QString::from(&title));
                                this.as_mut().set_ref_source(QString::from(&install_source));
                                this.as_mut().set_kind(QString::from("flatpakref"));
                            }
                        })
                        .ok();
                }
                LaunchIntent::AddRepo { content } => {
                    let (title, url) = deeplink::parse_flatpakrepo(&content);
                    qt_thread
                        .queue(move |mut this| {
                            this.as_mut().set_repo_title(QString::from(&title));
                            this.as_mut().set_repo_url(QString::from(&url));
                            this.as_mut().set_repo_content(QString::from(&content));
                            this.as_mut().set_kind(QString::from("addrepo"));
                        })
                        .ok();
                }
                LaunchIntent::InstallFile { path, file_name, pkg_name, is_appimage, is_bundle } => {
                    let alt = if !is_appimage && !is_bundle {
                        find_flatpak_alternative(&pkg_name).await
                    } else {
                        None
                    };

                    qt_thread
                        .queue(move |mut this| {
                            this.as_mut().set_file_path(QString::from(&path));
                            this.as_mut().set_file_name(QString::from(&file_name));
                            this.as_mut().set_file_pkg_name(QString::from(&pkg_name));
                            this.as_mut().set_file_is_appimage(is_appimage);
                            this.as_mut().set_file_is_bundle(is_bundle);
                            if let Some((id, name)) = alt {
                                this.as_mut().set_file_flatpak_alt_id(QString::from(&id));
                                this.as_mut().set_file_flatpak_alt_name(QString::from(&name));
                                this.as_mut().set_file_has_flatpak_alt(true);
                            }
                            this.as_mut().set_kind(QString::from("installfile"));
                        })
                        .ok();
                }
            }
        });
    }
}

async fn find_flatpak_alternative(pkg_name: &str) -> Option<(String, String)> {
    let proxy = runtime::proxy().await?;
    let json = proxy.search(pkg_name).await.ok()?;
    let pkgs: Vec<libarc::Package> = serde_json::from_str(&json).ok()?;
    pkgs.into_iter()
        .find(|p| p.provider == libarc::Provider::Flatpak)
        .map(|p| (p.id, p.name))
}
