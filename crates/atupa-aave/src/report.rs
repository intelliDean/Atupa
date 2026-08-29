//! [`LiquidationReport`], [`LabeledCall`], and the [`LiquidationAccumulator`]
//! that builds a report by processing trace steps one at a time.

use atupa_adapters::ProtocolAdapter;
use atupa_core::TraceStep;
use serde::{Deserialize, Serialize};

use crate::adapter::AaveV3Adapter;
use crate::selectors::{LIQUIDATION_EFFICIENCY_BASE, is_call_opcode, selector_from_stack};

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
    /// > **Approximation**: all gas from the first `liquidationCall` label
    /// > onwards is attributed to the liquidation. A depth-tracking approach
    /// > would be more precise.
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
        // Use .get(..10) to avoid panicking on short/synthetic hashes.
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

// ─── Accumulator ─────────────────────────────────────────────────────────────

/// Internal mutable accumulator used by [`crate::tracer::AaveDeepTracer`] to
/// build a [`LiquidationReport`] by processing trace steps one at a time.
///
/// Separating the accumulation state from the public API keeps
/// `analyze_liquidation` concise and the per-step logic independently testable.
#[derive(Default)]
pub(crate) struct LiquidationAccumulator {
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
    /// Incorporate a single trace step into the running totals.
    pub(crate) fn process_step(&mut self, step: &TraceStep, adapter: &AaveV3Adapter) {
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

    /// Process a call-opcode step: resolve its label and update relevant counters.
    ///
    /// # Note on address resolution
    ///
    /// In a real EVM trace the callee address sits on the stack at a
    /// well-known offset, but extracting it reliably requires full stack
    /// reconstruction which is beyond the current POC scope. We therefore pass
    /// `None` for the address and rely solely on the selector.
    fn process_call_step(&mut self, step: &TraceStep, adapter: &AaveV3Adapter) {
        self.external_calls += 1;

        let selector = selector_from_stack(step);
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

    /// Consume the accumulator and produce the final [`LiquidationReport`].
    pub(crate) fn into_report(self, tx_hash: &str, reverted: bool) -> LiquidationReport {
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

    #[test]
    fn accumulates_storage_reads_and_writes() {
        let adapter = AaveV3Adapter;
        let mut acc = LiquidationAccumulator::default();
        acc.process_step(&TraceStep::evm("SLOAD", 800), &adapter);
        acc.process_step(&TraceStep::evm("SLOAD", 800), &adapter);
        acc.process_step(&TraceStep::evm("SSTORE", 20_000), &adapter);
        let report = acc.into_report("0xabc", false);
        assert_eq!(report.storage_reads, 2);
        assert_eq!(report.storage_writes, 1);
        assert_eq!(report.total_gas, 21_600);
    }

    #[test]
    fn labels_liquidation_call_and_flips_in_liquidation() {
        let adapter = AaveV3Adapter;
        let mut acc = LiquidationAccumulator::default();
        acc.process_step(&call_step("0x00a718a9", 5_000), &adapter); // liquidationCall
        let report = acc.into_report("0xabc", false);
        assert_eq!(report.labeled_calls.len(), 1);
        assert_eq!(report.labeled_calls[0].label, "AaveV3Pool::liquidationCall");
        // All gas after the liquidationCall step is attributed
        assert!(report.liquidation_gas > 0);
    }

    #[test]
    fn efficiency_is_zero_without_liquidation_gas() {
        let acc = LiquidationAccumulator::default();
        let report = acc.into_report("0xabc", false);
        assert_eq!(report.liquidation_efficiency, 0.0);
    }

    #[test]
    fn summary_is_safe_on_short_hash() {
        let acc = LiquidationAccumulator::default();
        let report = acc.into_report("0x1", false);
        // Must not panic
        let s = report.summary();
        assert!(s.contains("0x1"));
    }

    #[test]
    fn reverted_flag_propagated() {
        let acc = LiquidationAccumulator::default();
        let report = acc.into_report("0xabc", true);
        assert!(report.reverted);
    }
}
