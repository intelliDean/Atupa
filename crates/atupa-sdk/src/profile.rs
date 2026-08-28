//! High-level profile execution engine, usable programmatically or via CLI.

use anyhow::Result;
use atupa_core::{CollapsedStack, VmKind};
use atupa_nitro::{NitroClient, VmKind as NitroVmKind};
use atupa_output::SvgGenerator;
use atupa_parser::{Parser as AtupaParser, aggregator::Aggregator};
use atupa_rpc::etherscan::EtherscanResolver;
use atupa_solana::{SolanaClient, SolanaLogStitcher};
use atupa_starknet::StarknetClient;
use atupa_stellar::StellarClient;
use indicatif::{ProgressBar, ProgressStyle};
use std::{fs, time::Duration};

/// Controls which VM runtime `execute_profile` targets.
///
/// If omitted (`None`), heuristic auto-detection determines the target runtime
/// from the RPC endpoint URL and transaction hash structure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VmHint {
    /// Standard EVM / Arbitrum Nitro (EVM + optional Stylus stitching)
    Evm,
    /// Force Arbitrum Stylus trace
    Stylus,
    /// Starknet Cairo VM
    Starknet,
    /// Solana Sealevel VM
    Solana,
    /// Stellar Soroban WASM VM
    Stellar,
}

/// Detects which VM to use given an optional explicit hint, the RPC URL, and the transaction hash.
pub fn detect_vm(vm_hint: Option<&VmHint>, rpc: &str, tx: &str) -> VmHint {
    if let Some(hint) = vm_hint {
        return hint.clone();
    }

    if rpc.contains("starknet") || tx.len() > 66 {
        VmHint::Starknet
    } else if rpc.contains("solana") || tx.len() == 44 {
        VmHint::Solana
    } else if rpc.contains("stellar") || rpc.contains("soroban") || tx.len() == 64 {
        VmHint::Stellar
    } else {
        VmHint::Evm
    }
}

/// Fetch (or generate a demo), aggregate, and render an SVG flamegraph for
/// the given transaction hash.
pub async fn execute_profile(
    tx: &str,
    rpc: &str,
    is_demo: bool,
    out: Option<String>,
    etherscan_key: Option<String>,
    vm_hint: Option<VmHint>,
) -> Result<(String, String)> {
    let pb = make_spinner();

    // 1. Fetch & Aggregate Stacks ──────────────────────────────────────────────
    let (mut stacks, network_name) = if is_demo {
        pb.set_message("Generating offline demo trace…");
        (demo_stacks(), "Demo".to_string())
    } else {
        pb.set_message("Detecting network and fetching execution trace…");
        let target_vm = detect_vm(vm_hint.as_ref(), rpc, tx);

        match target_vm {
            VmHint::Starknet => fetch_starknet_stacks(tx, rpc, &pb).await?,
            VmHint::Solana => fetch_solana_stacks(tx, rpc, &pb).await?,
            VmHint::Stellar => fetch_stellar_stacks(tx, rpc, &pb).await?,
            VmHint::Evm | VmHint::Stylus => {
                fetch_evm_nitro_stacks(tx, rpc, etherscan_key, &pb).await?
            }
        }
    };

    // Sort EVM stacks descending by weight; Stylus stacks come after
    let evm_end = stacks.partition_point(|s| s.vm_kind == VmKind::Evm);
    stacks[..evm_end].sort_by_key(|b| std::cmp::Reverse(b.weight));

    // 2. Render & Output ───────────────────────────────────────────────────────
    pb.set_message("Generating SVG flamegraph…");
    let out_path = render_and_save_flamegraph(&stacks, tx, is_demo, out)?;

    pb.finish_with_message(format!("✔ Profile saved → {out_path}"));
    Ok((out_path, network_name))
}

// ─── VM Fetch Handlers ────────────────────────────────────────────────────────

async fn fetch_starknet_stacks(
    tx: &str,
    rpc: &str,
    pb: &ProgressBar,
) -> Result<(Vec<CollapsedStack>, String)> {
    pb.set_message("Starknet node detected. Fetching Cairo VM trace…");
    let client = StarknetClient::new(rpc.to_string());
    let steps = client
        .profile_transaction(tx)
        .await
        .map_err(|e| anyhow::anyhow!("Starknet RPC error: {e}"))?;

    let normalized = AtupaParser::normalize_raw(steps);
    let combined = Aggregator::build_collapsed_stacks(&normalized);
    Ok((combined, "Starknet".to_string()))
}

async fn fetch_solana_stacks(
    tx: &str,
    rpc: &str,
    pb: &ProgressBar,
) -> Result<(Vec<CollapsedStack>, String)> {
    pb.set_message("Solana node detected. Reconstructing Sealevel VM trace…");
    let client = SolanaClient::new(rpc.to_string());
    let logs = client
        .get_transaction_logs(tx)
        .await
        .map_err(|e| anyhow::anyhow!("Solana RPC error: {e}"))?;

    let steps = SolanaLogStitcher::parse_logs(&logs);
    let normalized = AtupaParser::normalize_raw(steps);
    let combined = Aggregator::build_collapsed_stacks(&normalized);
    Ok((combined, "Solana".to_string()))
}

async fn fetch_stellar_stacks(
    tx: &str,
    rpc: &str,
    pb: &ProgressBar,
) -> Result<(Vec<CollapsedStack>, String)> {
    pb.set_message("Stellar node detected. Fetching Soroban diagnostic trace…");
    let client = StellarClient::new(rpc.to_string());
    let steps = client
        .get_transaction_trace(tx)
        .await
        .map_err(|e| anyhow::anyhow!("Stellar RPC error: {e}"))?;

    let normalized = AtupaParser::normalize_raw(steps);
    let combined = Aggregator::build_collapsed_stacks(&normalized);
    Ok((combined, "Stellar".to_string()))
}

async fn fetch_evm_nitro_stacks(
    tx: &str,
    rpc: &str,
    etherscan_key: Option<String>,
    pb: &ProgressBar,
) -> Result<(Vec<CollapsedStack>, String)> {
    let client = NitroClient::new(rpc.to_string());
    let report = tokio::time::timeout(Duration::from_secs(30), client.trace_transaction(tx))
        .await
        .map_err(|_| anyhow::anyhow!("RPC timed out after 30s — is the node reachable at {rpc}?"))?
        .map_err(|e| anyhow::anyhow!("RPC error: {e}"))?;

    let network = get_network_name(report.chain_id);
    let evm_count = report
        .steps
        .iter()
        .filter(|s| s.vm == NitroVmKind::Evm)
        .count();
    let wasm_count = report
        .steps
        .iter()
        .filter(|s| s.vm == NitroVmKind::Stylus)
        .count();
    pb.set_message(format!(
        "Processing {evm_count} EVM + {wasm_count} Stylus steps from {network}…"
    ));

    let unified_steps: Vec<atupa_core::TraceStep> =
        report.steps.iter().map(|s| s.to_trace_step()).collect();

    let normalized = AtupaParser::normalize_raw(unified_steps);
    let registry = crate::build_default_registry();
    let mut combined = Aggregator::build_collapsed_stacks_with_registry(&normalized, &registry);

    // Etherscan resolution — only meaningful for EVM steps with an address.
    pb.set_message("Resolving contract names via Etherscan…");
    let resolver = EtherscanResolver::new(etherscan_key, report.chain_id);
    for stack in &mut combined {
        if stack.vm_kind == VmKind::Evm
            && let Some(addr) = &stack.target_address
            && let Some(name) = resolver.resolve_contract_name(addr).await
        {
            stack.target_address = Some(name);
        }
    }

    Ok((combined, network))
}

fn render_and_save_flamegraph(
    stacks: &[CollapsedStack],
    tx: &str,
    is_demo: bool,
    out: Option<String>,
) -> Result<String> {
    let svg = SvgGenerator::generate_flamegraph(stacks)?;
    let out_path = out.unwrap_or_else(|| {
        if is_demo {
            "profile_demo.svg".to_string()
        } else {
            let short = tx.trim_start_matches("0x").get(..10).unwrap_or(tx);
            format!("profile_{short}.svg")
        }
    });
    fs::write(&out_path, svg)?;
    Ok(out_path)
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn make_spinner() -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("{spinner:.cyan} {msg}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    pb.enable_steady_tick(Duration::from_millis(80));
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

/// A rich offline demo trace showcasing nested calls, storage operations, reverts, and simulated Stylus steps.
pub fn demo_stacks() -> Vec<CollapsedStack> {
    vec![
        // ── Root frame ops (depth 1) ────────────────────────────────────
        CollapsedStack {
            stack: "CALL".to_string(),
            weight: 21_000,
            last_pc: Some(0),
            depth: 1,
            vm_kind: VmKind::Evm,
            target_address: None,
            resolved_label: Some("Root CALL (21,000 gas)".to_string()),
            reverted: false,
        },
        CollapsedStack {
            stack: "CALL;SLOAD".to_string(),
            weight: 2_100,
            last_pc: Some(10),
            depth: 2,
            vm_kind: VmKind::Evm,
            target_address: None,
            resolved_label: Some("Storage Read (2,100 gas)".to_string()),
            reverted: false,
        },
        CollapsedStack {
            stack: "CALL;SSTORE".to_string(),
            weight: 20_000,
            last_pc: Some(14),
            depth: 2,
            vm_kind: VmKind::Evm,
            target_address: None,
            resolved_label: Some("Storage Write (20,000 gas)".to_string()),
            reverted: false,
        },
        // ── Nested sub-call (depth 2 → 3) ──────────────────────────────
        CollapsedStack {
            stack: "CALL;CALL;KECCAK256".to_string(),
            weight: 30,
            last_pc: Some(20),
            depth: 3,
            vm_kind: VmKind::Evm,
            target_address: Some("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string()),
            resolved_label: Some("USDC: KECCAK256 (30 gas)".to_string()),
            reverted: false,
        },
        CollapsedStack {
            stack: "CALL;CALL;SLOAD".to_string(),
            weight: 2_100,
            last_pc: Some(24),
            depth: 3,
            vm_kind: VmKind::Evm,
            target_address: None,
            resolved_label: Some("Nested SLOAD (2,100 gas)".to_string()),
            reverted: false,
        },
        // ── Reverted sub-call (depth 2) ─────────────────────────────────
        CollapsedStack {
            stack: "CALL;REVERT".to_string(),
            weight: 5_000,
            last_pc: Some(40),
            depth: 2,
            vm_kind: VmKind::Evm,
            target_address: None,
            resolved_label: Some("REVERTED sub-call (5,000 gas)".to_string()),
            reverted: true,
        },
        // ── Simulated Stylus WASM steps ─────────────────────────────────
        CollapsedStack {
            stack: "storage_load_bytes32".to_string(),
            weight: 421,
            last_pc: None,
            depth: 1,
            vm_kind: VmKind::Stylus,
            target_address: None,
            resolved_label: Some(
                "storage_load_bytes32 (4,215 ink → 0.42 gas-equiv)".to_string(),
            ),
            reverted: false,
        },
        CollapsedStack {
            stack: "storage_flush_cache".to_string(),
            weight: 4_001,
            last_pc: None,
            depth: 1,
            vm_kind: VmKind::Stylus,
            target_address: None,
            resolved_label: Some(
                "storage_flush_cache (40,010 ink → 4.00 gas-equiv)".to_string(),
            ),
            reverted: false,
        },
        CollapsedStack {
            stack: "native_keccak256".to_string(),
            weight: 4,
            last_pc: None,
            depth: 1,
            vm_kind: VmKind::Stylus,
            target_address: None,
            resolved_label: Some("native_keccak256 (36 ink → 0.004 gas-equiv)".to_string()),
            reverted: false,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_explicit_vm_hint() {
        assert_eq!(
            detect_vm(Some(&VmHint::Starknet), "http://localhost:8545", "0x1234"),
            VmHint::Starknet
        );
        assert_eq!(
            detect_vm(Some(&VmHint::Solana), "http://localhost:8545", "0x1234"),
            VmHint::Solana
        );
    }

    #[test]
    fn detects_vm_heuristics() {
        // Starknet heuristic via URL
        assert_eq!(
            detect_vm(None, "https://starknet-mainnet.public.blastapi.io", "0x1234"),
            VmHint::Starknet
        );
        // Solana heuristic via 44-char signature
        let solana_sig = "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d";
        assert_eq!(
            detect_vm(None, "https://api.mainnet-beta.solana.com", solana_sig),
            VmHint::Solana
        );
        // Stellar heuristic via 64-char hash
        let stellar_hash = "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef";
        assert_eq!(
            detect_vm(None, "https://soroban-rpc.mainnet.stellar.org", stellar_hash),
            VmHint::Stellar
        );
    }

    #[test]
    fn network_names_mapping() {
        assert_eq!(get_network_name(1), "Ethereum Mainnet");
        assert_eq!(get_network_name(42161), "Arbitrum One");
        assert_eq!(get_network_name(8453), "Base Mainnet");
        assert_eq!(get_network_name(99999), "Chain ID: 99999");
    }

    #[test]
    fn demo_stacks_has_evm_and_stylus_items() {
        let stacks = demo_stacks();
        assert!(!stacks.is_empty());
        assert!(stacks.iter().any(|s| s.vm_kind == VmKind::Evm));
        assert!(stacks.iter().any(|s| s.vm_kind == VmKind::Stylus));
    }
}
