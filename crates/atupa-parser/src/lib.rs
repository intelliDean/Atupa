pub mod aggregator;

use atupa_core::{TraceStep, VmKind};
use atupa_rpc::RawStructLog;

pub struct Parser;

impl Parser {
    /// Normalizes a raw Anvil/Geth structLog into our universal TraceStep schema.
    pub fn normalize(raw_logs: Vec<RawStructLog>) -> Vec<TraceStep> {
        raw_logs
            .into_iter()
            .map(|log| {
                let reverted = log.error.is_some() || log.op == "REVERT" || log.op == "INVALID";
                TraceStep {
                    pc: log.pc,
                    op: log.op,
                    gas: log.gas,
                    gas_cost: log.gas_cost,
                    depth: log.depth,
                    stack: log.stack,
                    memory: log.memory,
                    error: log.error,
                    reverted,
                    vm_kind: VmKind::Evm,
                }
            })
            .collect()
    }

    /// Pass-through for steps that are already normalized (e.g. the unified
    /// `UnifiedStep` timeline from `atupa-nitro`). Applies the same revert
    /// detection logic so the Aggregator sees consistent flags.
    pub fn normalize_raw(steps: Vec<TraceStep>) -> Vec<TraceStep> {
        steps
            .into_iter()
            .map(|mut step| {
                if step.error.is_some() || step.op == "REVERT" || step.op == "INVALID" {
                    step.reverted = true;
                }
                step
            })
            .collect()
    }
}
