use crate::cache::{disk, namespace, writer, MemoryCache};
use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::spawn_blocking;

#[derive(Clone, Debug, PartialEq)]
pub struct Blob {
    pub bytes: Arc<Vec<u8>>,
    pub content_type: Arc<str>,
}

impl Blob {
    pub fn new(bytes: Vec<u8>, content_type: impl Into<Arc<str>>) -> Self {
        Self { bytes: Arc::new(bytes), content_type: content_type.into() }
    }
}

fn hash_key(key: &str) -> String {
    const FNV_OFFSET: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;
    let mut hash = FNV_OFFSET;
    for byte in key.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("{hash:016x}")
}

fn encode_blob(key: &str, blob: &Blob) -> Vec<u8> {
    let mut buf = Vec::with_capacity(key.len() + blob.content_type.len() + blob.bytes.len() + 2);
    buf.extend_from_slice(key.as_bytes());
    buf.push(b'\n');
    buf.extend_from_slice(blob.content_type.as_bytes());
    buf.push(b'\n');
    buf.extend_from_slice(&blob.bytes);
    buf
}

fn decode_blob(expected_key: &str, bytes: &[u8]) -> Option<Blob> {
    let first_nl = bytes.iter().position(|&b| b == b'\n')?;
    let key = std::str::from_utf8(&bytes[..first_nl]).ok()?;
    if key != expected_key {
        return None;
    }
    let rest = &bytes[first_nl + 1..];
    let second_nl = rest.iter().position(|&b| b == b'\n')?;
    let content_type = std::str::from_utf8(&rest[..second_nl]).ok()?;
    let data = rest[second_nl + 1..].to_vec();
    Some(Blob::new(data, content_type))
}

pub struct BlobCache {
    dir: PathBuf,
    ttl: Duration,
    mem: MemoryCache<String, Blob>,
}

impl BlobCache {
    pub fn new(ns: &str, ttl: Duration) -> Self {
        let dir = namespace(ns).unwrap_or_else(|| PathBuf::from(ns));
        Self { dir, ttl, mem: MemoryCache::new(ttl) }
    }

    fn path_for(&self, key: &str) -> PathBuf {
        let hash = hash_key(key);
        self.dir.join(&hash[..2]).join(format!("{hash}.blob"))
    }

    #[cfg(test)]
    pub(crate) fn path_for_test(&self, key: &str) -> PathBuf {
        self.path_for(key)
    }

    async fn read_disk(&self, key: &str) -> Option<Blob> {
        let path = self.path_for(key);
        let ttl = self.ttl;
        let key = key.to_string();
        spawn_blocking(move || {
            let age = disk::age(&path)?;
            if age > ttl {
                return None;
            }
            let bytes = disk::read(&path)?;
            decode_blob(&key, &bytes)
        })
        .await
        .ok()
        .flatten()
    }

    fn persist(&self, key: &str, blob: &Blob) {
        let path = self.path_for(key);
        let bytes = encode_blob(key, blob);
        writer::enqueue(path, bytes);
    }

    pub async fn get(&self, key: &str) -> Option<Blob> {
        if let Some(blob) = self.mem.get(&key.to_string()).await {
            return Some(blob);
        }
        let blob = self.read_disk(key).await?;
        self.mem.insert(key.to_string(), blob.clone()).await;
        Some(blob)
    }

    pub async fn insert(&self, key: &str, blob: Blob) {
        self.mem.insert(key.to_string(), blob.clone()).await;
        self.persist(key, &blob);
    }

    pub async fn insert_memory_only(&self, key: &str, blob: Blob) {
        self.mem.insert(key.to_string(), blob).await;
    }

    pub async fn get_or_fetch<F, Fut>(&self, key: &str, fetch: F) -> Option<Blob>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Option<Blob>>,
    {
        self.mem
            .get_or_fetch(key.to_string(), || async {
                if let Some(blob) = self.read_disk(key).await {
                    return Some(blob);
                }
                let blob = fetch().await?;
                self.persist(key, &blob);
                Some(blob)
            })
            .await
    }

    pub fn sweep(&self) -> usize {
        disk::sweep_older_than(&self.dir, self.ttl)
    }

    pub fn trim_to_bytes(&self, max_bytes: u64) -> usize {
        disk::trim_to_bytes(&self.dir, max_bytes)
    }

    pub fn clear(&self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
