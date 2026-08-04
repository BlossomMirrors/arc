use std::collections::HashMap;
use std::future::Future;
use std::hash::Hash;
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

pub struct Cache<K, V> {
    ttl: Duration,
    entries: RwLock<HashMap<K, (Instant, V)>>,
}

impl<K: Eq + Hash + Clone, V: Clone> Cache<K, V> {
    pub fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            entries: RwLock::new(HashMap::new()),
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

    pub async fn get_or_fetch<F, Fut>(&self, key: K, fetch: F) -> Option<V>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Option<V>>,
    {
        if let Some(v) = self.get(&key).await {
            return Some(v);
        }
        let value = fetch().await?;
        self.insert(key, value.clone()).await;
        Some(value)
    }
}

// icon byte cache keyed by packageId
static ICON_CACHE: OnceLock<Cache<String, (Vec<u8>, String)>> = OnceLock::new();

pub fn icon_cache() -> &'static Cache<String, (Vec<u8>, String)> {
    ICON_CACHE.get_or_init(|| Cache::new(Duration::from_secs(3600)))
}

pub async fn fetch_icon(
    client: &reqwest::Client,
    id: &str,
    url: &str,
) -> Option<(Vec<u8>, String)> {
    icon_cache()
        .get_or_fetch(id.to_string(), || async {
            let resp = client
                .get(url)
                .timeout(Duration::from_secs(10))
                .send()
                .await
                .ok()?;
            let ct = resp
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok().unwrap_or("image/png").to_string());
            let bytes = resp.bytes().await.ok()?.to_vec();
            Some((bytes, ct))
        })
        .await
}
