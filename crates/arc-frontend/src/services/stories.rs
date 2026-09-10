use super::home::Story;
use libarc::cache::PersistentMap;
use std::sync::OnceLock;
use std::time::Duration;

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct CachedStory {
    pub title: String,
    pub banner_url: String,
    pub body_html: String,
    pub blocks_json: String,
}

static STORIES: OnceLock<PersistentMap<CachedStory>> = OnceLock::new();

fn store() -> &'static PersistentMap<CachedStory> {
    STORIES.get_or_init(|| {
        PersistentMap::new("frontend", "stories.json")
            .with_entry_ttl(Duration::from_secs(7 * 24 * 3600))
            .with_capacity_limit(200)
    })
}

fn key(id: &str) -> String {
    format!("{}:{id}", super::forge::user_lang())
}

pub fn get(id: &str) -> Option<CachedStory> {
    store().get(&key(id))
}

pub fn remember(story: &Story) {
    let blocks_json = store()
        .get(&key(&story.id))
        .filter(|c| c.body_html == story.body_html)
        .map(|c| c.blocks_json)
        .unwrap_or_default();

    store().insert(
        key(&story.id),
        CachedStory {
            title: story.title.clone(),
            banner_url: story.banner_url.clone(),
            body_html: story.body_html.clone(),
            blocks_json,
        },
    );
}

pub fn remember_all(stories: &[Story]) {
    for story in stories {
        remember(story);
    }
}

pub fn seed(stories: &[Story]) {
    for story in stories {
        if !store().contains_key(&key(&story.id)) {
            remember(story);
        }
    }
}

pub fn store_blocks(id: &str, blocks_json: &str) {
    let json = blocks_json.to_string();
    store().update(&key(id), |c| c.blocks_json = json);
}
