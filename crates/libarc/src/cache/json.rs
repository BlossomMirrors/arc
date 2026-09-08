use crate::cache::{disk, namespace, writer};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::task::spawn_blocking;

#[derive(Deserialize)]
struct Envelope<T> {
    format: u32,
    #[serde(default)]
    schema: u32,
    written_at: u64,
    data: T,
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub struct JsonCache<T> {
    path: PathBuf,
    ttl: Option<Duration>,
    schema: u32,
    _pd: std::marker::PhantomData<T>,
}

impl<T: Serialize + DeserializeOwned> JsonCache<T> {
    pub fn new(ns: &str, file: &str) -> Self {
        let path = namespace(ns).unwrap_or_else(|| PathBuf::from(ns)).join(file);
        Self { path, ttl: None, schema: 0, _pd: std::marker::PhantomData }
    }

    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = Some(ttl);
        self
    }

    pub fn with_schema(mut self, schema: u32) -> Self {
        self.schema = schema;
        self
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    fn read_envelope(&self) -> Option<Envelope<T>> {
        let bytes = disk::read(&self.path)?;
        let envelope: Envelope<T> = serde_json::from_slice(&bytes).ok()?;
        if envelope.format != super::CACHE_FORMAT || envelope.schema != self.schema {
            return None;
        }
        if let Some(ttl) = self.ttl {
            let age = now_secs().saturating_sub(envelope.written_at);
            if age > ttl.as_secs() {
                return None;
            }
        }
        Some(envelope)
    }

    pub fn load(&self) -> Option<T> {
        self.read_envelope().map(|e| e.data)
    }

    pub fn load_with_age(&self) -> Option<(T, Duration)> {
        let envelope = self.read_envelope()?;
        let age = Duration::from_secs(now_secs().saturating_sub(envelope.written_at));
        Some((envelope.data, age))
    }

    pub fn load_if(&self, valid: impl FnOnce(&T) -> bool) -> Option<T> {
        let data = self.load()?;
        valid(&data).then_some(data)
    }

    pub async fn load_async(&self) -> Option<T>
    where
        T: Send + 'static,
    {
        let path = self.path.clone();
        let ttl = self.ttl;
        let schema = self.schema;
        spawn_blocking(move || {
            let bytes = disk::read(&path)?;
            let envelope: Envelope<T> = serde_json::from_slice(&bytes).ok()?;
            if envelope.format != super::CACHE_FORMAT || envelope.schema != schema {
                return None;
            }
            if let Some(ttl) = ttl {
                let age = now_secs().saturating_sub(envelope.written_at);
                if age > ttl.as_secs() {
                    return None;
                }
            }
            Some(envelope.data)
        })
        .await
        .ok()
        .flatten()
    }

    pub fn store(&self, value: &T) {
        if let Ok(bytes) = self.encode(value) {
            writer::enqueue(self.path.clone(), bytes);
        }
    }

    // like store, but takes ownership so the (possibly expensive, e.g. for a
    // large map) clone-and-serialize work happens on the writer thread
    // instead of blocking the caller
    pub fn store_owned(&self, value: T)
    where
        T: Send + 'static,
    {
        let path = self.path.clone();
        let schema = self.schema;
        writer::enqueue_with(path, move || {
            serde_json::to_vec(&Envelope {
                format: super::CACHE_FORMAT,
                schema,
                written_at: now_secs(),
                data: &value,
            })
            .unwrap_or_default()
        });
    }

    pub fn store_blocking(&self, value: &T) -> std::io::Result<()> {
        let bytes = self.encode(value).map_err(std::io::Error::other)?;
        disk::write_atomic(&self.path, &bytes)
    }

    fn encode(&self, value: &T) -> serde_json::Result<Vec<u8>> {
        serde_json::to_vec(&Envelope {
            format: super::CACHE_FORMAT,
            schema: self.schema,
            written_at: now_secs(),
            data: value,
        })
    }

    pub fn clear(&self) {
        disk::remove(&self.path);
    }
}

impl<T: Serialize> Serialize for Envelope<&T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("Envelope", 4)?;
        s.serialize_field("format", &self.format)?;
        s.serialize_field("schema", &self.schema)?;
        s.serialize_field("written_at", &self.written_at)?;
        s.serialize_field("data", self.data)?;
        s.end()
    }
}

struct Stamped<V> {
    value: V,
    stored_at: u64,
}

pub struct PersistentMap<V> {
    store: JsonCache<HashMap<String, Stamped<V>>>,
    entries: Mutex<HashMap<String, Stamped<V>>>,
    entry_ttl: Option<Duration>,
    capacity_limit: Option<usize>,
}

impl<V: Clone> Clone for Stamped<V> {
    fn clone(&self) -> Self {
        Self { value: self.value.clone(), stored_at: self.stored_at }
    }
}

impl<V: Serialize> Serialize for Stamped<V> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("Stamped", 2)?;
        s.serialize_field("value", &self.value)?;
        s.serialize_field("stored_at", &self.stored_at)?;
        s.end()
    }
}

impl<'de, V: Deserialize<'de>> Deserialize<'de> for Stamped<V> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw<V> {
            value: V,
            #[serde(default)]
            stored_at: u64,
        }
        let raw = Raw::<V>::deserialize(deserializer)?;
        Ok(Stamped { value: raw.value, stored_at: raw.stored_at })
    }
}

impl<V: Clone + Serialize + DeserializeOwned + Send + 'static> PersistentMap<V> {
    pub fn new(ns: &str, file: &str) -> Self {
        let store = JsonCache::new(ns, file);
        let entries = store.load().unwrap_or_default();
        Self { store, entries: Mutex::new(entries), entry_ttl: None, capacity_limit: None }
    }

    pub fn with_entry_ttl(mut self, ttl: Duration) -> Self {
        self.entry_ttl = Some(ttl);
        self
    }

    pub fn with_capacity_limit(mut self, max_entries: usize) -> Self {
        self.capacity_limit = Some(max_entries);
        self
    }

    fn is_fresh(&self, entry: &Stamped<V>) -> bool {
        match self.entry_ttl {
            Some(ttl) => now_secs().saturating_sub(entry.stored_at) <= ttl.as_secs(),
            None => true,
        }
    }

    pub fn get(&self, key: &str) -> Option<V> {
        let entries = self.entries.lock().unwrap();
        entries.get(key).filter(|e| self.is_fresh(e)).map(|e| e.value.clone())
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    fn persist(&self) {
        let snapshot = self.entries.lock().unwrap().clone();
        self.store.store_owned(snapshot);
    }

    fn enforce_capacity(&self, entries: &mut HashMap<String, Stamped<V>>) {
        let Some(limit) = self.capacity_limit else { return };
        while entries.len() > limit {
            if let Some(oldest_key) = entries
                .iter()
                .min_by_key(|(_, e)| e.stored_at)
                .map(|(k, _)| k.clone())
            {
                entries.remove(&oldest_key);
            } else {
                break;
            }
        }
    }

    pub fn insert(&self, key: impl Into<String>, value: V) {
        {
            let mut entries = self.entries.lock().unwrap();
            entries.insert(key.into(), Stamped { value, stored_at: now_secs() });
            self.enforce_capacity(&mut entries);
        }
        self.persist();
    }

    pub fn update(&self, key: &str, f: impl FnOnce(&mut V)) -> bool {
        let updated = {
            let mut entries = self.entries.lock().unwrap();
            match entries.get_mut(key) {
                Some(entry) => {
                    f(&mut entry.value);
                    entry.stored_at = now_secs();
                    true
                }
                None => false,
            }
        };
        if updated {
            self.persist();
        }
        updated
    }

    pub fn entry_or_default(&self, key: &str, f: impl FnOnce(&mut V))
    where
        V: Default,
    {
        {
            let mut entries = self.entries.lock().unwrap();
            let entry = entries.entry(key.to_string()).or_insert_with(|| Stamped {
                value: V::default(),
                stored_at: now_secs(),
            });
            f(&mut entry.value);
            entry.stored_at = now_secs();
        }
        self.persist();
    }

    pub fn remove(&self, key: &str) {
        {
            self.entries.lock().unwrap().remove(key);
        }
        self.persist();
    }

    pub fn clear(&self) {
        self.entries.lock().unwrap().clear();
        self.store.clear();
    }

    pub fn flush(&self) {
        self.persist();
    }
}
