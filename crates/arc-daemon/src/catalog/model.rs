use libarc::{Package, Provider};
use rusqlite::Row;
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, Default)]
pub struct CatalogApp {
    pub id: String,
    pub provider: Option<Provider>,
    pub remote: Option<String>,
    pub icon_url: Option<String>,
    pub version: String,
    pub is_runtime: bool,
    pub license: Option<String>,
    pub eula_url: Option<String>,
    pub homepage_url: Option<String>,
    pub content_rating: String,
    pub verified: bool,
    pub categories: Vec<String>,
    pub screenshots: Vec<String>,
    pub project_urls: HashMap<String, String>,
    pub name: String,
    pub summary: String,
    pub description: String,
    pub developer_name: Option<String>,
    pub installed: bool,
}

impl CatalogApp {
    pub fn into_package(self) -> Package {
        let name = if self.name.is_empty() {
            self.id.clone()
        } else {
            self.name
        };
        Package {
            id: self.id,
            name,
            version: self.version,
            description: self.summary,
            provider: self.provider.unwrap_or(Provider::Flatpak),
            installed: self.installed,
            icon_url: self.icon_url,
            remote: self.remote,
            screenshots: self.screenshots,
            developer_name: self.developer_name,
            homepage_url: self.homepage_url,
            content_rating: Some(self.content_rating).filter(|s| !s.is_empty()),
            is_runtime: self.is_runtime,
            categories: self.categories,
        }
    }

    pub fn into_list_package(self) -> Package {
        let mut pkg = self.into_package();
        pkg.screenshots = Vec::new();
        pkg
    }
}

#[derive(Debug, Clone, Default)]
pub struct NewApp {
    pub id: String,
    pub cid: String,
    pub provider: Option<Provider>,
    pub prio: i64,
    pub listable: bool,
    pub remote: Option<String>,
    pub icon_url: Option<String>,
    pub version: String,
    pub is_runtime: bool,
    pub license: Option<String>,
    pub eula_url: Option<String>,
    pub homepage_url: Option<String>,
    pub content_rating: String,
    pub verified: bool,
    pub categories: Vec<String>,
    pub screenshots: Vec<String>,
    pub project_urls: BTreeMap<String, String>,
    pub texts: BTreeMap<String, NewText>,
}

#[derive(Debug, Clone, Default)]
pub struct NewText {
    pub name: Option<String>,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub developer: Option<String>,
}

impl NewApp {
    pub fn from_package(pkg: &Package) -> Self {
        let mut texts = BTreeMap::new();
        texts.insert(
            "C".to_string(),
            NewText {
                name: Some(pkg.name.clone()),
                summary: Some(pkg.description.clone()),
                description: None,
                developer: pkg.developer_name.clone(),
            },
        );
        Self {
            id: pkg.id.clone(),
            cid: pkg.id.clone(),
            provider: Some(pkg.provider.clone()),
            prio: super::PRIO_PROVIDER,
            listable: !pkg.is_runtime,
            remote: pkg.remote.clone(),
            icon_url: pkg.icon_url.clone(),
            version: pkg.version.clone(),
            is_runtime: pkg.is_runtime,
            license: None,
            eula_url: None,
            homepage_url: pkg.homepage_url.clone(),
            content_rating: pkg.content_rating.clone().unwrap_or_default(),
            verified: false,
            categories: pkg.categories.iter().map(|c| c.to_lowercase()).collect(),
            screenshots: pkg.screenshots.clone(),
            project_urls: BTreeMap::new(),
            texts,
        }
    }
}

pub fn provider_str(p: &Provider) -> &'static str {
    match p {
        Provider::Flatpak => "Flatpak",
        Provider::Distrobox => "Distrobox",
        Provider::Lutris => "Lutris",
        Provider::AppImage => "AppImage",
        Provider::Pwa => "Pwa",
    }
}

pub fn parse_provider(s: &str) -> Option<Provider> {
    Some(match s {
        "Flatpak" => Provider::Flatpak,
        "Distrobox" => Provider::Distrobox,
        "Lutris" => Provider::Lutris,
        "AppImage" => Provider::AppImage,
        "Pwa" => Provider::Pwa,
        _ => return None,
    })
}

pub fn provider_source(p: &Provider) -> String {
    format!("provider:{}", provider_str(p))
}

pub fn locale_params(locales: &[String]) -> (String, String) {
    (
        locales.first().cloned().unwrap_or_default(),
        locales.get(1).cloned().unwrap_or_default(),
    )
}

// reads one row of app_rows in its column order
pub fn map_app(r: &Row) -> rusqlite::Result<CatalogApp> {
    let json_vec = |s: String| serde_json::from_str::<Vec<String>>(&s).unwrap_or_default();
    Ok(CatalogApp {
        id: r.get(0)?,
        provider: parse_provider(&r.get::<_, String>(1)?),
        remote: r.get(2)?,
        icon_url: r.get(3)?,
        version: r.get(4)?,
        is_runtime: r.get::<_, i64>(5)? != 0,
        license: r.get(6)?,
        eula_url: r.get(7)?,
        homepage_url: r.get(8)?,
        content_rating: r.get(9)?,
        verified: r.get::<_, i64>(10)? != 0,
        categories: json_vec(r.get(11)?),
        screenshots: json_vec(r.get(12)?),
        project_urls: serde_json::from_str(&r.get::<_, String>(13)?).unwrap_or_default(),
        name: r.get(14)?,
        summary: r.get(15)?,
        description: r.get(16)?,
        developer_name: r.get(17)?,
        installed: r.get::<_, i64>(18)? != 0,
    })
}
