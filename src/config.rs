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
pub enum BackendType {
    #[serde(rename = "openai_compatible")]
    OpenAiCompatible,
}

#[derive(Debug, Clone, Deserialize)]
pub enum BackendFlavor {
    #[serde(rename = "generic")]
    Generic,
    #[serde(rename = "llamacpp")]
    Llamacpp,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BackendConfig {
    #[serde(rename = "type")]
    pub backend_type: BackendType,
    #[serde(default = "default_backend_flavor")]
    pub flavor: BackendFlavor,
}

fn default_backend_flavor() -> BackendFlavor {
    BackendFlavor::Generic
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModelConfig {
    #[serde(default = "default_supports_reasoning")]
    pub supports_reasoning: bool,
    #[serde(default)]
    pub reasoning_field: Option<String>,
    #[serde(default)]
    pub supports_thinking_toggle: bool,
    #[serde(default)]
    pub thinking_toggle_path: Option<String>,
    #[serde(default)]
    pub chat_template_path: Option<String>,
}

fn default_supports_reasoning() -> bool {
    true
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
    pub backend: BackendConfig,
    pub model: ModelConfig,
    pub app: AppSettings,
    pub profiles: BTreeMap<ProfileName, InferenceProfile>,
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub backend: BackendConfig,
    pub model: ModelConfig,
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

        validate_model_config(&parsed.model)?;

        Ok(Self {
            server: parsed.server,
            backend: parsed.backend,
            model: parsed.model,
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

fn validate_model_config(model: &ModelConfig) -> Result<()> {
    if model.supports_reasoning && model.reasoning_field.is_none() {
        bail!("model.supports_reasoning=true requires model.reasoning_field")
    }

    if model.supports_thinking_toggle && model.thinking_toggle_path.is_none() {
        bail!("model.supports_thinking_toggle=true requires model.thinking_toggle_path")
    }

    if let Some(path) = &model.thinking_toggle_path {
        match path.as_str() {
            "chat_template_kwargs.enable_thinking" => {}
            _ => bail!(
                "unsupported model.thinking_toggle_path '{}' ; currently only 'chat_template_kwargs.enable_thinking' is supported",
                path
            ),
        }
    }

    Ok(())
}
