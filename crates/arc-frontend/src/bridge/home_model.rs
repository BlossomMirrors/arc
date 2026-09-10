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

impl qobject::StoryController {
    pub fn load(mut self: Pin<&mut Self>, story_id: QString) {
        let id = story_id.to_string();
        self.as_mut().set_story_id(story_id);

        let Some(story) = crate::services::stories::get(&id) else {
            self.as_mut().set_title(QString::default());
            self.as_mut().set_banner_url(QString::default());
            self.as_mut().set_blocks_json(QString::from("[]"));
            self.as_mut().set_loading(false);
            return;
        };

        self.as_mut().set_title(QString::from(&story.title));
        self.as_mut().set_banner_url(QString::from(&story.banner_url));

        let cached_blocks = serde_json::from_str::<Vec<crate::services::blocks::DescBlock>>(
            &story.blocks_json,
        )
        .ok()
        .filter(|b| crate::services::blocks::blocks_worth_caching(b));

        if cached_blocks.is_some() {
            self.as_mut().set_blocks_json(QString::from(&story.blocks_json));
            self.as_mut().set_loading(false);
            return;
        }

        self.as_mut().set_blocks_json(QString::from("[]"));
        self.as_mut().set_loading(true);

        let qt_thread = self.qt_thread();
        runtime::spawn(async move {
            let mut blocks = crate::services::blocks::html_to_blocks(&story.body_html);
            let proxy = runtime::proxy().await;
            crate::services::blocks::resolve_app_blocks(&mut blocks, proxy.as_ref()).await;
            let worth_caching = crate::services::blocks::blocks_worth_caching(&blocks);
            blocks.retain(|b| !b.is_app || !b.app_name.is_empty());
            let json = serde_json::to_string(&blocks).unwrap_or_else(|_| "[]".into());
            if worth_caching {
                crate::services::stories::store_blocks(&id, &json);
            }

            qt_thread
                .queue(move |mut this| {
                    if this.story_id.to_string() != id {
                        return;
                    }
                    this.as_mut().set_blocks_json(QString::from(&json));
                    this.as_mut().set_loading(false);
                })
                .ok();
        });
    }
}
