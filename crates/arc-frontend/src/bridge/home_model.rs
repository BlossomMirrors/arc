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
        #[qproperty(bool, loading)]
        #[qproperty(QString, story_id, cxx_name = "storyId")]
        #[qproperty(QString, title)]
        #[qproperty(QString, banner_url, cxx_name = "bannerUrl")]
        #[qproperty(QString, blocks_json, cxx_name = "blocksJson")]
        type StoryController = super::StoryControllerRust;

        #[qinvokable]
        fn load(self: Pin<&mut StoryController>, story_id: QString);

        #[qinvokable]
        fn clear(self: Pin<&mut StoryController>);
    }

    impl cxx_qt::Threading for StoryController {}
}

use crate::runtime;
use cxx_qt::Threading;
use cxx_qt_lib::QString;
use std::pin::Pin;

pub struct StoryControllerRust {
    loading: bool,
    story_id: QString,
    title: QString,
    banner_url: QString,
    blocks_json: QString,
}

impl Default for StoryControllerRust {
    fn default() -> Self {
        Self {
            loading: true,
            story_id: QString::default(),
            title: QString::default(),
            banner_url: QString::default(),
            blocks_json: QString::from("[]"),
        }
    }
}

async fn display_blocks(
    proxy: Option<&libarc::ArcDaemonProxy<'static>>,
    mut blocks: Vec<crate::services::blocks::DescBlock>,
) -> String {
    let urls = blocks
        .iter_mut()
        .flat_map(|b| [&mut b.image_url, &mut b.app_icon_url])
        .collect();
    crate::services::images::resolve_urls(proxy, urls, libarc::ImageKind::Image, 0).await;
    serde_json::to_string(&blocks).unwrap_or_else(|_| "[]".into())
}

impl qobject::StoryController {
    pub fn load(mut self: Pin<&mut Self>, story_id: QString) {
        let id = story_id.to_string();
        self.as_mut().set_story_id(story_id);
        self.as_mut().set_title(QString::default());
        self.as_mut().set_banner_url(QString::default());
        self.as_mut().set_blocks_json(QString::from("[]"));
        self.as_mut().set_loading(true);

        let qt_thread = self.qt_thread();
        runtime::spawn(async move {
            let lookup_id = id.clone();
            let story =
                tokio::task::spawn_blocking(move || crate::services::stories::get(&lookup_id))
                    .await
                    .ok()
                    .flatten();
            let Some(story) = story else {
                let _ = qt_thread.queue(move |mut this| {
                    if this.story_id.to_string() == id {
                        this.as_mut().set_loading(false);
                    }
                });
                return;
            };

            let cached_blocks = story
                .blocks
                .clone()
                .filter(|b| crate::services::blocks::blocks_worth_caching(b));
            let header_id = id.clone();
            let title = story.title.clone();
            let proxy = runtime::proxy().await;
            let mut banner = story.banner_url.clone();
            crate::services::images::resolve_urls(
                proxy.as_ref(),
                vec![&mut banner],
                libarc::ImageKind::Image,
                0,
            )
            .await;
            let ready = match &cached_blocks {
                Some(blocks) => Some(display_blocks(proxy.as_ref(), blocks.clone()).await),
                None => None,
            };
            let _ = qt_thread.queue(move |mut this| {
                if this.story_id.to_string() != header_id {
                    return;
                }
                this.as_mut().set_title(QString::from(&title));
                this.as_mut().set_banner_url(QString::from(&banner));
                if let Some(json) = ready {
                    this.as_mut().set_blocks_json(QString::from(&json));
                    this.as_mut().set_loading(false);
                }
            });
            if cached_blocks.is_some() {
                return;
            }

            let mut blocks = crate::services::blocks::html_to_blocks(&story.body_html);
            crate::services::blocks::resolve_app_blocks(&mut blocks, proxy.as_ref()).await;
            let worth_caching = crate::services::blocks::blocks_worth_caching(&blocks);
            blocks.retain(|b| !b.is_app || !b.app_name.is_empty());
            if worth_caching {
                let store_id = id.clone();
                let store_blocks = blocks.clone();
                let _ = tokio::task::spawn_blocking(move || {
                    crate::services::stories::store_blocks(&store_id, store_blocks)
                })
                .await;
            }

            let display = display_blocks(proxy.as_ref(), blocks).await;
            let _ = qt_thread.queue(move |mut this| {
                if this.story_id.to_string() != id {
                    return;
                }
                this.as_mut().set_blocks_json(QString::from(&display));
                this.as_mut().set_loading(false);
            });
        });
    }

    pub fn clear(mut self: Pin<&mut Self>) {
        self.as_mut().set_story_id(QString::default());
        self.as_mut().set_title(QString::default());
        self.as_mut().set_banner_url(QString::default());
        self.as_mut().set_blocks_json(QString::from("[]"));
    }
}
