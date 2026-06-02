use appstream::enums::{ComponentKind, ImageKind};
use appstream::{Collection, Component};
use libarc::{Package, Provider};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static FLATPAK_DB: OnceLock<AppStreamDb> = OnceLock::new();

// appstream is a big xml catalog of apps that distros ship alongside their packages
// so you get descriptions, icons and categories without hitting the network
pub struct AppStreamDb {
    components: Vec<(Component, Option<String>)>,
}

#[derive(serde::Serialize)]
pub struct AppStreamEntry {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub icon_url: Option<String>,
    pub remote: Option<String>,
    pub screenshots: Vec<String>,
}

impl AppStreamDb {
    pub fn get_static() -> &'static AppStreamDb {
        FLATPAK_DB.get_or_init(Self::load_flatpak)
    }

    pub fn load_flatpak() -> Self {
        let mut components = Vec::new();
        load_flatpak_root("/var/lib/flatpak/appstream", &mut components);
        // also check the user install location, not just the system one
        if let Some(home) = std::env::var_os("HOME") {
            let path = PathBuf::from(home).join(".local/share/flatpak/appstream");
            load_flatpak_root(&path, &mut components);
        }
        Self { components }
    }

    pub fn get_popular_apps(&self, limit: usize) -> Vec<AppStreamEntry> {
        self.components
            .iter()
            .filter(|(c, _)| {
                matches!(
                    c.kind,
                    ComponentKind::DesktopApplication | ComponentKind::ConsoleApplication
                )
            })
            .take(limit)
            .map(|(c, remote)| component_to_entry(c, remote.clone()))
            .collect()
    }

    pub fn get_recent_apps(&self, limit: usize) -> Vec<AppStreamEntry> {
        self.components
            .iter()
            .filter(|(c, _)| {
                matches!(
                    c.kind,
                    ComponentKind::DesktopApplication | ComponentKind::ConsoleApplication
                )
            })
            .rev()
            .take(limit)
            .map(|(c, remote)| component_to_entry(c, remote.clone()))
            .collect()
    }

    pub fn search_apps(&self, query: &str) -> Vec<AppStreamEntry> {
        let q = query.to_lowercase();
        self.components
            .iter()
            // skip runtimes, codecs, fonts etc, we only want things users
            // would actually think of as an app
            .filter(|(c, _)| {
                matches!(
                    c.kind,
                    ComponentKind::DesktopApplication | ComponentKind::ConsoleApplication
                )
            })
            .filter(|(c, _)| {
                let id = c.id.to_string().to_lowercase();
                let name = c
                    .name
                    .get_default()
                    .map(|s| s.to_lowercase())
                    .unwrap_or_default();
                let summary = c
                    .summary
                    .as_ref()
                    .and_then(|s| s.get_default())
                    .map(|s| s.to_lowercase())
                    .unwrap_or_default();
                id.contains(&q) || name.contains(&q) || summary.contains(&q)
            })
            .map(|(c, remote)| component_to_entry(c, remote.clone()))
            .collect()
    }

    pub fn find_by_id(&self, id: &str) -> Option<AppStreamEntry> {
        self.components
            .iter()
            .find(|(c, _)| c.id.to_string() == id)
            .map(|(c, remote)| component_to_entry(c, remote.clone()))
    }

    pub fn get_apps_by_category(&self, category: &str) -> Vec<AppStreamEntry> {
        self.components
            .iter()
            // skip runtimes, codecs, fonts etc, we only want things users
            // would actually think of as an app
            .filter(|(c, _)| {
                matches!(
                    c.kind,
                    ComponentKind::DesktopApplication | ComponentKind::ConsoleApplication
                )
            })
            .filter(|(c, _)| {
                c.categories
                    .iter()
                    .any(|cat| format!("{:?}", cat).to_lowercase() == category.to_lowercase())
            })
            .map(|(c, remote)| component_to_entry(c, remote.clone()))
            .collect()
    }
}

fn component_to_entry(c: &Component, remote: Option<String>) -> AppStreamEntry {
    // Extract icon URL from the first available icon
    // AppStream icons can be local (name only) or remote (full URL)
    let icon_url = c.icons.first().and_then(|icon| {
        // Prefer remote URLs, fall back to local icon names
        match icon {
            appstream::enums::Icon::Remote { url, .. } => Some(url.to_string()),
            appstream::enums::Icon::Local { path, .. } => Some(format!("local:{}", path.display())),
            appstream::enums::Icon::Cached { path, .. } => {
                Some(format!("local:{}", path.display()))
            }
            appstream::enums::Icon::Stock(name) => Some(format!("local:{}", name)),
        }
    });

    // Pick the source image URL from each screenshot; fall back to first thumbnail
    let screenshots: Vec<String> = c
        .screenshots
        .iter()
        .filter_map(|s| {
            s.images
                .iter()
                .find(|img| img.kind == ImageKind::Source)
                .or_else(|| s.images.first())
                .map(|img| img.url.to_string())
        })
        .take(5)
        .collect();

    AppStreamEntry {
        id: c.id.to_string(),
        // get_default() returns the untranslated string, good enough for search
        name: c.name.get_default().cloned().unwrap_or_default(),
        summary: c
            .summary
            .as_ref()
            .and_then(|s| s.get_default())
            .cloned()
            .unwrap_or_default(),
        icon_url,
        remote,
        screenshots,
    }
}

// flatpak lays out appstream data as <root>/<remote>/<arch>/active/appstream.xml.gz
// the "active" symlink points to the current generation, older ones sit next to it
fn load_flatpak_root(root: impl AsRef<Path>, out: &mut Vec<(Component, Option<String>)>) {
    let Ok(remotes) = std::fs::read_dir(root.as_ref()) else {
        return;
    };
    for remote_dir in remotes.flatten() {
        let remote_name = remote_dir.file_name().to_string_lossy().to_string();
        let Ok(arches) = std::fs::read_dir(remote_dir.path()) else {
            continue;
        };
        for arch in arches.flatten() {
            let base = arch.path().join("active");
            let gz = base.join("appstream.xml.gz");
            let xml = base.join("appstream.xml");
            if gz.exists() {
                if let Ok(col) = Collection::from_gzipped(gz) {
                    out.extend(
                        col.components
                            .into_iter()
                            .map(|c| (c, Some(remote_name.clone()))),
                    );
                }
            } else if xml.exists() {
                if let Ok(col) = Collection::from_path(xml) {
                    out.extend(
                        col.components
                            .into_iter()
                            .map(|c| (c, Some(remote_name.clone()))),
                    );
                }
            }
        }
    }
}

pub fn entry_to_flatpak_package(entry: AppStreamEntry, installed: bool) -> Package {
    let name = if entry.name.is_empty() {
        entry.id.clone()
    } else {
        entry.name
    };
    Package {
        id: entry.id,
        name,
        version: String::new(),
        description: entry.summary,
        provider: Provider::Flatpak,
        installed,
        icon_url: entry.icon_url,
        remote: entry.remote,
        screenshots: entry.screenshots,
    }
}
