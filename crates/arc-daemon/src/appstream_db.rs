use appstream::{Collection, Component};
use appstream::enums::ComponentKind;
use libarc::{Package, Provider};
use std::path::{Path, PathBuf};

// appstream is a big xml catalog of apps that distros ship alongside their packages
// so you get descriptions, icons and categories without hitting the network
pub struct AppStreamDb {
    components: Vec<Component>,
}

pub struct AppStreamEntry {
    pub id: String,
    pub name: String,
    pub summary: String,
    // pkgname is the native package name (e.g. "gimp") vs the appstream id
    // ("org.gimp.GIMP"), not always present, especially for flatpak entries
    pub pkgname: Option<String>,
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

    pub fn load_system() -> Self {
        let mut components = Vec::new();
        // distros put their appstream xmls in one of these two places depending
        // on whether its from a repo cache or installed via a package
        for dir in ["/usr/share/app-info/xmls", "/var/cache/app-info/xmls"] {
            load_xml_dir(Path::new(dir), &mut components);
        }
        Self { components }
    }

    pub fn search_apps(&self, query: &str) -> Vec<AppStreamEntry> {
        let q = query.to_lowercase();
        self.components
            .iter()
            // skip runtimes, codecs, fonts etc, we only want things users
            // would actually think of as an app
            .filter(|c| {
                matches!(
                    c.kind,
                    ComponentKind::DesktopApplication | ComponentKind::ConsoleApplication
                )
            })
            .filter(|c| {
                let id = c.id.to_string().to_lowercase();
                let name = c.name.get_default().map(|s| s.to_lowercase()).unwrap_or_default();
                let summary = c.summary.as_ref()
                    .and_then(|s| s.get_default())
                    .map(|s| s.to_lowercase())
                    .unwrap_or_default();
                id.contains(&q) || name.contains(&q) || summary.contains(&q)
            })
            .map(component_to_entry)
            .collect()
    }

    pub fn find_by_id(&self, id: &str) -> Option<AppStreamEntry> {
        self.components
            .iter()
            .find(|c| c.id.to_string() == id)
            .map(component_to_entry)
    }

    pub fn find_by_pkgname(&self, pkgname: &str) -> Option<AppStreamEntry> {
        self.components
            .iter()
            .find(|c| {
                c.pkgname
                    .as_deref()
                    .map(|n| n == pkgname)
                    .unwrap_or(false)
            })
            .map(component_to_entry)
    }
}

fn component_to_entry(c: &Component) -> AppStreamEntry {
    AppStreamEntry {
        id: c.id.to_string(),
        // get_default() returns the untranslated string, good enough for search
        name: c.name.get_default().cloned().unwrap_or_default(),
        summary: c.summary.as_ref().and_then(|s| s.get_default()).cloned().unwrap_or_default(),
        pkgname: c.pkgname.clone(),
    }
}

// flatpak lays out appstream data as <root>/<remote>/<arch>/active/appstream.xml.gz
// the "active" symlink points to the current generation, older ones sit next to it
fn load_flatpak_root(root: impl AsRef<Path>, out: &mut Vec<Component>) {
    let Ok(remotes) = std::fs::read_dir(root.as_ref()) else {
        return;
    };
    for remote in remotes.flatten() {
        let Ok(arches) = std::fs::read_dir(remote.path()) else {
            continue;
        };
        for arch in arches.flatten() {
            let base = arch.path().join("active");
            let gz = base.join("appstream.xml.gz");
            let xml = base.join("appstream.xml");
            if gz.exists() {
                if let Ok(col) = Collection::from_gzipped(gz) {
                    out.extend(col.components);
                }
            } else if xml.exists() {
                if let Ok(col) = Collection::from_path(xml) {
                    out.extend(col.components);
                }
            }
        }
    }
}

fn load_xml_dir(dir: &Path, out: &mut Vec<Component>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        match path.extension().and_then(|e| e.to_str()) {
            Some("gz") => {
                if let Ok(col) = Collection::from_gzipped(path) {
                    out.extend(col.components);
                }
            }
            Some("xml") => {
                if let Ok(col) = Collection::from_path(path) {
                    out.extend(col.components);
                }
            }
            _ => {}
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
    }
}

pub fn entry_to_native_package(entry: AppStreamEntry, installed: bool) -> Package {
    // prefer the pkgname (e.g. "gimp") over the appstream id ("org.gimp.GIMP")
    // because that is what packagekit expects when you install or remove
    let id = entry.pkgname.clone().unwrap_or_else(|| entry.id.clone());
    let name = if entry.name.is_empty() {
        id.clone()
    } else {
        entry.name
    };
    Package {
        id,
        name,
        version: String::new(),
        description: entry.summary,
        provider: Provider::Native,
        installed,
    }
}
