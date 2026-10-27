use libarc::cache::{Binary, BlobCache, PersistentMap};
use libarc::Transaction;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

const IMAGE_TTL: Duration = Duration::from_secs(7 * 24 * 3600);
const ICON_TTL: Duration = Duration::from_secs(6 * 3600);

const RETENTION: Duration = Duration::from_secs(30 * 24 * 3600);

const IMAGE_MAX_BYTES: u64 = 512 * 1024 * 1024;
const ICON_MAX_BYTES: u64 = 128 * 1024 * 1024;

pub fn images() -> &'static BlobCache {
    static CACHE: OnceLock<BlobCache> = OnceLock::new();
    CACHE.get_or_init(|| BlobCache::new("media", IMAGE_TTL).with_retention(RETENTION))
}

pub fn icons() -> &'static BlobCache {
    static CACHE: OnceLock<BlobCache> = OnceLock::new();
    CACHE.get_or_init(|| BlobCache::new("media-icons", ICON_TTL).with_retention(RETENTION))
}

pub fn forge() -> &'static PersistentMap<crate::forge_cache::Persisted, Binary> {
    static STORE: OnceLock<PersistentMap<crate::forge_cache::Persisted, Binary>> = OnceLock::new();
    STORE.get_or_init(|| PersistentMap::new("daemon", "forge"))
}

pub fn transactions() -> &'static PersistentMap<Vec<Transaction>, Binary> {
    static STORE: OnceLock<PersistentMap<Vec<Transaction>, Binary>> = OnceLock::new();
    STORE.get_or_init(|| PersistentMap::new("daemon", "transactions"))
}

pub fn catalog_path() -> PathBuf {
    libarc::cache::namespace("daemon")
        .unwrap_or_else(|| std::env::temp_dir().join("arc-daemon"))
        .join("catalog.sqlite")
}

pub fn sweep() -> usize {
    images().sweep()
        + icons().sweep()
        + images().trim_to_bytes(IMAGE_MAX_BYTES)
        + icons().trim_to_bytes(ICON_MAX_BYTES)
}

pub fn cleanup_legacy() {
    let Some(root) = libarc::cache::cache_root() else {
        return;
    };
    for file in [
        "daemon/appstream.json",
        "daemon/packages.json",
        "daemon/forge.json",
        "daemon/transactions.json",
        "package_cache.json",
        "frontend/packages.json",
        "frontend/details.json",
        "frontend/home.json",
        "frontend/stories.json",
    ] {
        let _ = fs::remove_file(root.join(file));
    }
    for dir in ["frontend/packages", "frontend/details", "frontend/stories"] {
        let _ = fs::remove_dir_all(root.join(dir));
    }
    let Ok(entries) = fs::read_dir(&root) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with("icon-pad-") || name.starts_with("test-") {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}
