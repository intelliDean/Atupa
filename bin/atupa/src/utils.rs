//! Shared path resolution, normalization, and formatting utilities for CLI commands.

use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use atupa_core::{GasCategory, TraceStep, VmKind as CoreVmKind};
use atupa_nitro::{StitchedReport, UnifiedStep, VmKind as NitroVmKind};
use atupa_rpc::RawStructLog;

/// Returns a standardized artifact filepath, nesting into `artifacts/<category>/`
/// if no explicit parent directory was specified by the user.
pub fn resolve_artifact_path(
    path: Option<String>,
    category: &str,
    tx_hash: &str,
    ext: &str,
) -> String {
    let filename = path.unwrap_or_else(|| {
        let short = tx_hash
            .trim_start_matches("0x")
            .get(..10)
            .unwrap_or(tx_hash);
        match ext {
            "json" => format!("report_{short}.json"),
            "svg" => format!("profile_{short}.svg"),
            _ => format!("artifact_{short}.{ext}"),
        }
    });

    let pb = PathBuf::from(&filename);
    if pb
        .parent()
        .map(|p| p.as_os_str().is_empty())
        .unwrap_or(true)
    {
        let dir = format!("artifacts/{category}");
        let _ = std::fs::create_dir_all(&dir);
        format!("{dir}/{filename}")
    } else {
        filename
    }
}

/// Normalise a transaction hash or signature.
/// EVM hashes get lowercased and `0x`-prefixed.
/// Solana signatures (Base58, >70 chars) are preserved exactly as provided.
pub fn normalise_hash(tx: &str) -> String {
    let t = tx.trim();
    if t.len() > 70 {
        return t.to_string();
    }
    if t.to_lowercase().starts_with("0x") {
        t.to_lowercase()
    } else {
        format!("0x{}", t.to_lowercase())
    }
}

/// Counts the number of EVM steps in a stitched report.
pub fn evm_count(r: &StitchedReport) -> usize {
    r.steps.iter().filter(|s| s.vm == NitroVmKind::Evm).count()
}

/// Converts a flat `Vec<TraceStep>` (from Starknet/Solana/Stellar adapters)
/// into a `StitchedReport` for studio visualizers and downstream tooling.
pub fn trace_steps_to_report(
    tx: &str,
    steps: Vec<TraceStep>,
    chain_vm: NitroVmKind,
) -> StitchedReport {
    let mut total_gas: u64 = 0;
    let mut category_costs: HashMap<GasCategory, f64> = HashMap::new();

    let unified: Vec<UnifiedStep> = steps
        .into_iter()
        .enumerate()
        .map(|(i, s)| {
            let cost = s.gas_cost as f64;
            total_gas = total_gas.saturating_add(s.gas_cost);
            let category = GasCategory::from_step(&s.op, &s.vm_kind);
            *category_costs.entry(category.clone()).or_insert(0.0) += cost;
            UnifiedStep {
                index: i,
                vm: chain_vm.clone(),
                label: s.op,
                gas_cost: s.gas_cost,
                cost_equiv: cost,
                depth: s.depth,
                is_vm_boundary: false,
                category,
                target_address: None,
                evm: None,
                stylus: None,
            }
        })
        .collect();

    StitchedReport {
        tx_hash: tx.to_string(),
        chain_id: 0,
        steps: unified,
        total_evm_gas: total_gas,
        total_stylus_ink: 0,
        vm_boundary_count: 0,
        total_stylus_gas_equiv: 0.0,
        total_unified_cost: total_gas as f64,
        category_costs,
        resolved_names: HashMap::new(),
        on_chain_gas_used: None,
    }
}

/// Bridges a `RawStructLog` from RPC into a `TraceStep`.
pub fn bridge_raw_to_trace_step(raw: &RawStructLog) -> TraceStep {
    TraceStep {
        pc: raw.pc,
        op: raw.op.clone(),
        gas: raw.gas,
        gas_cost: raw.gas_cost,
        depth: raw.depth,
        stack: raw.stack.clone(),
        memory: raw.memory.clone(),
        error: raw.error.clone(),
        reverted: raw.error.is_some(),
        vm_kind: CoreVmKind::Evm,
    }
}

/// Creates a stylized CLI terminal progress spinner.
pub fn make_spinner(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("{spinner:.cyan} {msg}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    pb.enable_steady_tick(Duration::from_millis(80));
    pb.set_message(msg.to_string());
    pb
}

/// Returns a human-friendly name for standard chain IDs.
pub fn get_network_name(chain_id: u64) -> String {
    match chain_id {
        1 => "Ethereum Mainnet".to_string(),
        11155111 => "Sepolia Testnet".to_string(),
        17000 => "Holesky Testnet".to_string(),
        42161 => "Arbitrum One".to_string(),
        42170 => "Arbitrum Nova".to_string(),
        421614 => "Arbitrum Sepolia".to_string(),
        8453 => "Base Mainnet".to_string(),
        84532 => "Base Sepolia".to_string(),
        10 => "Optimism".to_string(),
        11155420 => "Optimism Sepolia".to_string(),
        137 => "Polygon POS".to_string(),
        1337 | 31337 => "Local Devnet".to_string(),
        412346 => "Nitro Local Devnet".to_string(),
        0 => "Unknown Network".to_string(),
        id => format!("Chain ID: {id}"),
    }
}

/// Formats a divider line of standard width.
pub fn divider(len: usize) -> String {
    "─".repeat(len).dimmed().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalise_hash_handles_evm_and_solana() {
        assert_eq!(
            normalise_hash("0xABCDEF123456"),
            "0xabcdef123456"
        );
        assert_eq!(
            normalise_hash("ABCDEF123456"),
            "0xabcdef123456"
        );
        // Solana signature: Base58 string > 70 chars
        let solana_sig = "5VERv8NMvzbJMEdV8xnrLkEaWRtSz9CosKDYj7WNXTip3MrTKEjWAFAwDxj61GbyGhBsp89uNpnv1Fs31";
        assert_eq!(normalise_hash(solana_sig), solana_sig);
    }

    #[test]
    fn network_name_mappings() {
        assert_eq!(get_network_name(1), "Ethereum Mainnet");
        assert_eq!(get_network_name(42161), "Arbitrum One");
        assert_eq!(get_network_name(8453), "Base Mainnet");
        assert_eq!(get_network_name(999999), "Chain ID: 999999");
    }

    #[test]
    fn resolve_artifact_path_nested_and_custom() {
        let path = resolve_artifact_path(None, "capture", "0x1234567890abcdef", "json");
        assert!(path.contains("artifacts/capture/report_1234567890.json"));

        let custom = resolve_artifact_path(Some("/tmp/custom_report.json".to_string()), "capture", "0x1234", "json");
        assert_eq!(custom, "/tmp/custom_report.json");
    }

    #[test]
    fn trace_steps_to_report_conversion() {
        let steps = vec![
            TraceStep {
                pc: 0,
                op: "CALL".to_string(),
                gas: 50000,
                gas_cost: 2100,
                depth: 1,
                stack: None,
                memory: None,
                error: None,
                reverted: false,
                vm_kind: CoreVmKind::Evm,
            },
            TraceStep {
                pc: 1,
                op: "SLOAD".to_string(),
                gas: 47900,
                gas_cost: 2100,
                depth: 1,
                stack: None,
                memory: None,
                error: None,
                reverted: false,
                vm_kind: CoreVmKind::Evm,
            },
        ];

        let report = trace_steps_to_report("0x1234", steps, NitroVmKind::Evm);
        assert_eq!(report.steps.len(), 2);
        assert_eq!(report.total_evm_gas, 4200);
        assert_eq!(report.total_unified_cost, 4200.0);
    }

    #[test]
    fn bridge_raw_to_trace_step_conversion() {
        let raw = RawStructLog {
            pc: 42,
            op: "SSTORE".to_string(),
            gas: 100000,
            gas_cost: 20000,
            depth: 2,
            stack: Some(vec!["0x1".to_string(), "0x2".to_string()]),
            memory: None,
            storage: None,
            error: None,
        };

        let step = bridge_raw_to_trace_step(&raw);
        assert_eq!(step.pc, 42);
        assert_eq!(step.op, "SSTORE");
        assert_eq!(step.gas_cost, 20000);
        assert_eq!(step.depth, 2);
        assert!(!step.reverted);
    }
}

