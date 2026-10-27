use crate::catalog::{
    self, CatalogApp, NewApp, NewText, SourceStamp, PRIO_APPSTREAM, PRIO_BLOSSOMOS,
};
use appstream::enums::{
    Bundle, ComponentKind, ContentAttribute, ContentState, ImageKind, ProjectUrl,
};
use appstream::Component;
use memchr::memmem;
use std::collections::{BTreeMap, HashMap};
use std::env;
use std::fs;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use tracing::{info, warn};

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
    pub project_urls: HashMap<String, String>,
}

impl From<CatalogApp> for AppStreamEntry {
    fn from(a: CatalogApp) -> Self {
        Self {
            id: a.id,
            name: a.name,
            summary: a.summary,
            description: a.description,
            icon_url: a.icon_url,
            remote: a.remote,
            screenshots: a.screenshots,
            license: a.license,
            eula_url: a.eula_url,
            homepage_url: a.homepage_url,
            content_rating: a.content_rating,
            developer_name: a.developer_name,
            verified: a.verified,
            categories: a.categories,
            project_urls: a.project_urls,
        }
    }
}

// Build a priority list of locale codes from the process environment.
// For "de_DE.UTF-8" we return ["de_DE", "de"]; for "C" or unset we return [].
fn detect_locales() -> Vec<String> {
    let raw = env::var("LANGUAGE")
        .ok()
        .and_then(|l| l.split(':').next().map(|s| s.to_string()))
        .filter(|s| !s.is_empty())
        .or_else(|| env::var("LANG").ok())
        .or_else(|| env::var("LC_ALL").ok())
        .or_else(|| env::var("LC_MESSAGES").ok())
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

pub fn system_locales() -> Vec<String> {
    static LOCALES: OnceLock<Vec<String>> = OnceLock::new();
    LOCALES.get_or_init(detect_locales).clone()
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

// standalone lookup for a single installed app's own exported metainfo file
// works even before the catalogue has ever been built
pub fn load_local_metainfo(id: &str) -> Option<AppStreamEntry> {
    scan_exported_metainfo(id, &system_locales())
}

fn scan_exported_metainfo(id: &str, locales: &[String]) -> Option<AppStreamEntry> {
    let mut dirs = vec![
        PathBuf::from("/var/lib/flatpak/exports/share/metainfo"),
        PathBuf::from("/var/lib/flatpak/exports/share/appdata"),
    ];
    if let Some(home) = env::var_os("HOME") {
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
            let Ok(bytes) = fs::read(&path) else {
                continue;
            };
            if let Some(entry) = parse_metainfo_bytes(id, &bytes, locales) {
                return Some(entry);
            }
        }
    }
    None
}

fn component_developers(element: &xmltree::Element) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut add = |e: &xmltree::Element| {
        let Some(text) = e.get_text().map(|t| t.trim().to_string()).filter(|t| !t.is_empty()) else {
            return;
        };
        let locale = e.attributes.get("lang").cloned().unwrap_or_else(|| "C".to_string());
        out.entry(locale).or_insert(text);
    };
    let children = || element.children.iter().filter_map(|n| n.as_element());
    for developer in children().filter(|e| e.name == "developer") {
        for name in developer.children.iter().filter_map(|n| n.as_element()).filter(|e| e.name == "name") {
            add(name);
        }
    }
    for legacy in children().filter(|e| e.name == "developer_name") {
        add(legacy);
    }
    out
}

// Parse a single metainfo/appdata XML file and build an AppStreamEntry.
// The `id` argument is the canonical app ID (i.e. what we searched the file by)
// and is used directly rather than reading the potentially-.desktop-suffixed <id> element.
fn parse_metainfo_bytes(id: &str, bytes: &[u8], locales: &[String]) -> Option<AppStreamEntry> {
    let Ok(root) = xmltree::Element::parse(bytes) else {
        return None;
    };

    // Pick the best-matching localized value for a repeating tag (e.g. <name xml:lang="de">).
    let pick = |tag: &str| -> String {
        let mut default_val = String::new();
        let mut by_lang: HashMap<String, String> = HashMap::new();
        for child in root.children.iter().filter_map(|n| n.as_element()) {
            if child.name != tag {
                continue;
            }
            let text = child
                .get_text()
                .map(|t| t.trim().to_string())
                .unwrap_or_default();
            if let Some(lang) = child.attributes.get("lang") {
                by_lang.insert(lang.clone(), text);
            } else {
                default_val = text;
            }
        }
        locales
            .iter()
            .find_map(|l| by_lang.get(l))
            .cloned()
            .unwrap_or(default_val)
    };

    let name = pick("name");
    let summary = pick("summary");
    let developers = component_developers(&root);
    let developer_name = locales
        .iter()
        .find_map(|l| developers.get(l))
        .or_else(|| developers.get("C"))
        .cloned();

    // Description via the same locale-aware xmltree extractor used for catalog files.
    let desc_map = component_descriptions(&root);
    let description = locales
        .iter()
        .find_map(|l| desc_map.get(l))
        .or_else(|| desc_map.get("C"))
        .cloned()
        .unwrap_or_default();

    let license_raw = root
        .children
        .iter()
        .filter_map(|n| n.as_element())
        .find(|e| e.name == "project_license")
        .and_then(|e| e.get_text())
        .map(|t| t.trim().to_string());
    let (license, eula_url) = match license_raw {
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

    let homepage_url = root
        .children
        .iter()
        .filter_map(|n| n.as_element())
        .find(|e| {
            e.name == "url"
                && e.attributes
                    .get("type")
                    .map(|t| t == "homepage")
                    .unwrap_or(false)
        })
        .and_then(|e| e.get_text())
        .map(|t| t.trim().to_string());

    let content_rating = root
        .children
        .iter()
        .filter_map(|n| n.as_element())
        .find(|e| e.name == "content_rating")
        .map(|rating| {
            let max = rating
                .children
                .iter()
                .filter_map(|n| n.as_element())
                .filter_map(|e| e.get_text().map(|t| t.trim().to_string()))
                .map(|v| match v.as_str() {
                    "intense" => 3u8,
                    "moderate" => 2,
                    "mild" => 1,
                    _ => 0,
                })
                .max()
                .unwrap_or(0);
            match max {
                3 => "18+",
                2 => "12+",
                1 => "7+",
                _ => "All ages",
            }
            .to_string()
        })
        .unwrap_or_default();

    let screenshots: Vec<String> = root
        .children
        .iter()
        .filter_map(|n| n.as_element())
        .find(|e| e.name == "screenshots")
        .map(|ss| {
            ss.children
                .iter()
                .filter_map(|n| n.as_element())
                .filter(|e| e.name == "screenshot")
                .filter_map(|sc| {
                    sc.children
                        .iter()
                        .filter_map(|n| n.as_element())
                        .find(|e| e.name == "image")
                        .and_then(|img| img.get_text())
                        .map(|t| t.trim().to_string())
                })
                .take(5)
                .collect()
        })
        .unwrap_or_default();

    let project_urls = component_project_urls(&root);

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
        project_urls,
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

fn child_elements<'a>(e: &'a xmltree::Element) -> impl Iterator<Item = &'a xmltree::Element> {
    e.children.iter().filter_map(|n| n.as_element())
}

// descriptions come in two shapes in the wild and the appstream crate merges translated paragraphs into the default text
// format 1 has a lang on the description element and format 2 has a lang on each paragraph
fn component_descriptions(comp: &xmltree::Element) -> HashMap<String, String> {
    let mut out: HashMap<String, String> = HashMap::new();
    for desc in child_elements(comp).filter(|e| e.name == "description") {
        if let Some(lang) = desc.attributes.get("lang") {
            let html = children_to_html(desc);
            if !html.trim().is_empty() {
                out.insert(lang.clone(), html);
            }
        } else {
            let mut by_lang: HashMap<String, String> = HashMap::new();
            for para in child_elements(desc) {
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
                    out.entry(lang).or_insert(html);
                }
            }
        }
    }
    out
}

fn component_project_urls(comp: &xmltree::Element) -> HashMap<String, String> {
    child_elements(comp)
        .filter(|e| e.name == "url")
        .filter_map(|e| {
            let kind = e.attributes.get("type")?;
            let text = e.get_text()?.trim().to_string();
            (!text.is_empty()).then(|| (kind.clone(), text))
        })
        .collect()
}

// flathub marks verified apps in a custom block the appstream crate does not read
fn component_verified(comp: &xmltree::Element) -> bool {
    child_elements(comp)
        .filter(|e| e.name == "custom")
        .flat_map(child_elements)
        .find(|e| {
            e.name == "value"
                && e.attributes
                    .get("key")
                    .is_some_and(|k| k == "flathub::verification::verified")
        })
        .and_then(|e| e.get_text())
        .is_some_and(|t| t.trim() == "true")
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

fn canonical_component_id(c: &Component) -> String {
    c.bundles
        .iter()
        .find_map(|b| match b {
            Bundle::Flatpak { reference, .. } => reference.split('/').nth(1).map(|s| s.to_string()),
            _ => None,
        })
        .unwrap_or_else(|| c.id.to_string())
}

fn license_and_eula(raw: Option<String>) -> (Option<String>, Option<String>) {
    match raw {
        Some(l) if l.starts_with("LicenseRef-proprietary=http") => {
            let url = l
                .strip_prefix("LicenseRef-proprietary=")
                .unwrap_or("")
                .to_string();
            (Some("Proprietary".to_string()), Some(url))
        }
        Some(l) if l.contains("LicenseRef-proprietary") => (Some("Proprietary".to_string()), None),
        other => (other, None),
    }
}

// flatpak downloads a 128px icon for every app next to its catalogue so lists never wait on the network
fn pick_icon(c: &Component, icons_dir: &Path) -> Option<String> {
    let cached = c
        .icons
        .iter()
        .filter_map(|icon| match icon {
            appstream::enums::Icon::Cached {
                path,
                width,
                height,
            } => {
                let size = width.unwrap_or(64);
                let dir = format!("{}x{}", size, height.unwrap_or(size));
                Some((size, icons_dir.join(dir).join(path)))
            }
            _ => None,
        })
        .filter(|(_, p)| p.exists())
        .max_by_key(|(size, _)| *size)
        .map(|(_, p)| format!("{}{}", libarc::media::LOCAL_PREFIX, p.display()));
    cached
        .or_else(|| {
            c.icons.iter().find_map(|icon| match icon {
                appstream::enums::Icon::Remote { url, width, .. }
                    if width.map(|w| w >= 96).unwrap_or(true) =>
                {
                    Some(url.to_string())
                }
                _ => None,
            })
        })
        .or_else(|| {
            c.icons.first().map(|icon| match icon {
                appstream::enums::Icon::Remote { url, .. } => url.to_string(),
                appstream::enums::Icon::Local { path, .. }
                | appstream::enums::Icon::Cached { path, .. } => {
                    format!("{}{}", libarc::media::LOCAL_PREFIX, path.display())
                }
                appstream::enums::Icon::Stock(name) => {
                    format!("{}{}", libarc::media::LOCAL_PREFIX, name)
                }
            })
        })
}

fn content_rating_label(c: &Component) -> String {
    c.content_rating
        .as_ref()
        .map(|rating| {
            match rating
                .attributes
                .iter()
                .map(attr_severity)
                .max()
                .unwrap_or(0)
            {
                3 => "18+",
                2 => "12+",
                1 => "7+",
                _ => "All ages",
            }
            .to_string()
        })
        .unwrap_or_default()
}

// turns one parsed component element into a catalogue row with every translation it carries
fn component_row(element: &xmltree::Element, remote: &str, icons_dir: &Path) -> Option<NewApp> {
    let c = Component::try_from(element).ok()?;
    let raw_id = c.id.to_string();
    let cid = raw_id
        .strip_suffix(".desktop")
        .unwrap_or(&raw_id)
        .to_string();

    let mut texts: BTreeMap<String, NewText> = BTreeMap::new();
    for (locale, v) in &c.name.0 {
        texts.entry(locale.clone()).or_default().name = Some(v.clone());
    }
    if let Some(summary) = &c.summary {
        for (locale, v) in &summary.0 {
            texts.entry(locale.clone()).or_default().summary = Some(v.clone());
        }
    }
    for (locale, developer) in component_developers(element) {
        texts.entry(locale).or_default().developer = Some(developer);
    }
    let descriptions = component_descriptions(element);
    if !descriptions.is_empty() {
        for (locale, html) in descriptions {
            texts.entry(locale).or_default().description = Some(html);
        }
    } else if let Some(desc) = &c.description {
        for (locale, v) in &desc.0 {
            texts.entry(locale.clone()).or_default().description = Some(v.clone());
        }
    }

    let screenshots = c
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
    let (license, eula_url) = license_and_eula(c.project_license.as_ref().map(|l| l.to_string()));
    let homepage_url = c.urls.iter().find_map(|u| match u {
        ProjectUrl::Homepage(url) => Some(url.to_string()),
        _ => None,
    });

    Some(NewApp {
        id: canonical_component_id(&c),
        cid,
        provider: Some(libarc::Provider::Flatpak),
        prio: if remote == "blossomos" {
            PRIO_BLOSSOMOS
        } else {
            PRIO_APPSTREAM
        },
        listable: matches!(
            c.kind,
            ComponentKind::DesktopApplication | ComponentKind::ConsoleApplication
        ),
        remote: Some(remote.to_string()),
        icon_url: pick_icon(&c, icons_dir),
        version: String::new(),
        is_runtime: false,
        license,
        eula_url,
        homepage_url,
        content_rating: content_rating_label(&c),
        verified: component_verified(element),
        categories: c
            .categories
            .iter()
            .map(|cat| format!("{:?}", cat).to_lowercase())
            .collect(),
        screenshots,
        project_urls: component_project_urls(element).into_iter().collect(),
        texts,
    })
}

const CHUNK: usize = 64 * 1024;
const OPEN_TAG: &[u8] = b"<component";
const CLOSE_TAG: &[u8] = b"</component>";

// cuts a catalogue into its component blocks while reading so only one app is ever held at a time
struct ComponentBlocks<R> {
    reader: R,
    buf: Vec<u8>,
    eof: bool,
    error: Option<std::io::Error>,
}

impl<R: Read> ComponentBlocks<R> {
    fn new(reader: R) -> Self {
        Self {
            reader,
            buf: Vec::with_capacity(CHUNK * 2),
            eof: false,
            error: None,
        }
    }

    // a component tag is followed by a space or the closing bracket which rules out the components wrapper
    fn find_open(hay: &[u8]) -> Result<Option<usize>, ()> {
        let mut offset = 0;
        while let Some(i) = memmem::find(&hay[offset..], OPEN_TAG) {
            let at = offset + i;
            match hay.get(at + OPEN_TAG.len()) {
                Some(b' ' | b'>' | b'\t' | b'\n' | b'\r') => return Ok(Some(at)),
                Some(_) => offset = at + OPEN_TAG.len(),
                None => return Err(()),
            }
        }
        Ok(None)
    }
}

impl<R: Read> Iterator for ComponentBlocks<R> {
    type Item = Vec<u8>;

    fn next(&mut self) -> Option<Vec<u8>> {
        loop {
            match Self::find_open(&self.buf) {
                Ok(Some(start)) => {
                    if let Some(end) = memmem::find(&self.buf[start..], CLOSE_TAG) {
                        let end = start + end + CLOSE_TAG.len();
                        let block = self.buf[start..end].to_vec();
                        self.buf.drain(..end);
                        return Some(block);
                    }
                    self.buf.drain(..start);
                }
                Ok(None) => {
                    // the tail may hold the first bytes of the next tag
                    let keep_from = self.buf.len().saturating_sub(OPEN_TAG.len());
                    self.buf.drain(..keep_from);
                }
                Err(()) => {}
            }
            if self.eof {
                return None;
            }
            let len = self.buf.len();
            self.buf.resize(len + CHUNK, 0);
            match self.reader.read(&mut self.buf[len..]) {
                Ok(0) => {
                    self.buf.truncate(len);
                    self.eof = true;
                }
                Ok(n) => self.buf.truncate(len + n),
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => self.buf.truncate(len),
                Err(e) => {
                    self.buf.truncate(len);
                    self.error = Some(e);
                    self.eof = true;
                }
            }
        }
    }
}

struct CatalogFile {
    remote: String,
    path: PathBuf,
    mtime: i64,
    size: u64,
}

// flatpak lays out appstream data as <root>/<remote>/<arch>/active/appstream.xml[.gz]
// the path through the active link stays the same when a remote deploys a new catalogue
fn catalog_files() -> Vec<CatalogFile> {
    let mut roots = vec![PathBuf::from("/var/lib/flatpak/appstream")];
    if let Some(home) = env::var_os("HOME") {
        roots.push(PathBuf::from(home).join(".local/share/flatpak/appstream"));
    }
    let mut out = Vec::new();
    for root in roots {
        let Ok(remotes) = fs::read_dir(&root) else {
            continue;
        };
        for remote_dir in remotes.flatten() {
            let remote = remote_dir.file_name().to_string_lossy().to_string();
            let Ok(arches) = fs::read_dir(remote_dir.path()) else {
                continue;
            };
            for arch in arches.flatten() {
                let base = arch.path().join("active");
                let gz = base.join("appstream.xml.gz");
                let path = if gz.exists() {
                    gz
                } else {
                    base.join("appstream.xml")
                };
                let Ok(meta) = fs::metadata(&path) else {
                    continue;
                };
                let mtime = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);
                out.push(CatalogFile {
                    remote: remote.clone(),
                    path,
                    mtime,
                    size: meta.len(),
                });
            }
        }
    }
    // smallest first so curated remotes appear first and load first like the original order
    out.sort_by(|a, b| (a.size, &a.path).cmp(&(b.size, &b.path)));
    out
}

fn ingest(file: &CatalogFile, rank: i64) -> bool {
    let Ok(handle) = fs::File::open(&file.path) else {
        return false;
    };
    let reader: Box<dyn Read> = if file.path.extension().is_some_and(|e| e == "gz") {
        Box::new(flate2::read::GzDecoder::new(BufReader::new(handle)))
    } else {
        Box::new(BufReader::new(handle))
    };
    let icons_dir = file
        .path
        .parent()
        .map(|p| p.join("icons"))
        .unwrap_or_default();
    let path = file.path.to_string_lossy().to_string();
    let stamp = SourceStamp {
        path: &path,
        remote: Some(&file.remote),
        rank,
        mtime: file.mtime,
        size: file.size,
    };
    let mut added = 0usize;
    let mut skipped = 0usize;
    let ok = catalog::replace_source(&path, Some(stamp), |w| {
        let mut blocks = ComponentBlocks::new(reader);
        for block in blocks.by_ref() {
            let row = xmltree::Element::parse(block.as_slice())
                .ok()
                .and_then(|e| component_row(&e, &file.remote, &icons_dir));
            match row {
                Some(app) => {
                    w.add(&app)?;
                    added += 1;
                }
                None => skipped += 1,
            }
        }
        // a broken download keeps the previous catalogue instead of committing half of it
        match blocks.error {
            Some(e) => Err(rusqlite::Error::ToSqlConversionFailure(Box::new(e))),
            None => Ok(()),
        }
    });
    if ok {
        info!(
            "catalogue: {} apps from {} ({} skipped)",
            added,
            file.path.display(),
            skipped
        );
    } else {
        warn!("catalogue: failed to read {}", file.path.display());
    }
    ok
}

// rebuilds only the catalogue files whose stamp changed since the last run and returns whether anything changed
pub fn sync_if_stale() -> bool {
    static SYNC: Mutex<()> = Mutex::new(());
    let _guard = SYNC.lock().unwrap_or_else(|p| p.into_inner());

    let files = catalog_files();
    let stored = catalog::stored_sources();
    let mut changed = false;
    for (rank, file) in files.iter().enumerate() {
        let key = file.path.to_string_lossy().to_string();
        if stored.get(&key) == Some(&(file.mtime, file.size)) {
            continue;
        }
        changed |= ingest(file, rank as i64);
    }
    let live: std::collections::HashSet<String> = files
        .iter()
        .map(|f| f.path.to_string_lossy().to_string())
        .collect();
    for gone in stored.keys().filter(|k| !live.contains(*k)) {
        changed |= catalog::remove_source(gone);
    }
    if changed {
        let order: Vec<String> = files
            .iter()
            .map(|f| f.path.to_string_lossy().to_string())
            .collect();
        catalog::set_source_ranks(&order);
        // parsing leaves freed heap pages behind and this hands them back to the system
        unsafe { libc::malloc_trim(0) };
    }
    changed
}

// `flatpak update --appstream` is a real network fetch, not just reading a
// disk cache, and the daemon restarts on every login (xdg autostart) as well
// as whenever the frontend replaces a stale instance, so running it
// unconditionally on every startup means paying that cost far more often
// than remotes actually publish new data. throttle it with a stamp file
// instead, independent of any single daemon process's lifetime.
const REMOTE_REFRESH_INTERVAL: std::time::Duration = std::time::Duration::from_secs(3600);

fn remote_refresh_stamp_path() -> Option<PathBuf> {
    libarc::cache::namespace("daemon").map(|dir| dir.join("appstream-remote-refresh.stamp"))
}

pub async fn refresh_remotes_if_due() {
    if !should_refresh_remotes() {
        tracing::info!("AppStream remotes refreshed recently, skipping network update");
        return;
    }
    refresh_remotes_now().await;
}

pub async fn refresh_remotes_now() {
    tracing::info!("Refreshing AppStream data...");
    match tokio::process::Command::new("flatpak")
        .args(["update", "--appstream"])
        .status()
        .await
    {
        Ok(status) if status.success() => tracing::info!("AppStream data refreshed"),
        Ok(status) => tracing::warn!("flatpak update --appstream exited with {}", status),
        Err(e) => tracing::warn!("Failed to run flatpak update --appstream: {}", e),
    }
    mark_remotes_refreshed();
}

pub fn should_refresh_remotes() -> bool {
    let Some(path) = remote_refresh_stamp_path() else {
        return true;
    };
    match fs::metadata(&path).and_then(|m| m.modified()) {
        Ok(modified) => modified
            .elapsed()
            .map(|age| age >= REMOTE_REFRESH_INTERVAL)
            .unwrap_or(true),
        Err(_) => true,
    }
}

pub fn mark_remotes_refreshed() {
    if let Some(path) = remote_refresh_stamp_path() {
        let _ = fs::write(&path, []);
    }
}

pub fn find_local_flatpak_icon_bytes(id: &str) -> Option<(Vec<u8>, String)> {
    let path = libarc::icons::find_flatpak_appstream_icon(id)
        .or_else(|| libarc::icons::find_flatpak_export_icon(id))?;
    let bytes = fs::read(&path).ok()?;
    Some((bytes, libarc::icons::icon_content_type(&path).to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CATALOG_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<components version="0.8" origin="testremote">
  <component type="desktop-application">
    <id>org.example.Foo.desktop</id>
    <name>Foo</name>
    <name xml:lang="de">Fu</name>
    <summary>A test app</summary>
    <description><p>Hello</p><p xml:lang="de">Hallo</p></description>
    <categories>
      <category>Utility</category>
    </categories>
    <custom><value key="flathub::verification::verified">true</value></custom>
    <url type="homepage">https://example.org</url>
    <bundle type="flatpak">app/org.example.Foo/x86_64/stable</bundle>
  </component>
  <component type="desktop-application">
    <id>broken</id>
    <name>Broken
  </component>
  <component type="desktop-application">
    <id>org.example.Bar</id>
    <name>Bar</name>
    <summary>Another test app</summary>
  </component>
</components>"#;

    // reads one byte at a time so every tag boundary falls between two reads
    struct Trickle<'a>(&'a [u8]);

    impl Read for Trickle<'_> {
        fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
            if self.0.is_empty() || out.is_empty() {
                return Ok(0);
            }
            out[0] = self.0[0];
            self.0 = &self.0[1..];
            Ok(1)
        }
    }

    fn rows() -> Vec<Option<NewApp>> {
        ComponentBlocks::new(Trickle(CATALOG_XML.as_bytes()))
            .map(|b| {
                xmltree::Element::parse(b.as_slice())
                    .ok()
                    .and_then(|e| component_row(&e, "testremote", Path::new("/nonexistent")))
            })
            .collect()
    }

    #[test]
    fn blocks_split_across_reads() {
        let blocks: Vec<_> = ComponentBlocks::new(Trickle(CATALOG_XML.as_bytes())).collect();
        assert_eq!(blocks.len(), 3);
        assert!(std::str::from_utf8(&blocks[2])
            .unwrap()
            .contains("org.example.Bar"));
    }

    #[test]
    fn malformed_component_is_skipped_not_fatal() {
        let rows = rows();
        assert_eq!(rows.len(), 3);
        assert!(rows[1].is_none());
        assert!(rows[0].is_some() && rows[2].is_some());
    }

    #[test]
    fn component_row_keeps_every_translation() {
        let foo = rows().remove(0).unwrap();
        assert_eq!(foo.id, "org.example.Foo");
        assert_eq!(foo.cid, "org.example.Foo");
        assert_eq!(foo.texts["C"].name.as_deref(), Some("Foo"));
        assert_eq!(foo.texts["de"].name.as_deref(), Some("Fu"));
        assert_eq!(foo.texts["C"].description.as_deref(), Some("<p>Hello</p>"));
        assert_eq!(foo.texts["de"].description.as_deref(), Some("<p>Hallo</p>"));
        assert!(foo.verified);
        assert_eq!(foo.categories, vec!["utility".to_string()]);
        assert_eq!(
            foo.project_urls.get("homepage").map(String::as_str),
            Some("https://example.org")
        );
        assert_eq!(foo.homepage_url.as_deref(), Some("https://example.org/"));
    }

    #[test]
    fn locale_candidates_drop_english_and_split_region() {
        assert_eq!(
            parse_locale_candidates("de-DE"),
            vec!["de_DE".to_string(), "de".to_string()]
        );
        assert!(parse_locale_candidates("en_US").is_empty());
    }
}
