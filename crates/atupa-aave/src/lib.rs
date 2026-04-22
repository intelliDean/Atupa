//! # atupa-aave — DeepTracer
//!
//! Aave v3 & GHO protocol adapter for the Atupa EVM profiling engine.
//! Provides deep trace analysis for liquidation flows, supply/borrow
//! mechanics, and GHO stablecoin risk monitoring.

use atupa_adapters::ProtocolAdapter;
use atupa_core::TraceStep;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Selector tables
// ---------------------------------------------------------------------------

/// Known Aave v3 Pool function selectors → human-readable labels.
const POOL_SELECTORS: &[(&str, &str)] = &[
    ("0x617ba037", "supply"),
    ("0x69328dec", "withdraw"),
    ("0xa415bcad", "borrow"),
    ("0x573ade81", "repay"),
    ("0x563dd613", "repayWithPermit"),
    ("0x2dad97d4", "repayWithATokens"),
    ("0x00a718a9", "liquidationCall"),
    ("0xab9c4b5d", "flashLoan"),
    ("0x42b0b77c", "flashLoanSimple"),
    ("0xe8eda9df", "deposit"),      // v2 compat
    ("0xa9059cbb", "transfer"),     // ERC-20 — common inside traces
    ("0x23b872dd", "transferFrom"), // ERC-20
    ("0x095ea7b3", "approve"),      // ERC-20
    ("0x1e9a6950", "setUserUseReserveAsCollateral"),
    ("0x02c205f0", "swapBorrowRateMode"),
];

/// Known GHO Facilitators (Ethereum Mainnet).
const GHO_FACILITATORS: &[(&str, &str)] = &[
    (
        "0x5513224daaEABCa31af5280727878d52097afA05",
        "Direct Minter (Aave V3)",
    ),
    (
        "0xBc65ad17c5C0a2A4D159fa5a503f4992c7B545FE",
        "Spark (Sky) Facilitator",
    ),
];

/// Known Aave Oracles (Ethereum Mainnet).
const AAVE_ORACLES: &[(&str, &str)] = &[
    (
        "0x54586bE62E3c3580375aE3716C14bd2563060Ca0C2",
        "Aave Price Oracle",
    ),
    ("0xD81E9938...?", "GHO Price Oracle"),
];

/// Known GHO-specific selectors.
const GHO_SELECTORS: &[(&str, &str)] = &[
    ("0x40c10f19", "mint"),
    ("0x9dc29fac", "burn"),
    ("0xd73dd623", "increaseAllowance"),
    ("0x5d3a1f9b", "distributeFeesToTreasury"),
    ("0x2e0f2625", "updateFacilitatorBucketCapacity"),
    ("0xdb5a3c5e", "setVariableDebtToken"),
];

// ---------------------------------------------------------------------------
// Protocol Adapter implementation
// ---------------------------------------------------------------------------

/// Enhanced Aave v3 protocol adapter — identifies Pool & GHO operations.
#[derive(Default)]
pub struct AaveV3Adapter;

impl ProtocolAdapter for AaveV3Adapter {
    fn name(&self) -> &str {
        "Aave v3 / GHO"
    }

    fn resolve_label(&self, address: Option<&str>, selector: Option<&str>) -> Option<String> {
        // Resolve facilitator names if address is provided
        if let Some(addr) = address {
            for &(known_addr, name) in GHO_FACILITATORS {
                if addr.to_lowercase() == known_addr.to_lowercase() {
                    return Some(format!("Facilitator::{}", name));
                }
            }
            for &(known_addr, name) in AAVE_ORACLES {
                if addr.to_lowercase() == known_addr.to_lowercase() {
                    return Some(format!("Oracle::{}", name));
                }
            }
        }

        let sel = selector?;
        // Check Pool selectors first
        for &(known_sel, label) in POOL_SELECTORS {
            if sel == known_sel {
                return Some(format!("AaveV3Pool::{label}"));
            }
        }
        // Fall through to GHO selectors
        for &(known_sel, label) in GHO_SELECTORS {
            if sel == known_sel {
                return Some(format!("GHO::{label}"));
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------
// Liquidation Report
// ---------------------------------------------------------------------------

/// A human-readable breakdown of a single `liquidationCall` execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidationReport {
    /// Transaction hash being analyzed.
    pub tx_hash: String,
    /// Total gas consumed by the liquidation.
    pub total_gas: u64,
    /// Gas consumed by the `liquidationCall` itself.
    pub liquidation_gas: u64,
    /// Number of SLOAD opcodes (storage reads — proxy for oracle lookups).
    pub storage_reads: u32,
    /// Number of SSTORE opcodes (storage writes).
    pub storage_writes: u32,
    /// Number of cross-contract CALL opcodes.
    pub external_calls: u32,
    /// Whether the transaction reverted.
    pub reverted: bool,
    /// The deepest call depth reached.
    pub max_depth: u16,
    /// Liquidation Efficiency: (Gas Value / Debt Covered) -- lower is better.
    /// (Note: Simplification for trace-only analysis).
    pub liquidation_efficiency: f64,
    /// Number of identified Oracle calls during the trace.
    pub oracle_calls: u32,
    /// Labeled call sequence extracted from the trace.
    pub labeled_calls: Vec<LabeledCall>,
}

impl LiquidationReport {
    /// Returns a concise one-line summary for terminal output.
    pub fn summary(&self) -> String {
        format!(
            "[LiquidationReport] tx={} gas={} reads={} writes={} calls={} reverted={}",
            &self.tx_hash[..10],
            self.total_gas,
            self.storage_reads,
            self.storage_writes,
            self.external_calls,
            self.reverted,
        )
    }
}

/// A single labeled call extracted during trace analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabeledCall {
    pub depth: u16,
    pub label: String,
    pub gas_cost: u64,
}

// ---------------------------------------------------------------------------
// GHO Supply Metrics
// ---------------------------------------------------------------------------

/// Aggregated GHO supply-level metrics extracted from trace steps.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GhoSupplyMetrics {
    /// Number of `mint` calls observed in the trace.
    pub mint_count: u32,
    /// Number of `burn` calls observed in the trace.
    pub burn_count: u32,
    /// Number of `updateFacilitatorBucketCapacity` calls (risk signal).
    pub bucket_capacity_updates: u32,
    /// Number of `distributeFeesToTreasury` calls.
    pub fee_distributions: u32,
}

// ---------------------------------------------------------------------------
// DeepTracer — Main entry point
// ---------------------------------------------------------------------------

/// The main Aave DeepTracer entry point. Wraps the `AaveV3Adapter` and
/// provides higher-level analysis methods over raw Atupa `TraceStep` slices.
#[derive(Default)]
pub struct AaveDeepTracer {
    adapter: AaveV3Adapter,
}

impl AaveDeepTracer {
    pub fn new() -> Self {
        Self {
            adapter: AaveV3Adapter,
        }
    }

    /// Analyzes a raw trace and produces a `LiquidationReport`.
    pub fn analyze_liquidation(
        &self,
        tx_hash: &str,
        steps: &[TraceStep],
    ) -> anyhow::Result<LiquidationReport> {
        let mut storage_reads = 0u32;
        let mut storage_writes = 0u32;
        let mut external_calls = 0u32;
        let mut oracle_calls = 0u32;
        let mut max_depth = 0u16;
        let mut total_gas = 0u64;
        let mut liquidation_gas = 0u64;
        let mut labeled_calls: Vec<LabeledCall> = Vec::new();
        let mut in_liquidation = false;

        for step in steps {
            total_gas = total_gas.saturating_add(step.gas_cost);
            max_depth = max_depth.max(step.depth);

            match step.op.as_str() {
                "SLOAD" => storage_reads += 1,
                "SSTORE" => storage_writes += 1,
                "CALL" | "STATICCALL" | "DELEGATECALL" | "CALLCODE" => {
                    external_calls += 1;

                    // Attempt to resolve the selector and address
                    let selector = step
                        .stack
                        .as_ref()
                        .and_then(|s| s.last())
                        .map(|s| s.as_str());

                    // Note: In a real trace, the address would be on the stack,
                    // this is a simplified simulation for the POC
                    let address = None;

                    if let Some(label) = self.adapter.resolve_label(address, selector) {
                        if label.contains("liquidationCall") {
                            in_liquidation = true;
                        }
                        if label.contains("Oracle") {
                            oracle_calls += 1;
                        }
                        labeled_calls.push(LabeledCall {
                            depth: step.depth,
                            label,
                            gas_cost: step.gas_cost,
                        });
                    }
                }
                _ => {}
            }

            if in_liquidation {
                liquidation_gas = liquidation_gas.saturating_add(step.gas_cost);
            }
        }

        let reverted = steps.last().is_some_and(|s| s.reverted);

        // Mock efficiency calculation (simplified for trace analysis)
        let liquidation_efficiency = if liquidation_gas > 0 {
            (liquidation_gas as f64) / 100_000.0 // Normalizing against a base gas cost
        } else {
            0.0
        };

        Ok(LiquidationReport {
            tx_hash: tx_hash.to_string(),
            total_gas,
            liquidation_gas,
            storage_reads,
            storage_writes,
            external_calls,
            oracle_calls,
            reverted,
            max_depth,
            liquidation_efficiency,
            labeled_calls,
        })
    }

    /// Scans a trace for GHO supply-level signals.
    pub fn extract_gho_metrics(&self, steps: &[TraceStep]) -> GhoSupplyMetrics {
        let mut metrics = GhoSupplyMetrics::default();

        for step in steps {
            if step.op != "CALL" && step.op != "STATICCALL" {
                continue;
            }
            let selector = step
                .stack
                .as_ref()
                .and_then(|s| s.last())
                .map(|s| s.as_str());

            if let Some(label) = self.adapter.resolve_label(None, selector) {
                match label.as_str() {
                    "GHO::mint" => metrics.mint_count += 1,
                    "GHO::burn" => metrics.burn_count += 1,
                    "GHO::updateFacilitatorBucketCapacity" => metrics.bucket_capacity_updates += 1,
                    "GHO::distributeFeesToTreasury" => metrics.fee_distributions += 1,
                    _ => {}
                }
            }
        }

        metrics
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_call_step(op: &str, selector: &str, gas_cost: u64) -> TraceStep {
        TraceStep {
            op: op.to_string(),
            gas: 1_000_000,
            gas_cost,
            depth: 1,
            stack: Some(vec![selector.to_string()]),
            ..Default::default()
        }
    }

    #[test]
    fn adapter_resolves_liquidation_call() {
        let adapter = AaveV3Adapter;
        let label = adapter.resolve_label(None, Some("0x00a718a9"));
        assert_eq!(label, Some("AaveV3Pool::liquidationCall".to_string()));
    }

    #[test]
    fn adapter_resolves_gho_mint() {
        let adapter = AaveV3Adapter;
        let label = adapter.resolve_label(None, Some("0x40c10f19"));
        assert_eq!(label, Some("GHO::mint".to_string()));
    }

    #[test]
    fn adapter_returns_none_for_unknown_selector() {
        let adapter = AaveV3Adapter;
        assert!(adapter.resolve_label(None, Some("0xdeadbeef")).is_none());
    }

    #[test]
    fn liquidation_report_detects_storage_ops() {
        let tracer = AaveDeepTracer::new();
        let steps = vec![
            make_call_step("SLOAD", "", 800),
            make_call_step("SLOAD", "", 800),
            make_call_step("SSTORE", "", 20_000),
            make_call_step("CALL", "0x00a718a9", 5_000),
        ];
        let report = tracer.analyze_liquidation("0xdeadbeef", &steps).unwrap();
        assert_eq!(report.storage_reads, 2);
        assert_eq!(report.storage_writes, 1);
        assert_eq!(report.external_calls, 1);
        assert!(!report.reverted);
    }

    #[test]
    fn gho_metrics_extraction() {
        let tracer = AaveDeepTracer::new();
        let steps = vec![
            make_call_step("CALL", "0x40c10f19", 5_000), // mint
            make_call_step("CALL", "0x40c10f19", 5_000), // mint
            make_call_step("CALL", "0x9dc29fac", 3_000), // burn
        ];
        let metrics = tracer.extract_gho_metrics(&steps);
        assert_eq!(metrics.mint_count, 2);
        assert_eq!(metrics.burn_count, 1);
    }
}
