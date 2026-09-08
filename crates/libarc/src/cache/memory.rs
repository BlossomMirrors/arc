use std::collections::HashMap;
use std::future::Future;
use std::hash::Hash;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};

pub struct MemoryCache<K, V> {
    ttl: Duration,
    entries: RwLock<HashMap<K, (Instant, V)>>,
    inflight: Mutex<HashMap<K, Arc<Mutex<()>>>>,
}

impl<K: Eq + Hash + Clone, V: Clone> MemoryCache<K, V> {
    pub fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            entries: RwLock::new(HashMap::new()),
            inflight: Mutex::new(HashMap::new()),
        }
    }

    pub async fn get(&self, key: &K) -> Option<V> {
        let map = self.entries.read().await;
        map.get(key)
            .and_then(|(t, v)| (t.elapsed() < self.ttl).then(|| v.clone()))
    }

    pub async fn insert(&self, key: K, value: V) {
        self.entries
            .write()
            .await
            .insert(key, (Instant::now(), value));
    }

    pub async fn remove(&self, key: &K) {
        self.entries.write().await.remove(key);
    }

    pub async fn clear(&self) {
        self.entries.write().await.clear();
    }

    pub async fn retain_fresh(&self) {
        let ttl = self.ttl;
        self.entries.write().await.retain(|_, (t, _)| t.elapsed() < ttl);
    }

    pub async fn get_or_fetch<F, Fut>(&self, key: K, fetch: F) -> Option<V>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Option<V>>,
    {
        if let Some(v) = self.get(&key).await {
            return Some(v);
        }

        let key_lock = {
            let mut inflight = self.inflight.lock().await;
            inflight.entry(key.clone()).or_insert_with(|| Arc::new(Mutex::new(()))).clone()
        };
        let _guard = key_lock.lock().await;

        if let Some(v) = self.get(&key).await {
            self.inflight.lock().await.remove(&key);
            return Some(v);
        }

        let value = fetch().await;
        self.inflight.lock().await.remove(&key);
        let value = value?;
        self.insert(key, value.clone()).await;
        Some(value)
    }
}
