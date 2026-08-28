//! [`LidoDeepTracer`] — main entry point for Lido stETH trace analysis.

use atupa_core::{DiffRow, ProtocolDiffReport, TraceStep};

use crate::adapter::LidoAdapter;
use crate::report::{LidoAccumulator, LidoReport};

/// High-level analysis engine for Lido stETH liquid staking traces.
#[derive(Debug, Default, Clone)]
pub struct LidoDeepTracer {
    adapter: LidoAdapter,
}

impl LidoDeepTracer {
    /// Creates a new [`LidoDeepTracer`].
    pub fn new() -> Self {
        Self {
            adapter: LidoAdapter,
        }
    }

    /// Analyze a sequence of execution trace steps for Lido-specific patterns.
    pub fn analyze_staking(
        &self,
        tx_hash: &str,
        steps: &[TraceStep],
    ) -> anyhow::Result<LidoReport> {
        let mut accumulator = LidoAccumulator::default();
        for step in steps {
            accumulator.process_step(step, &self.adapter);
        }

        let reverted = steps.last().is_some_and(|s| s.reverted);
        Ok(accumulator.into_report(tx_hash, reverted))
    }

    /// Perform a deep field-by-field diff between two Lido executions.
    pub fn diff_reports(
        &self,
        base_tx: &str,
        base_steps: &[TraceStep],
        target_tx: &str,
        target_steps: &[TraceStep],
    ) -> anyhow::Result<ProtocolDiffReport> {
        let base = self.analyze_staking(base_tx, base_steps)?;
        let target = self.analyze_staking(target_tx, target_steps)?;

        Ok(ProtocolDiffReport {
            protocol: "Lido stETH".to_string(),
            rows: build_diff_rows(&base, &target),
        })
    }
}

// ─── Private helpers ──────────────────────────────────────────────────────────

/// Construct the ordered list of [`DiffRow`]s for a Lido protocol comparison.
fn build_diff_rows(base: &LidoReport, target: &LidoReport) -> Vec<DiffRow> {
    vec![
        DiffRow::new("Total Gas", base.total_gas as f64, target.total_gas as f64, true),
        DiffRow::new(
            "Storage Reads",
            base.storage_reads as f64,
            target.storage_reads as f64,
            true,
        ),
        DiffRow::new(
            "Storage Writes",
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
            "Shares Transfers",
            base.shares_transfers as f64,
            target.shares_transfers as f64,
            true,
        ),
        DiffRow::new(
            "Oracle Reports",
            base.oracle_reports as f64,
            target.oracle_reports as f64,
            true,
        ),
        DiffRow::new(
            "Withdrawal Requests",
            base.withdrawal_requests as f64,
            target.withdrawal_requests as f64,
            true,
        ),
        DiffRow::new(
            "Withdrawal Claims",
            base.withdrawal_claims as f64,
            target.withdrawal_claims as f64,
            true,
        ),
        DiffRow::new(
            "Wrapped Ops",
            base.wrapped_ops as f64,
            target.wrapped_ops as f64,
            true,
        ),
    ]
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn call_step(selector: &str, gas_cost: u64) -> TraceStep {
        TraceStep {
            op: "CALL".to_string(),
            gas_cost,
            depth: 1,
            stack: Some(vec![selector.to_string()]),
            ..Default::default()
        }
    }

    #[test]
    fn analyze_staking_produces_valid_report() {
        let tracer = LidoDeepTracer::new();
        let steps = vec![
            TraceStep::evm("SLOAD", 800),
            TraceStep::evm("SSTORE", 20_000),
            call_step("0xa1903eab", 5_000), // submit
        ];

        let report = tracer.analyze_staking("0x123", &steps).unwrap();
        assert_eq!(report.tx_hash, "0x123");
        assert_eq!(report.total_gas, 25_800);
        assert_eq!(report.storage_reads, 1);
        assert_eq!(report.storage_writes, 1);
        assert_eq!(report.external_calls, 1);
        assert!(!report.reverted);
    }

    #[test]
    fn analyze_staking_propagates_revert() {
        let tracer = LidoDeepTracer::new();
        let mut step = TraceStep::evm("REVERT", 0);
        step.reverted = true;
        let report = tracer.analyze_staking("0x123", &[step]).unwrap();
        assert!(report.reverted);
    }

    #[test]
    fn diff_reports_produces_nine_rows() {
        let tracer = LidoDeepTracer::new();
        let base = vec![TraceStep::evm("SLOAD", 800), TraceStep::evm("SSTORE", 20_000)];
        let target = vec![TraceStep::evm("SLOAD", 800)];

        let report = tracer.diff_reports("0xbase", &base, "0xtarget", &target).unwrap();
        assert_eq!(report.protocol, "Lido stETH");
        assert_eq!(report.rows.len(), 9);
    }

    #[test]
    fn diff_reports_identifies_regression() {
        let tracer = LidoDeepTracer::new();
        let base = vec![TraceStep::evm("SSTORE", 20_000)];
        let target = vec![
            TraceStep::evm("SSTORE", 20_000),
            TraceStep::evm("SSTORE", 20_000),
        ];

        let report = tracer.diff_reports("0xbase", &base, "0xtarget", &target).unwrap();
        let write_row = report
            .rows
            .iter()
            .find(|r| r.metric == "Storage Writes")
            .unwrap();
        assert!(write_row.is_regression());
    }
}
