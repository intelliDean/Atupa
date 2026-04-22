//! # Atupa
//!
//! **Unified Ethereum Execution Profiler** — the top-level façade crate for the
//! Atupa SDK. This crate re-exports every layer of the suite so that external
//! integrators only need to depend on a single crate:
//!
//! ```toml
//! [dependencies]
//! atupa = "0.1"
//! ```
//!
//! ## Crate Architecture
//!
//! ```text
//! atupa (this façade)
//! ├── atupa-core      → Types: TraceStep, CollapsedStack, GasCategory
//! ├── atupa-rpc       → JSON-RPC client (EthClient, EtherscanResolver)
//! ├── atupa-parser    → StructLog → TraceStep normalization
//! ├── atupa-adapters  → ProtocolAdapter trait
//! ├── atupa-output    → SvgGenerator flamegraphs
//! ├── atupa-aave      → AaveDeepTracer, GHO metrics
//! └── atupa-lido      → LidoDeepTracer, stETH / wstETH tracing
//! ```

// ─── Public re-exports ────────────────────────────────────────────────────────

/// Core types shared across the entire Atupa suite.
pub use atupa_core as core;

/// JSON-RPC transport layer: EthClient, EtherscanResolver, RawStructLog.
pub use atupa_rpc as rpc;

/// Trace normalization and aggregation.
pub use atupa_parser as parser;

/// ProtocolAdapter trait for pluggable DeFi recognizers.
pub use atupa_adapters as adapters;

/// SVG flamegraph renderer.
pub use atupa_output as output;

/// Aave v3 + GHO protocol adapter.
pub use atupa_aave as aave;

/// Lido stETH protocol adapter.
pub use atupa_lido as lido;

// ─── High-level API ───────────────────────────────────────────────────────────

pub use profile::execute_profile;

/// High-level profile execution logic, usable independently from the CLI.
pub mod profile {
    use anyhow::Result;
    use atupa_core::{CollapsedStack, VmKind};
    use atupa_nitro::{NitroClient, VmKind as NitroVmKind};
    use atupa_output::SvgGenerator;
    use atupa_parser::{Parser as AtupaParser, aggregator::Aggregator};
    use atupa_rpc::etherscan::EtherscanResolver;
    use indicatif::{ProgressBar, ProgressStyle};
    use std::{fs, time::Duration};

    /// Fetch (or generate a demo), aggregate, and render an SVG flamegraph for
    /// the given transaction hash.
    ///
    /// This is the same logic that `atupa profile` runs — exposed here so it can
    /// be called programmatically by other tools or tests.
    pub async fn execute_profile(
        tx: &str,
        rpc: &str,
        is_demo: bool,
        out: Option<String>,
        etherscan_key: Option<String>,
    ) -> Result<(String, String)> {
        let pb = make_spinner();

        // 1. Fetch ─────────────────────────────────────────────────────────────
        let (mut stacks, network_name) = if is_demo {
            pb.set_message("Generating offline demo trace…");
            (demo_stacks(), "Demo".to_string())
        } else {
            pb.set_message("Detecting network and fetching execution trace…");
            let client = NitroClient::new(rpc.to_string());
            let report = tokio::time::timeout(
                Duration::from_secs(30),
                client.trace_transaction(tx),
            )
            .await
            .map_err(|_| anyhow::anyhow!("RPC timed out after 30s — is the node reachable at {rpc}?"))?
            .map_err(|e| anyhow::anyhow!("RPC error: {e}"))?;

            let network = get_network_name(report.chain_id);
            let evm_count = report.steps.iter().filter(|s| s.vm == NitroVmKind::Evm).count();
            let wasm_count = report.steps.iter().filter(|s| s.vm == NitroVmKind::Stylus).count();
            pb.set_message(format!(
                "Processing {evm_count} EVM + {wasm_count} Stylus steps from {network}…"
            ));

            // ── Unified single-pass aggregation ────────────────────────────────
            // Convert the interleaved UnifiedStep timeline into core TraceSteps.
            // Stylus steps already carry depth = (parent CALL depth + 1) and a
            // gas_cost equal to ink / 10_000, so the Aggregator nests them under
            // their EVM CALL frames without any special-casing.
            let unified_steps: Vec<atupa_core::TraceStep> = report
                .steps
                .iter()
                .map(|s| s.to_trace_step())
                .collect();

            let normalized = AtupaParser::normalize_raw(unified_steps);
            let mut combined = Aggregator::build_collapsed_stacks(&normalized);

            // Etherscan resolution — only meaningful for EVM steps with an address.
            pb.set_message("Resolving contract names via Etherscan…");
            let resolver = EtherscanResolver::new(etherscan_key, report.chain_id);
            for stack in &mut combined {
                if stack.vm_kind == VmKind::Evm {
                    if let Some(addr) = &stack.target_address
                        && let Some(name) = resolver.resolve_contract_name(addr).await
                    {
                        stack.target_address = Some(name);
                    }
                }
            }

            (combined, network)
        };

        // Sort EVM stacks descending by weight; Stylus stacks come after
        let evm_end = stacks.partition_point(|s| s.vm_kind == VmKind::Evm);
        stacks[..evm_end].sort_by(|a, b| b.weight.cmp(&a.weight));

        // 2. Render + save ─────────────────────────────────────────────────────
        pb.set_message("Generating SVG flamegraph…");
        let svg = SvgGenerator::generate_flamegraph(&stacks)?;
        let out_path = out.unwrap_or_else(|| {
            if is_demo {
                "profile_demo.svg".to_string()
            } else {
                // Shorten to first 10 hex chars after 0x
                let short = tx.trim_start_matches("0x").get(..10).unwrap_or(tx);
                format!("profile_{short}.svg")
            }
        });
        fs::write(&out_path, svg)?;

        pb.finish_with_message(format!("✔ Profile saved → {out_path}"));
        Ok((out_path, network_name))
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

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

    fn get_network_name(chain_id: u64) -> String {
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

    /// A rich offline demo trace showcasing nested calls, reverts, and simulated Stylus steps.
    fn demo_stacks() -> Vec<CollapsedStack> {
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
                resolved_label: Some("storage_load_bytes32 (4,215 ink → 0.42 gas-equiv)".to_string()),
                reverted: false,
            },
            CollapsedStack {
                stack: "storage_flush_cache".to_string(),
                weight: 4_001,
                last_pc: None,
                depth: 1,
                vm_kind: VmKind::Stylus,
                target_address: None,
                resolved_label: Some("storage_flush_cache (40,010 ink → 4.00 gas-equiv)".to_string()),
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
}

