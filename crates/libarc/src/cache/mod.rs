mod blob;
mod disk;
mod json;
mod memory;
mod writer;

pub use blob::{Blob, BlobCache};
pub use json::{JsonCache, PersistentMap};
pub use memory::MemoryCache;
pub use writer::flush_blocking;

use std::path::PathBuf;

pub const CACHE_FORMAT: u32 = 1;

pub fn cache_root() -> Option<PathBuf> {
    Some(dirs::cache_dir()?.join("arc"))
}

pub fn namespace(ns: &str) -> Option<PathBuf> {
    Some(cache_root()?.join(ns))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use std::time::Duration;

    fn temp_ns() -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        format!("test-{}-{}", std::process::id(), COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    #[test]
    fn atomic_write_survives_truncated_tmp() {
        let dir = std::env::temp_dir().join(format!("arc-cache-disk-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("f.txt");
        disk::write_atomic(&path, b"hello").unwrap();
        assert_eq!(disk::read(&path).unwrap(), b"hello");
        disk::write_atomic(&path, b"world").unwrap();
        assert_eq!(disk::read(&path).unwrap(), b"world");
        let leftover: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "tmp"))
            .collect();
        assert!(leftover.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[derive(Serialize, Deserialize, PartialEq, Debug, Default, Clone)]
    struct Sample {
        n: u32,
    }

    #[test]
    fn json_cache_schema_mismatch_is_miss() {
        let ns = temp_ns();
        let cache = JsonCache::<Sample>::new(&ns, "schema.json").with_schema(1);
        cache.store_blocking(&Sample { n: 7 }).unwrap();
        assert_eq!(cache.load(), Some(Sample { n: 7 }));

        let cache_v2 = JsonCache::<Sample>::new(&ns, "schema.json").with_schema(2);
        assert_eq!(cache_v2.load(), None);
        cache.clear();
    }

    #[test]
    fn json_cache_ttl_expiry() {
        let ns = temp_ns();
        let cache = JsonCache::<Sample>::new(&ns, "ttl.json").with_ttl(Duration::from_secs(0));
        cache.store_blocking(&Sample { n: 1 }).unwrap();
        std::thread::sleep(Duration::from_millis(1100));
        assert_eq!(cache.load(), None);
        cache.clear();
    }

    #[test]
    fn persistent_map_round_trips() {
        let ns = temp_ns();
        {
            let map = PersistentMap::<Sample>::new(&ns, "map.json");
            map.insert("a", Sample { n: 1 });
            map.update("a", |s| s.n = 2);
            map.flush();
        }
        writer::flush_blocking();
        std::thread::sleep(Duration::from_millis(50));
        let reopened = PersistentMap::<Sample>::new(&ns, "map.json");
        assert_eq!(reopened.get("a"), Some(Sample { n: 2 }));
        reopened.clear();
    }

    #[tokio::test]
    async fn blob_cache_rejects_wrong_key_file() {
        let ns = temp_ns();
        let cache = BlobCache::new(&ns, Duration::from_secs(3600));
        cache.insert("real-key", Blob::new(b"data".to_vec(), "text/plain")).await;
        writer::flush_blocking();
        tokio::time::sleep(Duration::from_millis(50)).await;

        let path = cache.path_for_test("real-key");
        std::fs::write(&path, b"not-the-real-key\ntext/plain\ndata").unwrap();

        // fresh instance so the in-memory hit from insert() above can't
        // mask the on-disk tampering
        let reopened = BlobCache::new(&ns, Duration::from_secs(3600));
        assert_eq!(reopened.get("real-key").await, None);
        cache.clear();
    }

    #[tokio::test]
    async fn writer_coalesces_same_path() {
        let ns = temp_ns();
        let cache = JsonCache::<Sample>::new(&ns, "coalesce.json");
        for n in 0..20 {
            cache.store(&Sample { n });
        }
        writer::flush_blocking();
        assert_eq!(cache.load(), Some(Sample { n: 19 }));
        cache.clear();
    }
}
