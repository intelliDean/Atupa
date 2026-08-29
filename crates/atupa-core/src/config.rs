//! [`AtupaConfig`] — runtime configuration with multi-source merging and validation.

use figment::{
    Figment,
    providers::{Env, Format, Serialized, Toml},
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Runtime configuration for the Atupa profiling engine.
///
/// Configuration is loaded by merging multiple sources in the following priority
/// order (highest to lowest):
///
/// 1. **CLI flags** — applied by the caller *after* [`AtupaConfig::load`] returns.
/// 2. **`ATUPA_*` environment variables** — e.g. `ATUPA_RPC_URL`, `ATUPA_ETHERSCAN_KEY`.
/// 3. **`atupa.toml`** — local project config in the current working directory.
/// 4. **`~/.atupa/config.toml`** — global user config.
/// 5. **Built-in defaults** — see [`AtupaConfig::default`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtupaConfig {
    /// JSON-RPC endpoint URL for the target chain.
    pub rpc_url: String,
    /// Optional Etherscan API key for contract name resolution.
    pub etherscan_key: Option<String>,
    /// Directory where profiling artifacts (SVGs, JSON reports) are written.
    pub output_dir: String,
    /// Path to the Atupa Studio directory (overrides auto-detection when set).
    pub studio_dir: Option<PathBuf>,
    /// TCP port Atupa Studio's embedded server will bind to.
    pub studio_port: u16,
}

impl Default for AtupaConfig {
    fn default() -> Self {
        Self {
            rpc_url: "http://localhost:8547".to_string(),
            etherscan_key: None,
            output_dir: ".".to_string(),
            studio_dir: None,
            studio_port: 5173,
        }
    }
}

impl AtupaConfig {
    /// Load configuration by merging all available sources.
    ///
    /// Config parse errors are logged as warnings and fall back to defaults
    /// rather than panicking, ensuring the CLI remains usable even with a
    /// malformed config file.
    pub fn load() -> Self {
        match Self::build_figment().extract::<Self>() {
            Ok(config) => config,
            Err(e) => {
                log::warn!(
                    "Failed to parse Atupa configuration — falling back to defaults. \
                     Check your atupa.toml or ~/.atupa/config.toml. Error: {e}"
                );
                Self::default()
            }
        }
    }

    /// Validate that this configuration is internally coherent.
    ///
    /// Returns an error describing the problem if any required field is invalid.
    ///
    /// # Errors
    ///
    /// - [`rpc_url`](AtupaConfig::rpc_url) is empty or whitespace-only.
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.rpc_url.trim().is_empty() {
            anyhow::bail!(
                "rpc_url must not be empty. \
                 Set it via the ATUPA_RPC_URL environment variable, atupa.toml, or the --rpc flag."
            );
        }
        Ok(())
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    fn build_figment() -> Figment {
        let mut figment = Figment::from(Serialized::defaults(Self::default()));

        // 1. Global user config: ~/.atupa/config.toml
        if let Some(home) = dirs::home_dir() {
            figment = figment.merge(Toml::file(home.join(".atupa").join("config.toml")));
        }

        // 2. Local project config: ./atupa.toml
        figment = figment.merge(Toml::file("atupa.toml"));

        // 3. Environment variable overrides
        figment = figment.merge(Env::prefixed("ATUPA_"));

        figment
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Global mutex to serialise tests that mutate process environment variables.
    ///
    /// `std::env::set_var` / `remove_var` are inherently unsound in a
    /// multi-threaded process (they race with reads from other threads).
    /// Holding this lock ensures our env-mutating tests never overlap.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn default_values_are_sane() {
        let cfg = AtupaConfig::default();
        assert_eq!(cfg.rpc_url, "http://localhost:8547");
        assert_eq!(cfg.studio_port, 5173);
        assert_eq!(cfg.output_dir, ".");
        assert!(cfg.etherscan_key.is_none());
        assert!(cfg.studio_dir.is_none());
    }

    #[test]
    fn validate_rejects_empty_rpc_url() {
        let cfg = AtupaConfig {
            rpc_url: String::new(),
            ..Default::default()
        };
        assert!(
            cfg.validate().is_err(),
            "empty rpc_url should fail validation"
        );
    }

    #[test]
    fn validate_rejects_whitespace_only_rpc_url() {
        let cfg = AtupaConfig {
            rpc_url: "   ".to_string(),
            ..Default::default()
        };
        assert!(
            cfg.validate().is_err(),
            "whitespace-only rpc_url should fail validation"
        );
    }

    #[test]
    fn validate_accepts_default_config() {
        assert!(
            AtupaConfig::default().validate().is_ok(),
            "default config should pass validation"
        );
    }

    #[test]
    fn env_vars_override_rpc_url_and_key() {
        // Safety: ENV_LOCK ensures no other test mutates the environment concurrently.
        let _guard = ENV_LOCK.lock().unwrap();

        unsafe {
            std::env::set_var("ATUPA_RPC_URL", "http://test-rpc.local");
            std::env::set_var("ATUPA_ETHERSCAN_KEY", "test-key-123");
        }

        let cfg = AtupaConfig::load();

        // Restore env state before any assertion can panic.
        unsafe {
            std::env::remove_var("ATUPA_RPC_URL");
            std::env::remove_var("ATUPA_ETHERSCAN_KEY");
        }

        assert_eq!(cfg.rpc_url, "http://test-rpc.local");
        assert_eq!(cfg.etherscan_key, Some("test-key-123".to_string()));
    }
}
