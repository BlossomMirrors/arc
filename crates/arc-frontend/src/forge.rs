const FORGE_BASE: &str = "https://forge.blossomos.org";

/// Full PWA app record from `/api/pwas` — used as metadata fallback.
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

/// An item inside a `<carousel>` block, preserving document order.
#[derive(Debug, Clone)]
pub enum CarouselItem {
    Story(ForgeStory),
    App(String),
}

/// A link item inside a `<links>` block.
#[derive(Debug, Clone)]
pub enum FpLinksItem {
    Story(ForgeStory),
    App(String),
    Url { text: String, href: String },
}

/// A section parsed from `/api/frontpage`.
#[derive(Debug, Clone)]
pub enum FpSection {
    // Text/layout
    H1(String),
    H2(String),
    H3(String),
    P(String),
    Br,
    // App store sections
    CarouselSection {
        breakpoint: usize,
        items: Vec<CarouselItem>,
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
        .get(format!("{}/api/frontpage", FORGE_BASE))
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

/// Fetch app IDs from `/api/new`, `/api/top`, or `/api/trending`.
pub async fn fetch_app_ids(path: &str, limit: u32) -> Vec<String> {
    let url = format!(
        "{}/{}?limit={}",
        FORGE_BASE,
        path.trim_start_matches('/'),
        limit
    );
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

/// Fetch ranked app IDs from `/api/charts`.
pub async fn fetch_chart_ids(limit: u32) -> Vec<String> {
    let url = format!("{}/api/charts?limit={}", FORGE_BASE, limit);
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

/// Fetch PWA app metadata (used as fallback when daemon has no record).
pub async fn fetch_pwas(lang: &str) -> Vec<PwaApp> {
    let url = format!("{}/api/pwas?lang={}", FORGE_BASE, lang);
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

/// A story element parsed from the frontpage XML (inside a carousel).
/// Language selection happens at parse time via `extract_localized_title`.
#[derive(Debug, Clone)]
pub struct ForgeStory {
    pub banner_url: Option<String>,
    pub title: String,
    pub body: String,
}

pub async fn post_install(appid: String) {
    let body = serde_json::json!({ "appid": appid });
    let _ = reqwest::Client::new()
        .post(format!("{}/api/installs", FORGE_BASE))
        .timeout(std::time::Duration::from_secs(5))
        .json(&body)
        .send()
        .await;
}

pub fn is_flatpak_id(id: &str) -> bool {
    !id.contains('/')
        && !id.contains(';')
        && !id.starts_with("appimage:")
        && !id.starts_with("lutris:")
        && !id.starts_with("distrobox:")
        && id.matches('.').count() >= 2
}

fn parse_frontpage(xml: &str) -> Vec<FpSection> {
    let mut sections = Vec::new();
    let mut pos = 0;

    while pos < xml.len() {
        let Some(rel_lt) = xml[pos..].find('<') else {
            break;
        };
        let lt = pos + rel_lt;
        let Some(rel_gt) = xml[lt + 1..].find('>') else {
            break;
        };
        let gt = lt + 1 + rel_gt;

        let raw = xml[lt + 1..gt].trim();
        if raw.starts_with('/') || raw.starts_with('!') || raw.starts_with('?') {
            pos = gt + 1;
            continue;
        }

        let self_closing = raw.ends_with('/');
        let tag = raw.trim_end_matches('/').trim();
        let name = tag.split_ascii_whitespace().next().unwrap_or("");

        match name {
            "top" => {
                sections.push(FpSection::Top);
                pos = gt + 1;
            }
            "new" => {
                sections.push(FpSection::New);
                pos = gt + 1;
            }
            "trending" => {
                sections.push(FpSection::Trending);
                pos = gt + 1;
            }
            "charts" => {
                let cards = !tag.contains("cards=\"false\"");
                sections.push(FpSection::Charts { cards });
                pos = gt + 1;
            }
            "categories" => {
                sections.push(FpSection::Categories);
                pos = gt + 1;
            }
            "br" => {
                sections.push(FpSection::Br);
                pos = gt + 1;
            }
            "h1" if !self_closing => {
                let (text, end) = extract_text_content(xml, gt + 1, "h1");
                if !text.is_empty() {
                    sections.push(FpSection::H1(text));
                }
                pos = end;
            }
            "h2" if !self_closing => {
                let (text, end) = extract_text_content(xml, gt + 1, "h2");
                if !text.is_empty() {
                    sections.push(FpSection::H2(text));
                }
                pos = end;
            }
            "h3" if !self_closing => {
                let (text, end) = extract_text_content(xml, gt + 1, "h3");
                if !text.is_empty() {
                    sections.push(FpSection::H3(text));
                }
                pos = end;
            }
            "p" if !self_closing => {
                let (text, end) = extract_text_content(xml, gt + 1, "p");
                if !text.is_empty() {
                    sections.push(FpSection::P(text));
                }
                pos = end;
            }
            "carousel" if !self_closing => {
                let breakpoint = tag
                    .find("breakpoint=\"")
                    .and_then(|p| {
                        let after = &tag[p + 12..];
                        after
                            .find('"')
                            .and_then(|e| after[..e].parse::<usize>().ok())
                    })
                    .unwrap_or(usize::MAX);
                const CLOSE: &str = "</carousel>";
                let body_start = gt + 1;
                let body_end = xml[body_start..]
                    .find(CLOSE)
                    .map(|i| i + body_start)
                    .unwrap_or(xml.len());
                let items = extract_carousel_items(&xml[body_start..body_end]);
                if !items.is_empty() {
                    sections.push(FpSection::CarouselSection { breakpoint, items });
                }
                pos = body_end + CLOSE.len();
            }
            "custom" if !self_closing => {
                const CLOSE: &str = "</custom>";
                let body_start = gt + 1;
                let body_end = xml[body_start..]
                    .find(CLOSE)
                    .map(|i| i + body_start)
                    .unwrap_or(xml.len());
                let inner = &xml[body_start..body_end];
                let title = extract_localized_title(inner);
                let app_ids = extract_app_ids(inner);
                if !app_ids.is_empty() {
                    sections.push(FpSection::Custom { title, app_ids });
                }
                pos = body_end + CLOSE.len();
            }
            "links" if !self_closing => {
                const CLOSE: &str = "</links>";
                let body_start = gt + 1;
                let body_end = xml[body_start..]
                    .find(CLOSE)
                    .map(|i| i + body_start)
                    .unwrap_or(xml.len());
                let inner = &xml[body_start..body_end];
                let title = extract_localized_title(inner);
                let title_lang = primary_title_lang(inner);
                let items = extract_links_items(inner);
                sections.push(FpSection::LinksSection { title, title_lang, items });
                pos = body_end + CLOSE.len();
            }
            _ if self_closing => pos = gt + 1,
            _ => {
                let close = format!("</{}>", name);
                pos = xml[gt + 1..]
                    .find(close.as_str())
                    .map(|i| gt + 1 + i + close.len())
                    .unwrap_or(gt + 1);
            }
        }
    }

    // Two-pass locale filter for LinksSection entries: same logic as carousel stories.
    // If any links section has a title in the user's language, keep only those sections.
    // Otherwise fall back to showing all (typically "en").
    let sys = sys_locale::get_locale().unwrap_or_default();
    let user_lang = sys.split(['_', '-']).next().unwrap_or("en").to_string();
    let has_user_lang = sections.iter().any(|s| {
        matches!(s, FpSection::LinksSection { title_lang, .. } if title_lang == &user_lang)
    });
    if has_user_lang {
        sections.retain(|s| {
            !matches!(s, FpSection::LinksSection { title_lang, .. } if title_lang != &user_lang)
        });
    }

    sections
}

/// Returns the `lang` attribute of the first `<title>` element in `xml`, or `"en"`.
fn primary_title_lang(xml: &str) -> String {
    let p = match xml.find("<title") {
        Some(p) => p,
        None => return "en".to_string(),
    };
    let chunk = &xml[p..];
    let gt = match chunk.find('>') {
        Some(g) => g,
        None => return "en".to_string(),
    };
    let tag = &chunk[1..gt];
    tag.find("lang=\"")
        .and_then(|lp| {
            let after = &tag[lp + 6..];
            after.find('"').map(|e| after[..e].to_string())
        })
        .unwrap_or_else(|| "en".to_string())
}

fn extract_text_content(xml: &str, start: usize, tag: &str) -> (String, usize) {
    let close = format!("</{}>", tag);
    let end = xml[start..]
        .find(close.as_str())
        .map(|i| i + start)
        .unwrap_or(xml.len());
    let text = xml[start..end].trim().to_string();
    (text, end + close.len())
}

/// Extract all `(lang, title)` pairs from the inner XML of a `<story>` element.
fn extract_all_titles(xml: &str) -> Vec<(String, String)> {
    let mut titles = Vec::new();
    let mut t = xml;
    while let Some(tp) = t.find("<title") {
        let rest = &t[tp..];
        let Some(tgt) = rest.find('>') else { break };
        let tag = &rest[1..tgt];
        let lang = tag
            .find("lang=\"")
            .and_then(|p| {
                let a = &tag[p + 6..];
                a.find('"').map(|e| a[..e].to_string())
            })
            .unwrap_or_else(|| "en".to_string());
        let text_start = tp + tgt + 1;
        let text_end = t[text_start..]
            .find("</title>")
            .map(|e| text_start + e)
            .unwrap_or(t.len());
        let text = t[text_start..text_end].trim().to_string();
        if !text.is_empty() {
            titles.push((lang, text));
        }
        t = &t[text_end + 8..];
    }
    titles
}

/// Intermediate story before locale selection.
struct RawStory {
    banner_url: Option<String>,
    body: String,
    titles: Vec<(String, String)>,
}

enum RawCarouselItem {
    App(String),
    Story(RawStory),
}

/// Parse all items inside a `<carousel>` body in document order.
///
/// Stories whose `<title>` elements do not match the system locale (or its
/// "en" fallback) are excluded — each logical story is represented only once,
/// in the user's preferred language.
fn extract_carousel_items(xml: &str) -> Vec<CarouselItem> {
    let sys = sys_locale::get_locale().unwrap_or_default();
    let user_lang = sys.split(['_', '-']).next().unwrap_or("en").to_string();

    // Pass 1: collect raw items keeping every title lang
    let mut raw: Vec<RawCarouselItem> = Vec::new();
    let mut pos = 0;
    while pos < xml.len() {
        let Some(rel_lt) = xml[pos..].find('<') else {
            break;
        };
        let lt = pos + rel_lt;
        let Some(rel_gt) = xml[lt + 1..].find('>') else {
            break;
        };
        let gt = lt + 1 + rel_gt;

        let chunk = xml[lt + 1..gt].trim();
        if chunk.starts_with('/') || chunk.starts_with('!') || chunk.starts_with('?') {
            pos = gt + 1;
            continue;
        }

        let self_closing = chunk.ends_with('/');
        let tag = chunk.trim_end_matches('/').trim();
        let name = tag.split_ascii_whitespace().next().unwrap_or("");

        match name {
            "app" => {
                if let Some(id) = tag.find("id=\"").and_then(|p| {
                    let after = &tag[p + 4..];
                    after.find('"').map(|e| after[..e].trim().to_string())
                }) {
                    if !id.is_empty() {
                        raw.push(RawCarouselItem::App(id));
                    }
                }
                pos = gt + 1;
            }
            "story" if !self_closing => {
                let banner_url = tag.find("banner=\"").and_then(|p| {
                    let after = &tag[p + 8..];
                    after.find('"').map(|e| after[..e].to_string())
                });

                const CLOSE: &str = "</story>";
                let body_start = gt + 1;
                let body_end = xml[body_start..]
                    .find(CLOSE)
                    .map(|i| i + body_start)
                    .unwrap_or(xml.len());
                let inner = &xml[body_start..body_end];

                let titles = extract_all_titles(inner);
                let body_text = inner
                    .find("<body>")
                    .and_then(|bp| {
                        let after = &inner[bp + 6..];
                        after.find("</body>").map(|e| after[..e].trim().to_string())
                    })
                    .unwrap_or_default();

                if !titles.is_empty() {
                    raw.push(RawCarouselItem::Story(RawStory {
                        banner_url,
                        body: body_text,
                        titles,
                    }));
                }
                pos = body_end + CLOSE.len();
            }
            _ => {
                pos = gt + 1;
            }
        }
    }

    // Pass 2: locale selection
    // If any story has a title for the user's locale, prefer that; otherwise
    // fall back to "en" so e.g. a French user still sees the English stories.
    let has_user_lang = raw.iter().any(|item| {
        matches!(item, RawCarouselItem::Story(s) if s.titles.iter().any(|(l, _)| l == &user_lang))
    });
    let preferred: &str = if has_user_lang { &user_lang } else { "en" };

    let mut items = Vec::new();
    for item in raw {
        match item {
            RawCarouselItem::App(id) => items.push(CarouselItem::App(id)),
            RawCarouselItem::Story(s) => {
                // Only include this story element if it carries the preferred lang.
                // Stories with a single lang that isn't preferred belong to another locale.
                let title = s
                    .titles
                    .iter()
                    .find(|(l, _)| l.as_str() == preferred)
                    .map(|(_, t)| t.clone());

                if let Some(title) = title {
                    items.push(CarouselItem::Story(ForgeStory {
                        banner_url: s.banner_url,
                        title,
                        body: s.body,
                    }));
                }
            }
        }
    }

    items
}

fn extract_app_ids(xml: &str) -> Vec<String> {
    let mut ids = Vec::new();
    let mut s = xml;
    while let Some(p) = s.find("<app") {
        s = &s[p + 4..];
        if let Some(q) = s.find("id=\"") {
            s = &s[q + 4..];
            if let Some(e) = s.find('"') {
                let id = s[..e].trim().to_string();
                if !id.is_empty() {
                    ids.push(id);
                }
                s = &s[e + 1..];
            }
        }
    }
    ids
}

fn extract_links_items(xml: &str) -> Vec<FpLinksItem> {
    let sys = sys_locale::get_locale().unwrap_or_default();
    let user_lang = sys.split(['_', '-']).next().unwrap_or("en").to_string();

    // Pass 1: collect raw items preserving all title langs for stories
    enum RawLinksItem {
        Url { text: String, href: String },
        App(String),
        Story(RawStory),
    }

    let mut raw: Vec<RawLinksItem> = Vec::new();
    let mut pos = 0;
    while pos < xml.len() {
        let Some(rel_lt) = xml[pos..].find('<') else { break };
        let lt = pos + rel_lt;
        let Some(rel_gt) = xml[lt + 1..].find('>') else { break };
        let gt = lt + 1 + rel_gt;

        let chunk = xml[lt + 1..gt].trim();
        if chunk.starts_with('/') || chunk.starts_with('!') || chunk.starts_with('?') {
            pos = gt + 1;
            continue;
        }
        let self_closing = chunk.ends_with('/');
        let tag = chunk.trim_end_matches('/').trim();
        let name = tag.split_ascii_whitespace().next().unwrap_or("");

        match name {
            "a" if !self_closing => {
                let href = tag.find("href=\"").and_then(|p| {
                    let after = &tag[p + 6..];
                    after.find('"').map(|e| after[..e].to_string())
                }).unwrap_or_default();
                let text_start = gt + 1;
                let text_end = xml[text_start..].find("</a>").map(|e| text_start + e).unwrap_or(xml.len());
                let text = xml[text_start..text_end].trim().to_string();
                pos = text_end + 4;
                if !href.is_empty() {
                    raw.push(RawLinksItem::Url { text, href });
                }
            }
            "app" => {
                let id = tag.find("id=\"").and_then(|p| {
                    let after = &tag[p + 4..];
                    after.find('"').map(|e| after[..e].trim().to_string())
                }).unwrap_or_default();
                pos = gt + 1;
                if !id.is_empty() {
                    raw.push(RawLinksItem::App(id));
                }
            }
            "story" if !self_closing => {
                let banner_url = tag.find("banner=\"").and_then(|p| {
                    let after = &tag[p + 8..];
                    after.find('"').map(|e| after[..e].to_string())
                });
                const CLOSE: &str = "</story>";
                let body_start = gt + 1;
                let body_end = xml[body_start..].find(CLOSE).map(|i| i + body_start).unwrap_or(xml.len());
                let inner = &xml[body_start..body_end];
                let titles = extract_all_titles(inner);
                let body_text = inner.find("<body>").and_then(|bp| {
                    let after = &inner[bp + 6..];
                    after.find("</body>").map(|e| after[..e].trim().to_string())
                }).unwrap_or_default();
                pos = body_end + CLOSE.len();
                if !titles.is_empty() {
                    raw.push(RawLinksItem::Story(RawStory {
                        banner_url,
                        body: body_text,
                        titles,
                    }));
                }
            }
            _ => { pos = gt + 1; }
        }
    }

    // Pass 2: determine globally preferred language then filter stories
    let has_user_lang = raw.iter().any(|item| {
        matches!(item, RawLinksItem::Story(s) if s.titles.iter().any(|(l, _)| l == &user_lang))
    });
    let preferred: &str = if has_user_lang { &user_lang } else { "en" };

    let mut items = Vec::new();
    for item in raw {
        match item {
            RawLinksItem::Url { text, href } => items.push(FpLinksItem::Url { text, href }),
            RawLinksItem::App(id) => items.push(FpLinksItem::App(id)),
            RawLinksItem::Story(s) => {
                if let Some(title) = s.titles.iter().find(|(l, _)| l.as_str() == preferred).map(|(_, t)| t.clone()) {
                    items.push(FpLinksItem::Story(ForgeStory {
                        banner_url: s.banner_url,
                        title,
                        body: s.body,
                    }));
                }
            }
        }
    }
    items
}

fn extract_localized_title(xml: &str) -> String {
    let try_lang = |lang: &str| -> Option<String> {
        let marker = format!("lang=\"{}\"", lang);
        let mp = xml.find(&marker)?;
        let tag_start = xml[..mp].rfind('<')?;
        let chunk = &xml[tag_start..];
        let gt = chunk.find('>')?;
        let end = chunk[gt + 1..].find("</title>")? + gt + 1;
        Some(chunk[gt + 1..end].trim().to_string())
    };
    // Use system locale's language prefix, fall back to "en"
    let sys = sys_locale::get_locale().unwrap_or_default();
    let lang = sys.split(['_', '-']).next().unwrap_or("en");
    try_lang(lang)
        .or_else(|| try_lang("en"))
        .or_else(|| {
            let p = xml.find("<title")?;
            let chunk = &xml[p..];
            let gt = chunk.find('>')?;
            let end = chunk[gt + 1..].find("</title>")? + gt + 1;
            Some(chunk[gt + 1..end].trim().to_string())
        })
        .unwrap_or_default()
}
