use appstream::enums::{Bundle, ComponentKind, ContentAttribute, ContentState, ImageKind, ProjectUrl};
use appstream::{Collection, Component, MarkupTranslatableString, TranslatableString};
use libarc::{Package, Provider};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

// bumped whenever the on-disk Snapshot shape changes to invalidate old caches
const SNAPSHOT_SCHEMA: u32 = 1;

static FLATPAK_DB: OnceLock<RwLock<Arc<AppStreamDb>>> = OnceLock::new();
// flips once every catalog file has been folded into FLATPAK_DB
static FULLY_LOADED: AtomicBool = AtomicBool::new(false);
// a plain flag plus a detached thread lets try_get() kick the load off
// without waiting on it
static LOAD_KICKED_OFF: AtomicBool = AtomicBool::new(false);
// set once a loaded snapshot's key no longer matches the live catalog files
static SNAPSHOT_STALE: AtomicBool = AtomicBool::new(false);
static CURRENT_KEY: Mutex<Option<SnapshotKey>> = Mutex::new(None);

#[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone)]
struct SourceStamp {
    path: PathBuf,
    mtime_secs: i64,
    size: u64,
}

// identifies exactly which catalog files (and versions of them) a parse covers
// a mismatch against the live filesystem means the snapshot is stale
#[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq, Clone)]
struct SnapshotKey {
    schema: u32,
    sources: Vec<SourceStamp>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct Snapshot {
    key: SnapshotKey,
    components: Vec<(Component, Option<String>)>,
    descriptions: HashMap<String, HashMap<String, String>>,
    verifications: HashMap<String, bool>,
}

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
    // Verified app IDs extracted from <custom><value key="flathub::verification::verified">
    // The appstream crate only parses <metadata> tags, not <custom>, so we do this ourselves.
    verifications: HashMap<String, bool>,
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
    pub verified: bool,
    pub categories: Vec<String>,
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

fn slot() -> &'static RwLock<Arc<AppStreamDb>> {
    FLATPAK_DB.get_or_init(|| {
        RwLock::new(Arc::new(AppStreamDb {
            components: Vec::new(),
            locales: detect_locales(),
            descriptions: HashMap::new(),
            verifications: HashMap::new(),
        }))
    })
}

// kicks the load off once on a detached thread and returns right away
fn ensure_load_started() {
    if LOAD_KICKED_OFF.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
        return;
    }
    std::thread::spawn(|| {
        let live_key = compute_snapshot_key();
        if let Some(snapshot) = load_snapshot_from_disk() {
            *CURRENT_KEY.lock().unwrap() = Some(snapshot.key.clone());
            if snapshot.key != live_key {
                SNAPSHOT_STALE.store(true, Ordering::Relaxed);
            }
            *slot().write().unwrap() = Arc::new(AppStreamDb {
                components: snapshot.components,
                locales: detect_locales(),
                descriptions: snapshot.descriptions,
                verifications: snapshot.verifications,
            });
            FULLY_LOADED.store(true, Ordering::Relaxed);
            return;
        }

        let db = load_flatpak_progressive();
        persist_snapshot(&db, &live_key);
        *CURRENT_KEY.lock().unwrap() = Some(live_key);
        FULLY_LOADED.store(true, Ordering::Relaxed);
    });
}

impl AppStreamDb {
    // blocks until every catalog file is loaded
    pub fn get() -> Arc<AppStreamDb> {
        ensure_load_started();
        while !FULLY_LOADED.load(Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
        slot().read().unwrap().clone()
    }

    // never blocks and returns whatever has been folded in so far
    pub fn try_get() -> Arc<AppStreamDb> {
        ensure_load_started();
        slot().read().unwrap().clone()
    }

    pub fn snapshot_was_stale() -> bool {
        SNAPSHOT_STALE.load(Ordering::Relaxed)
    }

    // true once every catalog file has been folded in
    pub fn fully_loaded() -> bool {
        FULLY_LOADED.load(Ordering::Relaxed)
    }

    // re-parses and swaps in a fresh db if the catalog files changed
    pub fn refresh_if_stale() {
        let fresh_key = compute_snapshot_key();
        if CURRENT_KEY.lock().unwrap().as_ref() == Some(&fresh_key) {
            return;
        }
        let db = load_flatpak_progressive();
        persist_snapshot(&db, &fresh_key);
        *CURRENT_KEY.lock().unwrap() = Some(fresh_key);
        SNAPSHOT_STALE.store(false, Ordering::Relaxed);
    }

    pub fn get_popular_apps(&self, limit: usize) -> Vec<AppStreamEntry> {
        self.get_popular_apps_with_locales(limit, &self.locales)
    }

    pub fn get_popular_apps_with_locales(&self, limit: usize, locales: &[String]) -> Vec<AppStreamEntry> {
        self.components
            .iter()
            .filter(|(c, _)| matches!(c.kind, ComponentKind::DesktopApplication | ComponentKind::ConsoleApplication))
            .take(limit)
            .map(|(c, remote)| component_to_entry(c, remote.clone(), locales, &self.descriptions, &self.verifications))
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
            .map(|(c, remote)| component_to_entry(c, remote.clone(), locales, &self.descriptions, &self.verifications))
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
            .map(|(c, remote)| component_to_entry(c, remote.clone(), locales, &self.descriptions, &self.verifications))
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
            .map(|(c, remote)| component_to_entry(c, remote.clone(), locales, &self.descriptions, &self.verifications))
    }

    // Fallback for installed apps absent from any AppStream catalog.
    // Every installed Flatpak exports its own metainfo file; we parse that directly.
    pub fn load_from_exported_metainfo(&self, id: &str) -> Option<AppStreamEntry> {
        self.load_from_exported_metainfo_with_locales(id, &self.locales)
    }

    pub fn load_from_exported_metainfo_with_locales(&self, id: &str, locales: &[String]) -> Option<AppStreamEntry> {
        scan_exported_metainfo(id, locales)
    }

    pub fn get_apps_by_category(&self, category: &str) -> Vec<AppStreamEntry> {
        self.get_apps_by_category_with_locales(category, &self.locales)
    }

    pub fn get_apps_by_category_with_locales(&self, category: &str, locales: &[String]) -> Vec<AppStreamEntry> {
        self.components
            .iter()
            .filter(|(c, _)| matches!(c.kind, ComponentKind::DesktopApplication | ComponentKind::ConsoleApplication))
            .filter(|(c, _)| c.categories.iter().any(|cat| format!("{:?}", cat).to_lowercase() == category.to_lowercase()))
            .map(|(c, remote)| component_to_entry(c, remote.clone(), locales, &self.descriptions, &self.verifications))
            .collect()
    }
}

// standalone lookup for a single installed app's own exported metainfo file
// stays fast independent of the shared db's parse state
pub fn load_local_metainfo(id: &str) -> Option<AppStreamEntry> {
    scan_exported_metainfo(id, &detect_locales())
}

// resolves one id right now instead of waiting for the progressive bulk load
// checks whatever is already loaded first then decompresses and scans only
// the catalog files not yet folded in
pub fn resolve_now(id: &str) -> Option<AppStreamEntry> {
    if let Some(entry) = AppStreamDb::try_get().find_by_id(id) {
        return Some(entry);
    }

    let locales = detect_locales();
    let mut catalogs = Vec::new();
    collect_catalog_paths("/var/lib/flatpak/appstream", &mut catalogs);
    if let Some(home) = std::env::var_os("HOME") {
        let path = PathBuf::from(home).join(".local/share/flatpak/appstream");
        collect_catalog_paths(&path, &mut catalogs);
    }
    catalogs.sort_by_key(|(_, _, size)| *size);

    for (remote_name, path, _) in catalogs {
        let Some(bytes) = cached_raw_bytes(&path) else { continue };
        if let Some(entry) = resolve_from_catalog_bytes(&bytes, id, Some(remote_name), &locales) {
            return Some(entry);
        }
    }
    None
}

// gz/xml bytes for a catalog not yet folded into the shared db
// decoded at most once per file for the lifetime of the process
static RAW_CATALOG_CACHE: OnceLock<Mutex<HashMap<PathBuf, Arc<Vec<u8>>>>> = OnceLock::new();

fn cached_raw_bytes(path: &Path) -> Option<Arc<Vec<u8>>> {
    let cache = RAW_CATALOG_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(bytes) = cache.lock().unwrap().get(path) {
        return Some(bytes.clone());
    }
    let is_gz = path.extension().and_then(|e| e.to_str()) == Some("gz");
    let bytes = if is_gz { read_gz_bytes(path).ok()? } else { std::fs::read(path).ok()? };
    let bytes = Arc::new(bytes);
    cache.lock().unwrap().insert(path.to_path_buf(), bytes.clone());
    Some(bytes)
}

// pulls just the one <component>...</component> block matching id out of a
// full catalog's xml text and parses only that instead of the whole file
fn resolve_from_catalog_bytes(
    xml_bytes: &[u8],
    id: &str,
    remote: Option<String>,
    locales: &[String],
) -> Option<AppStreamEntry> {
    let block = extract_component_block(xml_bytes, id)?;
    let element = xmltree::Element::parse(block.as_slice()).ok()?;
    let collection = Collection::try_from(&element).ok()?;
    let component = collection.components.into_iter().next()?;
    let mut descriptions = HashMap::new();
    extract_descriptions(&block, &mut descriptions);
    let mut verifications = HashMap::new();
    extract_verifications(&block, &mut verifications);
    Some(component_to_entry(&component, remote, locales, &descriptions, &verifications))
}

fn extract_component_block(xml_bytes: &[u8], id: &str) -> Option<Vec<u8>> {
    let text = std::str::from_utf8(xml_bytes).ok()?;
    let needle_plain = format!("<id>{id}</id>");
    let needle_desktop = format!("<id>{id}.desktop</id>");
    let id_pos = text.find(&needle_plain).or_else(|| text.find(&needle_desktop))?;
    // components are siblings under <components> and never nested
    // so the nearest preceding opening tag is this component's own
    let start = text[..id_pos].rfind("<component")?;
    let end_tag = "</component>";
    let end = text[id_pos..].find(end_tag)? + id_pos + end_tag.len();
    let wrapped = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><components version="0.8">{}</components>"#,
        &text[start..end]
    );
    Some(wrapped.into_bytes())
}

fn scan_exported_metainfo(id: &str, locales: &[String]) -> Option<AppStreamEntry> {
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
            if !path.exists() {
                continue;
            }
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            if let Some(entry) = parse_metainfo_bytes(id, &bytes, locales) {
                return Some(entry);
            }
        }
    }
    None
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
        verified: false,
        categories: Vec::new(),
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
    verifications: &HashMap<String, bool>,
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

    let verified = remote.as_deref() == Some("blossomos")
        || verifications.get(c.id.to_string().as_str()).copied().unwrap_or(false);

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
        verified,
        categories: c.categories.iter().map(|cat| format!("{:?}", cat).to_lowercase()).collect(),
    }
}

/// Score an entry against a query using the same field-weight scheme as libarc.
/// Returns `None` if there is no match at all.
pub fn score_entry(entry: &AppStreamEntry, q: &str) -> Option<u32> {
    use libarc::score_field;
    let name = entry.name.to_lowercase();
    let id = entry.id.to_lowercase();
    let summary = entry.summary.to_lowercase();
    let best = score_field(&name, q)
        .max(score_field(&id, q).saturating_sub(10))
        .max(score_field(&summary, q).saturating_sub(20));
    if best > 0 { Some(best) } else { None }
}

// Flatpak lays out appstream data as <root>/<remote>/<arch>/active/appstream.xml.gz
fn extract_verifications(xml_bytes: &[u8], out: &mut HashMap<String, bool>) {
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
        let Some(id) = comp
            .children
            .iter()
            .filter_map(|n| n.as_element())
            .find(|e| e.name == "id")
            .and_then(|e| e.get_text())
            .map(|t| t.trim().to_string())
        else {
            continue;
        };
        let verified = comp
            .children
            .iter()
            .filter_map(|n| n.as_element())
            .find(|e| e.name == "custom")
            .and_then(|custom| {
                custom
                    .children
                    .iter()
                    .filter_map(|n| n.as_element())
                    .find(|e| {
                        e.name == "value"
                            && e.attributes
                                .get("key")
                                .map(|k| k == "flathub::verification::verified")
                                .unwrap_or(false)
                    })
                    .and_then(|e| e.get_text())
                    .map(|t| t.trim() == "true")
            })
            .unwrap_or(false);
        if verified {
            out.insert(id, true);
        }
    }
}

// Flatpak lays out appstream data as <root>/<remote>/<arch>/active/appstream.xml[.gz]
fn collect_catalog_paths(root: impl AsRef<Path>, out: &mut Vec<(String, PathBuf, u64)>) {
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
            let path = if gz.exists() {
                gz
            } else if xml.exists() {
                xml
            } else {
                continue;
            };
            let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            out.push((remote_name.clone(), path, size));
        }
    }
}

fn load_one_catalog(
    path: &Path,
    remote_name: &str,
    out: &mut Vec<(Component, Option<String>)>,
    out_descriptions: &mut HashMap<String, HashMap<String, String>>,
    out_verifications: &mut HashMap<String, bool>,
) {
    let is_gz = path.extension().and_then(|e| e.to_str()) == Some("gz");
    if is_gz {
        if let Ok(col) = Collection::from_gzipped(path.to_path_buf()) {
            out.extend(col.components.into_iter().map(|c| (c, Some(remote_name.to_string()))));
        }
        if let Ok(bytes) = read_gz_bytes(path) {
            extract_descriptions(&bytes, out_descriptions);
            extract_verifications(&bytes, out_verifications);
        }
    } else {
        if let Ok(col) = Collection::from_path(path.to_path_buf()) {
            out.extend(col.components.into_iter().map(|c| (c, Some(remote_name.to_string()))));
        }
        if let Ok(bytes) = std::fs::read(path) {
            extract_descriptions(&bytes, out_descriptions);
            extract_verifications(&bytes, out_verifications);
        }
    }
}

// parses smallest catalogs first and publishes to the shared slot after each
// one so callers see growing real results instead of nothing until the
// (possibly huge) last remote is done
fn load_flatpak_progressive() -> AppStreamDb {
    let mut catalogs = Vec::new();
    collect_catalog_paths("/var/lib/flatpak/appstream", &mut catalogs);
    if let Some(home) = std::env::var_os("HOME") {
        let path = PathBuf::from(home).join(".local/share/flatpak/appstream");
        collect_catalog_paths(&path, &mut catalogs);
    }
    catalogs.sort_by_key(|(_, _, size)| *size);

    let locales = detect_locales();
    let mut components = Vec::new();
    let mut descriptions: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut verifications: HashMap<String, bool> = HashMap::new();

    for (remote_name, path, _) in catalogs {
        load_one_catalog(&path, &remote_name, &mut components, &mut descriptions, &mut verifications);
        *slot().write().unwrap() = Arc::new(AppStreamDb {
            components: components.clone(),
            locales: locales.clone(),
            descriptions: descriptions.clone(),
            verifications: verifications.clone(),
        });
    }

    AppStreamDb { components, locales, descriptions, verifications }
}

fn snapshot_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".cache/arc-daemon/appstream_db.json"))
}

// walks the same active/appstream.xml[.gz] files collect_catalog_paths does
// a stamp changing means that remote redeployed a new catalog
fn collect_source_stamps(root: impl AsRef<Path>, out: &mut Vec<SourceStamp>) {
    let Ok(remotes) = std::fs::read_dir(root.as_ref()) else {
        return;
    };
    for remote_dir in remotes.flatten() {
        let Ok(arches) = std::fs::read_dir(remote_dir.path()) else {
            continue;
        };
        for arch in arches.flatten() {
            let base = arch.path().join("active");
            let gz = base.join("appstream.xml.gz");
            let xml = base.join("appstream.xml");
            let path = if gz.exists() { gz } else { xml };
            let Ok(canonical) = path.canonicalize() else {
                continue;
            };
            let Ok(meta) = std::fs::metadata(&canonical) else {
                continue;
            };
            let mtime_secs = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            out.push(SourceStamp { path: canonical, mtime_secs, size: meta.len() });
        }
    }
}

fn compute_snapshot_key() -> SnapshotKey {
    let mut sources = Vec::new();
    collect_source_stamps("/var/lib/flatpak/appstream", &mut sources);
    if let Some(home) = std::env::var_os("HOME") {
        let path = PathBuf::from(home).join(".local/share/flatpak/appstream");
        collect_source_stamps(&path, &mut sources);
    }
    sources.sort_by(|a, b| a.path.cmp(&b.path));
    SnapshotKey { schema: SNAPSHOT_SCHEMA, sources }
}

fn load_snapshot_from_disk() -> Option<Snapshot> {
    let path = snapshot_path()?;
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn persist_snapshot(db: &AppStreamDb, key: &SnapshotKey) {
    let Some(path) = snapshot_path() else { return };
    let Some(parent) = path.parent() else { return };
    if std::fs::create_dir_all(parent).is_err() {
        return;
    }
    let snapshot = Snapshot {
        key: key.clone(),
        components: db.components.clone(),
        descriptions: db.descriptions.clone(),
        verifications: db.verifications.clone(),
    };
    let Ok(bytes) = serde_json::to_vec(&snapshot) else { return };
    // write to a tmp file then rename since the rename is atomic
    // a crash mid-write never leaves a torn snapshot behind
    let tmp_path = path.with_extension("json.tmp");
    if std::fs::write(&tmp_path, bytes).is_err() {
        return;
    }
    let _ = std::fs::rename(&tmp_path, &path);
}

// bypass entry for GetAppMetadata while the real appstream data for this id
// hasn't been resolved yet
// built from whatever the disk-seeded package cache already has so the
// detail page shows something instead of nothing
pub fn partial_entry_from_package(pkg: &Package) -> AppStreamEntry {
    AppStreamEntry {
        id: pkg.id.clone(),
        name: pkg.name.clone(),
        summary: pkg.description.clone(),
        description: String::new(),
        icon_url: pkg.icon_url.clone(),
        remote: pkg.remote.clone(),
        screenshots: pkg.screenshots.clone(),
        license: None,
        eula_url: None,
        homepage_url: pkg.homepage_url.clone(),
        content_rating: pkg.content_rating.clone().unwrap_or_default(),
        developer_name: pkg.developer_name.clone(),
        verified: false,
        categories: pkg.categories.clone(),
    }
}

const ICON_SIZES: [&str; 7] = ["128x128", "256x256", "96x96", "64x64", "48x48", "32x32", "scalable"];
const ICON_EXTS: [&str; 3] = ["png", "svg", "svgz"];

fn icon_content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "svg" | "svgz" => "image/svg+xml",
        _ => "image/png",
    }
}

fn find_appstream_icon(id: &str) -> Option<PathBuf> {
    let mut roots = vec![PathBuf::from("/var/lib/flatpak/appstream")];
    if let Some(home) = std::env::var_os("HOME") {
        roots.push(PathBuf::from(home).join(".local/share/flatpak/appstream"));
    }
    for root in &roots {
        let Ok(remotes) = std::fs::read_dir(root) else { continue };
        for remote_dir in remotes.flatten() {
            let Ok(arches) = std::fs::read_dir(remote_dir.path()) else { continue };
            for arch in arches.flatten() {
                let icons_base = arch.path().join("active").join("icons");
                if !icons_base.exists() {
                    continue;
                }
                // Flatpak stores icons in both <icons>/<size>/ and <icons>/flatpak/<size>/.
                for search_root in [icons_base.clone(), icons_base.join("flatpak")] {
                    for size in ICON_SIZES {
                        for ext in ICON_EXTS {
                            let p = search_root.join(size).join(format!("{}.{}", id, ext));
                            if p.exists() {
                                return Some(p);
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

// The exported hicolor theme only has icons for apps that are actually installed.
fn find_exported_icon(id: &str) -> Option<PathBuf> {
    let mut bases = vec![PathBuf::from("/var/lib/flatpak/exports/share/icons/hicolor")];
    if let Some(home) = std::env::var_os("HOME") {
        bases.push(PathBuf::from(home).join(".local/share/flatpak/exports/share/icons/hicolor"));
    }
    for base in &bases {
        for size in ICON_SIZES {
            for ext in ICON_EXTS {
                let p = base.join(size).join("apps").join(format!("{}.{}", id, ext));
                if p.exists() {
                    return Some(p);
                }
            }
        }
    }
    None
}

pub fn find_local_flatpak_icon_bytes(id: &str) -> Option<(Vec<u8>, String)> {
    let path = find_appstream_icon(id).or_else(|| find_exported_icon(id))?;
    let bytes = std::fs::read(&path).ok()?;
    Some((bytes, icon_content_type(&path).to_string()))
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
        developer_name: entry.developer_name,
        homepage_url: entry.homepage_url,
        content_rating: Some(entry.content_rating).filter(|s| !s.is_empty()),
        is_runtime: false,
        categories: entry.categories,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CATALOG_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<components version="0.8">
  <component type="desktop-application">
    <id>org.example.Foo</id>
    <name>Foo</name>
    <summary>A test app</summary>
    <categories>
      <category>Utility</category>
    </categories>
  </component>
  <component type="desktop-application">
    <id>org.example.Bar</id>
    <name>Bar</name>
    <summary>Another test app</summary>
  </component>
</components>"#;

    fn parse_catalog() -> Vec<(Component, Option<String>)> {
        let element = xmltree::Element::parse(CATALOG_XML.as_bytes()).unwrap();
        let collection = Collection::try_from(&element).unwrap();
        collection.components.into_iter().map(|c| (c, Some("testremote".to_string()))).collect()
    }

    #[test]
    fn snapshot_roundtrips_through_json() {
        let components = parse_catalog();
        let key = SnapshotKey {
            schema: SNAPSHOT_SCHEMA,
            sources: vec![SourceStamp { path: PathBuf::from("/tmp/a.xml"), mtime_secs: 42, size: 7 }],
        };
        let snapshot = Snapshot {
            key: key.clone(),
            components: components.clone(),
            descriptions: HashMap::new(),
            verifications: HashMap::new(),
        };
        let bytes = serde_json::to_vec(&snapshot).unwrap();
        let restored: Snapshot = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(restored.key, key);
        assert_eq!(restored.components.len(), components.len());
        assert_eq!(restored.components[0].0.id, components[0].0.id);
    }

    #[test]
    fn snapshot_key_detects_changed_stamp() {
        let a = SnapshotKey {
            schema: SNAPSHOT_SCHEMA,
            sources: vec![SourceStamp { path: PathBuf::from("/tmp/a.xml"), mtime_secs: 1, size: 1 }],
        };
        let mut b = a.clone();
        b.sources[0].mtime_secs = 2;
        assert_ne!(a, b);
    }

    #[test]
    fn extract_component_block_finds_matching_component_only() {
        let block = extract_component_block(CATALOG_XML.as_bytes(), "org.example.Bar").unwrap();
        let text = std::str::from_utf8(&block).unwrap();
        assert!(text.contains("org.example.Bar"));
        assert!(!text.contains("org.example.Foo"));
    }

    #[test]
    fn resolve_from_catalog_bytes_returns_full_entry() {
        let entry = resolve_from_catalog_bytes(CATALOG_XML.as_bytes(), "org.example.Foo", Some("testremote".to_string()), &[])
            .unwrap();
        assert_eq!(entry.name, "Foo");
        assert_eq!(entry.summary, "A test app");
        assert_eq!(entry.categories, vec!["utility".to_string()]);
    }
}
