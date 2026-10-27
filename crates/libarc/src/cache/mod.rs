mod blob;
mod disk;
mod map;
mod memory;

pub use blob::{Blob, BlobCache};
pub use map::{Binary, Codec, Json, PersistentMap};
pub use memory::SingleFlight;

use std::path::PathBuf;

pub fn cache_root() -> Option<PathBuf> {
    Some(dirs::cache_dir()?.join("arc"))
}

pub fn namespace(ns: &str) -> Option<PathBuf> {
    Some(cache_root()?.join(ns))
}
