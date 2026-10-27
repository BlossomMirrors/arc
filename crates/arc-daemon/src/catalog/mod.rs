mod db;
mod model;
mod query;
mod search;
mod sources;
mod state;
mod tables;

pub use db::subscribe;
pub use model::{CatalogApp, NewApp, NewText};
pub use query::{appstream_slice, by_category, get, name_of, remote_of};
pub use search::{score, search, SearchFilter, SearchPage};
pub use sources::{
    remove_source, replace_provider, replace_source, set_source_ranks, stored_sources, SourceStamp,
};
pub use state::{
    forge_checked_age, store_forge_verification, ForgeVerification,
    installed_remote_icons, installed_runtimes_with_prefix, is_installed, list_installed,
    list_updates, mark_updates_checked, remote_refs_with_prefix, remove_update, replace_installed,
    replace_installed_runtimes, replace_remote_refs, replace_updates, updates_checked_age,
};

pub const PRIO_BLOSSOMOS: i64 = 0;
pub const PRIO_APPSTREAM: i64 = 1;
pub const PRIO_PROVIDER: i64 = 2;
