//! # atupa-aave — DeepTracer
//!
//! Aave v3 & GHO protocol adapter for the Atupa EVM profiling engine.
//!
//! Provides deep trace analysis for liquidation flows, supply/borrow mechanics,
//! and GHO stablecoin risk monitoring, producing structured [`LiquidationReport`]
//! and [`GhoSupplyMetrics`] from raw [`TraceStep`] slices.

use atupa_adapters::ProtocolAdapter;
use atupa_core::{DiffRow, ProtocolDiffReport, TraceStep};
use serde::{Deserialize, Serialize};

// ─── Selector & Address Tables ────────────────────────────────────────────────

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
    ("0xe8eda9df", "deposit"),           // v2 compatibility alias
    ("0xa9059cbb", "transfer"),           // ERC-20 — common inside traces
    ("0x23b872dd", "transferFrom"),       // ERC-20
    ("0x095ea7b3", "approve"),            // ERC-20
    ("0x1e9a6950", "setUserUseReserveAsCollateral"),
    ("0x02c205f0", "swapBorrowRateMode"),
    ("0x1e9d0e2e", "claimRewards"),
];

/// Known GHO-specific function selectors → human-readable labels.
const GHO_SELECTORS: &[(&str, &str)] = &[
    ("0x40c10f19", "mint"),
    ("0x9dc29fac", "burn"),
    ("0xd73dd623", "increaseAllowance"),
    ("0x5d3a1f9b", "distributeFeesToTreasury"),
    ("0x2e0f2625", "updateFacilitatorBucketCapacity"),
    ("0xdb5a3c5e", "setVariableDebtToken"),
];

/// Known GHO Facilitator addresses (Ethereum Mainnet).
const GHO_FACILITATORS: &[(&str, &str)] = &[
    (
        "0x5513224daaeabca31af5280727878d52097afa05",
        "Direct Minter (Aave V3)",
    ),
    (
        "0xbc65ad17c5c0a2a4d159fa5a503f4992c7b545fe",
        "Spark (Sky) Facilitator",
    ),
];

/// Known Aave oracle addresses (Ethereum Mainnet).
const AAVE_ORACLES: &[(&str, &str)] = &[
    (
        "0x54586be62e3c3580375ae3716c14bd2563060ca0",
        "Aave Price Oracle",
    ),
    (
        "0x3f12643d3f6f874d39c2a4c9f2cd6f2dbac877f",
        "GHO Price Oracle",
    ),
];

/// Gas cost baseline used to normalise the liquidation efficiency score.
const LIQUIDATION_EFFICIENCY_BASE: f64 = 100_000.0;

// ─── Protocol Adapter ────────────────────────────────────────────────────────

/// Aave v3 + GHO protocol adapter — maps addresses and 4-byte selectors to
/// human-readable labels for use in flamegraph annotation and deep-trace audits.
#[derive(Default)]
pub struct AaveV3Adapter;

impl ProtocolAdapter for AaveV3Adapter {
    fn name(&self) -> &str {
        "Aave v3 / GHO"
    }

    /// Resolve an optional contract address and/or 4-byte selector to a label.
    ///
    /// Resolution priority:
    /// 1. GHO Facilitator address → `"Facilitator::*"`
    /// 2. Aave Oracle address    → `"Oracle::*"`
    /// 3. Pool selector          → `"AaveV3Pool::*"`
    /// 4. GHO selector           → `"GHO::*"`
    fn resolve_label(&self, address: Option<&str>, selector: Option<&str>) -> Option<String> {
        if let Some(addr) = address
            && let Some(label) = resolve_address(addr)
        {
            return Some(label);
        }
        selector.and_then(resolve_selector)
    }
}

impl AaveV3Adapter {
    /// Resolve a 4-byte selector string to a human-readable label.
    ///
    /// Returns `None` if the selector is not recognised by either the Pool or
    /// GHO selector tables.
    pub fn resolve_selector_label(selector: &str) -> Option<String> {
        resolve_selector(selector)
    }
}

// ─── Private lookup helpers ───────────────────────────────────────────────────

/// Look up a contract address in the known facilitator and oracle tables.
///
/// Comparison is case-insensitive (all stored addresses are lowercase).
fn resolve_address(addr: &str) -> Option<String> {
    let lower = addr.to_lowercase();

    for &(known, name) in GHO_FACILITATORS {
        if lower == known {
            return Some(format!("Facilitator::{name}"));
        }
    }
    for &(known, name) in AAVE_ORACLES {
        if lower == known {
            return Some(format!("Oracle::{name}"));
        }
    }
    None
}

/// Look up a 4-byte selector in the Pool and GHO selector tables.
fn resolve_selector(selector: &str) -> Option<String> {
    for &(known, label) in POOL_SELECTORS {
        if selector == known {
            return Some(format!("AaveV3Pool::{label}"));
        }
    }
    for &(known, label) in GHO_SELECTORS {
        if selector == known {
            return Some(format!("GHO::{label}"));
        }
    }
    None
}

/// Returns `true` for EVM opcodes that initiate a new call frame.
#[inline]
fn is_call_opcode(op: &str) -> bool {
    matches!(op, "CALL" | "STATICCALL" | "DELEGATECALL" | "CALLCODE")
}

// ─── Report Structures ────────────────────────────────────────────────────────

/// A human-readable breakdown of a single `liquidationCall` execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LiquidationReport {
    /// Transaction hash being analyzed.
    pub tx_hash: String,
    /// Total gas consumed across all steps.
    pub total_gas: u64,
    /// Gas consumed after the first `liquidationCall` opcode was seen.
    ///
    /// > **Approximation**: this is a heuristic — all gas from the first
    /// > `liquidationCall` label onwards is attributed to the liquidation.
    /// > A depth-tracking approach would be more precise.
    pub liquidation_gas: u64,
    /// Number of `SLOAD` opcodes (proxy for oracle / state lookups).
    pub storage_reads: u32,
    /// Number of `SSTORE` opcodes.
    pub storage_writes: u32,
    /// Number of cross-contract call opcodes.
    pub external_calls: u32,
    /// Whether the transaction reverted.
    pub reverted: bool,
    /// Maximum call-stack depth reached.
    pub max_depth: u16,
    /// Liquidation efficiency score: `liquidation_gas / LIQUIDATION_EFFICIENCY_BASE`.
    ///
    /// A lower value indicates a more gas-efficient liquidation. Only meaningful
    /// when `liquidation_gas > 0`.
    pub liquidation_efficiency: f64,
    /// Number of oracle contract calls identified in the trace.
    pub oracle_calls: u32,
    /// Ordered sequence of labeled calls extracted from the trace.
    pub labeled_calls: Vec<LabeledCall>,
}

impl LiquidationReport {
    /// Returns a concise one-line summary for terminal output.
    pub fn summary(&self) -> String {
        // Use get(..10) to avoid panicking on short/synthetic hashes.
        let short_hash = self.tx_hash.get(..10).unwrap_or(&self.tx_hash);
        format!(
            "[LiquidationReport] tx={} gas={} reads={} writes={} calls={} reverted={}",
            short_hash,
            self.total_gas,
            self.storage_reads,
            self.storage_writes,
            self.external_calls,
            self.reverted,
        )
    }
}

/// A single labeled call extracted during trace analysis.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LabeledCall {
    pub depth: u16,
    pub label: String,
    pub gas_cost: u64,
}

// ─── GHO Supply Metrics ───────────────────────────────────────────────────────

/// Aggregated GHO supply-level metrics extracted from trace steps.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
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

// ─── DeepTracer ───────────────────────────────────────────────────────────────

/// The main Aave DeepTracer — wraps [`AaveV3Adapter`] and provides higher-level
/// analysis methods over raw [`TraceStep`] slices.
#[derive(Default)]
pub struct AaveDeepTracer {
    adapter: AaveV3Adapter,
}

impl AaveDeepTracer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Analyze a raw trace and produce a [`LiquidationReport`].
    pub fn analyze_liquidation(
        &self,
        tx_hash: &str,
        steps: &[TraceStep],
    ) -> anyhow::Result<LiquidationReport> {
        let mut acc = LiquidationAccumulator::default();

        for step in steps {
            acc.process_step(step, &self.adapter);
        }

        let reverted = steps.last().is_some_and(|s| s.reverted);
        Ok(acc.into_report(tx_hash, reverted))
    }

    /// Scan a trace for GHO supply-level signals.
    pub fn extract_gho_metrics(&self, steps: &[TraceStep]) -> GhoSupplyMetrics {
        let mut metrics = GhoSupplyMetrics::default();

        for step in steps.iter().filter(|s| is_call_opcode(&s.op)) {
            let selector = selector_from_stack(step);
            if let Some(label) = self.adapter.resolve_label(None, selector) {
                classify_gho_label(&label, &mut metrics);
            }
        }

        metrics
    }

    /// Compare two traces with full Aave protocol analysis and return a
    /// [`ProtocolDiffReport`] containing field-by-field deltas.
    pub fn diff_reports(
        &self,
        base_hash: &str,
        base_steps: &[TraceStep],
        target_hash: &str,
        target_steps: &[TraceStep],
    ) -> anyhow::Result<ProtocolDiffReport> {
        let base = self.analyze_liquidation(base_hash, base_steps)?;
        let target = self.analyze_liquidation(target_hash, target_steps)?;

        let base_gho = self.extract_gho_metrics(base_steps);
        let target_gho = self.extract_gho_metrics(target_steps);

        let rows = build_diff_rows(&base, &target, &base_gho, &target_gho);
        Ok(ProtocolDiffReport {
            protocol: "Aave v3 / GHO".to_string(),
            rows,
        })
    }
}

// ─── Private accumulator ──────────────────────────────────────────────────────

/// Internal accumulator that builds a [`LiquidationReport`] step-by-step.
///
/// Extracted from `analyze_liquidation` to keep the public method lean and the
/// per-step logic unit-testable.
#[derive(Default)]
struct LiquidationAccumulator {
    storage_reads: u32,
    storage_writes: u32,
    external_calls: u32,
    oracle_calls: u32,
    max_depth: u16,
    total_gas: u64,
    liquidation_gas: u64,
    in_liquidation: bool,
    labeled_calls: Vec<LabeledCall>,
}

impl LiquidationAccumulator {
    fn process_step(&mut self, step: &TraceStep, adapter: &AaveV3Adapter) {
        self.total_gas = self.total_gas.saturating_add(step.gas_cost);
        self.max_depth = self.max_depth.max(step.depth);

        match step.op.as_str() {
            "SLOAD" => self.storage_reads += 1,
            "SSTORE" => self.storage_writes += 1,
            op if is_call_opcode(op) => self.process_call_step(step, adapter),
            _ => {}
        }

        if self.in_liquidation {
            self.liquidation_gas = self.liquidation_gas.saturating_add(step.gas_cost);
        }
    }

    fn process_call_step(&mut self, step: &TraceStep, adapter: &AaveV3Adapter) {
        self.external_calls += 1;

        let selector = selector_from_stack(step);
        // Address resolution from the stack is complex in a real trace;
        // we pass None here as a known simplification (POC).
        let Some(label) = adapter.resolve_label(None, selector) else {
            return;
        };

        if label.contains("liquidationCall") {
            self.in_liquidation = true;
        }
        if label.contains("Oracle") {
            self.oracle_calls += 1;
        }

        self.labeled_calls.push(LabeledCall {
            depth: step.depth,
            label,
            gas_cost: step.gas_cost,
        });
    }

    fn into_report(self, tx_hash: &str, reverted: bool) -> LiquidationReport {
        let liquidation_efficiency = if self.liquidation_gas > 0 {
            self.liquidation_gas as f64 / LIQUIDATION_EFFICIENCY_BASE
        } else {
            0.0
        };

        LiquidationReport {
            tx_hash: tx_hash.to_string(),
            total_gas: self.total_gas,
            liquidation_gas: self.liquidation_gas,
            storage_reads: self.storage_reads,
            storage_writes: self.storage_writes,
            external_calls: self.external_calls,
            oracle_calls: self.oracle_calls,
            reverted,
            max_depth: self.max_depth,
            liquidation_efficiency,
            labeled_calls: self.labeled_calls,
        }
    }
}

// ─── Private helpers ──────────────────────────────────────────────────────────

/// Extract the top-of-stack value from a [`TraceStep`] as a selector string.
fn selector_from_stack(step: &TraceStep) -> Option<&str> {
    step.stack.as_ref()?.last().map(String::as_str)
}

/// Update [`GhoSupplyMetrics`] for a single recognized GHO label.
fn classify_gho_label(label: &str, metrics: &mut GhoSupplyMetrics) {
    match label {
        "GHO::mint" => metrics.mint_count += 1,
        "GHO::burn" => metrics.burn_count += 1,
        "GHO::updateFacilitatorBucketCapacity" => metrics.bucket_capacity_updates += 1,
        "GHO::distributeFeesToTreasury" => metrics.fee_distributions += 1,
        _ => {}
    }
}

/// Construct the ordered list of [`DiffRow`]s for a protocol diff report.
fn build_diff_rows(
    base: &LiquidationReport,
    target: &LiquidationReport,
    base_gho: &GhoSupplyMetrics,
    target_gho: &GhoSupplyMetrics,
) -> Vec<DiffRow> {
    vec![
        DiffRow::new("Total Gas", base.total_gas as f64, target.total_gas as f64, true),
        DiffRow::new(
            "Liquidation Gas",
            base.liquidation_gas as f64,
            target.liquidation_gas as f64,
            true,
        ),
        DiffRow::new(
            "Storage Reads (SLOAD)",
            base.storage_reads as f64,
            target.storage_reads as f64,
            true,
        ),
        DiffRow::new(
            "Storage Writes (SSTORE)",
            base.storage_writes as f64,
            target.storage_writes as f64,
            true,
        ),
        DiffRow::new(
            "External Calls",
            base.external_calls as f64,
            target.external_calls as f64,
            true,
        ),
        DiffRow::new(
            "Oracle Calls",
            base.oracle_calls as f64,
            target.oracle_calls as f64,
            true,
        ),
        DiffRow::new(
            "Max Call Depth",
            base.max_depth as f64,
            target.max_depth as f64,
            true,
        ),
        DiffRow::new(
            "Liq. Efficiency",
            base.liquidation_efficiency,
            target.liquidation_efficiency,
            true,
        ),
        DiffRow::new(
            "GHO Mint Count",
            base_gho.mint_count as f64,
            target_gho.mint_count as f64,
            false,
        ),
        DiffRow::new(
            "GHO Burn Count",
            base_gho.burn_count as f64,
            target_gho.burn_count as f64,
            false,
        ),
    ]
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Fixture helpers ───────────────────────────────────────────────────────

    /// Build a simple call-opcode step with a single selector on the stack.
    fn call_step(op: &str, selector: &str, gas_cost: u64) -> TraceStep {
        TraceStep {
            op: op.to_string(),
            gas: 1_000_000,
            gas_cost,
            depth: 1,
            stack: Some(vec![selector.to_string()]),
            ..Default::default()
        }
    }

    fn sload(gas_cost: u64) -> TraceStep {
        TraceStep::evm("SLOAD", gas_cost)
    }

    fn sstore(gas_cost: u64) -> TraceStep {
        TraceStep::evm("SSTORE", gas_cost)
    }

    // ── AaveV3Adapter ─────────────────────────────────────────────────────────

    #[test]
    fn adapter_resolves_liquidation_call() {
        let adapter = AaveV3Adapter;
        assert_eq!(
            adapter.resolve_label(None, Some("0x00a718a9")),
            Some("AaveV3Pool::liquidationCall".to_string())
        );
    }

    #[test]
    fn adapter_resolves_gho_mint() {
        let adapter = AaveV3Adapter;
        assert_eq!(
            adapter.resolve_label(None, Some("0x40c10f19")),
            Some("GHO::mint".to_string())
        );
    }

    #[test]
    fn adapter_returns_none_for_unknown_selector() {
        let adapter = AaveV3Adapter;
        assert!(adapter.resolve_label(None, Some("0xdeadbeef")).is_none());
        assert!(adapter.resolve_label(None, None).is_none());
    }

    #[test]
    fn adapter_resolves_facilitator_address() {
        let adapter = AaveV3Adapter;
        // Case-insensitive — mix of upper and lowercase
        let label = adapter.resolve_label(
            Some("0x5513224daaEABCa31af5280727878d52097afA05"),
            None,
        );
        assert_eq!(label, Some("Facilitator::Direct Minter (Aave V3)".to_string()));
    }

    #[test]
    fn adapter_resolves_oracle_address() {
        let adapter = AaveV3Adapter;
        let label = adapter.resolve_label(
            Some("0x54586bE62E3c3580375aE3716C14bd2563060Ca0"),
            None,
        );
        assert_eq!(label, Some("Oracle::Aave Price Oracle".to_string()));
    }

    #[test]
    fn resolve_selector_label_pool_and_gho() {
        assert_eq!(
            AaveV3Adapter::resolve_selector_label("0x617ba037"),
            Some("AaveV3Pool::supply".to_string())
        );
        assert_eq!(
            AaveV3Adapter::resolve_selector_label("0x9dc29fac"),
            Some("GHO::burn".to_string())
        );
        assert!(AaveV3Adapter::resolve_selector_label("0xdeadbeef").is_none());
    }

    // ── analyze_liquidation ───────────────────────────────────────────────────

    #[test]
    fn liquidation_report_detects_storage_ops() {
        let tracer = AaveDeepTracer::new();
        let steps = vec![
            sload(800),
            sload(800),
            sstore(20_000),
            call_step("CALL", "0x00a718a9", 5_000), // liquidationCall
        ];
        let report = tracer.analyze_liquidation("0xdeadbeef", &steps).unwrap();
        assert_eq!(report.storage_reads, 2);
        assert_eq!(report.storage_writes, 1);
        assert_eq!(report.external_calls, 1);
        assert!(!report.reverted);
        assert_eq!(report.total_gas, 800 + 800 + 20_000 + 5_000);
    }

    #[test]
    fn liquidation_report_counts_labeled_calls() {
        let tracer = AaveDeepTracer::new();
        let steps = vec![
            call_step("CALL", "0x00a718a9", 5_000), // liquidationCall
            call_step("CALL", "0x617ba037", 3_000), // supply
        ];
        let report = tracer.analyze_liquidation("0xabc", &steps).unwrap();
        assert_eq!(report.labeled_calls.len(), 2);
        assert_eq!(report.labeled_calls[0].label, "AaveV3Pool::liquidationCall");
        assert_eq!(report.labeled_calls[1].label, "AaveV3Pool::supply");
    }

    #[test]
    fn liquidation_report_summary_safe_on_short_hash() {
        let tracer = AaveDeepTracer::new();
        let report = tracer.analyze_liquidation("0xab", &[]).unwrap();
        // Must not panic on a hash shorter than 10 chars
        let summary = report.summary();
        assert!(summary.contains("0xab"));
    }

    #[test]
    fn liquidation_efficiency_is_zero_when_no_liquidation_gas() {
        let tracer = AaveDeepTracer::new();
        let steps = vec![sload(100)];
        let report = tracer.analyze_liquidation("0xabc123", &steps).unwrap();
        assert_eq!(report.liquidation_efficiency, 0.0);
    }

    #[test]
    fn reverted_trace_sets_reverted_flag() {
        let tracer = AaveDeepTracer::new();
        let mut step = TraceStep::evm("REVERT", 0);
        step.reverted = true;
        let report = tracer.analyze_liquidation("0xabc", &[step]).unwrap();
        assert!(report.reverted);
    }

    // ── extract_gho_metrics ───────────────────────────────────────────────────

    #[test]
    fn gho_metrics_extraction() {
        let tracer = AaveDeepTracer::new();
        let steps = vec![
            call_step("CALL", "0x40c10f19", 5_000), // mint
            call_step("CALL", "0x40c10f19", 5_000), // mint
            call_step("CALL", "0x9dc29fac", 3_000), // burn
        ];
        let metrics = tracer.extract_gho_metrics(&steps);
        assert_eq!(metrics.mint_count, 2);
        assert_eq!(metrics.burn_count, 1);
        assert_eq!(metrics.bucket_capacity_updates, 0);
        assert_eq!(metrics.fee_distributions, 0);
    }

    #[test]
    fn gho_metrics_ignores_non_call_opcodes() {
        let tracer = AaveDeepTracer::new();
        let steps = vec![
            // SLOAD with a GHO mint selector on the stack — should NOT be counted
            TraceStep {
                op: "SLOAD".to_string(),
                stack: Some(vec!["0x40c10f19".to_string()]),
                ..Default::default()
            },
        ];
        let metrics = tracer.extract_gho_metrics(&steps);
        assert_eq!(metrics.mint_count, 0);
    }

    // ── diff_reports ──────────────────────────────────────────────────────────

    #[test]
    fn diff_reports_produces_correct_row_count() {
        let tracer = AaveDeepTracer::new();
        let base = vec![sload(800), sstore(20_000)];
        let target = vec![sload(800)];
        let report = tracer.diff_reports("0xbase", &base, "0xtarget", &target).unwrap();
        assert_eq!(report.protocol, "Aave v3 / GHO");
        assert_eq!(report.rows.len(), 10);
    }

    #[test]
    fn diff_reports_detects_storage_write_regression() {
        let tracer = AaveDeepTracer::new();
        let base = vec![sstore(20_000)];
        let target = vec![sstore(20_000), sstore(20_000)]; // extra write = regression
        let report = tracer.diff_reports("0xbase", &base, "0xtarget", &target).unwrap();
        let write_row = report.rows.iter().find(|r| r.metric == "Storage Writes (SSTORE)").unwrap();
        assert!(write_row.is_regression());
    }
}
