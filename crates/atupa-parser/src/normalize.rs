//! Trace step normalization from raw RPC structLogs to universal [`TraceStep`] schema.

use atupa_core::{TraceStep, VmKind};
use atupa_rpc::RawStructLog;

/// Normalizes raw execution trace logs into the universal [`TraceStep`] representation.
pub struct Parser;

impl Parser {
    /// Normalizes a raw Anvil/Geth `structLog` list into the universal [`TraceStep`] schema.
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

    /// Pass-through normalization for steps that are already in `TraceStep` format
    /// (e.g. from `atupa-nitro`, `atupa-solana`, etc.).
    ///
    /// Ensures consistent error/revert flag propagation.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_raw_struct_log() {
        let raw = vec![
            RawStructLog {
                pc: 0,
                op: "PUSH1".to_string(),
                gas: 100,
                gas_cost: 3,
                depth: 1,
                error: None,
                stack: None,
                memory: None,
                storage: None,
            },
            RawStructLog {
                pc: 2,
                op: "REVERT".to_string(),
                gas: 97,
                gas_cost: 0,
                depth: 1,
                error: Some("execution reverted".to_string()),
                stack: None,
                memory: None,
                storage: None,
            },
        ];

        let normalized = Parser::normalize(raw);
        assert_eq!(normalized.len(), 2);
        assert_eq!(normalized[0].op, "PUSH1");
        assert!(!normalized[0].reverted);
        assert_eq!(normalized[1].op, "REVERT");
        assert!(normalized[1].reverted);
    }

    #[test]
    fn normalize_raw_marks_revert_flag() {
        let steps = vec![
            TraceStep { op: "ADD".to_string(), reverted: false, ..Default::default() },
            TraceStep { op: "INVALID".to_string(), reverted: false, ..Default::default() },
        ];

        let normalized = Parser::normalize_raw(steps);
        assert!(!normalized[0].reverted);
        assert!(normalized[1].reverted);
    }
}
