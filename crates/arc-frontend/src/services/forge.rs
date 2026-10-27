use roxmltree::Node;

const FORGE: &str = libarc::FORGE_BASE_URL;

pub struct ListDef {
    pub slug: &'static str,
    pub icon_name: &'static str,
    pub color: &'static str,
}

pub const LISTS: &[ListDef] = &[
    ListDef {
        slug: "office",
        icon_name: "applications-office",
        color: "#ab4e1c",
    },
    ListDef {
        slug: "creative",
        icon_name: "applications-graphics",
        color: "#8f45c9",
    },
    ListDef {
        slug: "chat",
        icon_name: "folder-image-people-symbolic",
        color: "#1a5fb4",
    },
    ListDef {
        slug: "gaming",
        icon_name: "applications-games",
        color: "#26a269",
    },
    ListDef {
        slug: "browser",
        icon_name: "applications-internet",
        color: "#f66151",
    },
    ListDef {
        slug: "music",
        icon_name: "folder-music-symbolic",
        color: "#ff3d3d",
    },
    ListDef {
        slug: "code",
        icon_name: "applications-development",
        color: "#2d89f2",
    },
];

pub fn list_def(slug: &str) -> Option<&'static ListDef> {
    LISTS.iter().find(|l| l.slug == slug)
}

pub fn category_list_slug(category_id: &str) -> Option<&'static str> {
    match category_id {
        "Office" => Some("office"),
        "Development" => Some("code"),
        "Graphics" => Some("creative"),
        "AudioVideo" => Some("music"),
        "Network" => Some("browser"),
        _ => None,
    }
}

#[derive(serde::Deserialize, Clone)]
pub struct ForgeListApp {
    #[serde(rename = "ref")]
    pub app_ref: String,
    pub name: String,
    pub icon_url: Option<String>,
}

#[derive(serde::Deserialize, Clone)]
pub struct ForgeList {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub apps: Vec<ForgeListApp>,
}

pub async fn fetch_list(slug: &str) -> Option<ForgeList> {
    let url = format!("{}/api/lists/{}", FORGE, slug);
    reqwest::Client::new()
        .get(&url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .ok()?
        .json::<ForgeList>()
        .await
        .ok()
}

#[derive(serde::Deserialize, Clone)]
pub struct HomeAppMeta {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub icon_url: Option<String>,
}

pub async fn fetch_home_app_metadata() -> std::collections::HashMap<String, HomeAppMeta> {
    match reqwest::Client::new()
        .get(&format!("{}/api/app-metadata", FORGE))
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
    {
        Ok(r) => r
            .json::<Vec<HomeAppMeta>>()
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|m| (m.id.clone(), m))
            .collect(),
        Err(_) => Default::default(),
    }
}

#[derive(serde::Deserialize, Clone)]
pub struct PwaApp {
    pub appid: String,
    pub name: String,
    pub summary: String,
    pub icon_url: Option<String>,
}

#[derive(serde::Deserialize)]
struct ChartEntry {
    pub id: String,
}

#[derive(Debug, Clone)]
pub enum CarouselItem {
    Story(ForgeStory),
    App(String),
}

#[derive(Debug, Clone)]
pub enum FpLinksItem {
    Story(ForgeStory),
    App(String),
    Url { text: String, href: String },
}

#[derive(Debug, Clone)]
pub enum FpSection {
    H1(String),
    H2(String),
    H3(String),
    P(String),
    Br,
    CarouselSection {
        breakpoint: usize,
        items: Vec<CarouselItem>,
        app_cloud_overlay: bool,
    },
    Custom {
        title: String,
        app_ids: Vec<String>,
    },
    Top,
    New,
    Trending,
    Charts {
        cards: bool,
    },
    Categories,
    LinksSection {
        title: String,
        title_lang: String,
        items: Vec<FpLinksItem>,
    },
}

pub async fn fetch_frontpage() -> Vec<FpSection> {
    let xml = match reqwest::Client::new()
        .get(format!("{}/api/frontpage", FORGE))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
    {
        Ok(r) => r.text().await.unwrap_or_default(),
        Err(_) => return vec![],
    };
    if xml.trim().is_empty() {
        return vec![];
    }
    parse_frontpage(&xml)
}

pub async fn fetch_app_ids(path: &str, limit: u32) -> Vec<String> {
    let url = format!("{}/{}?limit={}", FORGE, path.trim_start_matches('/'), limit);
    match reqwest::Client::new()
        .get(&url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
    {
        Ok(r) => r.json::<Vec<String>>().await.unwrap_or_default(),
        Err(_) => vec![],
    }
}

pub async fn fetch_chart_ids(limit: u32) -> Vec<String> {
    let url = format!("{}/api/charts?limit={}", FORGE, limit);
    match reqwest::Client::new()
        .get(&url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
    {
        Ok(r) => r
            .json::<Vec<ChartEntry>>()
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|e| e.id)
            .collect(),
        Err(_) => vec![],
    }
}

pub async fn fetch_pwas(lang: &str) -> Vec<PwaApp> {
    let url = format!("{}/api/pwas?lang={}", FORGE, lang);
    match reqwest::Client::new()
        .get(&url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
    {
        Ok(r) => r.json::<Vec<PwaApp>>().await.unwrap_or_default(),
        Err(_) => vec![],
    }
}

#[derive(Debug, Clone)]
pub struct ForgeStory {
    pub banner_url: Option<String>,
    pub title: String,
    pub body: String,
    pub app_ids: Vec<String>,
}

pub async fn post_install(appid: String) {
    let body = serde_json::json!({ "appid": appid });
    let _ = reqwest::Client::new()
        .post(format!("{}/api/installs", FORGE))
        .timeout(std::time::Duration::from_secs(5))
        .json(&body)
        .send()
        .await;
}

pub fn user_lang() -> String {
    sys_locale::get_locale()
        .unwrap_or_default()
        .split(['_', '-'])
        .next()
        .unwrap_or("en")
        .to_string()
}

fn keep_one_links_language(sections: &mut Vec<FpSection>, user_lang: &str) {
    let mut langs: Vec<&str> = sections
        .iter()
        .filter_map(|s| match s {
            FpSection::LinksSection { title_lang, .. } => Some(title_lang.as_str()),
            _ => None,
        })
        .collect();
    langs.dedup();
    if langs.len() < 2 {
        return;
    }
    let chosen = if langs.contains(&user_lang) {
        user_lang.to_string()
    } else if langs.contains(&"en") {
        "en".to_string()
    } else {
        langs[0].to_string()
    };
    sections.retain(
        |s| !matches!(s, FpSection::LinksSection { title_lang, .. } if title_lang != &chosen),
    );
}

fn parse_frontpage(xml: &str) -> Vec<FpSection> {
    parse_frontpage_for(xml, &user_lang())
}

fn parse_frontpage_for(xml: &str, user_lang: &str) -> Vec<FpSection> {
    let body = match xml.trim_start().strip_prefix("<?xml") {
        Some(rest) => rest.split_once("?>").map(|(_, after)| after).unwrap_or(""),
        None => xml,
    };
    let wrapped = format!("<frontpage>{body}</frontpage>");
    let doc = match roxmltree::Document::parse(&wrapped) {
        Ok(doc) => doc,
        Err(e) => {
            tracing::warn!("forge frontpage is not valid xml: {e}");
            return Vec::new();
        }
    };

    let mut sections = Vec::new();
    for node in doc.root_element().children().filter(Node::is_element) {
        let section = match node.tag_name().name() {
            "top" => Some(FpSection::Top),
            "new" => Some(FpSection::New),
            "trending" => Some(FpSection::Trending),
            "charts" => Some(FpSection::Charts {
                cards: node.attribute("cards") != Some("false"),
            }),
            "categories" => Some(FpSection::Categories),
            "br" => Some(FpSection::Br),
            "h1" => text_section(node, FpSection::H1),
            "h2" => text_section(node, FpSection::H2),
            "h3" => text_section(node, FpSection::H3),
            "p" => text_section(node, FpSection::P),
            "carousel" => {
                let items = carousel_items(node, &wrapped, user_lang);
                (!items.is_empty()).then(|| FpSection::CarouselSection {
                    breakpoint: node
                        .attribute("breakpoint")
                        .and_then(|b| b.parse().ok())
                        .unwrap_or(usize::MAX),
                    items,
                    app_cloud_overlay: node.attribute("app-cloud") == Some("true"),
                })
            }
            "custom" => {
                let app_ids = app_ids(node);
                (!app_ids.is_empty()).then(|| FpSection::Custom {
                    title: localized_title(node, user_lang),
                    app_ids,
                })
            }
            "links" => Some(FpSection::LinksSection {
                title: localized_title(node, user_lang),
                title_lang: titles(node)
                    .first()
                    .map(|(lang, _)| lang.clone())
                    .unwrap_or_else(|| "en".into()),
                items: links_items(node, &wrapped, user_lang),
            }),
            _ => None,
        };
        sections.extend(section);
    }

    keep_one_links_language(&mut sections, user_lang);
    sections
}

fn text_section(node: Node, make: fn(String) -> FpSection) -> Option<FpSection> {
    let text = text_of(node);
    (!text.is_empty()).then(|| make(text))
}

fn text_of(node: Node) -> String {
    node.descendants()
        .filter(Node::is_text)
        .filter_map(|t| t.text())
        .collect::<String>()
        .trim()
        .to_string()
}

fn inner_markup(node: Node, source: &str) -> String {
    match (node.first_child(), node.last_child()) {
        (Some(first), Some(last)) => source[first.range().start..last.range().end]
            .trim()
            .to_string(),
        _ => String::new(),
    }
}

fn titles(node: Node) -> Vec<(String, String)> {
    node.children()
        .filter(|c| c.has_tag_name("title"))
        .map(|t| (t.attribute("lang").unwrap_or("en").to_string(), text_of(t)))
        .filter(|(_, text)| !text.is_empty())
        .collect()
}

fn localized_title(node: Node, user_lang: &str) -> String {
    let titles = titles(node);
    let in_lang = |lang: &str| {
        titles
            .iter()
            .find(|(l, _)| l == lang)
            .map(|(_, t)| t.clone())
    };
    in_lang(user_lang)
        .or_else(|| in_lang("en"))
        .or_else(|| titles.first().map(|(_, t)| t.clone()))
        .unwrap_or_default()
}

fn app_ids(node: Node) -> Vec<String> {
    node.descendants()
        .filter(|d| d.is_element() && d.tag_name().name().eq_ignore_ascii_case("app"))
        .filter_map(|d| d.attribute("id").map(str::trim))
        .filter(|id| !id.is_empty())
        .map(str::to_string)
        .collect()
}

struct RawStory {
    banner_url: Option<String>,
    body: String,
    app_ids: Vec<String>,
    titles: Vec<(String, String)>,
}

fn raw_story(node: Node, source: &str) -> Option<RawStory> {
    let titles = titles(node);
    if titles.is_empty() {
        return None;
    }
    let body = node.children().find(|c| c.has_tag_name("body"));
    Some(RawStory {
        banner_url: node.attribute("banner").map(libarc::media::proxied),
        body: body.map(|b| inner_markup(b, source)).unwrap_or_default(),
        app_ids: body.map(app_ids).unwrap_or_default(),
        titles,
    })
}

fn preferred_lang<'a>(mut stories: impl Iterator<Item = &'a RawStory>, user_lang: &str) -> String {
    let has_user_lang = stories.any(|s| s.titles.iter().any(|(l, _)| l == user_lang));
    if has_user_lang { user_lang } else { "en" }.to_string()
}

fn title_in(story: &RawStory, lang: &str) -> Option<String> {
    story
        .titles
        .iter()
        .find(|(l, _)| l == lang)
        .map(|(_, t)| t.clone())
}

enum RawCarouselItem {
    App(String),
    Story(RawStory),
}

fn carousel_items(node: Node, source: &str, user_lang: &str) -> Vec<CarouselItem> {
    let raw: Vec<RawCarouselItem> = node
        .children()
        .filter_map(|c| match c.tag_name().name() {
            "app" => c
                .attribute("id")
                .map(str::trim)
                .filter(|id| !id.is_empty())
                .map(|id| RawCarouselItem::App(id.to_string())),
            "story" => raw_story(c, source).map(RawCarouselItem::Story),
            _ => None,
        })
        .collect();
    let preferred = preferred_lang(
        raw.iter().filter_map(|i| match i {
            RawCarouselItem::Story(s) => Some(s),
            _ => None,
        }),
        user_lang,
    );

    let mut group_of: Vec<usize> = Vec::with_capacity(raw.len());
    let mut group_langs: Vec<Vec<String>> = Vec::new();
    let mut current: Option<usize> = None;
    for item in &raw {
        match item {
            RawCarouselItem::Story(s) => {
                let langs: Vec<String> = s.titles.iter().map(|(l, _)| l.clone()).collect();
                let repeats = current
                    .map(|g| langs.iter().any(|l| group_langs[g].contains(l)))
                    .unwrap_or(true);
                if repeats {
                    group_langs.push(langs);
                    current = Some(group_langs.len() - 1);
                } else if let Some(g) = current {
                    group_langs[g].extend(langs);
                }
                group_of.push(current.unwrap_or(usize::MAX));
            }
            RawCarouselItem::App(_) => {
                current = None;
                group_of.push(usize::MAX);
            }
        }
    }
    let mut apps_by_group: std::collections::HashMap<usize, Vec<String>> =
        std::collections::HashMap::new();
    for (i, item) in raw.iter().enumerate() {
        if let RawCarouselItem::Story(s) = item {
            if !s.app_ids.is_empty() {
                apps_by_group
                    .entry(group_of[i])
                    .or_insert_with(|| s.app_ids.clone());
            }
        }
    }

    let mut items = Vec::new();
    let mut hide_following_apps = false;
    for (i, item) in raw.into_iter().enumerate() {
        match item {
            RawCarouselItem::App(id) => {
                if !hide_following_apps {
                    items.push(CarouselItem::App(id));
                }
            }
            RawCarouselItem::Story(s) => {
                let title = title_in(&s, &preferred);
                hide_following_apps = title.is_none();
                let Some(title) = title else { continue };
                let mut body = s.body;
                let mut app_ids = s.app_ids;
                if app_ids.is_empty() {
                    if let Some(shared) = apps_by_group.get(&group_of[i]) {
                        app_ids = shared.clone();
                        for id in &app_ids {
                            body.push_str(&format!("\n<App id=\"{id}\" />"));
                        }
                    }
                }
                items.push(CarouselItem::Story(ForgeStory {
                    banner_url: s.banner_url,
                    title,
                    body,
                    app_ids,
                }));
            }
        }
    }
    items
}

fn links_items(node: Node, source: &str, user_lang: &str) -> Vec<FpLinksItem> {
    enum RawLinksItem {
        Url { text: String, href: String },
        App(String),
        Story(RawStory),
    }

    let raw: Vec<RawLinksItem> = node
        .children()
        .filter_map(|c| match c.tag_name().name() {
            "a" => c
                .attribute("href")
                .filter(|href| !href.is_empty())
                .map(|href| RawLinksItem::Url {
                    text: text_of(c),
                    href: href.to_string(),
                }),
            "app" => c
                .attribute("id")
                .map(str::trim)
                .filter(|id| !id.is_empty())
                .map(|id| RawLinksItem::App(id.to_string())),
            "story" => raw_story(c, source).map(RawLinksItem::Story),
            _ => None,
        })
        .collect();
    let preferred = preferred_lang(
        raw.iter().filter_map(|i| match i {
            RawLinksItem::Story(s) => Some(s),
            _ => None,
        }),
        user_lang,
    );

    let mut items = Vec::new();
    let mut hide_following = false;
    for item in raw {
        match item {
            RawLinksItem::Url { text, href } if !hide_following => {
                items.push(FpLinksItem::Url { text, href })
            }
            RawLinksItem::App(id) if !hide_following => items.push(FpLinksItem::App(id)),
            RawLinksItem::Story(s) => {
                let title = title_in(&s, &preferred);
                hide_following = title.is_none();
                if let Some(title) = title {
                    items.push(FpLinksItem::Story(ForgeStory {
                        banner_url: s.banner_url,
                        title,
                        body: s.body,
                        app_ids: s.app_ids,
                    }));
                }
            }
            _ => {}
        }
    }
    items
}
