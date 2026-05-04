use appstream::{Collection, Component};
use std::path::{Path, PathBuf};

// AppStream database for loading app metadata from Flatpak remotes
pub struct AppStreamDb {
    components: Vec<(Component, Option<String>)>,
}

pub struct AppStreamEntry {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub icon_url: Option<String>,
    pub remote: Option<String>,
}

impl AppStreamDb {
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
        // For now, just return the first N desktop applications
        // In the future, we could sort by some metric if available in AppStream data
        self.components
            .iter()
            .filter(|(c, _)| {
                matches!(
                    c.kind,
                    appstream::enums::ComponentKind::DesktopApplication
                        | appstream::enums::ComponentKind::ConsoleApplication
                )
            })
            .take(limit)
            .map(|(c, remote)| component_to_entry(c, remote.clone()))
            .collect()
    }

    pub fn get_recent_apps(&self, limit: usize) -> Vec<AppStreamEntry> {
        // For now, just return apps from the end of the list
        // AppStream doesn't have a "recently added" concept, so this is a placeholder
        self.components
            .iter()
            .filter(|(c, _)| {
                matches!(
                    c.kind,
                    appstream::enums::ComponentKind::DesktopApplication
                        | appstream::enums::ComponentKind::ConsoleApplication
                )
            })
            .rev()
            .take(limit)
            .map(|(c, remote)| component_to_entry(c, remote.clone()))
            .collect()
    }

    pub fn get_apps_by_category(&self, category: &str) -> Vec<AppStreamEntry> {
        self.components
            .iter()
            .filter(|(c, _)| {
                matches!(
                    c.kind,
                    appstream::enums::ComponentKind::DesktopApplication
                        | appstream::enums::ComponentKind::ConsoleApplication
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

    pub fn search_apps(&self, query: &str) -> Vec<AppStreamEntry> {
        let q = query.to_lowercase();
        self.components
            .iter()
            .filter(|(c, _)| {
                matches!(
                    c.kind,
                    appstream::enums::ComponentKind::DesktopApplication
                        | appstream::enums::ComponentKind::ConsoleApplication
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
}

fn component_to_entry(c: &Component, remote: Option<String>) -> AppStreamEntry {
    let icon_url = c.icons.first().and_then(|icon| match icon {
        appstream::enums::Icon::Remote { url, .. } => Some(url.to_string()),
        appstream::enums::Icon::Local { path, .. } => Some(format!("local:{}", path.display())),
        appstream::enums::Icon::Cached { path, .. } => Some(format!("local:{}", path.display())),
        appstream::enums::Icon::Stock(name) => Some(format!("local:{}", name)),
    });

    AppStreamEntry {
        id: c.id.to_string(),
        name: c.name.get_default().cloned().unwrap_or_default(),
        summary: c
            .summary
            .as_ref()
            .and_then(|s| s.get_default())
            .cloned()
            .unwrap_or_default(),
        icon_url,
        remote,
    }
}

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

pub fn appstream_db_for_home() -> AppStreamDb {
    AppStreamDb::load_flatpak()
}
