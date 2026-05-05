use appstream::enums::{ContentAttribute, ContentState, ProjectUrl};
use appstream::{Collection, Component};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static FLATPAK_DB: OnceLock<AppStreamDb> = OnceLock::new();

// AppStream database for loading app metadata from Flatpak remotes
pub struct AppStreamDb {
    components: Vec<(Component, Option<String>)>,
}

#[allow(dead_code)]
pub struct AppStreamEntry {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub icon_url: Option<String>,
    pub remote: Option<String>,
    pub license: Option<String>,
    pub homepage_url: Option<String>,
    pub content_rating_age: String,
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

    #[allow(dead_code)]
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

    #[allow(dead_code)]
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

    #[allow(dead_code)]
    pub fn find_by_id(&self, id: &str) -> Option<AppStreamEntry> {
        self.components
            .iter()
            .find(|(c, _)| c.id.to_string() == id)
            .map(|(c, remote)| component_to_entry(c, remote.clone()))
    }
}

fn attr_severity(attr: &ContentAttribute) -> u8 {
    fn state_sev(s: &ContentState) -> u8 {
        match s {
            ContentState::None => 0,
            ContentState::Mild => 1,
            ContentState::Moderate => 2,
            ContentState::Intense => 3,
        }
    }
    match attr {
        ContentAttribute::ViolenceCartoon(s) | ContentAttribute::ViolenceFantasy(s)
        | ContentAttribute::ViolenceRealistic(s) | ContentAttribute::ViolenceBloodshed(s)
        | ContentAttribute::ViolenceSexual(s) | ContentAttribute::ViolenceDesecration(s)
        | ContentAttribute::ViolenceSlavery(s) | ContentAttribute::ViolenceWorship(s)
        | ContentAttribute::DrugsAlcohol(s) | ContentAttribute::DrugsNarcotics(s)
        | ContentAttribute::DrugsTobacco(s) | ContentAttribute::SexNudity(s)
        | ContentAttribute::SexThemes(s) | ContentAttribute::SexHomosexuality(s)
        | ContentAttribute::SexProstitution(s) | ContentAttribute::SexAdultery(s)
        | ContentAttribute::SexAppearance(s) | ContentAttribute::LanguageProfanity(s)
        | ContentAttribute::LanguageHumor(s) | ContentAttribute::LanguageDiscrimination(s)
        | ContentAttribute::SocialChat(s) | ContentAttribute::SocialInfo(s)
        | ContentAttribute::SocialAudio(s) | ContentAttribute::SocialLocation(s)
        | ContentAttribute::SocialContacts(s) | ContentAttribute::MoneyAdvertising(s)
        | ContentAttribute::MoneyPurchasing(s) | ContentAttribute::MoneyGambling(s) => state_sev(s),
        _ => 0,
    }
}

fn component_to_entry(c: &Component, remote: Option<String>) -> AppStreamEntry {
    let icon_url = c.icons.first().and_then(|icon| match icon {
        appstream::enums::Icon::Remote { url, .. } => Some(url.to_string()),
        appstream::enums::Icon::Local { path, .. } => Some(format!("local:{}", path.display())),
        appstream::enums::Icon::Cached { path, .. } => Some(format!("local:{}", path.display())),
        appstream::enums::Icon::Stock(name) => Some(format!("local:{}", name)),
    });

    let license = c.project_license.as_ref().map(|l| l.to_string());

    let homepage_url = c.urls.iter().find_map(|u| match u {
        ProjectUrl::Homepage(url) => Some(url.to_string()),
        _ => None,
    });

    let content_rating_age = c
        .content_rating
        .as_ref()
        .map(|rating| {
            let max = rating.attributes.iter().map(attr_severity).max().unwrap_or(0);
            match max {
                3 => "18+".to_string(),
                2 => "12+".to_string(),
                1 => "7+".to_string(),
                _ => "All ages".to_string(),
            }
        })
        .unwrap_or_default();

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
        license,
        homepage_url,
        content_rating_age,
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
