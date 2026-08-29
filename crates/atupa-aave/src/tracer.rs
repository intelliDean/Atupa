//! [`AaveDeepTracer`] — main entry point for Aave v3 trace analysis.

use atupa_adapters::ProtocolAdapter;
use atupa_core::{DiffRow, ProtocolDiffReport, TraceStep};

use crate::adapter::AaveV3Adapter;
use crate::gho::{classify_gho_label, GhoSupplyMetrics};
use crate::report::{LiquidationAccumulator, LiquidationReport};
use crate::selectors::is_call_opcode;
use crate::selectors::selector_from_stack;

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
    /// [`ProtocolDiffReport`] with field-by-field deltas.
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
        Ok(ProtocolDiffReport {
            protocol: "Aave v3 / GHO".to_string(),
            rows: build_diff_rows(&base, &target, &base_gho, &target_gho),
        })
    }
}

// ─── Private helper ───────────────────────────────────────────────────────────

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
        DiffRow::new("Oracle Calls", base.oracle_calls as f64, target.oracle_calls as f64, true),
        DiffRow::new("Max Call Depth", base.max_depth as f64, target.max_depth as f64, true),
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
    use atupa_core::TraceStep;

    fn call_step(selector: &str, gas_cost: u64) -> TraceStep {
        TraceStep {
            op: "CALL".to_string(),
            gas_cost,
            depth: 1,
            stack: Some(vec![selector.to_string()]),
            ..Default::default()
        }
    }

    // ── analyze_liquidation ───────────────────────────────────────────────────

    #[test]
    fn detects_storage_ops_and_external_call() {
        let tracer = AaveDeepTracer::new();
        let steps = vec![
            TraceStep::evm("SLOAD", 800),
            TraceStep::evm("SLOAD", 800),
            TraceStep::evm("SSTORE", 20_000),
            call_step("0x00a718a9", 5_000), // liquidationCall
        ];
        let report = tracer.analyze_liquidation("0xdeadbeef", &steps).unwrap();
        assert_eq!(report.storage_reads, 2);
        assert_eq!(report.storage_writes, 1);
        assert_eq!(report.external_calls, 1);
        assert_eq!(report.total_gas, 800 + 800 + 20_000 + 5_000);
        assert!(!report.reverted);
    }

    #[test]
    fn counts_labeled_calls_in_order() {
        let tracer = AaveDeepTracer::new();
        let steps = vec![
            call_step("0x00a718a9", 5_000), // liquidationCall
            call_step("0x617ba037", 3_000), // supply
        ];
        let report = tracer.analyze_liquidation("0xabc", &steps).unwrap();
        assert_eq!(report.labeled_calls.len(), 2);
        assert_eq!(report.labeled_calls[0].label, "AaveV3Pool::liquidationCall");
        assert_eq!(report.labeled_calls[1].label, "AaveV3Pool::supply");
    }

    #[test]
    fn reverted_trace_sets_flag() {
        let tracer = AaveDeepTracer::new();
        let mut step = TraceStep::evm("REVERT", 0);
        step.reverted = true;
        let report = tracer.analyze_liquidation("0xabc", &[step]).unwrap();
        assert!(report.reverted);
    }

    // ── extract_gho_metrics ───────────────────────────────────────────────────

    #[test]
    fn extracts_mint_and_burn_counts() {
        let tracer = AaveDeepTracer::new();
        let steps = vec![
            call_step("0x40c10f19", 5_000), // mint
            call_step("0x40c10f19", 5_000), // mint
            call_step("0x9dc29fac", 3_000), // burn
        ];
        let metrics = tracer.extract_gho_metrics(&steps);
        assert_eq!(metrics.mint_count, 2);
        assert_eq!(metrics.burn_count, 1);
    }

    #[test]
    fn ignores_non_call_opcodes_for_gho_metrics() {
        let tracer = AaveDeepTracer::new();
        let steps = vec![TraceStep {
            op: "SLOAD".to_string(),
            stack: Some(vec!["0x40c10f19".to_string()]),
            ..Default::default()
        }];
        let metrics = tracer.extract_gho_metrics(&steps);
        assert_eq!(metrics.mint_count, 0);
    }

    // ── diff_reports ──────────────────────────────────────────────────────────

    #[test]
    fn diff_produces_10_rows_with_correct_protocol_name() {
        let tracer = AaveDeepTracer::new();
        let base = vec![TraceStep::evm("SLOAD", 800), TraceStep::evm("SSTORE", 20_000)];
        let target = vec![TraceStep::evm("SLOAD", 800)];
        let report = tracer.diff_reports("0xbase", &base, "0xtarget", &target).unwrap();
        assert_eq!(report.protocol, "Aave v3 / GHO");
        assert_eq!(report.rows.len(), 10);
    }

    #[test]
    fn diff_detects_storage_write_regression() {
        let tracer = AaveDeepTracer::new();
        let base = vec![TraceStep::evm("SSTORE", 20_000)];
        let target = vec![TraceStep::evm("SSTORE", 20_000), TraceStep::evm("SSTORE", 20_000)];
        let report = tracer.diff_reports("0xbase", &base, "0xtarget", &target).unwrap();
        let write_row = report.rows.iter().find(|r| r.metric == "Storage Writes (SSTORE)").unwrap();
        assert!(write_row.is_regression());
    }
}
