use super::home::{Card, Category, HeroItem, HomeSection, LinkItem, Story};
use std::path::PathBuf;

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

fn disk_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".cache/arc-frontend-qml/home_feed.json"))
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
    let path = disk_path()?;
    let bytes = std::fs::read(path).ok()?;
    let snapshot: Snapshot = serde_json::from_slice(&bytes).ok()?;
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
        })
        .collect();
    Some((sections, snapshot.stories))
}

pub fn save(sections: &[HomeSection], stories: &[Story]) {
    let Some(path) = disk_path() else { return };
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
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(bytes) = serde_json::to_vec(&snapshot) {
        let _ = std::fs::write(path, bytes);
    }
}
