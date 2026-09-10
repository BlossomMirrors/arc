use libarc::cache::{Blob, BlobCache};
use libarc::media::{classify, MediaRef};
use std::sync::OnceLock;
use std::time::Duration;
use tokio::task::spawn_blocking;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    Icon,
    Screenshot,
}

static HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

pub fn http_client() -> &'static reqwest::Client {
    HTTP_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .build()
            .unwrap_or_default()
    })
}

const MEDIA_TTL: Duration = Duration::from_secs(7 * 24 * 3600);

const ICON_TTL: Duration = Duration::from_secs(6 * 3600);

static MEDIA: OnceLock<BlobCache> = OnceLock::new();
static ICONS: OnceLock<BlobCache> = OnceLock::new();

fn media_cache() -> &'static BlobCache {
    MEDIA.get_or_init(|| BlobCache::new("media", MEDIA_TTL))
}

fn icon_cache() -> &'static BlobCache {
    ICONS.get_or_init(|| BlobCache::new("media-icons", ICON_TTL))
}

fn cache_for(kind: MediaKind) -> &'static BlobCache {
    match kind {
        MediaKind::Icon => icon_cache(),
        MediaKind::Screenshot => media_cache(),
    }
}

pub fn sweep() -> usize {
    media_cache().sweep() + icon_cache().sweep()
}

pub fn clear_icons() {
    icon_cache().clear();
}

fn is_svg(content_type: &str, url: &str) -> bool {
    content_type.contains("svg") || url.ends_with(".svg") || url.ends_with(".svgz")
}

pub async fn fetch_raw(url: &str) -> Option<(Vec<u8>, String)> {
    fetch_raw_into(MediaKind::Icon, url).await
}

async fn fetch_raw_into(kind: MediaKind, url: &str) -> Option<(Vec<u8>, String)> {
    let key = format!("raw:{url}");
    let url_owned = url.to_string();
    let blob = cache_for(kind)
        .get_or_fetch(&key, || async move {
            let resp = http_client().get(&url_owned).send().await.ok()?;
            if !resp.status().is_success() {
                return None;
            }
            let ct = resp
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("application/octet-stream")
                .to_string();
            let bytes = resp.bytes().await.ok()?.to_vec();
            Some(Blob::new(bytes, ct))
        })
        .await?;
    Some((blob.bytes.as_ref().clone(), blob.content_type.to_string()))
}

pub async fn fetch(kind: MediaKind, url: &str, width: Option<u32>) -> Option<(Vec<u8>, String)> {
    let key = match (kind, width) {
        (MediaKind::Icon, _) => format!("icon:{url}"),
        (MediaKind::Screenshot, Some(w)) => format!("shot:{url}#w={w}"),
        (MediaKind::Screenshot, _) => format!("shot:{url}"),
    };
    let url_owned = url.to_string();
    let blob = cache_for(kind)
        .get_or_fetch(&key, || async move {
            let (bytes, content_type) = fetch_raw_into(kind, &url_owned).await?;
            let result = match kind {
                MediaKind::Icon => {
                    let padded = if is_svg(&content_type, &url_owned) {
                        libarc::icons::normalize_padding_svg(&bytes)
                    } else {
                        libarc::icons::normalize_padding(&bytes)
                    };
                    padded.map(|p| (p, "image/png".to_string())).unwrap_or((bytes, content_type))
                }
                MediaKind::Screenshot => match width {
                    Some(w) => {
                        let b = bytes.clone();
                        let resized = spawn_blocking(move || resize_image(&b, w)).await.ok().flatten();
                        resized.map(|j| (j, "image/jpeg".to_string())).unwrap_or((bytes, content_type))
                    }
                    _ => (bytes, content_type),
                },
            };
            Some(Blob::new(result.0, result.1))
        })
        .await?;
    Some((blob.bytes.as_ref().clone(), blob.content_type.to_string()))
}

fn resize_image(bytes: &[u8], target_w: u32) -> Option<Vec<u8>> {
    let img = image::load_from_memory(bytes).ok()?;
    if img.width() <= target_w {
        return None;
    }
    let target_h = ((img.height() as u64 * target_w as u64) / img.width() as u64).max(1) as u32;
    let resized = img.resize(target_w, target_h, image::imageops::FilterType::Triangle);
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(resized.into_rgb8())
        .write_to(&mut out, image::ImageFormat::Jpeg)
        .ok()?;
    Some(out.into_inner())
}

pub fn icon_proxy_url(package_id: &str) -> String {
    format!("{}/api/v1/apps/{}/icon", libarc::DAEMON_HTTP_BASE, package_id)
}

pub fn image_proxy_url(remote_url: &str) -> String {
    let encoded = percent_encoding::utf8_percent_encode(remote_url, percent_encoding::NON_ALPHANUMERIC);
    format!("{}/api/v1/image?url={encoded}", libarc::DAEMON_HTTP_BASE)
}

pub fn forge_icon_proxy_url(package_id: &str) -> String {
    format!("{}/forge/icon/{}", libarc::DAEMON_HTTP_BASE, package_id)
}

pub fn proxy_icon_url_field(icon_url: &mut Option<String>, package_id: &str) {
    if let Some(url) = icon_url.as_deref() {
        if matches!(classify(url), MediaRef::Remote(_)) {
            *icon_url = Some(icon_proxy_url(package_id));
        }
    }
}

pub fn proxy_screenshot_urls(screenshots: &mut [String]) {
    for s in screenshots.iter_mut() {
        if let MediaRef::Remote(u) = classify(s) {
            *s = image_proxy_url(u);
        }
    }
}
