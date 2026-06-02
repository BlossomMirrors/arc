use appstream::enums::{Bundle, ComponentKind, ContentAttribute, ContentState, ImageKind, ProjectUrl};
use appstream::{Collection, Component, MarkupTranslatableString, TranslatableString};
use libarc::{Package, Provider};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static FLATPAK_DB: OnceLock<AppStreamDb> = OnceLock::new();

// appstream is a big xml catalog of apps that distros ship alongside their packages
// so you get descriptions, icons and categories without hitting the network
pub struct AppStreamDb {
    components: Vec<(Component, Option<String>)>,
    // Locale candidates in priority order, e.g. ["de_DE", "de"] for a German system.
    locales: Vec<String>,
    // Supplemental description map built by re-parsing the raw XML with xmltree.
    // The appstream crate merges all <p xml:lang="de"> paragraphs into the "C" entry
    // (dropping the language tag), so we extract them ourselves.
    // Keyed by app-id → locale-code → HTML string.
    descriptions: HashMap<String, HashMap<String, String>>,
}

#[derive(serde::Serialize)]
pub struct AppStreamEntry {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub description: String,
    pub icon_url: Option<String>,
    pub remote: Option<String>,
    pub screenshots: Vec<String>,
    pub license: Option<String>,
    pub eula_url: Option<String>,
    pub homepage_url: Option<String>,
    pub content_rating: String,
    pub developer_name: Option<String>,
}

// Build a priority list of locale codes from the process environment.
// For "de_DE.UTF-8" we return ["de_DE", "de"]; for "C" or unset we return [].
fn detect_locales() -> Vec<String> {
    let raw = std::env::var("LANGUAGE")
        .ok()
        .and_then(|l| l.split(':').next().map(|s| s.to_string()))
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("LANG").ok())
        .or_else(|| std::env::var("LC_ALL").ok())
        .or_else(|| std::env::var("LC_MESSAGES").ok())
        .unwrap_or_default();

    let locale = raw
        .split('.')
        .next()
        .unwrap_or("")
        .split('@')
        .next()
        .unwrap_or("")
        .to_string();

    if locale.is_empty() || locale == "C" || locale == "POSIX" {
        return vec![];
    }

    let mut candidates = vec![locale.clone()];
    if let Some(lang) = locale.split('_').next() {
        if lang != locale {
            candidates.push(lang.to_string());
        }
    }
    candidates
}

// Convert a BCP-47 / POSIX language tag into AppStream locale candidates.
// "de-DE" or "de_DE" → ["de_DE", "de"]; "de" → ["de"]; "" or "en" → [].
pub fn parse_locale_candidates(lang: &str) -> Vec<String> {
    let normalized = lang.replace('-', "_");
    let base = normalized
        .split('.')
        .next()
        .unwrap_or("")
        .split('@')
        .next()
        .unwrap_or("")
        .trim();
    if base.is_empty() || base == "C" || base == "POSIX" || base.starts_with("en") {
        return vec![];
    }
    let mut candidates = vec![base.to_string()];
    if let Some(lang_only) = base.split('_').next() {
        if lang_only != base {
            candidates.push(lang_only.to_string());
        }
    }
    candidates
}

// Return the best available translation for the given locale candidates,
// falling back to the AppStream default locale ("C" = English).
fn localize_ts<'a>(ts: &'a TranslatableString, locales: &[String]) -> Option<&'a str> {
    locales
        .iter()
        .find_map(|l| ts.get_for_locale(l))
        .or_else(|| ts.get_default())
        .map(|s| s.as_str())
}

fn localize_mts<'a>(ts: &'a MarkupTranslatableString, locales: &[String]) -> Option<&'a str> {
    locales
        .iter()
        .find_map(|l| ts.get_for_locale(l))
        .or_else(|| ts.get_default())
        .map(|s| s.as_str())
}

impl AppStreamDb {
    pub fn get_static() -> &'static AppStreamDb {
        FLATPAK_DB.get_or_init(Self::load_flatpak)
    }

    pub fn load_flatpak() -> Self {
        let locales = detect_locales();
        let mut components = Vec::new();
        let mut descriptions: HashMap<String, HashMap<String, String>> = HashMap::new();
        load_flatpak_root(
            "/var/lib/flatpak/appstream",
            &mut components,
            &mut descriptions,
        );
        if let Some(home) = std::env::var_os("HOME") {
            let path = PathBuf::from(home).join(".local/share/flatpak/appstream");
            load_flatpak_root(&path, &mut components, &mut descriptions);
        }
        Self {
            components,
            locales,
            descriptions,
        }
    }

    pub fn get_popular_apps(&self, limit: usize) -> Vec<AppStreamEntry> {
        self.get_popular_apps_with_locales(limit, &self.locales)
    }

    pub fn get_popular_apps_with_locales(&self, limit: usize, locales: &[String]) -> Vec<AppStreamEntry> {
        self.components
            .iter()
            .filter(|(c, _)| matches!(c.kind, ComponentKind::DesktopApplication | ComponentKind::ConsoleApplication))
            .take(limit)
            .map(|(c, remote)| component_to_entry(c, remote.clone(), locales, &self.descriptions))
            .collect()
    }

    pub fn get_recent_apps(&self, limit: usize) -> Vec<AppStreamEntry> {
        self.get_recent_apps_with_locales(limit, &self.locales)
    }

    pub fn get_recent_apps_with_locales(&self, limit: usize, locales: &[String]) -> Vec<AppStreamEntry> {
        self.components
            .iter()
            .filter(|(c, _)| matches!(c.kind, ComponentKind::DesktopApplication | ComponentKind::ConsoleApplication))
            .rev()
            .take(limit)
            .map(|(c, remote)| component_to_entry(c, remote.clone(), locales, &self.descriptions))
            .collect()
    }

    pub fn search_apps(&self, query: &str) -> Vec<AppStreamEntry> {
        self.search_apps_with_locales(query, &self.locales)
    }

    pub fn search_apps_with_locales(&self, query: &str, locales: &[String]) -> Vec<AppStreamEntry> {
        let q = query.to_lowercase();
        self.components
            .iter()
            .filter(|(c, _)| matches!(c.kind, ComponentKind::DesktopApplication | ComponentKind::ConsoleApplication))
            .filter(|(c, _)| {
                let id = c.id.to_string().to_lowercase();
                let name_default = c.name.get_default().map(|s| s.to_lowercase()).unwrap_or_default();
                let name_localized = locales.iter().find_map(|l| c.name.get_for_locale(l)).map(|s| s.to_lowercase()).unwrap_or_default();
                let summary_default = c.summary.as_ref().and_then(|s| s.get_default()).map(|s| s.to_lowercase()).unwrap_or_default();
                let summary_localized = c.summary.as_ref().and_then(|s| locales.iter().find_map(|l| s.get_for_locale(l))).map(|s| s.to_lowercase()).unwrap_or_default();
                id.contains(&q) || name_default.contains(&q) || name_localized.contains(&q) || summary_default.contains(&q) || summary_localized.contains(&q)
            })
            .map(|(c, remote)| component_to_entry(c, remote.clone(), locales, &self.descriptions))
            .collect()
    }

    pub fn find_by_id(&self, id: &str) -> Option<AppStreamEntry> {
        self.find_by_id_with_locales(id, &self.locales)
    }

    pub fn find_by_id_with_locales(&self, id: &str, locales: &[String]) -> Option<AppStreamEntry> {
        let with_desktop = format!("{}.desktop", id);
        self.components
            .iter()
            .find(|(c, _)| { let cid = c.id.to_string(); cid == id || cid == with_desktop })
            .map(|(c, remote)| component_to_entry(c, remote.clone(), locales, &self.descriptions))
    }

    // Fallback for installed apps absent from any AppStream catalog.
    // Every installed Flatpak exports its own metainfo file; we parse that directly.
    pub fn load_from_exported_metainfo(&self, id: &str) -> Option<AppStreamEntry> {
        self.load_from_exported_metainfo_with_locales(id, &self.locales)
    }

    pub fn load_from_exported_metainfo_with_locales(&self, id: &str, locales: &[String]) -> Option<AppStreamEntry> {
        let mut dirs = vec![
            PathBuf::from("/var/lib/flatpak/exports/share/metainfo"),
            PathBuf::from("/var/lib/flatpak/exports/share/appdata"),
        ];
        if let Some(home) = std::env::var_os("HOME") {
            let h = PathBuf::from(home);
            dirs.push(h.join(".local/share/flatpak/exports/share/metainfo"));
            dirs.push(h.join(".local/share/flatpak/exports/share/appdata"));
        }
        for dir in &dirs {
            for suffix in &[".metainfo.xml", ".appdata.xml"] {
                let path = dir.join(format!("{}{}", id, suffix));
                if !path.exists() { continue; }
                let Ok(bytes) = std::fs::read(&path) else { continue; };
                if let Some(entry) = parse_metainfo_bytes(id, &bytes, locales) {
                    return Some(entry);
                }
            }
        }
        None
    }

    pub fn get_apps_by_category(&self, category: &str) -> Vec<AppStreamEntry> {
        self.get_apps_by_category_with_locales(category, &self.locales)
    }

    pub fn get_apps_by_category_with_locales(&self, category: &str, locales: &[String]) -> Vec<AppStreamEntry> {
        self.components
            .iter()
            .filter(|(c, _)| matches!(c.kind, ComponentKind::DesktopApplication | ComponentKind::ConsoleApplication))
            .filter(|(c, _)| c.categories.iter().any(|cat| format!("{:?}", cat).to_lowercase() == category.to_lowercase()))
            .map(|(c, remote)| component_to_entry(c, remote.clone(), locales, &self.descriptions))
            .collect()
    }
}

// Parse a single metainfo/appdata XML file and build an AppStreamEntry.
// The `id` argument is the canonical app ID (i.e. what we searched the file by)
// and is used directly rather than reading the potentially-.desktop-suffixed <id> element.
fn parse_metainfo_bytes(id: &str, bytes: &[u8], locales: &[String]) -> Option<AppStreamEntry> {
    let Ok(root) = xmltree::Element::parse(bytes) else { return None; };

    // Pick the best-matching localized value for a repeating tag (e.g. <name xml:lang="de">).
    let pick = |tag: &str| -> String {
        let mut default_val = String::new();
        let mut by_lang: HashMap<String, String> = HashMap::new();
        for child in root.children.iter().filter_map(|n| n.as_element()) {
            if child.name != tag { continue; }
            let text = child.get_text().map(|t| t.trim().to_string()).unwrap_or_default();
            if let Some(lang) = child.attributes.get("lang") {
                by_lang.insert(lang.clone(), text);
            } else {
                default_val = text;
            }
        }
        locales.iter().find_map(|l| by_lang.get(l)).cloned().unwrap_or(default_val)
    };

    let name = pick("name");
    let summary = pick("summary");
    let developer_name = root.children.iter().filter_map(|n| n.as_element()).find_map(|e| {
        if e.name == "developer_name" {
            e.get_text().map(|t| t.trim().to_string())
        } else if e.name == "developer" {
            e.children.iter().filter_map(|n| n.as_element())
                .find(|c| c.name == "name")
                .and_then(|c| c.get_text())
                .map(|t| t.trim().to_string())
        } else {
            None
        }
    });

    // Description via the same locale-aware xmltree extractor used for catalog files.
    let mut desc_map: HashMap<String, HashMap<String, String>> = HashMap::new();
    extract_descriptions(bytes, &mut desc_map);
    // The map may be keyed by the .desktop-suffixed ID from the <id> element.
    let comp_id = root.children.iter().filter_map(|n| n.as_element())
        .find(|e| e.name == "id")
        .and_then(|e| e.get_text())
        .map(|t| t.trim().to_string())
        .unwrap_or_else(|| id.to_string());
    let description = desc_map.get(comp_id.as_str())
        .and_then(|lm| locales.iter().find_map(|l| lm.get(l)).or_else(|| lm.get("C")))
        .cloned()
        .unwrap_or_default();

    let license_raw = root.children.iter().filter_map(|n| n.as_element())
        .find(|e| e.name == "project_license")
        .and_then(|e| e.get_text())
        .map(|t| t.trim().to_string());
    let (license, eula_url) = match license_raw {
        Some(l) if l.starts_with("LicenseRef-proprietary=http") => {
            let url = l.strip_prefix("LicenseRef-proprietary=").unwrap_or("").to_string();
            (Some("Proprietary".to_string()), Some(url))
        }
        Some(l) if l.contains("LicenseRef-proprietary") => (Some("Proprietary".to_string()), None),
        other => (other, None),
    };

    let homepage_url = root.children.iter().filter_map(|n| n.as_element())
        .find(|e| e.name == "url" && e.attributes.get("type").map(|t| t == "homepage").unwrap_or(false))
        .and_then(|e| e.get_text())
        .map(|t| t.trim().to_string());

    let content_rating = root.children.iter().filter_map(|n| n.as_element())
        .find(|e| e.name == "content_rating")
        .map(|rating| {
            let max = rating.children.iter().filter_map(|n| n.as_element())
                .filter_map(|e| e.get_text().map(|t| t.trim().to_string()))
                .map(|v| match v.as_str() { "intense" => 3u8, "moderate" => 2, "mild" => 1, _ => 0 })
                .max().unwrap_or(0);
            match max { 3 => "18+", 2 => "12+", 1 => "7+", _ => "All ages" }.to_string()
        })
        .unwrap_or_default();

    let screenshots: Vec<String> = root.children.iter().filter_map(|n| n.as_element())
        .find(|e| e.name == "screenshots")
        .map(|ss| {
            ss.children.iter().filter_map(|n| n.as_element())
                .filter(|e| e.name == "screenshot")
                .filter_map(|sc| sc.children.iter().filter_map(|n| n.as_element())
                    .find(|e| e.name == "image")
                    .and_then(|img| img.get_text())
                    .map(|t| t.trim().to_string()))
                .take(5).collect()
        })
        .unwrap_or_default();

    Some(AppStreamEntry {
        id: id.to_string(),
        name,
        summary,
        description,
        icon_url: None,
        remote: None,
        screenshots,
        license,
        eula_url,
        homepage_url,
        content_rating,
        developer_name,
    })
}

fn element_to_html(e: &xmltree::Element) -> String {
    let inner: String = e
        .children
        .iter()
        .map(|node| match node {
            xmltree::XMLNode::Element(c) => element_to_html(c),
            xmltree::XMLNode::Text(t) => t.clone(),
            _ => String::new(),
        })
        .collect();
    format!("<{}>{}</{}>", e.name, inner, e.name)
}

fn children_to_html(e: &xmltree::Element) -> String {
    e.children
        .iter()
        .map(|node| match node {
            xmltree::XMLNode::Element(c) => element_to_html(c),
            xmltree::XMLNode::Text(t) => t.clone(),
            _ => String::new(),
        })
        .collect()
}

// Parse raw AppStream XML bytes and extract per-locale descriptions.
// Handles two formats used in the wild:
//   Format 1 (catalog): <description xml:lang="de"><p>…</p></description>
//   Format 2 (metainfo/some remotes): <description><p xml:lang="de">…</p></description>
// In both cases xmltree stores the "xml:lang" attribute under the local name "lang".
fn extract_descriptions(xml_bytes: &[u8], out: &mut HashMap<String, HashMap<String, String>>) {
    let Ok(root) = xmltree::Element::parse(xml_bytes) else {
        return;
    };

    let components: Vec<&xmltree::Element> =
        if root.name == "components" || root.name == "collection" {
            root.children
                .iter()
                .filter_map(|n| n.as_element())
                .filter(|e| e.name == "component")
                .collect()
        } else if root.name == "component" {
            vec![&root]
        } else {
            return;
        };

    for comp in components {
        let id = comp
            .children
            .iter()
            .filter_map(|n| n.as_element())
            .find(|e| e.name == "id")
            .and_then(|e| e.get_text())
            .map(|t| t.trim().to_string());
        let Some(id) = id else {
            continue;
        };

        let lang_map = out.entry(id).or_default();

        for desc_elem in comp
            .children
            .iter()
            .filter_map(|n| n.as_element())
            .filter(|e| e.name == "description")
        {
            if let Some(lang) = desc_elem.attributes.get("lang") {
                // Format 1: the <description> element itself has a lang attribute.
                let html = children_to_html(desc_elem);
                if !html.trim().is_empty() {
                    lang_map.insert(lang.clone(), html);
                }
            } else {
                // Format 2: each child paragraph may carry its own lang attribute.
                // Paragraphs without a lang attribute belong to the default locale.
                let mut by_lang: HashMap<String, String> = HashMap::new();
                for para in desc_elem.children.iter().filter_map(|n| n.as_element()) {
                    let lang = para
                        .attributes
                        .get("lang")
                        .cloned()
                        .unwrap_or_else(|| "C".to_string());
                    by_lang
                        .entry(lang)
                        .or_default()
                        .push_str(&element_to_html(para));
                }
                for (lang, html) in by_lang {
                    if !html.trim().is_empty() {
                        // Don't overwrite an entry already set by a format-1 sibling element.
                        lang_map.entry(lang).or_insert(html);
                    }
                }
            }
        }
    }
}

fn read_gz_bytes(path: &Path) -> std::io::Result<Vec<u8>> {
    use flate2::read::GzDecoder;
    use std::io::Read;
    let file = std::fs::File::open(path)?;
    let mut decoder = GzDecoder::new(file);
    let mut buf = Vec::new();
    decoder.read_to_end(&mut buf)?;
    Ok(buf)
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
        ContentAttribute::ViolenceCartoon(s)
        | ContentAttribute::ViolenceFantasy(s)
        | ContentAttribute::ViolenceRealistic(s)
        | ContentAttribute::ViolenceBloodshed(s)
        | ContentAttribute::ViolenceSexual(s)
        | ContentAttribute::ViolenceDesecration(s)
        | ContentAttribute::ViolenceSlavery(s)
        | ContentAttribute::ViolenceWorship(s)
        | ContentAttribute::DrugsAlcohol(s)
        | ContentAttribute::DrugsNarcotics(s)
        | ContentAttribute::DrugsTobacco(s)
        | ContentAttribute::SexNudity(s)
        | ContentAttribute::SexThemes(s)
        | ContentAttribute::SexHomosexuality(s)
        | ContentAttribute::SexProstitution(s)
        | ContentAttribute::SexAdultery(s)
        | ContentAttribute::SexAppearance(s)
        | ContentAttribute::LanguageProfanity(s)
        | ContentAttribute::LanguageHumor(s)
        | ContentAttribute::LanguageDiscrimination(s)
        | ContentAttribute::SocialChat(s)
        | ContentAttribute::SocialInfo(s)
        | ContentAttribute::SocialAudio(s)
        | ContentAttribute::SocialLocation(s)
        | ContentAttribute::SocialContacts(s)
        | ContentAttribute::MoneyAdvertising(s)
        | ContentAttribute::MoneyPurchasing(s)
        | ContentAttribute::MoneyGambling(s) => state_sev(s),
        _ => 0,
    }
}

fn component_to_entry(
    c: &Component,
    remote: Option<String>,
    locales: &[String],
    descriptions: &HashMap<String, HashMap<String, String>>,
) -> AppStreamEntry {
    // Prefer a remote 128×128 URL — local cached files may not be downloaded yet.
    // Fall back to the first available icon (cached → local → stock) otherwise.
    let icon_url = c.icons.iter().find_map(|icon| match icon {
        appstream::enums::Icon::Remote { url, width, .. }
            if width.map(|w| w >= 96).unwrap_or(true) => Some(url.to_string()),
        _ => None,
    }).or_else(|| c.icons.first().and_then(|icon| match icon {
        appstream::enums::Icon::Remote { url, .. } => Some(url.to_string()),
        appstream::enums::Icon::Local { path, .. } => Some(format!("local:{}", path.display())),
        appstream::enums::Icon::Cached { path, .. } => Some(format!("local:{}", path.display())),
        appstream::enums::Icon::Stock(name) => Some(format!("local:{}", name)),
    }));

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

    let (license, eula_url) = match c.project_license.as_ref().map(|l| l.to_string()) {
        Some(l) if l.starts_with("LicenseRef-proprietary=http") => {
            let url = l
                .strip_prefix("LicenseRef-proprietary=")
                .unwrap_or("")
                .to_string();
            (Some("Proprietary".to_string()), Some(url))
        }
        Some(l) if l.contains("LicenseRef-proprietary") => (Some("Proprietary".to_string()), None),
        other => (other, None),
    };

    let homepage_url = c.urls.iter().find_map(|u| match u {
        ProjectUrl::Homepage(url) => Some(url.to_string()),
        _ => None,
    });

    let content_rating = c
        .content_rating
        .as_ref()
        .map(|rating| {
            let max = rating
                .attributes
                .iter()
                .map(attr_severity)
                .max()
                .unwrap_or(0);
            match max {
                3 => "18+".to_string(),
                2 => "12+".to_string(),
                1 => "7+".to_string(),
                _ => "All ages".to_string(),
            }
        })
        .unwrap_or_default();

    // Description: prefer the supplemental map (handles both per-element and
    // per-paragraph xml:lang formats). Fall back to the appstream crate's parse.
    let description = {
        let id = c.id.to_string();
        if let Some(lang_map) = descriptions.get(&id) {
            locales
                .iter()
                .find_map(|l| lang_map.get(l.as_str()))
                .or_else(|| lang_map.get("C"))
                .map(|s| s.as_str())
                .unwrap_or_default()
                .to_string()
        } else {
            c.description
                .as_ref()
                .and_then(|d| localize_mts(d, locales))
                .unwrap_or_default()
                .to_string()
        }
    };

    // Use the Flatpak bundle reference as the canonical app ID when available.
    // AppStream catalogs often store a legacy ".desktop" suffix in the component ID
    // (e.g. "io.github.flattool.Warehouse.desktop") while the actual Flatpak app ID
    // omits it ("io.github.flattool.Warehouse"). The bundle ref is authoritative.
    let canonical_id = c.bundles.iter().find_map(|b| match b {
        Bundle::Flatpak { reference, .. } => reference.split('/').nth(1).map(|s| s.to_string()),
        _ => None,
    }).unwrap_or_else(|| c.id.to_string());

    AppStreamEntry {
        id: canonical_id,
        name: localize_ts(&c.name, locales)
            .unwrap_or_default()
            .to_string(),
        summary: c
            .summary
            .as_ref()
            .and_then(|s| localize_ts(s, locales))
            .unwrap_or_default()
            .to_string(),
        description,
        icon_url,
        remote,
        screenshots,
        license,
        eula_url,
        homepage_url,
        content_rating,
        developer_name: c
            .developer_name
            .as_ref()
            .and_then(|d| localize_ts(d, locales))
            .map(|s| s.to_string()),
    }
}

// Flatpak lays out appstream data as <root>/<remote>/<arch>/active/appstream.xml.gz
fn load_flatpak_root(
    root: impl AsRef<Path>,
    out: &mut Vec<(Component, Option<String>)>,
    out_descriptions: &mut HashMap<String, HashMap<String, String>>,
) {
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
                if let Ok(col) = Collection::from_gzipped(gz.clone()) {
                    out.extend(
                        col.components
                            .into_iter()
                            .map(|c| (c, Some(remote_name.clone()))),
                    );
                }
                // Second pass with xmltree to capture per-paragraph xml:lang descriptions.
                if let Ok(bytes) = read_gz_bytes(&gz) {
                    extract_descriptions(&bytes, out_descriptions);
                }
            } else if xml.exists() {
                if let Ok(col) = Collection::from_path(xml.clone()) {
                    out.extend(
                        col.components
                            .into_iter()
                            .map(|c| (c, Some(remote_name.clone()))),
                    );
                }
                if let Ok(bytes) = std::fs::read(&xml) {
                    extract_descriptions(&bytes, out_descriptions);
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
