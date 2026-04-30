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
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            preferred_provider: Provider::Flatpak,
            ignore_native_preference: false,
        }
    }
}

impl Settings {
    fn path() -> PathBuf {
        // xdg_config_home is the standard linux equivalent of appdata on windows,
        // falls back to ~/.config if the env var is not set
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_default()
                    .join(".config")
            });
        base.join("arc").join("settings.toml")
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
