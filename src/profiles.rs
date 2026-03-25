use std::fmt;
use std::str::FromStr;

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ProfileName {
    Direct,
    Reasoning,
    Tool,
    Agent,
}

impl ProfileName {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Reasoning => "reasoning",
            Self::Tool => "tool",
            Self::Agent => "agent",
        }
    }
}

impl fmt::Display for ProfileName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ProfileName {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "direct" => Ok(Self::Direct),
            "reasoning" => Ok(Self::Reasoning),
            "tool" => Ok(Self::Tool),
            "agent" => Ok(Self::Agent),
            _ => bail!("unknown profile '{value}'"),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct InferenceProfile {
    pub system_prompt: String,
    pub temperature: f32,
    pub top_p: f32,
    pub max_tokens: u32,
    pub stream: bool,
    #[serde(default)]
    pub reasoning: bool,
    #[serde(default)]
    pub prefer_thinking: Option<bool>,
    #[serde(default)]
    pub model: Option<String>,
}
