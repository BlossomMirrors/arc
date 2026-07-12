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
use crate::services::home::Story;
use cxx_qt::Threading;
use cxx_qt_lib::QString;
use std::pin::Pin;
use std::sync::{Mutex, OnceLock};

// stashed by HomeFeedModel::load() so stories don't need a refetch
static STORIES: OnceLock<Mutex<Vec<Story>>> = OnceLock::new();

pub fn stash_stories(stories: Vec<Story>) {
    *STORIES.get_or_init(|| Mutex::new(Vec::new())).lock().unwrap() = stories;
}

#[derive(Default)]
pub struct StoryControllerRust {
    loading: bool,
    story_id: QString,
    title: QString,
    banner_url: QString,
    blocks_json: QString,
}

impl qobject::StoryController {
    pub fn load(mut self: Pin<&mut Self>, story_id: QString) {
        let id = story_id.to_string();
        self.as_mut().set_loading(true);
        self.as_mut().set_story_id(story_id);
        self.as_mut().set_blocks_json(QString::from("[]"));

        let story = STORIES
            .get()
            .and_then(|lock| lock.lock().unwrap().iter().find(|s| s.id == id).cloned());

        let Some(story) = story else {
            self.as_mut().set_loading(false);
            return;
        };

        self.as_mut().set_title(QString::from(&story.title));
        self.as_mut().set_banner_url(QString::from(&story.banner_url));

        let qt_thread = self.qt_thread();
        runtime::spawn(async move {
            let mut blocks = crate::services::blocks::html_to_blocks(&story.body_html);
            let proxy = runtime::proxy().await;
            crate::services::blocks::resolve_app_blocks(&mut blocks, proxy.as_ref()).await;
            let json = serde_json::to_string(&blocks).unwrap_or_else(|_| "[]".into());

            qt_thread
                .queue(move |mut this| {
                    this.as_mut().set_blocks_json(QString::from(&json));
                    this.as_mut().set_loading(false);
                })
                .ok();
        });
    }
}
