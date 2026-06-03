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
    Carousel(Vec<String>),
    Custom { title: String, app_ids: Vec<String> },
    Top,
    New,
    Trending,
    Charts { cards: bool },
    Categories,
    Story(ForgeStory),
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
#[derive(Debug, Clone)]
pub struct ForgeStory {
    pub banner_url: Option<String>,
    pub title: String,
    pub body: String,
    pub lang: String,
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
                const CLOSE: &str = "</carousel>";
                let body_start = gt + 1;
                let body_end = xml[body_start..]
                    .find(CLOSE)
                    .map(|i| i + body_start)
                    .unwrap_or(xml.len());
                let body = &xml[body_start..body_end];
                let ids = extract_app_ids(body);
                let stories = extract_stories(body);
                if !ids.is_empty() {
                    sections.push(FpSection::Carousel(ids));
                }
                for s in stories {
                    sections.push(FpSection::Story(s));
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

    sections
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

/// Parse all `<story>` elements from a block of XML and return them as `ForgeStory` values.
fn extract_stories(xml: &str) -> Vec<ForgeStory> {
    let mut stories = Vec::new();
    let mut s = xml;
    while let Some(rel) = s.find("<story") {
        let rest = &s[rel..];
        let tag_end = match rest.find('>') {
            Some(e) => e,
            None => break,
        };
        let tag = &rest[1..tag_end]; // e.g. `story banner="..."`

        // Extract banner attribute
        let banner_url = tag
            .find("banner=\"")
            .map(|p| {
                let after = &tag[p + 8..];
                after.find('"').map(|e| after[..e].to_string())
            })
            .flatten();

        let body_start = rel + tag_end + 1;

        // Find closing </story>
        const CLOSE: &str = "</story>";
        let body_end = match s[body_start..].find(CLOSE) {
            Some(e) => body_start + e,
            None => break,
        };
        let inner = &s[body_start..body_end];

        // Extract all <title lang="XX"> entries
        let mut titles: Vec<(String, String)> = Vec::new();
        let mut t = inner;
        while let Some(tp) = t.find("<title") {
            let rest_t = &t[tp..];
            let tgt = match rest_t.find('>') {
                Some(e) => e,
                None => break,
            };
            let tag_t = &rest_t[1..tgt];
            let lang = tag_t
                .find("lang=\"")
                .map(|p| {
                    let a = &tag_t[p + 6..];
                    a.find('"')
                        .map(|e| a[..e].to_string())
                        .unwrap_or_else(|| "en".to_string())
                })
                .unwrap_or_else(|| "en".to_string());

            let text_start = tp + tgt + 1;
            let text_end = t[text_start..]
                .find("</title>")
                .map(|e| text_start + e)
                .unwrap_or(t.len());
            let title_text = t[text_start..text_end].trim().to_string();

            if !title_text.is_empty() {
                titles.push((lang, title_text));
            }
            t = &t[text_end + 8..]; // skip past </title>
        }

        // Extract <body>
        let body_text = inner
            .find("<body>")
            .map(|bp| {
                let after = &inner[bp + 6..];
                after
                    .find("</body>")
                    .map(|e| after[..e].trim().to_string())
                    .unwrap_or_default()
            })
            .unwrap_or_default();

        for (lang, title) in titles {
            stories.push(ForgeStory {
                banner_url: banner_url.clone(),
                title,
                body: body_text.clone(),
                lang,
            });
        }

        s = &s[body_end + CLOSE.len()..];
    }
    stories
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
