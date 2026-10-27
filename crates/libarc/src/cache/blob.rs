use crate::cache::{disk, namespace, SingleFlight};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::task::spawn_blocking;

#[derive(Clone, Debug, PartialEq)]
pub struct Blob {
    pub bytes: Arc<Vec<u8>>,
    pub content_type: Arc<str>,
}

impl Blob {
    pub fn new(bytes: Vec<u8>, content_type: impl Into<Arc<str>>) -> Self {
        Self {
            bytes: Arc::new(bytes),
            content_type: content_type.into(),
        }
    }
}

pub(crate) fn hash_key(key: &str) -> String {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut hash = FNV_OFFSET;
    for byte in key.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("{hash:016x}")
}

const META_MAGIC: &str = "ARC3";
const META_EXT: &str = "meta";
const LEGACY_MAGIC: &[u8] = b"ARC2\n";
const LEGACY_EXT: &str = "blob";

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn extension_for(content_type: &str, bytes: &[u8]) -> &'static str {
    let ct = content_type.to_ascii_lowercase();
    if ct.contains("svg") {
        return "svg";
    }
    match image::guess_format(bytes) {
        Ok(image::ImageFormat::Png) => "png",
        Ok(image::ImageFormat::Jpeg) => "jpg",
        Ok(image::ImageFormat::WebP) => "webp",
        Ok(image::ImageFormat::Gif) => "gif",
        _ if ct.contains("png") => "png",
        _ if ct.contains("jpeg") || ct.contains("jpg") => "jpg",
        _ if ct.contains("webp") => "webp",
        _ if ct.contains("gif") => "gif",
        _ => "bin",
    }
}

struct Meta {
    stored_at: u64,
    ext: String,
    content_type: String,
}

fn encode_meta(key: &str, stored_at: u64, ext: &str, content_type: &str) -> String {
    format!("{META_MAGIC}\n{stored_at}\n{ext}\n{content_type}\n{key}")
}

fn decode_meta(expected_key: &str, text: &str) -> Option<Meta> {
    let mut lines = text.splitn(5, '\n');
    if lines.next()? != META_MAGIC {
        return None;
    }
    let stored_at = lines.next()?.parse().ok()?;
    let ext = lines.next()?.to_string();
    let content_type = lines.next()?.to_string();
    if lines.next()? != expected_key {
        return None;
    }
    Some(Meta {
        stored_at,
        ext,
        content_type,
    })
}

fn decode_legacy(expected_key: &str, bytes: &[u8]) -> Option<(u64, String, Vec<u8>)> {
    let mut rest = bytes.strip_prefix(LEGACY_MAGIC)?;
    let mut field = || {
        let nl = rest.iter().position(|&b| b == b'\n')?;
        let value = std::str::from_utf8(&rest[..nl]).ok()?.to_string();
        rest = &rest[nl + 1..];
        Some(value)
    };
    let stored_at = field()?.parse().ok()?;
    if field()? != expected_key {
        return None;
    }
    let content_type = field()?;
    Some((stored_at, content_type, rest.to_vec()))
}

struct Entry {
    file: PathBuf,
    stored_at: u64,
    content_type: String,
}

pub struct BlobCache {
    dir: PathBuf,
    ttl: Duration,
    retention: Duration,
    inflight: SingleFlight<String, Blob>,
}

const TOUCH_AFTER: Duration = Duration::from_secs(3600);

impl BlobCache {
    pub fn new(ns: &str, ttl: Duration) -> Self {
        let dir = namespace(ns).unwrap_or_else(|| PathBuf::from(ns));
        Self {
            dir,
            ttl,
            retention: ttl,
            inflight: SingleFlight::new(),
        }
    }

    pub fn with_retention(mut self, retention: Duration) -> Self {
        self.retention = retention.max(self.ttl);
        self
    }

    fn base_for(&self, key: &str) -> PathBuf {
        let hash = hash_key(key);
        self.dir.join(&hash[..2]).join(hash)
    }



    fn locate(&self, key: &str) -> Option<Entry> {
        let base = self.base_for(key);
        if let Some(meta) = std::fs::read_to_string(base.with_extension(META_EXT))
            .ok()
            .and_then(|t| decode_meta(key, &t))
        {
            let file = base.with_extension(&meta.ext);
            return file.exists().then_some(Entry {
                file,
                stored_at: meta.stored_at,
                content_type: meta.content_type,
            });
        }
        let legacy = base.with_extension(LEGACY_EXT);
        let (stored_at, content_type, bytes) = decode_legacy(key, &disk::read(&legacy)?)?;
        let file = write_entry(&base, key, stored_at, &content_type, &bytes).ok()?;
        disk::remove(&legacy);
        Some(Entry {
            file,
            stored_at,
            content_type,
        })
    }

    fn is_fresh(&self, entry: &Entry) -> bool {
        now_secs().saturating_sub(entry.stored_at) <= self.ttl.as_secs()
    }

    async fn read_disk(&'static self, key: &str) -> Option<(Blob, bool)> {
        let key = key.to_string();
        spawn_blocking(move || {
            let entry = self.locate(&key)?;
            let bytes = disk::read(&entry.file)?;
            touch_if_old(&entry.file);
            let fresh = self.is_fresh(&entry);
            Some((Blob::new(bytes, entry.content_type), fresh))
        })
        .await
        .ok()
        .flatten()
    }

    async fn persist(&self, key: &str, blob: &Blob) {
        let base = self.base_for(key);
        let key = key.to_string();
        let blob = blob.clone();
        let _ = spawn_blocking(move || {
            write_entry(&base, &key, now_secs(), &blob.content_type, &blob.bytes)
        })
        .await;
    }

    pub async fn get(&'static self, key: &str) -> Option<Blob> {
        self.read_disk(key).await.map(|(blob, _)| blob)
    }

    pub async fn insert(&self, key: &str, blob: Blob) {
        self.persist(key, &blob).await;
    }

    fn refresh_in_background<F, Fut>(&'static self, key: &str, fetch: F)
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Option<Blob>> + Send + 'static,
    {
        let key = key.to_string();
        tokio::spawn(async move {
            self.inflight
                .run(key.clone(), || async {
                    let fresh = fetch().await?;
                    self.persist(&key, &fresh).await;
                    Some(fresh)
                })
                .await;
        });
    }

    pub async fn get_or_fetch<F, Fut>(&'static self, key: &str, fetch: F) -> Option<Blob>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Option<Blob>> + Send + 'static,
    {
        match self.read_disk(key).await {
            Some((blob, true)) => return Some(blob),
            Some((blob, false)) => {
                self.refresh_in_background(key, fetch);
                return Some(blob);
            }
            None => {}
        }
        self.inflight
            .run(key.to_string(), || async {
                if let Some((blob, _)) = self.read_disk(key).await {
                    return Some(blob);
                }
                let blob = fetch().await?;
                self.persist(key, &blob).await;
                Some(blob)
            })
            .await
    }

    pub fn local_file<F, Fut>(&'static self, key: &str, fetch: F) -> Option<PathBuf>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Option<Blob>> + Send + 'static,
    {
        let Some(entry) = self.locate(key) else {
            self.refresh_in_background(key, fetch);
            return None;
        };
        touch_if_old(&entry.file);
        if !self.is_fresh(&entry) {
            self.refresh_in_background(key, fetch);
        }
        Some(entry.file)
    }

    pub fn sweep(&self) -> usize {
        let retention = self.retention;
        let mut removed = 0;
        for (file, _, modified) in entry_files(&self.dir) {
            if modified.elapsed().is_ok_and(|age| age > retention) && remove_entry(&file) {
                removed += 1;
            }
        }
        remove_orphan_meta(&self.dir);
        removed
    }

    pub fn trim_to_bytes(&self, max_bytes: u64) -> usize {
        let mut files = entry_files(&self.dir);
        let mut total: u64 = files.iter().map(|(_, size, _)| size).sum();
        if total <= max_bytes {
            return 0;
        }
        files.sort_by_key(|(_, _, modified)| *modified);
        let mut removed = 0;
        for (file, size, _) in files {
            if total <= max_bytes {
                break;
            }
            if remove_entry(&file) {
                total = total.saturating_sub(size);
                removed += 1;
            }
        }
        removed
    }

    pub fn clear(&self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn write_entry(
    base: &Path,
    key: &str,
    stored_at: u64,
    content_type: &str,
    bytes: &[u8],
) -> std::io::Result<PathBuf> {
    let ext = extension_for(content_type, bytes);
    let file = base.with_extension(ext);
    disk::write_atomic(&file, bytes)?;
    disk::write_atomic(
        &base.with_extension(META_EXT),
        encode_meta(key, stored_at, ext, content_type).as_bytes(),
    )?;
    Ok(file)
}

fn touch_if_old(file: &Path) {
    if disk::age(file).is_some_and(|age| age > TOUCH_AFTER) {
        disk::touch(file);
    }
}

fn entry_files(dir: &Path) -> Vec<(PathBuf, u64, SystemTime)> {
    let mut out = Vec::new();
    let Ok(shards) = std::fs::read_dir(dir) else {
        return out;
    };
    for shard in shards.flatten() {
        let Ok(files) = std::fs::read_dir(shard.path()) else {
            continue;
        };
        for file in files.flatten() {
            let path = file.path();
            if path.extension().is_some_and(|e| e == META_EXT) {
                continue;
            }
            let Ok(meta) = file.metadata() else { continue };
            if let (true, Ok(modified)) = (meta.is_file(), meta.modified()) {
                out.push((path, meta.len(), modified));
            }
        }
    }
    out
}

fn remove_entry(file: &Path) -> bool {
    let removed = std::fs::remove_file(file).is_ok();
    disk::remove(&file.with_extension(META_EXT));
    removed
}

fn remove_orphan_meta(dir: &Path) {
    let Ok(shards) = std::fs::read_dir(dir) else {
        return;
    };
    for shard in shards.flatten() {
        let Ok(files) = std::fs::read_dir(shard.path()) else {
            continue;
        };
        for file in files.flatten() {
            let path = file.path();
            if !path.extension().is_some_and(|e| e == META_EXT) {
                continue;
            }
            let ext = std::fs::read_to_string(&path)
                .ok()
                .and_then(|t| t.lines().nth(2).map(str::to_string));
            if !ext.is_some_and(|ext| path.with_extension(ext).exists()) {
                disk::remove(&path);
            }
        }
    }
}
