use super::home::{Card, Category, HeroItem, HomeSection, LinkItem, Story};
use libarc::cache::JsonCache;
use std::sync::{Mutex, OnceLock};

#[derive(serde::Serialize, serde::Deserialize)]
struct SectionDto {
    item_type: String,
    text: String,
    title: String,
    cards: Vec<Card>,
    hero_items: Vec<HeroItem>,
    editorial_items: Vec<HeroItem>,
    link_title: String,
    link_items: Vec<LinkItem>,
    app_cloud_overlay: bool,
    categories: Vec<Category>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Snapshot {
    sections: Vec<SectionDto>,
    stories: Vec<Story>,
}

fn store() -> &'static JsonCache<Snapshot> {
    static STORE: OnceLock<JsonCache<Snapshot>> = OnceLock::new();
    STORE.get_or_init(|| JsonCache::new("frontend", "home.json"))
}

// mirrors whatever the current sections resolved to, so StoryController can
// look a story up without a refetch even before the next disk save happens
static STORIES: OnceLock<Mutex<Vec<Story>>> = OnceLock::new();

pub fn set_stories(stories: Vec<Story>) {
    *STORIES.get_or_init(|| Mutex::new(Vec::new())).lock().unwrap() = stories;
}

pub fn find_story(id: &str) -> Option<Story> {
    STORIES.get()?.lock().unwrap().iter().find(|s| s.id == id).cloned()
}

fn static_item_type(s: &str) -> &'static str {
    match s {
        "h1" => "h1",
        "h2" => "h2",
        "h3" => "h3",
        "br" => "br",
        "categories" => "categories",
        "app-row" => "app-row",
        "app-grid" => "app-grid",
        "carousel" => "carousel",
        "links" => "links",
        _ => "p",
    }
}

pub fn load() -> Option<(Vec<HomeSection>, Vec<Story>)> {
    let snapshot = store().load()?;
    let sections = snapshot
        .sections
        .into_iter()
        .map(|d| HomeSection {
            item_type: static_item_type(&d.item_type),
            text: d.text,
            title: d.title,
            cards: d.cards,
            hero_items: d.hero_items,
            editorial_items: d.editorial_items,
            link_title: d.link_title,
            link_items: d.link_items,
            app_cloud_overlay: d.app_cloud_overlay,
            categories: d.categories,
            // save() only ever runs once every pending row has resolved, so
            // a cached snapshot is always fully-resolved by construction
            loading: false,
        })
        .collect();
    set_stories(snapshot.stories.clone());
    Some((sections, snapshot.stories))
}

pub fn save(sections: &[HomeSection], stories: &[Story]) {
    let snapshot = Snapshot {
        sections: sections
            .iter()
            .map(|s| SectionDto {
                item_type: s.item_type.to_string(),
                text: s.text.clone(),
                title: s.title.clone(),
                cards: s.cards.clone(),
                hero_items: s.hero_items.clone(),
                editorial_items: s.editorial_items.clone(),
                link_title: s.link_title.clone(),
                link_items: s.link_items.clone(),
                app_cloud_overlay: s.app_cloud_overlay,
                categories: s.categories.clone(),
            })
            .collect(),
        stories: stories.to_vec(),
    };
    store().store(&snapshot);
}
