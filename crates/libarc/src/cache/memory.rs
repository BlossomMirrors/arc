use std::collections::HashMap;
use std::future::Future;
use std::hash::Hash;
use std::sync::{Arc, Mutex};
use tokio::sync::OnceCell;

pub struct SingleFlight<K, V> {
    inflight: Mutex<HashMap<K, Arc<OnceCell<Option<V>>>>>,
}

impl<K: Eq + Hash + Clone, V: Clone> Default for SingleFlight<K, V> {
    fn default() -> Self {
        Self {
            inflight: Mutex::new(HashMap::new()),
        }
    }
}

impl<K: Eq + Hash + Clone, V: Clone> SingleFlight<K, V> {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn run<F, Fut>(&self, key: K, fetch: F) -> Option<V>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Option<V>>,
    {
        let cell = self
            .inflight
            .lock()
            .unwrap()
            .entry(key.clone())
            .or_insert_with(|| Arc::new(OnceCell::new()))
            .clone();
        let value = cell.get_or_init(fetch).await.clone();
        let mut inflight = self.inflight.lock().unwrap();
        if inflight.get(&key).is_some_and(|c| Arc::ptr_eq(c, &cell)) {
            inflight.remove(&key);
        }
        value
    }
}
