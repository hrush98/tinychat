use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::profiles::{InferenceProfile, ProfileName};

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub base_url: String,
    pub default_model: String,
    pub timeout_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AppSettings {
    pub default_profile: ProfileName,
    #[serde(default)]
    pub debug: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct RawConfig {
    pub server: ServerConfig,
    pub app: AppSettings,
    pub profiles: BTreeMap<ProfileName, InferenceProfile>,
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub app: AppSettings,
    pub profiles: BTreeMap<ProfileName, InferenceProfile>,
}

impl AppConfig {
    pub fn load(path: &Path) -> Result<Self> {
        let raw = fs::read_to_string(path)
            .with_context(|| format!("unable to read config file {}", path.display()))?;
        let parsed: RawConfig = toml::from_str(&raw)
            .with_context(|| format!("unable to parse config file {}", path.display()))?;
        if !parsed.profiles.contains_key(&parsed.app.default_profile) {
            bail!(
                "default_profile '{}' is missing from [profiles]",
                parsed.app.default_profile.as_str()
            );
        }

        Ok(Self {
            server: parsed.server,
            app: parsed.app,
            profiles: parsed.profiles,
        })
    }

    pub fn profile(&self, name: &ProfileName) -> Result<&InferenceProfile> {
        self.profiles
            .get(name)
            .with_context(|| format!("profile '{}' is not configured", name.as_str()))
    }
}
