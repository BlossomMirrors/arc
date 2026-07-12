use crate::services::forge::{self, FpLinksItem, FpSection, ForgeStory};
use crate::services::icons;
use futures_util::future::join_all;
use libarc::ArcDaemonProxy;
use std::collections::{HashMap, HashSet};

#[derive(Clone, serde::Serialize, Default)]
pub struct Card {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub icon_url: String,
    pub installed: bool,
}

#[derive(Clone, serde::Serialize, Default)]
pub struct HeroItem {
    pub id: String,
    pub banner_url: String,
    pub icon_url: String,
    pub title: String,
    pub body: String,
    pub is_story: bool,
    pub story_index: i32,
}

#[derive(Clone, serde::Serialize, Default)]
pub struct LinkItem {
    pub text: String,
    pub href: String,
    pub story_index: i32,
    pub app_id: String,
}

#[derive(Clone, serde::Serialize, Default)]
pub struct Category {
    pub id: String,
    pub label: String,
    pub icon_name: String,
    pub color: String,
}

pub struct HomeSection {
    pub item_type: &'static str,
    pub text: String,
    pub title: String,
    pub cards: Vec<Card>,
    pub hero_items: Vec<HeroItem>,
    pub editorial_items: Vec<HeroItem>,
    pub link_title: String,
    pub link_items: Vec<LinkItem>,
    pub app_cloud_overlay: bool,
    pub categories: Vec<Category>,
}

impl Default for HomeSection {
    fn default() -> Self {
        Self {
            item_type: "p",
            text: String::new(),
            title: String::new(),
            cards: vec![],
            hero_items: vec![],
            editorial_items: vec![],
            link_title: String::new(),
            link_items: vec![],
            app_cloud_overlay: false,
            categories: vec![],
        }
    }
}

#[derive(Clone)]
pub struct Story {
    pub id: String,
    pub title: String,
    pub banner_url: String,
    pub body_html: String,
}

struct AppEntry {
    id: String,
    name: String,
    summary: String,
    icon_url: Option<String>,
}

const CATEGORY_DEFS: &[(&str, &str, &str, &str)] = &[
    ("AudioVideo", "Multimedia", "applications-multimedia", "#2980b9"),
    ("Development", "Developer Tools", "applications-development", "#8e44ad"),
    ("Education", "Education", "applications-education", "#27ae60"),
    ("Graphics", "Graphics", "applications-graphics", "#c0392b"),
    ("Network", "Internet", "applications-internet", "#16a085"),
    ("Office", "Office", "applications-office", "#d35400"),
    ("Science", "Science", "applications-science", "#2980b9"),
    ("System", "System", "applications-system", "#64646e"),
    ("Utility", "Utilities", "applications-utilities", "#c0392b"),
];

fn strip_html_tags(s: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }
    result.split_whitespace().collect::<Vec<_>>().join(" ")
}

async fn resolve_ids_cached(
    ids: &[String],
    meta_map: &HashMap<String, forge::HomeAppMeta>,
    proxy: Option<&ArcDaemonProxy<'static>>,
    pwa_map: &HashMap<String, forge::PwaApp>,
) -> Vec<AppEntry> {
    let mut result = Vec::with_capacity(ids.len());
    let mut missing: Vec<String> = Vec::new();

    for id in ids {
        if let Some(m) = meta_map.get(id) {
            result.push(Some(AppEntry {
                id: m.id.clone(),
                name: m.name.clone(),
                summary: m.summary.clone(),
                icon_url: m.icon_url.clone(),
            }));
        } else {
            missing.push(id.clone());
            result.push(None);
        }
    }

    let fallbacks = resolve_ids(&missing, proxy, pwa_map).await;
    let mut fb_iter = fallbacks.into_iter();
    for slot in result.iter_mut() {
        if slot.is_none() {
            *slot = fb_iter.next();
        }
    }
    result.into_iter().flatten().collect()
}

async fn resolve_ids(
    ids: &[String],
    proxy: Option<&ArcDaemonProxy<'static>>,
    pwa_map: &HashMap<String, forge::PwaApp>,
) -> Vec<AppEntry> {
    let futs: Vec<_> = ids
        .iter()
        .map(|id| {
            let id = id.clone();
            let p = proxy.cloned();
            let pwa = pwa_map.get(&id).cloned();
            async move {
                let daemon = if let Some(p) = p {
                    let pkgs: Vec<libarc::Package> = p
                        .search(&id)
                        .await
                        .ok()
                        .and_then(|j| serde_json::from_str(&j).ok())
                        .unwrap_or_default();
                    pkgs.into_iter().find(|pkg| pkg.id == id).map(|pkg| AppEntry {
                        id: pkg.id,
                        name: pkg.name,
                        summary: pkg.description,
                        icon_url: pkg.icon_url,
                    })
                } else {
                    None
                };
                daemon.or_else(|| {
                    pwa.map(|p| AppEntry {
                        id: p.appid,
                        name: p.name,
                        summary: p.summary,
                        icon_url: p.icon_url,
                    })
                })
            }
        })
        .collect();

    join_all(futs).await.into_iter().flatten().collect()
}

async fn entries_to_cards(entries: Vec<AppEntry>, installed: &HashSet<String>) -> Vec<Card> {
    entries
        .into_iter()
        .map(|e| {
            let icon_url = icons::resolve(&e.id, e.icon_url.as_deref());
            Card {
                installed: installed.contains(&e.id),
                id: e.id,
                name: e.name,
                summary: e.summary,
                icon_url,
            }
        })
        .collect()
}

pub async fn load_home(proxy: Option<ArcDaemonProxy<'static>>) -> (Vec<HomeSection>, Vec<Story>) {
    let lang = sys_locale::get_locale()
        .unwrap_or_default()
        .split(['_', '-'])
        .next()
        .unwrap_or("en")
        .to_string();

    let (fp_sections, installed_raw, pwa_list, app_meta_raw) = tokio::join!(
        forge::fetch_frontpage(),
        async {
            if let Some(ref p) = proxy {
                p.list_installed()
                    .await
                    .ok()
                    .and_then(|json| serde_json::from_str::<Vec<libarc::Package>>(&json).ok())
                    .unwrap_or_default()
                    .into_iter()
                    .map(|pkg| pkg.id)
                    .collect::<HashSet<_>>()
            } else {
                HashSet::new()
            }
        },
        forge::fetch_pwas(&lang),
        forge::fetch_home_app_metadata(),
    );

    let pwa_map: HashMap<String, forge::PwaApp> =
        pwa_list.into_iter().map(|a| (a.appid.clone(), a)).collect();
    let installed_ids = installed_raw;
    let app_meta_map = app_meta_raw;

    let show_categories = fp_sections.iter().any(|s| matches!(s, FpSection::Categories));

    struct SecOut {
        item: HomeSection,
        stories: Vec<Story>,
    }

    let section_futs = fp_sections.into_iter().map(|section| {
        let proxy = proxy.clone();
        let pwa_map = &pwa_map;
        let installed_ids = &installed_ids;
        let app_meta_map = &app_meta_map;
        async move {
            let p = proxy.as_ref();
            match section {
                FpSection::H1(t) => SecOut {
                    item: HomeSection { item_type: "h1", text: t, ..Default::default() },
                    stories: vec![],
                },
                FpSection::H2(t) => SecOut {
                    item: HomeSection { item_type: "h2", text: t, ..Default::default() },
                    stories: vec![],
                },
                FpSection::H3(t) => SecOut {
                    item: HomeSection { item_type: "h3", text: t, ..Default::default() },
                    stories: vec![],
                },
                FpSection::P(t) => SecOut {
                    item: HomeSection { item_type: "p", text: t, ..Default::default() },
                    stories: vec![],
                },
                FpSection::Br => SecOut {
                    item: HomeSection { item_type: "br", ..Default::default() },
                    stories: vec![],
                },
                FpSection::Categories => SecOut {
                    item: HomeSection {
                        item_type: "categories",
                        categories: if show_categories { build_categories() } else { vec![] },
                        ..Default::default()
                    },
                    stories: vec![],
                },
                FpSection::Top => {
                    let ids = forge::fetch_app_ids("api/top", 12).await;
                    let entries = resolve_ids_cached(&ids, app_meta_map, p, pwa_map).await;
                    let cards = entries_to_cards(entries, installed_ids).await;
                    SecOut {
                        item: HomeSection {
                            item_type: "app-grid",
                            title: "Popular".into(),
                            cards,
                            ..Default::default()
                        },
                        stories: vec![],
                    }
                }
                FpSection::New => {
                    let ids = forge::fetch_app_ids("api/new", 20).await;
                    let entries = resolve_ids_cached(&ids, app_meta_map, p, pwa_map).await;
                    let cards = entries_to_cards(entries, installed_ids).await;
                    SecOut {
                        item: HomeSection {
                            item_type: "app-row",
                            title: "Recently Added".into(),
                            cards,
                            ..Default::default()
                        },
                        stories: vec![],
                    }
                }
                FpSection::Trending => {
                    let ids = forge::fetch_app_ids("api/trending", 12).await;
                    let entries = resolve_ids_cached(&ids, app_meta_map, p, pwa_map).await;
                    let cards = entries_to_cards(entries, installed_ids).await;
                    SecOut {
                        item: HomeSection {
                            item_type: "app-row",
                            title: "Trending".into(),
                            cards,
                            ..Default::default()
                        },
                        stories: vec![],
                    }
                }
                FpSection::Charts { cards: as_cards } => {
                    let ids = forge::fetch_chart_ids(12).await;
                    let entries = resolve_ids_cached(&ids, app_meta_map, p, pwa_map).await;
                    let cards = entries_to_cards(entries, installed_ids).await;
                    SecOut {
                        item: HomeSection {
                            item_type: if as_cards { "app-grid" } else { "app-row" },
                            title: "Charts".into(),
                            cards,
                            ..Default::default()
                        },
                        stories: vec![],
                    }
                }
                FpSection::Custom { title, app_ids } => {
                    let entries = resolve_ids_cached(&app_ids, app_meta_map, p, pwa_map).await;
                    let cards = entries_to_cards(entries, installed_ids).await;
                    SecOut {
                        item: HomeSection { item_type: "app-row", title, cards, ..Default::default() },
                        stories: vec![],
                    }
                }
                FpSection::CarouselSection { breakpoint, items, app_cloud_overlay } => {
                    let (item, stories) = build_carousel(
                        breakpoint, items, p, pwa_map, installed_ids, app_meta_map, app_cloud_overlay,
                    )
                    .await;
                    SecOut { item, stories }
                }
                FpSection::LinksSection { title, items, .. } => {
                    let (item, stories) = build_links(title, items, p, pwa_map, installed_ids, app_meta_map).await;
                    SecOut { item, stories }
                }
            }
        }
    });

    let section_results = join_all(section_futs).await;

    let mut sections: Vec<HomeSection> = Vec::new();
    let mut stories: Vec<Story> = Vec::new();
    for mut so in section_results {
        let offset = stories.len() as i32;
        for hi in so.item.hero_items.iter_mut().chain(so.item.editorial_items.iter_mut()) {
            if hi.story_index >= 0 {
                hi.story_index += offset;
            }
        }
        for li in &mut so.item.link_items {
            if li.story_index >= 0 {
                li.story_index += offset;
            }
        }
        stories.extend(so.stories);
        sections.push(so.item);
    }

    (sections, stories)
}

fn build_categories() -> Vec<Category> {
    CATEGORY_DEFS
        .iter()
        .map(|(id, label, icon_name, color)| Category {
            id: id.to_string(),
            label: label.to_string(),
            icon_name: icon_name.to_string(),
            color: color.to_string(),
        })
        .collect()
}

async fn load_stories(
    stories: Vec<(usize, ForgeStory)>,
    proxy: Option<&ArcDaemonProxy<'static>>,
    pwa_map: &HashMap<String, forge::PwaApp>,
    installed: &HashSet<String>,
) -> Vec<Story> {
    let _ = (proxy, pwa_map, installed);
    stories
        .into_iter()
        .map(|(idx, s)| Story {
            id: format!("story-{idx}"),
            title: s.title,
            banner_url: s.banner_url.unwrap_or_default(),
            body_html: s.body,
        })
        .collect()
}

async fn build_carousel(
    breakpoint: usize,
    items: Vec<forge::CarouselItem>,
    proxy: Option<&ArcDaemonProxy<'static>>,
    pwa_map: &HashMap<String, forge::PwaApp>,
    installed: &HashSet<String>,
    app_meta_map: &HashMap<String, forge::HomeAppMeta>,
    app_cloud_overlay: bool,
) -> (HomeSection, Vec<Story>) {
    use forge::CarouselItem;

    let hero_count = breakpoint.min(items.len());
    let mut story_positions: Vec<usize> = Vec::new();
    let mut story_forge: Vec<ForgeStory> = Vec::new();
    let mut app_id_list: Vec<String> = Vec::new();

    for (pos, item) in items.iter().enumerate() {
        match item {
            CarouselItem::Story(s) => {
                story_positions.push(pos);
                story_forge.push(s.clone());
            }
            CarouselItem::App(id) => app_id_list.push(id.clone()),
        }
    }

    let indexed = story_forge.into_iter().enumerate().collect::<Vec<_>>();
    let (loaded_stories, app_entries) = tokio::join!(
        load_stories(indexed, proxy, pwa_map, installed),
        resolve_ids_cached(&app_id_list, app_meta_map, proxy, pwa_map),
    );
    let app_cards = entries_to_cards(app_entries, installed).await;

    let card_by_id: HashMap<&str, &Card> = app_cards.iter().map(|c| (c.id.as_str(), c)).collect();
    let pos_to_story_idx: HashMap<usize, usize> =
        story_positions.iter().enumerate().map(|(si, &pos)| (pos, si)).collect();

    let mut hero_items: Vec<HeroItem> = Vec::new();
    let mut editorial_items: Vec<HeroItem> = Vec::new();
    let mut editorial_app_cards: Vec<Card> = Vec::new();

    for (pos, carousel_item) in items.iter().enumerate() {
        match carousel_item {
            CarouselItem::Story(s) => {
                let story_idx = pos_to_story_idx[&pos];
                let rs = loaded_stories.get(story_idx);
                let hero_item = HeroItem {
                    id: rs.map(|r| r.id.clone()).unwrap_or_else(|| format!("story-{story_idx}")),
                    banner_url: rs.map(|r| r.banner_url.clone()).unwrap_or_default(),
                    icon_url: String::new(),
                    title: s.title.clone(),
                    body: strip_html_tags(&s.body),
                    is_story: true,
                    story_index: story_idx as i32,
                };
                if pos < hero_count {
                    hero_items.push(hero_item);
                } else {
                    editorial_items.push(hero_item);
                }
            }
            CarouselItem::App(id) => {
                let card = card_by_id.get(id.as_str()).copied();
                if pos < hero_count {
                    hero_items.push(HeroItem {
                        id: id.clone(),
                        banner_url: String::new(),
                        icon_url: card.map(|c| c.icon_url.clone()).unwrap_or_default(),
                        title: card.map(|c| c.name.clone()).unwrap_or_default(),
                        body: card.map(|c| c.summary.clone()).unwrap_or_default(),
                        is_story: false,
                        story_index: -1,
                    });
                } else if let Some(c) = card {
                    editorial_app_cards.push(c.clone());
                }
            }
        }
    }

    (
        HomeSection {
            item_type: "carousel",
            cards: editorial_app_cards,
            hero_items,
            editorial_items,
            app_cloud_overlay,
            ..Default::default()
        },
        loaded_stories,
    )
}

async fn build_links(
    title: String,
    items: Vec<FpLinksItem>,
    proxy: Option<&ArcDaemonProxy<'static>>,
    pwa_map: &HashMap<String, forge::PwaApp>,
    installed: &HashSet<String>,
    app_meta_map: &HashMap<String, forge::HomeAppMeta>,
) -> (HomeSection, Vec<Story>) {
    enum LinkKind {
        Story(usize),
        App(String),
        Url,
    }
    let mut link_kinds: Vec<LinkKind> = Vec::new();
    let mut link_items: Vec<LinkItem> = Vec::new();
    let mut story_forge: Vec<ForgeStory> = Vec::new();
    let mut app_ids_batch: Vec<String> = Vec::new();

    for item in &items {
        match item {
            FpLinksItem::Story(s) => {
                let idx = story_forge.len();
                story_forge.push(s.clone());
                link_kinds.push(LinkKind::Story(idx));
                link_items.push(LinkItem { text: s.title.clone(), href: String::new(), story_index: -1, app_id: String::new() });
            }
            FpLinksItem::App(id) => {
                app_ids_batch.push(id.clone());
                link_kinds.push(LinkKind::App(id.clone()));
                link_items.push(LinkItem { text: String::new(), href: String::new(), story_index: -1, app_id: id.clone() });
            }
            FpLinksItem::Url { text, href } => {
                link_kinds.push(LinkKind::Url);
                link_items.push(LinkItem { text: text.clone(), href: href.clone(), story_index: -1, app_id: String::new() });
            }
        }
    }

    let indexed = story_forge.into_iter().enumerate().collect::<Vec<_>>();
    let (loaded_stories, app_entries) = tokio::join!(
        load_stories(indexed, proxy, pwa_map, installed),
        resolve_ids_cached(&app_ids_batch, app_meta_map, proxy, pwa_map),
    );

    let app_name_map: HashMap<String, String> = app_entries.into_iter().map(|e| (e.id, e.name)).collect();

    for (kind, item) in link_kinds.iter().zip(link_items.iter_mut()) {
        match kind {
            LinkKind::Story(idx) => {
                item.story_index = *idx as i32;
                if let Some(rs) = loaded_stories.get(*idx) {
                    item.text = rs.title.clone();
                }
            }
            LinkKind::App(id) => {
                item.text = app_name_map.get(id.as_str()).cloned().unwrap_or_else(|| id.clone());
            }
            LinkKind::Url => {}
        }
    }

    (
        HomeSection { item_type: "links", link_title: title, link_items, ..Default::default() },
        loaded_stories,
    )
}
