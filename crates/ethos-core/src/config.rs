use figment::{Figment, providers::{Format, Toml, Env, Serialized}};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EthosConfig {
    pub rpc_url: String,
    pub etherscan_key: Option<String>,
    pub output_dir: String,
}

impl Default for EthosConfig {
    fn default() -> Self {
        Self {
            rpc_url: "http://localhost:8545".to_string(),
            etherscan_key: None,
            output_dir: ".".to_string(),
        }
    }
}

impl EthosConfig {
    /// Load configuration by merging multiple sources.
    /// Priority: CLI Flags > Env Vars > ethos.toml > Defaults
    pub fn load() -> Self {
        Figment::from(Serialized::defaults(Self::default()))
            .merge(Toml::file("ethos.toml"))
            .merge(Env::prefixed("ETHOS_"))
            .extract()
            .unwrap_or_else(|_| Self::default())
    }
}
