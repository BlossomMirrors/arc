use futures_util::future::join_all;
use libarc::cache::JsonCache;
use std::collections::HashMap;
use std::sync::OnceLock;
use tokio::spawn;
use tokio::sync::{Mutex, RwLock};
use tokio::task::spawn_blocking;
use tracing::{info, warn};

const FORGE_BASE: &str = libarc::FORGE_BASE_URL;

#[derive(Default)]
struct Inner {
    top_json: String,
    new_json: String,
    trending_json: String,
    charts_json: String,
    // JSON: [{id, name, summary, icon_url}] where icon_url points to /forge/icon/{id}
    // for remote icons, or keeps "local:..." for locally-installed apps
    app_metadata_json: String,
    // id -> original remote icon URL (used by the icon proxy endpoint)
    original_icon_urls: HashMap<String, String>,
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Persisted {
    top_json: String,
    new_json: String,
    trending_json: String,
    charts_json: String,
    app_metadata_json: String,
    original_icon_urls: HashMap<String, String>,
}

static CACHE: OnceLock<RwLock<Inner>> = OnceLock::new();
static REFRESH_GATE: OnceLock<Mutex<()>> = OnceLock::new();

fn lock() -> &'static RwLock<Inner> {
    CACHE.get_or_init(|| RwLock::new(Inner::default()))
}

fn refresh_gate() -> &'static Mutex<()> {
    REFRESH_GATE.get_or_init(|| Mutex::new(()))
}

static STORE: OnceLock<JsonCache<Persisted>> = OnceLock::new();

fn store() -> &'static JsonCache<Persisted> {
    STORE.get_or_init(|| JsonCache::new("daemon", "forge.json"))
}

pub async fn warm_from_disk() {
    let Some(p) = store().load_async().await else { return };
    let mut w = lock().write().await;
    w.top_json = p.top_json;
    w.new_json = p.new_json;
    w.trending_json = p.trending_json;
    w.charts_json = p.charts_json;
    w.app_metadata_json = p.app_metadata_json;
    w.original_icon_urls = p.original_icon_urls;
    info!("Forge cache warmed from disk");
}

async fn save_to_disk() {
    let p = {
        let r = lock().read().await;
        Persisted {
            top_json: r.top_json.clone(),
            new_json: r.new_json.clone(),
            trending_json: r.trending_json.clone(),
            charts_json: r.charts_json.clone(),
            app_metadata_json: r.app_metadata_json.clone(),
            original_icon_urls: r.original_icon_urls.clone(),
        }
    };
    store().store(&p);
}

async fn ensure_fresh() {
    let empty = lock().read().await.top_json.is_empty();
    if !empty {
        return;
    }
    let _guard = refresh_gate().lock().await;
    if lock().read().await.top_json.is_empty() {
        refresh().await;
    }
}

#[derive(serde::Deserialize)]
struct VerifiedResponse {
    verified: bool,
    developer_name: Option<String>,
}

async fn fetch_blossomos_verification(
    client: &reqwest::Client,
    app_ids: &[String],
) -> HashMap<String, crate::appstream_db::ForgeVerification> {
    use futures_util::stream::{self, StreamExt};

    stream::iter(app_ids.iter().cloned())
        .map(|id| {
            let client = client.clone();
            async move {
                let url = format!("{}/api/verified/{}", FORGE_BASE, id);
                let resp = client
                    .get(&url)
                    .timeout(std::time::Duration::from_secs(10))
                    .send()
                    .await
                    .ok()?;
                if !resp.status().is_success() {
                    return None;
                }
                let parsed: VerifiedResponse = resp.json().await.ok()?;
                Some((
                    id,
                    crate::appstream_db::ForgeVerification {
                        verified: parsed.verified,
                        developer_name: parsed.developer_name,
                    },
                ))
            }
        })
        .buffer_unordered(8)
        .filter_map(|r| async move { r })
        .collect()
        .await
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

fn collect_home_app_ids(
    top_json: &str,
    new_json: &str,
    trending_json: &str,
    charts_json: &str,
) -> Vec<String> {
    let mut ids: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Extract all <app id="..."> tags from the top/new/trending/charts JSON
    for json in [top_json, new_json, trending_json, charts_json] {
        let mut rest = json;
        while let Some(p) = rest.find("<app") {
            rest = &rest[p + 4..];
            if let Some(q) = rest.find("id=\"") {
                rest = &rest[q + 4..];
                if let Some(e) = rest.find('"') {
                    let id = rest[..e].trim().to_string();
                    if !id.is_empty() {
                        ids.insert(id);
                    }
                    rest = &rest[e + 1..];
                }
            }
        }
    }

    // top/new/trending are Vec<String>
    for json in [top_json, new_json, trending_json] {
        if let Ok(list) = serde_json::from_str::<Vec<String>>(json) {
            ids.extend(list);
        }
    }

    // charts is Vec<{id: ...}>
    #[derive(serde::Deserialize)]
    struct ChartEntry {
        id: String,
    }
    if let Ok(list) = serde_json::from_str::<Vec<ChartEntry>>(charts_json) {
        for e in list {
            ids.insert(e.id);
        }
    }

    ids.into_iter().collect()
}

pub async fn refresh() {
    let client = crate::media::http_client();
    let url_top = format!("{}/api/top?limit=12", FORGE_BASE);
    let url_new = format!("{}/api/new?limit=20", FORGE_BASE);
    let url_trending = format!("{}/api/trending?limit=12", FORGE_BASE);
    let url_charts = format!("{}/api/charts?limit=12", FORGE_BASE);
    let (top, new, trending, charts) = tokio::join!(
        fetch_text(client, &url_top),
        fetch_text(client, &url_new),
        fetch_text(client, &url_trending),
        fetch_text(client, &url_charts),
    );

    let top_str = top.as_deref().unwrap_or("");
    let new_str = new.as_deref().unwrap_or("");
    let trend_str = trending.as_deref().unwrap_or("");
    let chart_str = charts.as_deref().unwrap_or("");

    let app_ids = collect_home_app_ids(top_str, new_str, trend_str, chart_str);

    // Resolve metadata from AppStreamDb. Runs in a blocking thread since the
    // db scan is CPU-bound, and returns (metadata_json, original_icon_urls, blossomos_ids)
    let app_ids_for_db = app_ids.clone();
    let (app_metadata_json, original_icon_urls, blossomos_ids) = spawn_blocking(move || {
        let db = crate::appstream_db::AppStreamDb::get();
        let mut original_urls: HashMap<String, String> = HashMap::new();
        let entries: Vec<serde_json::Value> = app_ids_for_db
            .iter()
            .filter_map(|id| db.find_by_id(id).map(|e| (id, e)))
            .map(|(id, e)| {
                let display_url = match &e.icon_url {
                    Some(url) if matches!(libarc::media::classify(url), libarc::media::MediaRef::Remote(_)) => {
                        original_urls.insert(id.clone(), url.clone());
                        crate::media::forge_icon_proxy_url(id)
                    }
                    other => other.clone().unwrap_or_default(),
                };
                serde_json::json!({
                    "id": e.id,
                    "name": e.name,
                    "summary": e.summary,
                    "icon_url": display_url,
                })
            })
            .collect();
        (
            serde_json::to_string(&entries).unwrap_or_default(),
            original_urls,
            db.blossomos_app_ids(),
        )
    })
    .await
    .unwrap_or_default();

    if !blossomos_ids.is_empty() {
        let verification = fetch_blossomos_verification(client, &blossomos_ids).await;
        let count = verification.len();
        crate::appstream_db::set_blossomos_verification(verification);
        info!("Blossomos verification refreshed ({}/{} apps)", count, blossomos_ids.len());
    }

    {
        let mut w = lock().write().await;
        if let Some(v) = top {
            w.top_json = v;
        }
        if let Some(v) = new {
            w.new_json = v;
        }
        if let Some(v) = trending {
            w.trending_json = v;
        }
        if let Some(v) = charts {
            w.charts_json = v;
        }
        if !app_metadata_json.is_empty() {
            w.app_metadata_json = app_metadata_json;
        }
        let icon_url_count = original_icon_urls.len();
        w.original_icon_urls = original_icon_urls.clone();
        info!("Forge cache refreshed ({} apps resolved)", icon_url_count);
    }

    save_to_disk().await;

    spawn(async move {
        let futs: Vec<_> = original_icon_urls
            .into_values()
            .map(|url| async move { crate::media::fetch(crate::media::MediaKind::Icon, &url, None).await.is_some() })
            .collect();

        let results = join_all(futs).await;
        let total = results.len();
        let ok = results.into_iter().filter(|ok| *ok).count();
        info!("Icon cache warmed ({} ok, {} failed)", ok, total - ok);
    });
}

pub async fn top() -> String {
    ensure_fresh().await;
    lock().read().await.top_json.clone()
}
pub async fn new_apps() -> String {
    ensure_fresh().await;
    lock().read().await.new_json.clone()
}
pub async fn trending() -> String {
    ensure_fresh().await;
    lock().read().await.trending_json.clone()
}
pub async fn charts() -> String {
    ensure_fresh().await;
    lock().read().await.charts_json.clone()
}
pub async fn app_metadata() -> String {
    ensure_fresh().await;
    lock().read().await.app_metadata_json.clone()
}

pub async fn icon_bytes(id: &str) -> Option<(Vec<u8>, String)> {
    let original_url = lock().read().await.original_icon_urls.get(id).cloned()?;
    crate::media::fetch(crate::media::MediaKind::Icon, &original_url, None).await
}
