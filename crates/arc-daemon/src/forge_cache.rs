use futures_util::future::join_all;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;
use tokio::sync::{Mutex, RwLock};
use tracing::{info, warn};

const FORGE_BASE: &str = "https://forge.blossomos.org";
const DAEMON_BASE: &str = "http://localhost:1312";

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
    // id -> (bytes, content_type)
    icons: HashMap<String, (Vec<u8>, String)>,
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

fn disk_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".cache/arc-daemon/forge_cache.json"))
}

pub async fn warm_from_disk() {
    let Some(path) = disk_path() else { return };
    let Ok(bytes) = std::fs::read(&path) else { return };
    let Ok(p) = serde_json::from_slice::<Persisted>(&bytes) else { return };
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
    let Some(path) = disk_path() else { return };
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
    tokio::task::spawn_blocking(move || {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(bytes) = serde_json::to_vec(&p) {
            let _ = std::fs::write(&path, bytes);
        }
    })
    .await
    .ok();
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
    let client = reqwest::Client::new();
    let url_top = format!("{}/api/top?limit=12", FORGE_BASE);
    let url_new = format!("{}/api/new?limit=20", FORGE_BASE);
    let url_trending = format!("{}/api/trending?limit=12", FORGE_BASE);
    let url_charts = format!("{}/api/charts?limit=12", FORGE_BASE);
    let (top, new, trending, charts) = tokio::join!(
        fetch_text(&client, &url_top),
        fetch_text(&client, &url_new),
        fetch_text(&client, &url_trending),
        fetch_text(&client, &url_charts),
    );

    let top_str = top.as_deref().unwrap_or("");
    let new_str = new.as_deref().unwrap_or("");
    let trend_str = trending.as_deref().unwrap_or("");
    let chart_str = charts.as_deref().unwrap_or("");

    let app_ids = collect_home_app_ids(top_str, new_str, trend_str, chart_str);

    // Resolve metadata from AppStreamDb — runs in a blocking thread since the
    // db scan is CPU-bound, and returns (metadata_json, original_icon_urls)
    let app_ids_for_db = app_ids.clone();
    let (app_metadata_json, original_icon_urls) = tokio::task::spawn_blocking(move || {
        let db = crate::appstream_db::AppStreamDb::get();
        let mut original_urls: HashMap<String, String> = HashMap::new();
        let entries: Vec<serde_json::Value> = app_ids_for_db
            .iter()
            .filter_map(|id| db.find_by_id(id).map(|e| (id, e)))
            .map(|(id, e)| {
                let display_url = match &e.icon_url {
                    Some(url) if url.starts_with("http") => {
                        original_urls.insert(id.clone(), url.clone());
                        format!("{}/forge/icon/{}", DAEMON_BASE, id)
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
        )
    })
    .await
    .unwrap_or_default();

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

    // Download icon bytes in the background so the cache is warm for the
    // next request without blocking startup
    tokio::spawn(async move {
        let futs: Vec<_> = original_icon_urls
            .into_iter()
            .map(|(id, url)| async move {
                let resp = reqwest::Client::new()
                    .get(&url)
                    .timeout(std::time::Duration::from_secs(10))
                    .send()
                    .await;
                match resp {
                    Ok(r) => {
                        let ct = r
                            .headers()
                            .get(reqwest::header::CONTENT_TYPE)
                            .and_then(|v| v.to_str().ok())
                            .unwrap_or("image/png")
                            .to_string();
                        r.bytes().await.ok().map(|b| (id, b.to_vec(), ct))
                    }
                    Err(_) => None,
                }
            })
            .collect();

        let results = join_all(futs).await;
        let total = results.len();
        let mut icons: HashMap<String, (Vec<u8>, String)> = HashMap::new();
        for entry in results.into_iter().flatten() {
            icons.insert(entry.0, (entry.1, entry.2));
        }
        let (ok, fail) = (icons.len(), total - icons.len());
        lock().write().await.icons = icons;
        info!("Icon cache populated ({} ok, {} failed)", ok, fail);
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
    // Try the warm cache first
    if let Some(entry) = lock().read().await.icons.get(id).cloned() {
        return Some(entry);
    }
    // Cache miss: fetch from the original URL and store for next time
    let original_url = lock().read().await.original_icon_urls.get(id).cloned()?;
    let resp = reqwest::Client::new()
        .get(&original_url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .ok()?;
    let ct = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("image/png")
        .to_string();
    let bytes = resp.bytes().await.ok()?.to_vec();
    lock()
        .write()
        .await
        .icons
        .insert(id.to_string(), (bytes.clone(), ct.clone()));
    Some((bytes, ct))
}
