//! TOML configuration parsing and CI threshold evaluation for Atupa.
//!
//! Evaluates gas regressions, execution budget breaches, and cross-VM boundary
//! increases against limits configured in `atupa.toml` or passed via CLI flags.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Top-level configuration representation matching `atupa.toml`.
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
pub struct AtupaConfigToml {
    /// RPC URL endpoint override.
    pub rpc_url: Option<String>,
    /// Etherscan API key for contract name resolution.
    pub etherscan_key: Option<String>,
    /// Default output directory for reports and SVG artifacts.
    pub output_dir: Option<String>,
    /// Port for the embedded Studio visualizer dev server.
    pub studio_port: Option<u16>,
    /// CI gas regression and diff threshold configuration.
    pub diff: Option<DiffConfig>,
}

/// Differential budget limits for CI/CD checks.
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiffConfig {
    /// Fail CI if total on-chain gas increases by more than this percentage.
    pub max_total_gas_increase_percent: Option<f64>,
    /// Fail CI if execution gas (excluding intrinsic base/calldata cost) increases by > X%.
    pub max_execution_gas_increase_percent: Option<f64>,
    /// Maximum additional EVM opcode steps allowed across a change.
    pub max_evm_steps_increase: Option<i64>,
    /// Maximum additional Stylus cross-VM calls allowed (0 = disallow any new cross-VM calls).
    pub max_stylus_calls_increase: Option<i64>,
}

impl AtupaConfigToml {
    /// Loads and parses configuration from a given file path.
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file at {path:?}"))?;
        let config: Self = toml::from_str(&content)
            .with_context(|| format!("Failed to parse TOML config from {path:?}"))?;
        Ok(config)
    }

    /// Attempts to auto-load `atupa.toml` from the current working directory.
    pub fn auto_load() -> Option<Self> {
        let path = Path::new("atupa.toml");
        if path.exists() {
            Self::load(path).ok()
        } else {
            None
        }
    }

    /// Resolves configuration from an explicit path or auto-detects `atupa.toml`.
    pub fn resolve(custom_path: Option<&str>) -> Option<Self> {
        if let Some(p) = custom_path {
            Self::load(Path::new(p)).ok()
        } else {
            Self::auto_load()
        }
    }
}

impl DiffConfig {
    /// Evaluates EVM / Arbitrum Nitro diff metrics against configured thresholds.
    pub fn evaluate_nitro(
        &self,
        total_gas_pct: f64,
        unified_pct: f64,
        evm_delta: f64,
        stylus_delta: f64,
    ) -> Vec<String> {
        let mut failures = Vec::new();

        if let Some(max_total) = self.max_total_gas_increase_percent
            && total_gas_pct > max_total
        {
            failures.push(format!(
                "Total Gas increased by {total_gas_pct:.1}% (limit: {max_total:.1}%)"
            ));
        }

        if let Some(max_exec) = self.max_execution_gas_increase_percent
            && unified_pct > max_exec
        {
            failures.push(format!(
                "Execution Gas increased by {unified_pct:.1}% (limit: {max_exec:.1}%)"
            ));
        }

        if let Some(max_evm) = self.max_evm_steps_increase
            && evm_delta > max_evm as f64
        {
            failures.push(format!(
                "EVM Steps increased by {evm_delta:.0} (limit: {max_evm})"
            ));
        }

        if let Some(max_stylus) = self.max_stylus_calls_increase
            && stylus_delta > max_stylus as f64
        {
            failures.push(format!(
                "Stylus Calls increased by {stylus_delta:.0} (limit: {max_stylus})"
            ));
        }

        failures
    }

    /// Evaluates a simple single percentage threshold against metric percentage change.
    pub fn evaluate_simple_threshold(
        unit_name: &str,
        cost_pct: f64,
        threshold_limit: f64,
    ) -> Option<String> {
        if cost_pct > threshold_limit {
            Some(format!(
                "Total {unit_name} increased by {cost_pct:.1}% (limit: {threshold_limit:.1}%)"
            ))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_atupa_toml() {
        let toml_str = r#"
            rpc_url = "http://localhost:8545"
            etherscan_key = "secret_key"
            output_dir = "artifacts/custom"
            studio_port = 8080

            [diff]
            max_total_gas_increase_percent = 3.5
            max_execution_gas_increase_percent = 2.0
            max_evm_steps_increase = 75
            max_stylus_calls_increase = 1
        "#;

        let config: AtupaConfigToml = toml::from_str(toml_str).unwrap();
        assert_eq!(config.rpc_url.as_deref(), Some("http://localhost:8545"));
        assert_eq!(config.etherscan_key.as_deref(), Some("secret_key"));
        assert_eq!(config.output_dir.as_deref(), Some("artifacts/custom"));
        assert_eq!(config.studio_port, Some(8080));

        let diff = config.diff.unwrap();
        assert_eq!(diff.max_total_gas_increase_percent, Some(3.5));
        assert_eq!(diff.max_execution_gas_increase_percent, Some(2.0));
        assert_eq!(diff.max_evm_steps_increase, Some(75));
        assert_eq!(diff.max_stylus_calls_increase, Some(1));
    }

    #[test]
    fn evaluates_nitro_thresholds_correctly() {
        let diff = DiffConfig {
            max_total_gas_increase_percent: Some(2.0),
            max_execution_gas_increase_percent: Some(1.5),
            max_evm_steps_increase: Some(50),
            max_stylus_calls_increase: Some(0),
        };

        // Within limits -> no failures
        let passes = diff.evaluate_nitro(1.8, 1.2, 40.0, 0.0);
        assert!(passes.is_empty());

        // Breaches total gas and evm steps
        let failures = diff.evaluate_nitro(2.5, 1.2, 80.0, 0.0);
        assert_eq!(failures.len(), 2);
        assert!(failures[0].contains("Total Gas increased by 2.5%"));
        assert!(failures[1].contains("EVM Steps increased by 80"));

        // Breaches stylus calls
        let stylus_failure = diff.evaluate_nitro(0.0, 0.0, 0.0, 2.0);
        assert_eq!(stylus_failure.len(), 1);
        assert!(stylus_failure[0].contains("Stylus Calls increased by 2"));
    }

    #[test]
    fn evaluates_simple_threshold() {
        let failure = DiffConfig::evaluate_simple_threshold("Compute Units", 10.5, 5.0);
        assert_eq!(
            failure,
            Some("Total Compute Units increased by 10.5% (limit: 5.0%)".to_string())
        );

        let pass = DiffConfig::evaluate_simple_threshold("Compute Units", 3.0, 5.0);
        assert_eq!(pass, None);
    }

    #[test]
    fn load_non_existent_file_returns_error() {
        let res = AtupaConfigToml::load(Path::new("/non/existent/atupa.toml"));
        assert!(res.is_err());
    }
}
