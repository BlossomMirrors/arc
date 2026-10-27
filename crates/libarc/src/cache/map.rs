use crate::cache::{disk, namespace};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub trait Codec {
    const EXT: &'static str;
    fn encode<T: Serialize>(value: &T) -> Option<Vec<u8>>;
    fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Option<T>;
}

pub struct Json;

impl Codec for Json {
    const EXT: &'static str = "json";
    fn encode<T: Serialize>(value: &T) -> Option<Vec<u8>> {
        serde_json::to_vec(value).ok()
    }
    fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Option<T> {
        serde_json::from_slice(bytes).ok()
    }
}

pub struct Binary;

impl Codec for Binary {
    const EXT: &'static str = "bin";
    fn encode<T: Serialize>(value: &T) -> Option<Vec<u8>> {
        postcard::to_allocvec(value).ok()
    }
    fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Option<T> {
        postcard::from_bytes(bytes).ok()
    }
}

#[derive(Serialize)]
struct StoredRef<'a, V> {
    key: &'a str,
    stored_at: u64,
    value: &'a V,
}

#[derive(Deserialize)]
struct Stored<V> {
    key: String,
    stored_at: u64,
    value: V,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

const TRIM_EVERY: u32 = 32;

// a key value store with one small file per key so only the entries someone asks for are ever in memory
pub struct PersistentMap<V, C: Codec = Json> {
    dir: PathBuf,
    entry_ttl: Option<Duration>,
    capacity_limit: Option<usize>,
    inserts: AtomicU32,
    _pd: PhantomData<(V, C)>,
}

impl<V: Clone + Serialize + DeserializeOwned + Send + 'static, C: Codec> PersistentMap<V, C> {
    pub fn new(ns: &str, file: &str) -> Self {
        let base = namespace(ns).unwrap_or_else(|| PathBuf::from(ns));
        let stem = file.strip_suffix(".json").unwrap_or(file);
        disk::remove(&base.join(file));
        Self {
            dir: base.join(stem),
            entry_ttl: None,
            capacity_limit: None,
            inserts: AtomicU32::new(0),
            _pd: PhantomData,
        }
    }

    pub fn with_entry_ttl(mut self, ttl: Duration) -> Self {
        self.entry_ttl = Some(ttl);
        self
    }

    pub fn with_capacity_limit(mut self, max_entries: usize) -> Self {
        self.capacity_limit = Some(max_entries);
        self
    }

    fn path_for(&self, key: &str) -> PathBuf {
        self.dir
            .join(format!("{}.{}", super::blob::hash_key(key), C::EXT))
    }

    fn read(&self, key: &str) -> Option<Stored<V>> {
        let stored: Stored<V> = C::decode(&disk::read(&self.path_for(key))?)?;
        (stored.key == key).then_some(stored)
    }

    fn is_fresh(&self, stored_at: u64) -> bool {
        match self.entry_ttl {
            Some(ttl) => now_secs().saturating_sub(stored_at) <= ttl.as_secs(),
            None => true,
        }
    }

    // no fsync because losing a cache entry in a crash only costs a refetch
    fn write(&self, key: &str, value: &V) {
        let Some(bytes) = C::encode(&StoredRef {
            key,
            stored_at: now_secs(),
            value,
        }) else {
            return;
        };
        let _ = disk::write_replace(&self.path_for(key), &bytes);
        if self.inserts.fetch_add(1, Ordering::Relaxed) % TRIM_EVERY == 0 {
            if let Some(limit) = self.capacity_limit {
                disk::trim_to_count(&self.dir, limit);
            }
        }
    }

    pub fn get(&self, key: &str) -> Option<V> {
        self.read(key)
            .filter(|s| self.is_fresh(s.stored_at))
            .map(|s| s.value)
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    pub fn insert(&self, key: impl Into<String>, value: V) {
        let key = key.into();
        self.write(&key, &value);
    }

    pub fn update(&self, key: &str, f: impl FnOnce(&mut V)) -> bool {
        let Some(mut stored) = self.read(key) else {
            return false;
        };
        f(&mut stored.value);
        self.write(key, &stored.value);
        true
    }

    pub fn entry_or_default(&self, key: &str, f: impl FnOnce(&mut V))
    where
        V: Default,
    {
        let mut value = self.read(key).map(|s| s.value).unwrap_or_default();
        f(&mut value);
        self.write(key, &value);
    }

    pub fn remove(&self, key: &str) {
        disk::remove(&self.path_for(key));
    }

    pub fn clear(&self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }

    pub fn flush(&self) {}
}
