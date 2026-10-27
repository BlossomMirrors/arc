use libarc::cache::{Binary, PersistentMap};
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

const DAY: Duration = Duration::from_secs(24 * 3600);

pub fn home() -> &'static PersistentMap<crate::services::home_cache::Snapshot, Binary> {
    static STORE: OnceLock<PersistentMap<crate::services::home_cache::Snapshot, Binary>> =
        OnceLock::new();
    STORE.get_or_init(|| {
        PersistentMap::new("frontend", "home")
            .with_entry_ttl(DAY)
            .with_capacity_limit(8)
    })
}

pub fn stories() -> &'static PersistentMap<crate::services::stories::CachedStory, Binary> {
    static STORE: OnceLock<PersistentMap<crate::services::stories::CachedStory, Binary>> =
        OnceLock::new();
    STORE.get_or_init(|| {
        PersistentMap::new("frontend", "story-pages")
            .with_entry_ttl(7 * DAY)
            .with_capacity_limit(200)
    })
}

pub fn forge_lists() -> &'static PersistentMap<Vec<String>> {
    static STORE: OnceLock<PersistentMap<Vec<String>>> = OnceLock::new();
    STORE.get_or_init(|| {
        PersistentMap::new("frontend", "forge-lists.json")
            .with_entry_ttl(7 * DAY)
            .with_capacity_limit(64)
    })
}

pub fn list_pages() -> &'static PersistentMap<crate::services::home::ListData> {
    static STORE: OnceLock<PersistentMap<crate::services::home::ListData>> = OnceLock::new();
    STORE.get_or_init(|| {
        PersistentMap::new("frontend", "list-pages.json")
            .with_entry_ttl(7 * DAY)
            .with_capacity_limit(32)
    })
}

pub fn padded_icons_dir() -> Option<PathBuf> {
    libarc::cache::namespace("icon-pad")
}
