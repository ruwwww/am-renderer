use anyhow::Result;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub disabled_effects: Vec<String>,
}

impl Config {
    pub fn from_file(path: &Path) -> Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&contents)?;
        Ok(config)
    }

    pub fn effect_is_disabled(&self, type_name: &str) -> bool {
        self.disabled_effects.iter().any(|d| d == type_name)
    }

    pub fn disabled_effects_slice(&self) -> &[String] {
        &self.disabled_effects
    }
}
