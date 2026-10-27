pub const LOCAL_PREFIX: &str = "local:";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaRef<'a> {
    Remote(&'a str),
    LocalFile(&'a str),
    Local,
    Name(&'a str),
}

pub fn classify(raw: &str) -> MediaRef<'_> {
    if raw.starts_with("http://") || raw.starts_with("https://") {
        MediaRef::Remote(raw)
    } else if raw.starts_with("file://") || raw.starts_with("qrc:") {
        MediaRef::LocalFile(raw)
    } else if raw.starts_with(LOCAL_PREFIX) {
        MediaRef::Local
    } else {
        MediaRef::Name(raw)
    }
}

pub fn image_proxy_url(remote_url: &str) -> String {
    let encoded =
        percent_encoding::utf8_percent_encode(remote_url, percent_encoding::NON_ALPHANUMERIC);
    format!("{}/api/v1/image?url={encoded}", crate::DAEMON_HTTP_BASE)
}

pub fn image_proxy_url_sized(remote_url: &str, width: Option<u32>) -> String {
    match width {
        Some(w) => format!("{}&w={w}", image_proxy_url(remote_url)),
        None => image_proxy_url(remote_url),
    }
}

pub fn icon_image_proxy_url(remote_url: &str) -> String {
    format!("{}&kind=icon", image_proxy_url(remote_url))
}

pub fn proxied(raw: &str) -> String {
    match classify(raw) {
        MediaRef::Remote(url) => image_proxy_url(url),
        _ => raw.to_string(),
    }
}

pub fn normalize_local_ref(raw: &str) -> String {
    if raw.starts_with('/') {
        format!("file://{raw}")
    } else {
        raw.to_string()
    }
}
