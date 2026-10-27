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

fn cache_for(kind: MediaKind) -> &'static BlobCache {
    match kind {
        MediaKind::Icon => crate::cache::icons(),
        MediaKind::Screenshot => crate::cache::images(),
    }
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

fn cache_key(kind: MediaKind, url: &str, width: Option<u32>) -> String {
    match (kind, width) {
        (MediaKind::Icon, _) => format!("icon:{url}"),
        (MediaKind::Screenshot, Some(w)) => format!("shot:{url}#w={w}"),
        (MediaKind::Screenshot, _) => format!("shot:{url}"),
    }
}

// downloads an image and turns it into what gets stored so serving it and handing out its file share one recipe
async fn download(kind: MediaKind, url: String, width: Option<u32>) -> Option<Blob> {
    let (bytes, content_type) = fetch_raw_into(kind, &url).await?;
    let (bytes, content_type) = match kind {
        MediaKind::Icon => {
            let padded = if is_svg(&content_type, &url) {
                libarc::icons::normalize_padding_svg(&bytes)
            } else {
                libarc::icons::normalize_padding(&bytes)
            };
            padded
                .map(|p| (p, "image/png".to_string()))
                .unwrap_or((bytes, content_type))
        }
        MediaKind::Screenshot => match width {
            Some(w) => {
                let b = bytes.clone();
                let resized = spawn_blocking(move || resize_image(&b, w))
                    .await
                    .ok()
                    .flatten();
                resized
                    .map(|j| (j, "image/jpeg".to_string()))
                    .unwrap_or((bytes, content_type))
            }
            _ => (bytes, content_type),
        },
    };
    Some(Blob::new(bytes, content_type))
}

pub async fn fetch(kind: MediaKind, url: &str, width: Option<u32>) -> Option<(Vec<u8>, String)> {
    let url_owned = url.to_string();
    let blob = cache_for(kind)
        .get_or_fetch(&cache_key(kind, url, width), move || {
            download(kind, url_owned, width)
        })
        .await?;
    Some((blob.bytes.as_ref().clone(), blob.content_type.to_string()))
}

pub fn local_file_url(kind: MediaKind, url: &str, width: Option<u32>) -> Option<String> {
    let url_owned = url.to_string();
    let file = cache_for(kind).local_file(&cache_key(kind, url, width), move || {
        download(kind, url_owned, width)
    })?;
    Some(format!("file://{}", file.display()))
}

// what the local frontend should load for an image which is its stored file when there is one and else the proxy url
pub fn display_url(kind: MediaKind, remote_url: &str, width: Option<u32>) -> String {
    local_file_url(kind, remote_url, width).unwrap_or_else(|| match kind {
        MediaKind::Icon => libarc::media::icon_image_proxy_url(remote_url),
        MediaKind::Screenshot => libarc::media::image_proxy_url_sized(remote_url, width),
    })
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
    format!(
        "{}/api/v1/apps/{}/icon",
        libarc::DAEMON_HTTP_BASE,
        package_id
    )
}

pub use libarc::media::image_proxy_url;

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

// icons in d-bus answers point at the daemon by their own address so resolving them later needs no provider lookup
pub fn icon_url_field(icon_url: &mut Option<String>) {
    if let Some(MediaRef::Remote(url)) = icon_url.as_deref().map(classify) {
        if !url.starts_with(libarc::DAEMON_HTTP_BASE) {
            *icon_url = Some(libarc::media::icon_image_proxy_url(url));
        }
    }
}

// an image address the daemon itself handed out earlier
pub enum OwnUrl {
    Image {
        kind: MediaKind,
        url: String,
        width: Option<u32>,
    },
    AppIcon(String),
    ForgeIcon(String),
}

pub fn parse_own_url(raw: &str) -> Option<OwnUrl> {
    if !raw.starts_with(libarc::DAEMON_HTTP_BASE) {
        return None;
    }
    let parsed = reqwest::Url::parse(raw).ok()?;
    let path = parsed.path();
    if path == "/api/v1/image" {
        let mut url = None;
        let mut width = None;
        let mut kind = MediaKind::Screenshot;
        for (k, v) in parsed.query_pairs() {
            match k.as_ref() {
                "url" => url = Some(v.into_owned()),
                "w" => width = v.parse().ok(),
                "kind" if v == "icon" => kind = MediaKind::Icon,
                _ => {}
            }
        }
        return Some(OwnUrl::Image {
            kind,
            url: url?,
            width,
        });
    }
    if let Some(id) = path
        .strip_prefix("/api/v1/apps/")
        .and_then(|rest| rest.strip_suffix("/icon"))
    {
        return Some(OwnUrl::AppIcon(id.to_string()));
    }
    path.strip_prefix("/forge/icon/")
        .map(|id| OwnUrl::ForgeIcon(id.to_string()))
}

pub fn proxy_screenshot_urls(screenshots: &mut [String]) {
    for s in screenshots.iter_mut() {
        if let MediaRef::Remote(u) = classify(s) {
            *s = image_proxy_url(u);
        }
    }
}
