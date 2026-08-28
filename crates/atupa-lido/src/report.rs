//! [`LidoReport`], [`LabeledCall`], and [`LidoAccumulator`] for Lido execution traces.

use atupa_adapters::ProtocolAdapter;
use atupa_core::TraceStep;
use serde::{Deserialize, Serialize};

use crate::adapter::LidoAdapter;
use crate::selectors::{is_call_opcode, selector_from_stack};

/// Detailed metrics and audit signals extracted from a Lido protocol interaction.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LidoReport {
    /// Transaction hash being analyzed.
    pub tx_hash: String,
    /// Total gas consumed across all steps.
    pub total_gas: u64,
    /// Number of `SLOAD` opcodes (state reads).
    pub storage_reads: u32,
    /// Number of `SSTORE` opcodes (state modifications).
    pub storage_writes: u32,
    /// Number of cross-contract call opcodes.
    pub external_calls: u32,
    /// Number of `transferShares` / `transferSharesFrom` calls observed.
    pub shares_transfers: u32,
    /// Number of oracle consensus reports (`handleOracleReport`).
    pub oracle_reports: u32,
    /// Number of withdrawal requests (`requestWithdrawals`).
    pub withdrawal_requests: u32,
    /// Number of withdrawal claims (`claimWithdrawals`).
    pub withdrawal_claims: u32,
    /// Number of wstETH `wrap` or `unwrap` operations.
    pub wrapped_ops: u32,
    /// Maximum call stack depth reached.
    pub max_depth: u16,
    /// Whether the transaction reverted.
    pub reverted: bool,
    /// Deduplicated list of labeled calls extracted from the trace.
    pub labeled_calls: Vec<LabeledCall>,
}

impl LidoReport {
    /// Returns a concise one-line summary of the report.
    pub fn summary(&self) -> String {
        let short_hash = self.tx_hash.get(..10).unwrap_or(&self.tx_hash);
        format!(
            "[LidoReport] tx={} gas={} reads={} writes={} calls={} shares_tx={} oracle_rpt={} reverted={}",
            short_hash,
            self.total_gas,
            self.storage_reads,
            self.storage_writes,
            self.external_calls,
            self.shares_transfers,
            self.oracle_reports,
            self.reverted,
        )
    }
}

/// A single labeled call extracted during trace analysis.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LabeledCall {
    /// Call depth at which this call occurred.
    pub depth: u16,
    /// Human-readable label for the call.
    pub label: String,
    /// Gas cost of this step.
    pub gas_cost: u64,
}

// ─── Internal Accumulator ─────────────────────────────────────────────────────

/// Internal accumulator that folds trace steps into a [`LidoReport`].
#[derive(Default)]
pub(crate) struct LidoAccumulator {
    total_gas: u64,
    storage_reads: u32,
    storage_writes: u32,
    external_calls: u32,
    shares_transfers: u32,
    oracle_reports: u32,
    withdrawal_requests: u32,
    withdrawal_claims: u32,
    wrapped_ops: u32,
    max_depth: u16,
    labeled_calls: Vec<LabeledCall>,
}

impl LidoAccumulator {
    /// Incorporate a single trace step into the accumulator state.
    pub(crate) fn process_step(&mut self, step: &TraceStep, adapter: &LidoAdapter) {
        self.total_gas = self.total_gas.saturating_add(step.gas_cost);
        self.max_depth = self.max_depth.max(step.depth);

        match step.op.as_str() {
            "SLOAD" => self.storage_reads += 1,
            "SSTORE" => self.storage_writes += 1,
            op if is_call_opcode(op) => self.process_call_step(step, adapter),
            _ => {}
        }
    }

    fn process_call_step(&mut self, step: &TraceStep, adapter: &LidoAdapter) {
        self.external_calls += 1;

        let selector = selector_from_stack(step);
        if let Some(label) = adapter.resolve_label(None, selector) {
            if label.contains("transferShares") {
                self.shares_transfers += 1;
            } else if label.contains("handleOracleReport") {
                self.oracle_reports += 1;
            } else if label.contains("requestWithdrawals") {
                self.withdrawal_requests += 1;
            } else if label.contains("claimWithdrawals") {
                self.withdrawal_claims += 1;
            } else if label.contains("wrap") || label.contains("unwrap") {
                self.wrapped_ops += 1;
            }

            self.labeled_calls.push(LabeledCall {
                depth: step.depth,
                label,
                gas_cost: step.gas_cost,
            });
        }
    }

    /// Produce the final [`LidoReport`].
    pub(crate) fn into_report(mut self, tx_hash: &str, reverted: bool) -> LidoReport {
        self.labeled_calls
            .dedup_by(|a, b| a.label == b.label && a.depth == b.depth);

        LidoReport {
            tx_hash: tx_hash.to_string(),
            total_gas: self.total_gas,
            storage_reads: self.storage_reads,
            storage_writes: self.storage_writes,
            external_calls: self.external_calls,
            shares_transfers: self.shares_transfers,
            oracle_reports: self.oracle_reports,
            withdrawal_requests: self.withdrawal_requests,
            withdrawal_claims: self.withdrawal_claims,
            wrapped_ops: self.wrapped_ops,
            max_depth: self.max_depth,
            reverted,
            labeled_calls: self.labeled_calls,
        }
    }
}

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
    fn accumulator_tracks_storage_and_calls() {
        let adapter = LidoAdapter;
        let mut acc = LidoAccumulator::default();
        acc.process_step(&TraceStep::evm("SLOAD", 800), &adapter);
        acc.process_step(&TraceStep::evm("SSTORE", 20_000), &adapter);
        acc.process_step(&call_step("0xa1903eab", 5_000), &adapter); // submit

        let report = acc.into_report("0x1234567890abcdef", false);
        assert_eq!(report.storage_reads, 1);
        assert_eq!(report.storage_writes, 1);
        assert_eq!(report.external_calls, 1);
        assert_eq!(report.total_gas, 25_800);
        assert_eq!(report.labeled_calls.len(), 1);
        assert_eq!(report.labeled_calls[0].label, "stETH::submit");
    }

    #[test]
    fn summary_formatting_safe_on_short_hash() {
        let acc = LidoAccumulator::default();
        let report = acc.into_report("0x1", false);
        let summary = report.summary();
        assert!(summary.contains("0x1"));
    }

    #[test]
    fn tracks_specialized_lido_operations() {
        let adapter = LidoAdapter;
        let mut acc = LidoAccumulator::default();
        acc.process_step(&call_step("0x39ba163b", 1_000), &adapter); // transferShares
        acc.process_step(&call_step("0x8b6ca260", 2_000), &adapter); // handleOracleReport
        acc.process_step(&call_step("0xea598cb0", 3_000), &adapter); // requestWithdrawals
        acc.process_step(&call_step("0xe35ea9a5", 4_000), &adapter); // claimWithdrawals
        acc.process_step(&call_step("0x0a19ea81", 5_000), &adapter); // wrap

        let report = acc.into_report("0xabcdef", false);
        assert_eq!(report.shares_transfers, 1);
        assert_eq!(report.oracle_reports, 1);
        assert_eq!(report.withdrawal_requests, 1);
        assert_eq!(report.withdrawal_claims, 1);
        assert_eq!(report.wrapped_ops, 1);
    }
}
