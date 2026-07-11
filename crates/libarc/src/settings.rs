use crate::Provider;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const NATIVE_PREFERENCE_JSON: &str = include_str!("native_preference.json");

fn native_preference_list() -> Vec<String> {
    serde_json::from_str(NATIVE_PREFERENCE_JSON).unwrap_or_default()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub preferred_provider: Provider,
    // when true the internal native_preference list is ignored and preferred_provider wins
    pub ignore_native_preference: bool,
    #[serde(default = "Settings::default_auto_updates")]
    pub auto_updates: bool,
    #[serde(default = "Settings::default_concurrent_downloads")]
    pub concurrent_downloads: u32,
    #[serde(default = "Settings::default_show_security_warnings")]
    pub show_security_warnings: bool,
}

impl Settings {
    fn default_auto_updates() -> bool { true }
    fn default_concurrent_downloads() -> u32 { 3 }
    fn default_show_security_warnings() -> bool { true }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            preferred_provider: Provider::Flatpak,
            ignore_native_preference: false,
            auto_updates: true,
            concurrent_downloads: 3,
            show_security_warnings: true,
        }
    }
}

impl Settings {
    fn path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_default()
            .join("arc")
            .join("settings.toml")
    }

    pub fn load() -> Self {
        let path = Self::path();
        // if the file does not exist yet just use defaults, no error
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, toml::to_string_pretty(self)?)?;
        Ok(())
    }

    // call this instead of reading preferred_provider directly, it handles
    // the native preference list and ignore flag so the caller does not have to think about it
    pub fn preferred_for(&self, app_id: &str) -> Provider {
        if !self.ignore_native_preference
            && native_preference_list().iter().any(|id| id == app_id)
        {
            return Provider::Distrobox;
        }
        self.preferred_provider.clone()
    }
}
