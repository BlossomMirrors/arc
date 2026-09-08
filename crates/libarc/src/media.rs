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

pub fn normalize_local_ref(raw: &str) -> String {
    if raw.starts_with('/') {
        format!("file://{raw}")
    } else {
        raw.to_string()
    }
}
