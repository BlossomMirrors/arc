use std::sync::OnceLock;
use tokio::sync::RwLock;
use tracing::{info, warn};

const FORGE_BASE: &str = "https://forge.blossomos.org";

#[derive(Default)]
struct Inner {
    frontpage_xml: String,
    top_json: String,
    new_json: String,
    trending_json: String,
    charts_json: String,
}

static CACHE: OnceLock<RwLock<Inner>> = OnceLock::new();

fn lock() -> &'static RwLock<Inner> {
    CACHE.get_or_init(|| RwLock::new(Inner::default()))
}

async fn fetch_text(client: &reqwest::Client, url: &str) -> Option<String> {
    match client
        .get(url)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
    {
        Ok(r) => r.text().await.ok().filter(|s| !s.trim().is_empty()),
        Err(e) => {
            warn!("Forge fetch {}: {}", url, e);
            None
        }
    }
}

pub async fn refresh() {
    let client = reqwest::Client::new();
    let fp = fetch_text(&client, &format!("{}/api/frontpage", FORGE_BASE)).await;
    let top = fetch_text(&client, &format!("{}/api/top?limit=12", FORGE_BASE)).await;
    let new = fetch_text(&client, &format!("{}/api/new?limit=20", FORGE_BASE)).await;
    let trending = fetch_text(&client, &format!("{}/api/trending?limit=12", FORGE_BASE)).await;
    let charts = fetch_text(&client, &format!("{}/api/charts?limit=12", FORGE_BASE)).await;

    let mut w = lock().write().await;
    if let Some(v) = fp { w.frontpage_xml = v; }
    if let Some(v) = top { w.top_json = v; }
    if let Some(v) = new { w.new_json = v; }
    if let Some(v) = trending { w.trending_json = v; }
    if let Some(v) = charts { w.charts_json = v; }
    info!("Forge cache refreshed");
}

pub async fn frontpage() -> String { lock().read().await.frontpage_xml.clone() }
pub async fn top() -> String { lock().read().await.top_json.clone() }
pub async fn new_apps() -> String { lock().read().await.new_json.clone() }
pub async fn trending() -> String { lock().read().await.trending_json.clone() }
pub async fn charts() -> String { lock().read().await.charts_json.clone() }
