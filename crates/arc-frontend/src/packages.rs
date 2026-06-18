use crate::helpers::check_flathub_verification;
use tr::tr;

fn push_decoded(text: &str, out: &mut String) {
    let mut rest = text;
    while !rest.is_empty() {
        match rest.find('&') {
            None => {
                out.push_str(rest);
                break;
            }
            Some(pos) => {
                out.push_str(&rest[..pos]);
                rest = &rest[pos..];
                if rest.starts_with("&amp;") {
                    out.push('&');
                    rest = &rest[5..];
                } else if rest.starts_with("&lt;") {
                    out.push('<');
                    rest = &rest[4..];
                } else if rest.starts_with("&gt;") {
                    out.push('>');
                    rest = &rest[4..];
                } else if rest.starts_with("&quot;") {
                    out.push('"');
                    rest = &rest[6..];
                } else if rest.starts_with("&apos;") {
                    out.push('\'');
                    rest = &rest[6..];
                } else {
                    out.push('&');
                    rest = &rest[1..];
                }
            }
        }
    }
}

pub(crate) struct DescBlock {
    text: String,
    is_list_item: bool,
    is_heading: bool,
    is_bold: bool,
    is_image: bool,
    image_url: String,
    pub image: Option<RawIcon>,
    pub is_app: bool,
    pub app_id: String,
    pub app_card: Option<RawCard>,
}

// Split a block's text on `**...**` markers into bold and normal segments.
// Headings are already bold so we skip splitting them.
fn text_block(text: String, is_list_item: bool, is_heading: bool, is_bold: bool) -> DescBlock {
    DescBlock { text, is_list_item, is_heading, is_bold, is_image: false, image_url: String::new(), image: None, is_app: false, app_id: String::new(), app_card: None }
}

fn split_bold(text: String, is_list_item: bool, is_heading: bool) -> Vec<DescBlock> {
    if is_heading || !text.contains("**") {
        return vec![text_block(text, is_list_item, is_heading, false)];
    }
    let mut result = Vec::new();
    let mut remaining = text.as_str();
    while !remaining.is_empty() {
        match remaining.find("**") {
            None => {
                result.push(text_block(remaining.to_string(), is_list_item, is_heading, false));
                break;
            }
            Some(start) => {
                if start > 0 {
                    result.push(text_block(remaining[..start].to_string(), is_list_item, is_heading, false));
                }
                remaining = &remaining[start + 2..];
                match remaining.find("**") {
                    None => {
                        result.push(text_block(remaining.to_string(), is_list_item, is_heading, false));
                        break;
                    }
                    Some(end) => {
                        let bold = &remaining[..end];
                        if !bold.is_empty() {
                            result.push(text_block(bold.to_string(), is_list_item, is_heading, true));
                        }
                        remaining = &remaining[end + 2..];
                    }
                }
            }
        }
    }
    result
}

fn attr_from_tag(tag: &str, name: &str) -> String {
    let needle = format!("{}=\"", name);
    let lower = tag.to_ascii_lowercase();
    if let Some(pos) = lower.find(&needle) {
        let after = &tag[pos + needle.len()..];
        if let Some(end) = after.find('"') {
            return after[..end].to_string();
        }
    }
    String::new()
}

fn parse_heading(s: &str) -> Option<String> {
    let s = s.trim();
    if s.starts_with("===") && s.ends_with("===") && s.len() > 6 {
        Some(s.trim_matches('=').trim().to_string())
    } else {
        None
    }
}

fn html_to_blocks(html: &str) -> Vec<DescBlock> {
    let mut blocks: Vec<DescBlock> = Vec::new();
    let mut current = String::new();
    let mut rest = html;
    while !rest.is_empty() {
        match rest.find('<') {
            None => {
                push_decoded(rest, &mut current);
                break;
            }
            Some(tag_start) => {
                push_decoded(&rest[..tag_start], &mut current);
                rest = &rest[tag_start + 1..];
                let tag_end = match rest.find('>') {
                    Some(e) => e,
                    None => {
                        current.push('<');
                        continue;
                    }
                };
                let raw_tag = rest[..tag_end].trim();
                let closing = raw_tag.starts_with('/');
                let name = raw_tag
                    .trim_start_matches('/')
                    .split_ascii_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_ascii_lowercase();
                match (closing, name.as_str()) {
                    (false, "li") => {
                        let t = current.trim().to_string();
                        if !t.is_empty() {
                            blocks.extend(split_bold(t, false, false));
                        }
                        current.clear();
                    }
                    (true, "li") => {
                        let t = current.trim().to_string();
                        if !t.is_empty() {
                            blocks.extend(split_bold(t, true, false));
                        }
                        current.clear();
                    }
                    (true, "p") | (true, "figcaption") => {
                        let t = current.trim().to_string();
                        if !t.is_empty() {
                            if let Some(heading) = parse_heading(&t) {
                                blocks.push(text_block(heading, false, true, false));
                            } else {
                                blocks.extend(split_bold(t, false, false));
                            }
                        }
                        current.clear();
                    }
                    (true, "h1") | (true, "h2") | (true, "h3") => {
                        let t = current.trim().to_string();
                        if !t.is_empty() {
                            blocks.push(text_block(t, false, true, false));
                        }
                        current.clear();
                    }
                    // <App id="..."> — inline app link; flush pending text then push app block
                    (false, "app") => {
                        let t = current.trim().to_string();
                        if !t.is_empty() {
                            blocks.extend(split_bold(t, false, false));
                            current.clear();
                        }
                        let id = attr_from_tag(raw_tag, "id");
                        if !id.is_empty() {
                            blocks.push(DescBlock {
                                text: String::new(),
                                is_list_item: false,
                                is_heading: false,
                                is_bold: false,
                                is_image: false,
                                image_url: String::new(),
                                image: None,
                                is_app: true,
                                app_id: id,
                                app_card: None,
                            });
                        }
                    }
                    // <img src="..."> — self-closing; flush pending text then push image block
                    (_, "img") => {
                        let t = current.trim().to_string();
                        if !t.is_empty() {
                            blocks.extend(split_bold(t, false, false));
                            current.clear();
                        }
                        let src = attr_from_tag(raw_tag, "src");
                        if !src.is_empty() {
                            blocks.push(DescBlock {
                                text: attr_from_tag(raw_tag, "alt"),
                                is_list_item: false,
                                is_heading: false,
                                is_bold: false,
                                is_image: true,
                                image_url: src,
                                image: None,
                                is_app: false,
                                app_id: String::new(),
                                app_card: None,
                            });
                        }
                    }
                    // <figure> / </figure> are container elements — just flush pending text
                    (true, "figure") => {
                        let t = current.trim().to_string();
                        if !t.is_empty() {
                            blocks.extend(split_bold(t, false, false));
                            current.clear();
                        }
                    }
                    _ => {}
                }
                rest = &rest[tag_end + 1..];
            }
        }
    }
    let t = current.trim().to_string();
    if !t.is_empty() {
        blocks.extend(split_bold(t, false, false));
    }
    blocks
}

use crate::icons::{self, RawIcon};
use crate::transactions::{has_ongoing_transaction_for_package, TxStore};
use futures_util::future::join_all;
use libarc::{ArcDaemonProxy, Package, Provider};
use slint::{Model, SharedString};

#[derive(Clone)]
pub struct RawCard {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub icon: Option<RawIcon>,
    pub installed: bool,
}

#[derive(serde::Deserialize, Clone)]
#[allow(dead_code)]
pub struct CachedAppInfo {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub description: String,
    pub icon_available: bool,
}

pub struct RawStory {
    pub id: String,
    pub title: String,
    pub screenshot: Option<RawIcon>,
    pub description_blocks: Vec<DescBlock>,
    pub featured_apps: Vec<RawCard>,
}

pub struct RawHeroItem {
    pub id: String,
    pub banner: Option<RawIcon>,
    pub icon: Option<RawIcon>,
    pub title: String,
    pub body: String,
    pub is_story: bool,
    pub story_index: i32,
}

pub struct RawLinkItem {
    pub text: String,
    pub href: String,
    pub story_index: i32,
    pub app_id: String,
}

impl RawLinkItem {
    fn to_slint(&self) -> crate::LinkItem {
        crate::LinkItem {
            text: SharedString::from(self.text.as_str()),
            href: SharedString::from(self.href.as_str()),
            story_index: self.story_index,
            app_id: SharedString::from(self.app_id.as_str()),
        }
    }
}

impl RawHeroItem {
    fn to_slint(&self) -> crate::HeroItem {
        crate::HeroItem {
            id: SharedString::from(self.id.as_str()),
            banner: self.banner.as_ref().map(|r| r.to_slint_image()).unwrap_or_default(),
            icon: self.icon.as_ref().map(|r| r.to_slint_image()).unwrap_or_default(),
            title: SharedString::from(self.title.as_str()),
            body: SharedString::from(self.body.as_str()),
            is_story: self.is_story,
            story_index: self.story_index,
        }
    }
}

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

pub struct RawCategoryData {
    pub id: String,
    pub label: String,
    pub icon: Option<RawIcon>,
    pub color: (u8, u8, u8),
}

pub struct RawDetailData {
    pub name: String,
    pub developer: String,
    pub description_blocks: Vec<DescBlock>,
    pub summary: String,
    pub version: String,
    pub icon: Option<RawIcon>,
    pub flatpak_id: String,
    pub native_id: String,
    pub lutris_id: String,
    pub appimage_id: String,
    pub flatpak_installed: bool,
    pub native_installed: bool,
    pub lutris_installed: bool,
    pub appimage_installed: bool,
    pub verified: bool,
    pub pwa: bool,
    pub license: String,
    pub eula_url: String,
    pub homepage_url: String,
    pub content_rating: String,
}

#[derive(Clone)]
pub struct RawPackage {
    pub pkg: libarc::Package,
    pub icon: Option<RawIcon>,
}

impl RawPackage {
    pub fn to_slint(&self) -> crate::PackageItem {
        crate::PackageItem {
            id: SharedString::from(self.pkg.id.as_str()),
            name: SharedString::from(self.pkg.name.as_str()),
            version: SharedString::from(self.pkg.version.as_str()),
            description: SharedString::from(self.pkg.description.as_str()),
            installed: self.pkg.installed,
            icon: self
                .icon
                .as_ref()
                .map(|r| r.to_slint_image())
                .unwrap_or_default(),
            busy: false,
            progress: 0.0,
            transaction_id: Default::default(),
        }
    }
}

impl RawCard {
    pub fn to_slint(&self) -> crate::AppCardData {
        crate::AppCardData {
            id: SharedString::from(self.id.as_str()),
            name: SharedString::from(self.name.as_str()),
            summary: SharedString::from(self.summary.as_str()),
            icon: self
                .icon
                .as_ref()
                .map(|r| r.to_slint_image())
                .unwrap_or_default(),
            installed: self.installed,
        }
    }
}

#[derive(serde::Deserialize)]
struct AppMetadata {
    summary: String,
    description: String,
    license: Option<String>,
    eula_url: Option<String>,
    homepage_url: Option<String>,
    content_rating: String,
    developer_name: Option<String>,
}

struct AppEntry {
    id: String,
    name: String,
    summary: String,
    icon_url: Option<String>,
}

/// Resolve app IDs using the daemon-cached metadata map, falling back to a
/// live D-Bus search only for IDs absent from the cache.
async fn resolve_ids_cached(
    ids: &[String],
    meta_map: &std::collections::HashMap<String, crate::forge::HomeAppMeta>,
    proxy: Option<&ArcDaemonProxy<'static>>,
    pwa_map: &std::collections::HashMap<String, crate::forge::PwaApp>,
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

/// Resolve a list of app IDs to entries using the daemon, falling back to the
/// forge PWA map for apps that the daemon doesn't know about.
async fn resolve_ids(
    ids: &[String],
    proxy: Option<&ArcDaemonProxy<'static>>,
    pwa_map: &std::collections::HashMap<String, crate::forge::PwaApp>,
) -> Vec<AppEntry> {
    let futs: Vec<_> = ids
        .iter()
        .map(|id| {
            let id = id.clone();
            let p = proxy.cloned();
            let pwa = pwa_map.get(&id).cloned();
            tokio::spawn(async move {
                let daemon = if let Some(ref p) = p {
                    let pkgs: Vec<libarc::Package> = p
                        .search(&id)
                        .await
                        .ok()
                        .and_then(|j| serde_json::from_str(&j).ok())
                        .unwrap_or_default();
                    pkgs.into_iter()
                        .find(|pkg| pkg.id == id)
                        .map(|pkg| AppEntry {
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
            })
        })
        .collect();

    join_all(futs)
        .await
        .into_iter()
        .filter_map(|r| r.ok().flatten())
        .collect()
}

/// Load icons for a list of app entries and return `RawCard` values.
async fn entries_to_cards(
    entries: Vec<AppEntry>,
    installed: &std::collections::HashSet<String>,
) -> Vec<RawCard> {
    let tasks: Vec<_> = entries
        .into_iter()
        .map(|e| {
            let installed_flag = installed.contains(&e.id);
            tokio::task::spawn_blocking(move || {
                let icon = e.icon_url.as_ref().and_then(|url| {
                    if url.starts_with("local:") {
                        icons::load_local_flatpak_icon(&e.id)
                    } else {
                        let rt = tokio::runtime::Handle::current();
                        rt.block_on(icons::load_icon(url))
                    }
                });
                RawCard {
                    id: e.id,
                    name: e.name,
                    summary: e.summary,
                    icon,
                    installed: installed_flag,
                }
            })
        })
        .collect();

    join_all(tasks)
        .await
        .into_iter()
        .filter_map(|r| r.ok())
        .collect()
}

fn cards_to_model(cards: Vec<RawCard>) -> slint::ModelRc<crate::AppCardData> {
    let data: Vec<crate::AppCardData> = cards.iter().map(|c| c.to_slint()).collect();
    slint::ModelRc::new(slint::VecModel::from(data))
}

/// Load stories: download banners, parse HTML body, resolve featured apps.
async fn load_raw_stories(
    stories: Vec<(usize, crate::forge::ForgeStory)>,
    proxy: Option<&libarc::ArcDaemonProxy<'static>>,
    pwa_map: &std::collections::HashMap<String, crate::forge::PwaApp>,
    installed: &std::collections::HashSet<String>,
) -> Vec<RawStory> {
    let mut out = Vec::new();
    for (idx, s) in stories {
        let mut description_blocks = html_to_blocks(&s.body);

        // Collect unique app IDs from inline <App> blocks
        let mut seen_ids = std::collections::HashSet::new();
        let inline_ids: Vec<String> = description_blocks
            .iter()
            .filter(|b| b.is_app && !b.app_id.is_empty())
            .filter_map(|b| {
                if seen_ids.insert(b.app_id.clone()) { Some(b.app_id.clone()) } else { None }
            })
            .collect();

        let (screenshot, inline_cards) = tokio::join!(
            async {
                if let Some(ref url) = s.banner_url {
                    icons::load_banner(url).await
                } else {
                    None
                }
            },
            async {
                let entries = resolve_ids(&inline_ids, proxy, pwa_map).await;
                entries_to_cards(entries, installed).await
            }
        );

        // Map app ID -> resolved card
        let card_map: std::collections::HashMap<String, RawCard> =
            inline_cards.into_iter().map(|c| (c.id.clone(), c)).collect();

        for block in &mut description_blocks {
            if block.is_app && !block.app_id.is_empty() {
                block.app_card = card_map.get(&block.app_id).cloned();
            }
            if block.is_image && !block.image_url.is_empty() {
                block.image = icons::load_banner(&block.image_url).await;
            }
        }

        out.push(RawStory {
            id: format!("story-{}", idx),
            title: s.title,
            screenshot,
            description_blocks,
            featured_apps: vec![],
        });
    }
    out
}

/// One entry in the home-items model: either a text block, app section, or carousel.
struct RawHomeItem {
    item_type: &'static str,
    text: String,
    title: String,
    cards: Vec<RawCard>,
    hero_items: Vec<RawHeroItem>,
    editorial_items: Vec<RawHeroItem>,
    link_title: String,
    link_items: Vec<RawLinkItem>,
}

pub async fn load_home(
    app_weak: slint::Weak<crate::AppWindow>,
    proxy: Option<ArcDaemonProxy<'static>>,
) {
    use crate::forge::FpSection;
    use std::sync::Arc;

    let lang = sys_locale::get_locale()
        .unwrap_or_default()
        .split(['_', '-'])
        .next()
        .unwrap_or("en")
        .to_string();
    let lang_ref = lang.as_str();

    let (fp_sections, installed_raw, pwa_list, app_meta_raw) = tokio::join!(
        crate::forge::fetch_frontpage(),
        async {
            if let Some(ref p) = proxy {
                p.list_installed()
                    .await
                    .ok()
                    .and_then(|json| serde_json::from_str::<Vec<libarc::Package>>(&json).ok())
                    .unwrap_or_default()
                    .into_iter()
                    .map(|pkg| pkg.id)
                    .collect::<std::collections::HashSet<_>>()
            } else {
                std::collections::HashSet::new()
            }
        },
        crate::forge::fetch_pwas(lang_ref),
        crate::forge::fetch_home_app_metadata(),
    );

    let pwa_map: Arc<std::collections::HashMap<String, crate::forge::PwaApp>> =
        Arc::new(pwa_list.into_iter().map(|a| (a.appid.clone(), a)).collect());
    let installed_ids: Arc<std::collections::HashSet<String>> = Arc::new(installed_raw);
    let app_meta_map: Arc<std::collections::HashMap<String, crate::forge::HomeAppMeta>> =
        Arc::new(app_meta_raw);
    let proxy: Arc<Option<ArcDaemonProxy<'static>>> = Arc::new(proxy);

    let show_categories = fp_sections.iter().any(|s| matches!(s, FpSection::Categories));

    struct SecOut {
        item: RawHomeItem,
        stories: Vec<RawStory>,
    }

    let section_futs: Vec<_> = fp_sections
        .into_iter()
        .map(|section| {
            let proxy = proxy.clone();
            let pwa_map = pwa_map.clone();
            let installed_ids = installed_ids.clone();
            let app_meta_map = app_meta_map.clone();
            async move {
                let p = proxy.as_ref().as_ref();
                let pm = &*pwa_map;
                let inst = &*installed_ids;
                let meta = &*app_meta_map;
                match section {
                    FpSection::H1(t) => SecOut {
                        item: RawHomeItem {
                            item_type: "h1", text: t, title: String::new(), cards: vec![],
                            hero_items: vec![], editorial_items: vec![], link_title: String::new(), link_items: vec![],
                        },
                        stories: vec![],
                    },
                    FpSection::H2(t) => SecOut {
                        item: RawHomeItem {
                            item_type: "h2", text: t, title: String::new(), cards: vec![],
                            hero_items: vec![], editorial_items: vec![], link_title: String::new(), link_items: vec![],
                        },
                        stories: vec![],
                    },
                    FpSection::H3(t) => SecOut {
                        item: RawHomeItem {
                            item_type: "h3", text: t, title: String::new(), cards: vec![],
                            hero_items: vec![], editorial_items: vec![], link_title: String::new(), link_items: vec![],
                        },
                        stories: vec![],
                    },
                    FpSection::P(t) => SecOut {
                        item: RawHomeItem {
                            item_type: "p", text: t, title: String::new(), cards: vec![],
                            hero_items: vec![], editorial_items: vec![], link_title: String::new(), link_items: vec![],
                        },
                        stories: vec![],
                    },
                    FpSection::Br => SecOut {
                        item: RawHomeItem {
                            item_type: "br", text: String::new(), title: String::new(), cards: vec![],
                            hero_items: vec![], editorial_items: vec![], link_title: String::new(), link_items: vec![],
                        },
                        stories: vec![],
                    },
                    FpSection::Categories => SecOut {
                        item: RawHomeItem {
                            item_type: "categories", text: String::new(), title: String::new(), cards: vec![],
                            hero_items: vec![], editorial_items: vec![], link_title: String::new(), link_items: vec![],
                        },
                        stories: vec![],
                    },
                    FpSection::Top => {
                        let ids = crate::forge::fetch_app_ids("api/top", 12).await;
                        let entries = resolve_ids_cached(&ids, meta, p, pm).await;
                        let cards = entries_to_cards(entries, inst).await;
                        SecOut {
                            item: RawHomeItem {
                                item_type: "app-grid", text: String::new(), title: "Popular".into(), cards,
                                hero_items: vec![], editorial_items: vec![], link_title: String::new(), link_items: vec![],
                            },
                            stories: vec![],
                        }
                    }
                    FpSection::New => {
                        let ids = crate::forge::fetch_app_ids("api/new", 20).await;
                        let entries = resolve_ids_cached(&ids, meta, p, pm).await;
                        let cards = entries_to_cards(entries, inst).await;
                        SecOut {
                            item: RawHomeItem {
                                item_type: "app-row", text: String::new(), title: tr!("Recently Added").to_string(), cards,
                                hero_items: vec![], editorial_items: vec![], link_title: String::new(), link_items: vec![],
                            },
                            stories: vec![],
                        }
                    }
                    FpSection::Trending => {
                        let ids = crate::forge::fetch_app_ids("api/trending", 12).await;
                        let entries = resolve_ids_cached(&ids, meta, p, pm).await;
                        let cards = entries_to_cards(entries, inst).await;
                        SecOut {
                            item: RawHomeItem {
                                item_type: "app-row", text: String::new(), title: tr!("Trending").to_string(), cards,
                                hero_items: vec![], editorial_items: vec![], link_title: String::new(), link_items: vec![],
                            },
                            stories: vec![],
                        }
                    }
                    FpSection::Charts { cards: as_cards } => {
                        let ids = crate::forge::fetch_chart_ids(12).await;
                        let entries = resolve_ids_cached(&ids, meta, p, pm).await;
                        let cards = entries_to_cards(entries, inst).await;
                        SecOut {
                            item: RawHomeItem {
                                item_type: if as_cards { "app-grid" } else { "app-row" },
                                text: String::new(), title: tr!("Charts").to_string(), cards,
                                hero_items: vec![], editorial_items: vec![], link_title: String::new(), link_items: vec![],
                            },
                            stories: vec![],
                        }
                    }
                    FpSection::Custom { title, app_ids } => {
                        let entries = resolve_ids_cached(&app_ids, meta, p, pm).await;
                        let cards = entries_to_cards(entries, inst).await;
                        SecOut {
                            item: RawHomeItem {
                                item_type: "app-row", text: String::new(), title, cards,
                                hero_items: vec![], editorial_items: vec![], link_title: String::new(), link_items: vec![],
                            },
                            stories: vec![],
                        }
                    }
                    FpSection::CarouselSection { breakpoint, items, .. } => {
                        use crate::forge::CarouselItem;
                        let hero_count = breakpoint.min(items.len());
                        let mut story_positions: Vec<usize> = Vec::new();
                        let mut story_forge: Vec<crate::forge::ForgeStory> = Vec::new();
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
                            load_raw_stories(indexed, p, pm, inst),
                            resolve_ids_cached(&app_id_list, meta, p, pm),
                        );
                        let app_cards = entries_to_cards(app_entries, inst).await;

                        let card_by_id: std::collections::HashMap<&str, &RawCard> =
                            app_cards.iter().map(|c| (c.id.as_str(), c)).collect();
                        let pos_to_story_idx: std::collections::HashMap<usize, usize> =
                            story_positions.iter().enumerate().map(|(si, &pos)| (pos, si)).collect();

                        let mut hero_items: Vec<RawHeroItem> = Vec::new();
                        let mut editorial_items: Vec<RawHeroItem> = Vec::new();

                        for (pos, carousel_item) in items.iter().enumerate() {
                            let hero_item = match carousel_item {
                                CarouselItem::Story(s) => {
                                    let story_idx = pos_to_story_idx[&pos];
                                    let rs = loaded_stories.get(story_idx);
                                    RawHeroItem {
                                        id: rs.map(|r| r.id.clone()).unwrap_or_else(|| format!("story-{story_idx}")),
                                        banner: rs.and_then(|r| r.screenshot.clone()),
                                        icon: None,
                                        title: s.title.clone(),
                                        body: strip_html_tags(&s.body),
                                        is_story: true,
                                        story_index: story_idx as i32,
                                    }
                                }
                                CarouselItem::App(id) => {
                                    let card = card_by_id.get(id.as_str()).copied();
                                    RawHeroItem {
                                        id: id.clone(),
                                        banner: None,
                                        icon: card.and_then(|c| c.icon.clone()),
                                        title: card.map(|c| c.name.clone()).unwrap_or_default(),
                                        body: card.map(|c| c.summary.clone()).unwrap_or_default(),
                                        is_story: false,
                                        story_index: -1,
                                    }
                                }
                            };
                            if pos < hero_count {
                                hero_items.push(hero_item);
                            } else {
                                editorial_items.push(hero_item);
                            }
                        }

                        SecOut {
                            item: RawHomeItem {
                                item_type: "carousel",
                                text: String::new(),
                                title: String::new(),
                                cards: vec![],
                                hero_items,
                                editorial_items,
                                link_title: String::new(),
                                link_items: vec![],
                            },
                            stories: loaded_stories,
                        }
                    }
                    FpSection::LinksSection { title, items, .. } => {
                        enum LinkKind { Story(usize), App(String), Url }
                        let mut link_kinds: Vec<LinkKind> = Vec::new();
                        let mut link_items: Vec<RawLinkItem> = Vec::new();
                        let mut story_forge: Vec<crate::forge::ForgeStory> = Vec::new();
                        let mut app_ids_batch: Vec<String> = Vec::new();

                        for item in &items {
                            match item {
                                crate::forge::FpLinksItem::Story(s) => {
                                    let idx = story_forge.len();
                                    story_forge.push(s.clone());
                                    link_kinds.push(LinkKind::Story(idx));
                                    link_items.push(RawLinkItem {
                                        text: s.title.clone(),
                                        href: String::new(),
                                        story_index: -1,
                                        app_id: String::new(),
                                    });
                                }
                                crate::forge::FpLinksItem::App(id) => {
                                    app_ids_batch.push(id.clone());
                                    link_kinds.push(LinkKind::App(id.clone()));
                                    link_items.push(RawLinkItem {
                                        text: String::new(),
                                        href: String::new(),
                                        story_index: -1,
                                        app_id: id.clone(),
                                    });
                                }
                                crate::forge::FpLinksItem::Url { text, href } => {
                                    link_kinds.push(LinkKind::Url);
                                    link_items.push(RawLinkItem {
                                        text: text.clone(),
                                        href: href.clone(),
                                        story_index: -1,
                                        app_id: String::new(),
                                    });
                                }
                            }
                        }

                        let indexed = story_forge.into_iter().enumerate().collect::<Vec<_>>();
                        let (loaded_stories, app_entries) = tokio::join!(
                            load_raw_stories(indexed, p, pm, inst),
                            resolve_ids_cached(&app_ids_batch, meta, p, pm),
                        );

                        let app_name_map: std::collections::HashMap<String, String> =
                            app_entries.into_iter().map(|e| (e.id, e.name)).collect();

                        for (kind, item) in link_kinds.iter().zip(link_items.iter_mut()) {
                            match kind {
                                LinkKind::Story(idx) => {
                                    item.story_index = *idx as i32;
                                    if let Some(rs) = loaded_stories.get(*idx) {
                                        item.text = rs.title.clone();
                                    }
                                }
                                LinkKind::App(id) => {
                                    item.text = app_name_map
                                        .get(id.as_str())
                                        .cloned()
                                        .unwrap_or_else(|| id.clone());
                                }
                                LinkKind::Url => {}
                            }
                        }

                        SecOut {
                            item: RawHomeItem {
                                item_type: "links",
                                text: String::new(),
                                title: String::new(),
                                cards: vec![],
                                hero_items: vec![],
                                editorial_items: vec![],
                                link_title: title,
                                link_items,
                            },
                            stories: loaded_stories,
                        }
                    }
                }
            }
        })
        .collect();

    let section_results = join_all(section_futs).await;

    let mut raw_items: Vec<RawHomeItem> = Vec::new();
    let mut raw_stories: Vec<RawStory> = Vec::new();
    for mut so in section_results {
        let offset = raw_stories.len() as i32;
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
        raw_stories.extend(so.stories);
        raw_items.push(so.item);
    }

    let raw_cats: Vec<RawCategoryData> = if show_categories {
        let cat_defs: &[(&str, &str, &str, (u8, u8, u8))] = &[
            ("AudioVideo",  "Multimedia",      "applications-multimedia",  (41,  128, 185)),
            ("Development", "Developer Tools", "applications-development", (142, 68,  173)),
            ("Education",   "Education",       "applications-education",   (39,  174, 96)),
            ("Graphics",    "Graphics",        "applications-graphics",    (192, 57,  43)),
            ("Network",     "Internet",        "applications-internet",    (22,  160, 133)),
            ("Office",      "Office",          "applications-office",      (211, 84,  0)),
            ("Science",     "Science",         "applications-science",     (41,  128, 185)),
            ("System",      "System",          "applications-system",      (100, 100, 110)),
            ("Utility",     "Utilities",       "applications-utilities",   (192, 57,  43)),
        ];
        join_all(cat_defs.iter().map(|(id, label, icon_name, color)| {
            let id = id.to_string();
            let label = label.to_string();
            let name = icon_name.to_string();
            let color = *color;
            async move {
                let icon = tokio::task::spawn_blocking(move || icons::load_category_icon(&name))
                    .await
                    .ok()
                    .and_then(|d| d.icon);
                RawCategoryData { id, label, icon, color }
            }
        }))
        .await
    } else {
        vec![]
    };
    let _ = app_weak.upgrade_in_event_loop(move |app| {
        let home_items: Vec<crate::HomeItem> = raw_items
            .into_iter()
            .map(|item| crate::HomeItem {
                item_type: item.item_type.into(),
                text: item.text.into(),
                title: item.title.into(),
                apps: cards_to_model(item.cards),
                hero_items: slint::ModelRc::new(slint::VecModel::from(
                    item.hero_items.iter().map(|h| h.to_slint()).collect::<Vec<_>>(),
                )),
                editorial_items: slint::ModelRc::new(slint::VecModel::from(
                    item.editorial_items.iter().map(|h| h.to_slint()).collect::<Vec<_>>(),
                )),
                link_title: item.link_title.into(),
                link_items: slint::ModelRc::new(slint::VecModel::from(
                    item.link_items.iter().map(|l| l.to_slint()).collect::<Vec<_>>(),
                )),
            })
            .collect();

        let cats: Vec<crate::CategoryItem> = raw_cats
            .iter()
            .map(|c| crate::CategoryItem {
                id: SharedString::from(c.id.as_str()),
                label: SharedString::from(c.label.as_str()),
                icon: c
                    .icon
                    .as_ref()
                    .map(|r| r.to_slint_image())
                    .unwrap_or_default(),
                bg_color: slint::Color::from_rgb_u8(c.color.0, c.color.1, c.color.2),
            })
            .collect();

        let stories: Vec<crate::StoryItem> = raw_stories
            .into_iter()
            .map(|s| {
                let blocks: Vec<crate::DescriptionBlock> = s
                    .description_blocks
                    .into_iter()
                    .map(|b| crate::DescriptionBlock {
                        text: b.text.into(),
                        is_list_item: b.is_list_item,
                        is_heading: b.is_heading,
                        is_bold: b.is_bold,
                        is_image: b.is_image,
                        image: b.image.as_ref().map(|r| r.to_slint_image()).unwrap_or_default(),
                        is_app: b.is_app,
                        app_card: b.app_card.as_ref().map(|c| c.to_slint()).unwrap_or_default(),
                    })
                    .collect();
                let apps: Vec<crate::AppCardData> =
                    s.featured_apps.iter().map(|c| c.to_slint()).collect();
                crate::StoryItem {
                    id: SharedString::from(s.id.as_str()),
                    title: SharedString::from(s.title.as_str()),
                    screenshot: s.screenshot.as_ref().map(|r| r.to_slint_image()).unwrap_or_default(),
                    description_blocks: slint::ModelRc::new(slint::VecModel::from(blocks)),
                    featured_apps: slint::ModelRc::new(slint::VecModel::from(apps)),
                }
            })
            .collect();

        app.set_home_items(home_items.as_slice().into());
        if !cats.is_empty() {
            app.set_categories(cats.as_slice().into());
        }
        if !stories.is_empty() {
            app.set_home_stories(stories.as_slice().into());
        }
        app.set_home_loading(false);
    });
}

pub async fn refresh_home_installed(
    app_weak: slint::Weak<crate::AppWindow>,
    proxy: Option<ArcDaemonProxy<'static>>,
) {
    let installed_ids: std::collections::HashSet<String> = if let Some(ref p) = proxy {
        p.list_installed()
            .await
            .ok()
            .and_then(|json| serde_json::from_str::<Vec<libarc::Package>>(&json).ok())
            .unwrap_or_default()
            .into_iter()
            .map(|pkg| pkg.id)
            .collect()
    } else {
        std::collections::HashSet::new()
    };

    let _ = app_weak.upgrade_in_event_loop(move |app| {
        let model = app.get_home_items();
        let count = model.row_count();
        let mut new_items: Vec<crate::HomeItem> =
            (0..count).filter_map(|i| model.row_data(i)).collect();
        for item in &mut new_items {
            let n = item.apps.row_count();
            let mut new_apps: Vec<crate::AppCardData> =
                (0..n).filter_map(|j| item.apps.row_data(j)).collect();
            let mut changed = false;
            for app_data in &mut new_apps {
                let installed = installed_ids.contains(app_data.id.as_str());
                if app_data.installed != installed {
                    app_data.installed = installed;
                    changed = true;
                }
            }
            if changed {
                item.apps = slint::ModelRc::new(slint::VecModel::from(new_apps));
            }
        }
        app.set_home_items(new_items.as_slice().into());
    });
}

pub async fn build_installed_cache(proxy: Option<ArcDaemonProxy<'static>>) -> Vec<RawPackage> {
    let pkgs: Vec<libarc::Package> = if let Some(p) = &proxy {
        p.list_installed()
            .await
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default()
    } else {
        vec![]
    };
    load_package_icons(pkgs).await
}

pub async fn load_package_icons(pkgs: Vec<libarc::Package>) -> Vec<RawPackage> {
    let icon_futures: Vec<_> = pkgs
        .iter()
        .map(|pkg| async {
            match pkg.provider {
                Provider::Flatpak => {
                    let id = pkg.id.clone();
                    let icon_url = pkg.icon_url.clone();
                    let local =
                        tokio::task::spawn_blocking(move || icons::load_local_flatpak_icon(&id))
                            .await
                            .unwrap_or(None);
                    if local.is_some() {
                        local
                    } else {
                        match icon_url.as_deref() {
                            Some(url) if !url.starts_with("local:") => icons::load_icon(url).await,
                            _ => None,
                        }
                    }
                }
                Provider::Lutris => {
                    if let Some(url) = &pkg.icon_url {
                        let url = url.clone();
                        tokio::spawn(async move { icons::load_lutris_icon(&url).await })
                            .await
                            .ok()
                            .flatten()
                    } else {
                        None
                    }
                }
                Provider::AppImage => {
                    let stem = pkg
                        .id
                        .strip_prefix("appimage:")
                        .unwrap_or(&pkg.id)
                        .to_string();
                    let icon_url = pkg.icon_url.clone();
                    tokio::task::spawn_blocking(move || {
                        icons::load_appimage_icon(icon_url.as_deref(), &stem)
                    })
                    .await
                    .unwrap_or(None)
                }
                Provider::Distrobox => {
                    let name = pkg.name.clone();
                    let icon_url = pkg.icon_url.clone();
                    tokio::task::spawn_blocking(move || {
                        icons::load_distrobox_icon(icon_url.as_deref(), &name)
                    })
                    .await
                    .unwrap_or(None)
                }
                Provider::Pwa => {
                    // Try local icon installed by the PWA provider first, then remote URL.
                    let appid = pkg.id.strip_prefix("pwa:").unwrap_or(&pkg.id).to_string();
                    let icon_url = pkg.icon_url.clone();
                    let local = tokio::task::spawn_blocking(move || {
                        let home = std::path::PathBuf::from(
                            std::env::var("HOME").unwrap_or_else(|_| "/root".to_string()),
                        );
                        let icons_dir = home.join(".local/share/icons/hicolor/256x256/apps");
                        for ext in ["png", "svg", "webp"] {
                            let p = icons_dir.join(format!("arc-pwa-{}.{}", appid, ext));
                            if p.exists() {
                                if let Some(icon) = icons::load_local_pwa_icon(&p) {
                                    return Some(icon);
                                }
                            }
                        }
                        None
                    })
                    .await
                    .unwrap_or(None);
                    if local.is_some() {
                        local
                    } else {
                        match icon_url.as_deref() {
                            Some(url) => icons::load_icon(url).await,
                            None => None,
                        }
                    }
                }
            }
        })
        .collect();
    let icons_result: Vec<_> = join_all(icon_futures).await;
    pkgs.into_iter()
        .zip(icons_result)
        .map(|(pkg, icon)| RawPackage { pkg, icon })
        .collect()
}

pub async fn load_detail(
    id: slint::SharedString,
    proxy: Option<ArcDaemonProxy<'static>>,
    store: TxStore,
    app_weak: slint::Weak<crate::AppWindow>,
) {
    let app_id = id.to_string();

    let app_name = app_id.split(';').next().unwrap_or(&app_id).to_string();

    let (search_pkgs, installed_pkgs, metadata): (Vec<Package>, Vec<Package>, Option<AppMetadata>) =
        if let Some(ref p) = proxy {
            tokio::join!(
                async {
                    p.search(&app_name)
                        .await
                        .ok()
                        .and_then(|j| serde_json::from_str(&j).ok())
                        .unwrap_or_default()
                },
                async {
                    p.list_installed()
                        .await
                        .ok()
                        .and_then(|j| serde_json::from_str(&j).ok())
                        .unwrap_or_default()
                },
                async {
                    p.get_app_metadata(&app_id)
                        .await
                        .ok()
                        .and_then(|j| serde_json::from_str(&j).ok())
                }
            )
        } else {
            (vec![], vec![], None)
        };

    let name_lower = app_name.to_lowercase();
    let all_pkgs: Vec<&Package> = search_pkgs.iter().chain(installed_pkgs.iter()).collect();

    let flatpak_pkg = all_pkgs.iter().copied().find(|p| {
        p.provider == Provider::Flatpak
            && (p.id == app_id.as_str()
                || p.id.to_lowercase() == app_id.to_string().to_lowercase()
                || p.name.to_lowercase() == name_lower)
    });

    let native_pkg = all_pkgs.iter().copied().find(|p| {
        p.provider == Provider::Distrobox
            && (p.id == app_id.as_str()
                || p.id.split(';').next().map(|n| n.to_lowercase()).as_deref()
                    == Some(name_lower.as_str())
                || p.name.to_lowercase() == name_lower)
    });

    let lutris_pkg = all_pkgs.iter().copied().find(|p| {
        p.provider == Provider::Lutris
            && (p.id == app_id.as_str() || p.name.to_lowercase() == name_lower)
    });

    let appimage_pkg = all_pkgs.iter().copied().find(|p| {
        p.provider == Provider::AppImage
            && (p.id == app_id.as_str() || p.name.to_lowercase() == name_lower)
    });

    let pwa_pkg = all_pkgs.iter().copied().find(|p| {
        p.provider == Provider::Pwa
            && (p.id == app_id.as_str() || p.name.to_lowercase() == name_lower)
    });

    let (developer, verified) = if let Some(pwa) = pwa_pkg {
        let dev = pwa.developer_name.clone().unwrap_or_default();
        (dev, true)
    } else if appimage_pkg.is_some() {
        (String::new(), false)
    } else if let Some(fp_pkg) = &flatpak_pkg {
        let remote = fp_pkg.remote.as_deref().unwrap_or("");
        let dev_name = metadata
            .as_ref()
            .and_then(|m| m.developer_name.clone())
            .or_else(|| fp_pkg.id.split('.').last().map(|s| s.to_string()))
            .unwrap_or_else(|| fp_pkg.name.clone());

        if remote == "blossomos" {
            (dev_name, true)
        } else if remote == "flathub" {
            let app_id_for_api = fp_pkg.id.clone();
            let api_verified = check_flathub_verification(&app_id_for_api).await;
            (dev_name, api_verified)
        } else {
            (dev_name, false)
        }
    } else {
        let dev_name = metadata
            .as_ref()
            .and_then(|m| m.developer_name.clone())
            .unwrap_or_else(|| app_name.clone());
        (dev_name, false)
    };

    // PWAs are surfaced through the flatpak_id slot so the existing detail UI
    // (install / remove / run buttons) works without any Slint changes.
    let flatpak_id = pwa_pkg
        .map(|p| p.id.clone())
        .or_else(|| flatpak_pkg.map(|p| p.id.clone()))
        .unwrap_or_else(|| {
            // Only infer a Flatpak ID from the raw app_id if it looks like a reverse-DNS
            // Flatpak identifier. Exclude AppImage/Lutris/PWA IDs which also contain dots.
            if app_id.contains('.')
                && !app_id.contains(';')
                && !app_id.starts_with("appimage:")
                && !app_id.starts_with("lutris:")
                && !app_id.starts_with("distrobox:")
                && !app_id.starts_with("pwa:")
            {
                app_id.to_string()
            } else {
                String::new()
            }
        });
    let native_id = native_pkg.map(|p| p.id.clone()).unwrap_or_default();
    let lutris_id = lutris_pkg.map(|p| p.id.clone()).unwrap_or_default();

    let flatpak_installed = pwa_pkg
        .map(|p| p.installed)
        .or_else(|| flatpak_pkg.map(|p| p.installed))
        .unwrap_or(false)
        || installed_pkgs
            .iter()
            .any(|p| p.provider == Provider::Flatpak && p.id == flatpak_id);
    let native_installed = native_pkg.map(|p| p.installed).unwrap_or(false);
    let lutris_installed = lutris_pkg.map(|p| p.installed).unwrap_or(false);
    let appimage_id = appimage_pkg.map(|p| p.id.clone()).unwrap_or_default();
    let appimage_installed = appimage_pkg.map(|p| p.installed).unwrap_or(false);

    let icon = if let Some(pwa) = pwa_pkg {
        // Try local icon written by the PWA provider, fall back to remote URL.
        let appid = pwa.id.strip_prefix("pwa:").unwrap_or(&pwa.id).to_string();
        let home = std::path::PathBuf::from(
            std::env::var("HOME").unwrap_or_else(|_| "/root".to_string()),
        );
        let icons_dir = home.join(".local/share/icons/hicolor/256x256/apps");
        let local = tokio::task::spawn_blocking(move || {
            for ext in ["png", "svg", "webp"] {
                let p = icons_dir.join(format!("arc-pwa-{}.{}", appid, ext));
                if p.exists() {
                    if let Some(icon) = icons::load_local_pwa_icon(&p) {
                        return Some(icon);
                    }
                }
            }
            None
        })
        .await
        .unwrap_or(None);
        if local.is_some() {
            local
        } else {
            match pwa.icon_url.as_deref() {
                Some(url) => icons::load_icon(url).await,
                None => None,
            }
        }
    } else if !flatpak_id.is_empty() {
        let fid = flatpak_id.clone();
        let remote_icon_url = flatpak_pkg
            .and_then(|p| p.icon_url.clone())
            .filter(|u| !u.starts_with("local:"));
        let local = tokio::task::spawn_blocking(move || icons::load_local_flatpak_icon(&fid))
            .await
            .unwrap_or(None);
        if local.is_some() {
            local
        } else {
            match remote_icon_url.as_deref() {
                Some(url) => icons::load_icon(url).await,
                None => None,
            }
        }
    } else if !lutris_id.is_empty() {
        if let Some(lutris) = &lutris_pkg {
            if let Some(icon_url) = &lutris.icon_url {
                let url = icon_url.clone();
                tokio::spawn(async move { icons::load_lutris_icon(&url).await })
                    .await
                    .ok()
                    .flatten()
            } else {
                None
            }
        } else {
            None
        }
    } else if !native_id.is_empty() {
        let name = native_pkg.map(|p| p.name.clone()).unwrap_or_default();
        let icon_url = native_pkg.and_then(|p| p.icon_url.clone());
        tokio::task::spawn_blocking(move || icons::load_distrobox_icon(icon_url.as_deref(), &name))
            .await
            .unwrap_or(None)
    } else if !appimage_id.is_empty() {
        let stem = appimage_id
            .strip_prefix("appimage:")
            .unwrap_or(&appimage_id)
            .to_string();
        let icon_url = appimage_pkg.and_then(|p| p.icon_url.clone());
        tokio::task::spawn_blocking(move || icons::load_appimage_icon(icon_url.as_deref(), &stem))
            .await
            .unwrap_or(None)
    } else {
        None
    };

    let name = pwa_pkg
        .or(flatpak_pkg)
        .or(native_pkg)
        .or(lutris_pkg)
        .or(appimage_pkg)
        .map(|p| p.name.clone())
        .unwrap_or_else(|| app_name.clone());

    let plain_description: Option<String> = if flatpak_pkg.is_none() {
        pwa_pkg
            .or(native_pkg)
            .or(lutris_pkg)
            .or(appimage_pkg)
            .map(|p| p.description.clone())
            .filter(|s| !s.is_empty())
    } else {
        None
    };

    let version = flatpak_pkg
        .or(native_pkg)
        .or(lutris_pkg)
        .or(appimage_pkg)
        .map(|p| p.version.clone())
        .unwrap_or_default();

    let screenshot_urls: Vec<String> = pwa_pkg
        .or(flatpak_pkg)
        .or(lutris_pkg)
        .or(native_pkg)
        .map(|p| p.screenshots.clone())
        .unwrap_or_default();

    let raw = RawDetailData {
        name,
        developer,
        description_blocks: metadata
            .as_ref()
            .map(|m| html_to_blocks(&m.description))
            .or_else(|| plain_description.map(|t| split_bold(t, false, false)))
            .unwrap_or_default(),
        summary: metadata
            .as_ref()
            .map(|m| m.summary.clone())
            .unwrap_or_default(),
        version,
        icon,
        flatpak_id,
        native_id,
        lutris_id,
        appimage_id,
        flatpak_installed,
        native_installed,
        lutris_installed,
        appimage_installed,
        verified,
        pwa: pwa_pkg.is_some(),
        license: metadata
            .as_ref()
            .and_then(|m| m.license.clone())
            .unwrap_or_default(),
        eula_url: metadata
            .as_ref()
            .and_then(|m| m.eula_url.clone())
            .unwrap_or_default(),
        homepage_url: metadata
            .as_ref()
            .and_then(|m| m.homepage_url.clone())
            .or_else(|| pwa_pkg.and_then(|p| p.homepage_url.clone()))
            .unwrap_or_default(),
        content_rating: metadata
            .as_ref()
            .map(|m| m.content_rating.clone())
            .or_else(|| pwa_pkg.and_then(|p| p.content_rating.clone()))
            .unwrap_or_default(),
    };

    let pkg_id_for_busy_check = if !raw.flatpak_id.is_empty() {
        raw.flatpak_id.clone()
    } else {
        raw.appimage_id.clone()
    };
    let flatpak_id_for_extensions = raw.flatpak_id.clone();
    let raw_description_blocks = raw.description_blocks;
    let _ = app_weak.upgrade_in_event_loop(move |app| {
        let description_blocks: Vec<crate::DescriptionBlock> = raw_description_blocks
            .into_iter()
            .map(|b| crate::DescriptionBlock {
                text: b.text.into(),
                is_list_item: b.is_list_item,
                is_heading: b.is_heading,
                is_bold: b.is_bold,
                is_image: b.is_image,
                image: b.image.as_ref().map(|r| r.to_slint_image()).unwrap_or_default(),
                is_app: b.is_app,
                app_card: b.app_card.as_ref().map(|c| c.to_slint()).unwrap_or_default(),
            })
            .collect();
        app.set_detail_screenshots([].as_slice().into());
        app.set_detail_extensions([].as_slice().into());
        app.set_detail_description_blocks(description_blocks.as_slice().into());
        app.set_detail_app(crate::AppDetailData {
            id: Default::default(),
            name: raw.name.into(),
            developer: raw.developer.into(),
            summary: raw.summary.into(),
            version: raw.version.into(),
            icon: raw
                .icon
                .as_ref()
                .map(|r| r.to_slint_image())
                .unwrap_or_default(),
            flatpak_id: raw.flatpak_id.into(),
            native_id: raw.native_id.into(),
            lutris_id: raw.lutris_id.into(),
            appimage_id: raw.appimage_id.into(),
            flatpak_installed: raw.flatpak_installed,
            native_installed: raw.native_installed,
            lutris_installed: raw.lutris_installed,
            appimage_installed: raw.appimage_installed,
            installed: raw.flatpak_installed
                || raw.native_installed
                || raw.lutris_installed
                || raw.appimage_installed,
            verified: raw.verified,
            pwa: raw.pwa,
            license: raw.license.into(),
            eula_url: raw.eula_url.into(),
            homepage_url: raw.homepage_url.into(),
            content_rating: raw.content_rating.into(),
        });
        app.set_detail_loading(false);
        let is_busy = has_ongoing_transaction_for_package(&store, &pkg_id_for_busy_check);
        app.set_detail_busy(is_busy);
    });

    if !flatpak_id_for_extensions.is_empty() {
        let flatpak_id = flatpak_id_for_extensions;
        let proxy_clone = proxy.clone();
        let app_weak3 = app_weak.clone();
        tokio::spawn(async move {
            let extensions: Vec<libarc::Package> = if let Some(p) = &proxy_clone {
                p.list_extensions(&flatpak_id)
                    .await
                    .ok()
                    .and_then(|j| serde_json::from_str(&j).ok())
                    .unwrap_or_default()
            } else {
                vec![]
            };
            if !extensions.is_empty() {
                let _ = app_weak3.upgrade_in_event_loop(move |app| {
                    let items: Vec<crate::ExtensionItem> = extensions
                        .iter()
                        .map(|e| crate::ExtensionItem {
                            id: SharedString::from(e.id.as_str()),
                            name: SharedString::from(e.name.as_str()),
                            installed: e.installed,
                        })
                        .collect();
                    app.set_detail_extensions(items.as_slice().into());
                });
            }
        });
    }

    if !screenshot_urls.is_empty() {
        let app_weak2 = app_weak.clone();
        tokio::spawn(async move {
            let futs: Vec<_> = screenshot_urls
                .iter()
                .map(|url| {
                    let url = url.clone();
                    tokio::spawn(async move { icons::load_screenshot(&url).await })
                })
                .collect();
            let raw_shots: Vec<RawIcon> = join_all(futs)
                .await
                .into_iter()
                .filter_map(|r| r.ok().flatten())
                .collect();
            if !raw_shots.is_empty() {
                let _ = app_weak2.upgrade_in_event_loop(move |app| {
                    let shots: Vec<slint::Image> =
                        raw_shots.iter().map(|r| r.to_slint_image()).collect();
                    app.set_detail_screenshots(shots.as_slice().into());
                });
            }
        });
    }
}
