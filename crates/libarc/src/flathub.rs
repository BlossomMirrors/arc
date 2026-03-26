use serde::Deserialize;

const BASE: &str = "https://flathub.org/api/v2";

const EXCLUDED_CATEGORIES: &[&str] = &["game", "games", "amusement"];

const EXCLUDED_IDS: &[&str] = &[
    "net.supertuxkart.SuperTuxKart",
    "org.tuxpaint.Tuxpaint",
    "com.visualstudio.code",
];

#[derive(Debug, Clone, Deserialize)]
pub struct FlathubApp {
    pub app_id: String,
    pub name: String,
    pub summary: String,
    pub icon: Option<String>,
    pub installs_last_month: Option<u64>,
    pub main_categories: Option<String>,
    pub description: Option<String>,
    pub developer_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct HitsResponse {
    hits: Vec<FlathubApp>,
}

pub const CATEGORIES: &[(&str, &str, &str)] = &[
    ("AudioVideo", "Multimedia", "applications-multimedia"),
    ("Development", "Developer Tools", "applications-development"),
    ("Education", "Education", "applications-education"),
    ("Graphics", "Graphics", "applications-graphics"),
    ("Network", "Internet", "applications-internet"),
    ("Office", "Office", "applications-office"),
    ("Science", "Science", "applications-science"),
    ("System", "System", "applications-system"),
    ("Utility", "Utilities", "applications-utilities"),
];

pub fn filter_apps(apps: Vec<FlathubApp>) -> Vec<FlathubApp> {
    apps.into_iter()
        .filter(|app| {
            if app.icon.as_deref().map(|s| s.is_empty()).unwrap_or(true) {
                return false;
            }
            if EXCLUDED_IDS.contains(&app.app_id.as_str()) {
                return false;
            }
            if let Some(ref cats) = app.main_categories {
                let cats_lower = cats.to_lowercase();
                for excluded in EXCLUDED_CATEGORIES {
                    if cats_lower.contains(excluded) {
                        return false;
                    }
                }
            }
            true
        })
        .collect()
}

pub async fn fetch_popular() -> anyhow::Result<Vec<FlathubApp>> {
    let client = reqwest::Client::new();
    let resp: HitsResponse = client
        .get(format!("{}/popular/apps", BASE))
        .send()
        .await?
        .json()
        .await?;
    Ok(filter_apps(resp.hits))
}

pub async fn fetch_recently_added() -> anyhow::Result<Vec<FlathubApp>> {
    let client = reqwest::Client::new();
    let resp: HitsResponse = client
        .get(format!("{}/collection/recently-added", BASE))
        .send()
        .await?
        .json()
        .await?;
    Ok(filter_apps(resp.hits))
}

pub async fn fetch_category(category: &str) -> anyhow::Result<Vec<FlathubApp>> {
    let client = reqwest::Client::new();
    let resp: HitsResponse = client
        .get(format!("{}/collection/category/{}", BASE, category))
        .send()
        .await?
        .json()
        .await?;
    Ok(filter_apps(resp.hits))
}

pub async fn search(query: &str) -> anyhow::Result<Vec<FlathubApp>> {
    let client = reqwest::Client::new();
    let resp: HitsResponse = client
        .post(format!("{}/search", BASE))
        .json(&serde_json::json!({"query": query, "per_page": 25}))
        .send()
        .await?
        .json()
        .await?;
    Ok(resp.hits)
}

pub async fn fetch_app(app_id: &str) -> anyhow::Result<Option<FlathubApp>> {
    let apps = search(app_id).await?;
    Ok(apps.into_iter().find(|a| a.app_id == app_id))
}
