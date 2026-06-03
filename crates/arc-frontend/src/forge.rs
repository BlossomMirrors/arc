const FORGE_BASE: &str = "https://forge.blossomos.org";

#[derive(serde::Deserialize, Clone)]
pub struct ForgeApp {
    pub appid: String,
    pub name: String,
    pub summary: String,
    pub icon_url: Option<String>,
}

/// A section parsed from `/api/frontpage`.
#[derive(Debug, Clone)]
pub enum FpSection {
    /// Explicit list of app IDs to show (from `<carousel>`).
    Carousel(Vec<String>),
    /// Curated list of app IDs (from `<custom>`).
    Custom(Vec<String>),
    /// Top apps by total installs.
    Top,
    /// Recently added apps.
    New,
    /// Trending apps (last 30 days).
    Trending,
    /// App charts.
    Charts,
    /// Full category grid.
    Categories,
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

pub async fn fetch_apps(path: &str, limit: u32) -> Vec<ForgeApp> {
    let url = format!("{}/{}?limit={}", FORGE_BASE, path.trim_start_matches('/'), limit);
    match reqwest::Client::new()
        .get(&url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
    {
        Ok(r) => r.json::<Vec<ForgeApp>>().await.unwrap_or_default(),
        Err(_) => vec![],
    }
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
        let Some(rel_lt) = xml[pos..].find('<') else { break };
        let lt = pos + rel_lt;
        let Some(rel_gt) = xml[lt + 1..].find('>') else { break };
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
            "top" => { sections.push(FpSection::Top); pos = gt + 1; }
            "new" => { sections.push(FpSection::New); pos = gt + 1; }
            "trending" => { sections.push(FpSection::Trending); pos = gt + 1; }
            "charts" => { sections.push(FpSection::Charts); pos = gt + 1; }
            "categories" => { sections.push(FpSection::Categories); pos = gt + 1; }
            "carousel" if !self_closing => {
                const CLOSE: &str = "</carousel>";
                let body_start = gt + 1;
                let body_end = xml[body_start..].find(CLOSE)
                    .map(|i| i + body_start)
                    .unwrap_or(xml.len());
                let ids = extract_app_ids(&xml[body_start..body_end]);
                if !ids.is_empty() {
                    sections.push(FpSection::Carousel(ids));
                }
                pos = body_end + CLOSE.len();
            }
            "custom" if !self_closing => {
                const CLOSE: &str = "</custom>";
                let body_start = gt + 1;
                let body_end = xml[body_start..].find(CLOSE)
                    .map(|i| i + body_start)
                    .unwrap_or(xml.len());
                let ids = extract_app_ids(&xml[body_start..body_end]);
                if !ids.is_empty() {
                    sections.push(FpSection::Custom(ids));
                }
                pos = body_end + CLOSE.len();
            }
            _ if self_closing => pos = gt + 1,
            _ => {
                let close = format!("</{}>", name);
                pos = xml[gt + 1..].find(close.as_str())
                    .map(|i| gt + 1 + i + close.len())
                    .unwrap_or(gt + 1);
            }
        }
    }

    sections
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
                if !id.is_empty() { ids.push(id); }
                s = &s[e + 1..];
            }
        }
    }
    ids
}
