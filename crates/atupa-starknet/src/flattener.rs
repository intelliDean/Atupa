//! Flattens hierarchical Starknet call trees into linear [`TraceStep`] execution timelines.

use crate::types::{ExecutionResources, FunctionInvocation, StarknetTransactionTrace};
use atupa_core::{TraceStep, VmKind};

// ─── Builtin Gas Weights ──────────────────────────────────────────────────────

/// Relative gas-equivalent weight per Pedersen hash invocation.
pub const PEDERSEN_WEIGHT: u64 = 32;

/// Relative gas-equivalent weight per Range Check operation.
pub const RANGE_CHECK_WEIGHT: u64 = 16;

/// Relative gas-equivalent weight per Bitwise builtin operation.
pub const BITWISE_WEIGHT: u64 = 64;

/// Relative gas-equivalent weight per Poseidon hash invocation.
pub const POSEIDON_WEIGHT: u64 = 32;

/// Relative gas-equivalent weight per Elliptic Curve operation.
pub const EC_OP_WEIGHT: u64 = 1024;

/// Relative gas-equivalent weight per ECDSA signature verification.
pub const ECDSA_WEIGHT: u64 = 2048;

/// Recursively flattens a [`FunctionInvocation`] and its nested sub-calls into [`TraceStep`]s.
pub fn flatten_invocation(invocation: &FunctionInvocation, depth: u16) -> Vec<TraceStep> {
    let mut steps = Vec::new();

    let selector_label = if invocation.entry_point_selector.len() > 12 {
        &invocation.entry_point_selector[0..12]
    } else {
        &invocation.entry_point_selector
    };

    // Root step for this call frame
    steps.push(TraceStep {
        pc: 0,
        op: format!("CALL:{selector_label}"),
        gas: 0,
        gas_cost: invocation.execution_resources.steps, // Base Cairo step count
        depth,
        stack: Some(vec![invocation.contract_address.clone()]),
        memory: None,
        error: None,
        reverted: false,
        vm_kind: VmKind::Starknet,
    });

    // Virtual steps for builtins
    append_builtin_steps(&invocation.execution_resources, depth + 1, &mut steps);

    // Recursively process nested child calls
    for sub_call in &invocation.calls {
        steps.extend(flatten_invocation(sub_call, depth + 1));
    }

    steps
}

/// Flattens all top-level phases (validate, execute, fee transfer) of a [`StarknetTransactionTrace`].
pub fn flatten_trace(trace: &StarknetTransactionTrace) -> Vec<TraceStep> {
    let mut all_steps = Vec::new();

    if let Some(invoke) = &trace.validate_invocation {
        all_steps.extend(flatten_invocation(invoke, 1));
    }
    if let Some(invoke) = &trace.execute_invocation {
        all_steps.extend(flatten_invocation(invoke, 1));
    }
    if let Some(invoke) = &trace.fee_transfer_invocation {
        all_steps.extend(flatten_invocation(invoke, 1));
    }

    all_steps
}

fn append_builtin_steps(resources: &ExecutionResources, depth: u16, steps: &mut Vec<TraceStep>) {
    let mut add_builtin = |op: &str, count: u64, weight: u64| {
        if count > 0 {
            steps.push(TraceStep {
                pc: 0,
                op: op.to_string(),
                gas: 0,
                gas_cost: count.saturating_mul(weight),
                depth,
                stack: None,
                memory: None,
                error: None,
                reverted: false,
                vm_kind: VmKind::Starknet,
            });
        }
    };

    add_builtin("PEDERSEN", resources.pedersen_builtin, PEDERSEN_WEIGHT);
    add_builtin(
        "RANGE_CHECK",
        resources.range_check_builtin,
        RANGE_CHECK_WEIGHT,
    );
    add_builtin("BITWISE", resources.bitwise_builtin, BITWISE_WEIGHT);
    add_builtin("POSEIDON", resources.poseidon_builtin, POSEIDON_WEIGHT);
    add_builtin("EC_OP", resources.ec_op_builtin, EC_OP_WEIGHT);
    add_builtin("ECDSA", resources.ecdsa_builtin, ECDSA_WEIGHT);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flattens_recursive_invocation_with_builtins() {
        let invocation = FunctionInvocation {
            contract_address: "0x1".to_string(),
            entry_point_selector: "0xabcdef123456789".to_string(),
            calldata: vec![],
            execution_resources: ExecutionResources {
                steps: 100,
                pedersen_builtin: 1,
                range_check_builtin: 2,
                ..Default::default()
            },
            calls: vec![FunctionInvocation {
                contract_address: "0x2".to_string(),
                entry_point_selector: "0xdeadbeef".to_string(),
                calldata: vec![],
                execution_resources: ExecutionResources {
                    steps: 50,
                    ..Default::default()
                },
                calls: vec![],
            }],
        };

        let steps = flatten_invocation(&invocation, 1);

        // 1 (root) + 1 (pedersen) + 1 (range_check) + 1 (sub-call) = 4 steps
        assert_eq!(steps.len(), 4);
        assert_eq!(steps[0].op, "CALL:0xabcdef1234");
        assert_eq!(steps[0].depth, 1);
        assert_eq!(steps[0].gas_cost, 100);

        assert_eq!(steps[1].op, "PEDERSEN");
        assert_eq!(steps[1].depth, 2);
        assert_eq!(steps[1].gas_cost, 32);

        assert_eq!(steps[2].op, "RANGE_CHECK");
        assert_eq!(steps[2].depth, 2);
        assert_eq!(steps[2].gas_cost, 32); // 2 * 16

        assert_eq!(steps[3].op, "CALL:0xdeadbeef");
        assert_eq!(steps[3].depth, 2);
        assert_eq!(steps[3].gas_cost, 50);
    }

    #[test]
    fn flattens_full_trace_phases() {
        let trace = StarknetTransactionTrace {
            validate_invocation: Some(FunctionInvocation {
                contract_address: "0xAccount".to_string(),
                entry_point_selector: "0xvalidate".to_string(),
                execution_resources: ExecutionResources {
                    steps: 40,
                    ..Default::default()
                },
                ..Default::default()
            }),
            execute_invocation: Some(FunctionInvocation {
                contract_address: "0xDapp".to_string(),
                entry_point_selector: "0xexecute".to_string(),
                execution_resources: ExecutionResources {
                    steps: 200,
                    ..Default::default()
                },
                ..Default::default()
            }),
            fee_transfer_invocation: None,
        };

        let steps = flatten_trace(&trace);
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].op, "CALL:0xvalidate");
        assert_eq!(steps[1].op, "CALL:0xexecute");
    }
}
