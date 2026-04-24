use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Default, Deserialize)]
pub struct AtupaConfigToml {
    pub diff: Option<DiffConfig>,
}

#[derive(Debug, Default, Deserialize)]
pub struct DiffConfig {
    pub max_total_gas_increase_percent: Option<f64>,
    pub max_execution_gas_increase_percent: Option<f64>,
    pub max_evm_steps_increase: Option<i64>,
    pub max_stylus_calls_increase: Option<i64>,
}

impl AtupaConfigToml {
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file at {:?}", path))?;
        let config: Self = toml::from_str(&content)
            .with_context(|| format!("Failed to parse TOML config from {:?}", path))?;
        Ok(config)
    }

    pub fn auto_load() -> Option<Self> {
        let path = Path::new("atupa.toml");
        if path.exists() {
            Self::load(path).ok()
        } else {
            None
        }
    }
}
