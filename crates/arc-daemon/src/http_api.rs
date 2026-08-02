use axum::{
    extract::{Path, Query},
    http::{header, StatusCode},
    response::{IntoResponse, Json, Response},
    routing::get,
    Router,
};
use serde::Deserialize;
use tower_http::cors::{Any, CorsLayer};

use crate::appstream_db::{parse_locale_candidates, score_entry, AppStreamDb, AppStreamEntry};

pub fn router() -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/api/v1/search", get(search))
        .route("/api/v1/home", get(home))
        .route("/api/v1/category/{name}", get(category))
        .route("/api/v1/apps/{id}", get(app_metadata))
        .route("/api/v1/apps/{id}/icon", get(app_icon))
        .route("/api/v1/image", get(proxy_image))
        .route("/forge/api/top", get(forge_top))
        .route("/forge/api/new", get(forge_new))
        .route("/forge/api/trending", get(forge_trending))
        .route("/forge/api/charts", get(forge_charts))
        .route("/forge/api/pwas", get(forge_pwas))
        .route("/forge/api/app-metadata", get(forge_app_metadata))
        .route("/forge/icon/{id}", get(forge_icon))
        .layer(cors)
}

// ---------------------------------------------------------------------------
// Shared lang param — present on every endpoint
// ---------------------------------------------------------------------------

#[derive(Deserialize, Default)]
struct LangParam {
    /// BCP-47 / POSIX language tag, e.g. "de" or "de_DE".
    /// Omit or pass "en" for English (the AppStream default).
    lang: Option<String>,
}

impl LangParam {
    fn locales(&self) -> Vec<String> {
        self.lang
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(parse_locale_candidates)
            .unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// Query param structs
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct SearchParams {
    q: String,
    #[serde(flatten)]
    lang: LangParam,
}

#[derive(Deserialize)]
struct HomeParams {
    #[serde(default = "default_popular")]
    popular: u32,
    #[serde(default = "default_recent")]
    recent: u32,
    #[serde(flatten)]
    lang: LangParam,
}
fn default_popular() -> u32 {
    12
}
fn default_recent() -> u32 {
    24
}

#[derive(Deserialize)]
struct CategoryParams {
    #[serde(flatten)]
    lang: LangParam,
}

#[derive(Deserialize)]
struct AppParams {
    #[serde(flatten)]
    lang: LangParam,
}

#[derive(Deserialize)]
struct ImageParams {
    url: String,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn search(Query(p): Query<SearchParams>) -> impl IntoResponse {
    let locales = p.lang.locales();
    let mut results: Vec<(AppStreamEntry, u32)> = tokio::task::spawn_blocking(move || {
        AppStreamDb::get()
            .search_apps_with_locales(&p.q, &locales)
            .into_iter()
            .filter_map(|e| score_entry(&e, &p.q.to_lowercase()).map(|s| (e, s)))
            .collect()
    })
    .await
    .unwrap_or_default();
    results.sort_by(|a, b| b.1.cmp(&a.1));
    let response: Vec<serde_json::Value> = results
        .into_iter()
        .map(|(entry, score)| {
            let mut v = serde_json::to_value(&entry).unwrap_or_default();
            if let Some(obj) = v.as_object_mut() {
                obj.insert("score".into(), score.into());
            }
            v
        })
        .collect();
    Json(response)
}

async fn home(Query(p): Query<HomeParams>) -> impl IntoResponse {
    let locales = p.lang.locales();
    let (popular, recent) = tokio::task::spawn_blocking(move || {
        let db = AppStreamDb::get();
        (
            db.get_popular_apps_with_locales(p.popular as usize, &locales),
            db.get_recent_apps_with_locales(p.recent as usize, &locales),
        )
    })
    .await
    .unwrap_or_default();

    Json(serde_json::json!({ "popular": popular, "recent": recent }))
}

async fn category(Path(name): Path<String>, Query(p): Query<CategoryParams>) -> impl IntoResponse {
    let locales = p.lang.locales();
    let results = tokio::task::spawn_blocking(move || {
        AppStreamDb::get().get_apps_by_category_with_locales(&name, &locales)
    })
    .await
    .unwrap_or_default();
    Json(results)
}

async fn app_metadata(Path(id): Path<String>, Query(p): Query<AppParams>) -> Response {
    let locales = p.lang.locales();
    let result = tokio::task::spawn_blocking(move || {
        let db = AppStreamDb::get();
        db.find_by_id_with_locales(&id, &locales)
            .or_else(|| db.load_from_exported_metainfo_with_locales(&id, &locales))
    })
    .await
    .unwrap_or(None);

    match result {
        Some(entry) => Json(entry).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn app_icon(Path(id): Path<String>) -> Response {
    let icon_url = tokio::task::spawn_blocking(move || {
        AppStreamDb::get()
            .find_by_id(&id)
            .and_then(|e| e.icon_url)
    })
    .await
    .unwrap_or(None);

    let Some(url) = icon_url else {
        return StatusCode::NOT_FOUND.into_response();
    };

    if !url.starts_with("https://") && !url.starts_with("http://") {
        return StatusCode::NOT_FOUND.into_response();
    }

    fetch_and_forward(&url).await
}

// Proxy a remote image URL (e.g. screenshots). Only http(s) URLs are accepted.
async fn proxy_image(Query(p): Query<ImageParams>) -> Response {
    if !p.url.starts_with("https://") && !p.url.starts_with("http://") {
        return (StatusCode::BAD_REQUEST, "Only http(s) URLs are supported").into_response();
    }
    fetch_and_forward(&p.url).await
}

async fn forge_top() -> impl IntoResponse {
    let json = crate::forge_cache::top().await;
    ([(header::CONTENT_TYPE, "application/json")], json)
}

async fn forge_new() -> impl IntoResponse {
    let json = crate::forge_cache::new_apps().await;
    ([(header::CONTENT_TYPE, "application/json")], json)
}

async fn forge_trending() -> impl IntoResponse {
    let json = crate::forge_cache::trending().await;
    ([(header::CONTENT_TYPE, "application/json")], json)
}

async fn forge_charts() -> impl IntoResponse {
    let json = crate::forge_cache::charts().await;
    ([(header::CONTENT_TYPE, "application/json")], json)
}

#[derive(Deserialize)]
struct PwasParams {
    #[serde(default)]
    lang: String,
}

async fn forge_app_metadata() -> impl IntoResponse {
    let json = crate::forge_cache::app_metadata().await;
    ([(header::CONTENT_TYPE, "application/json")], json)
}

async fn forge_icon(Path(id): Path<String>) -> Response {
    match crate::forge_cache::icon_bytes(&id).await {
        Some((bytes, ct)) => ([(header::CONTENT_TYPE, ct)], bytes).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn forge_pwas(Query(p): Query<PwasParams>) -> Response {
    let url = format!("https://forge.blossomos.org/api/pwas?lang={}", p.lang);
    fetch_and_forward(&url).await
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn fetch_and_forward(url: &str) -> Response {
    let resp = match reqwest::get(url).await {
        Ok(r) => r,
        Err(_) => return StatusCode::BAD_GATEWAY.into_response(),
    };

    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();

    let bytes = match resp.bytes().await {
        Ok(b) => b,
        Err(_) => return StatusCode::BAD_GATEWAY.into_response(),
    };

    ([(header::CONTENT_TYPE, content_type)], bytes).into_response()
}
