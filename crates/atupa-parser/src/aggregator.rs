//! Execution trace aggregator — collapses sequential trace steps into weighted call stacks.

use atupa_adapters::AdapterRegistry;
use atupa_core::{CollapsedStack, TraceStep, VmKind};
use log::debug;
use std::collections::HashMap;

use crate::decoder::{extract_memory_selector, extract_target_address};

/// Aggregates linear execution steps into weighted, collapsed call-stack paths for flamegraph visualization.
pub struct Aggregator;

impl Aggregator {
    /// Build collapsed stacks from a sequence of raw trace steps using the default [`AdapterRegistry`].
    pub fn build_collapsed_stacks(steps: &[TraceStep]) -> Vec<CollapsedStack> {
        let registry = AdapterRegistry::new();
        Self::build_collapsed_stacks_with_registry(steps, &registry)
    }

    /// Build collapsed stacks from a sequence of trace steps using a custom [`AdapterRegistry`].
    pub fn build_collapsed_stacks_with_registry(
        steps: &[TraceStep],
        registry: &AdapterRegistry,
    ) -> Vec<CollapsedStack> {
        debug!(
            "Building collapsed stacks from {} execution steps",
            steps.len()
        );

        let mut stack_map: HashMap<String, AggregatedData> = HashMap::new();
        let mut call_stack: Vec<String> = Vec::new();

        for step in steps {
            let operation = &step.op;
            let current_depth = step.depth as usize;

            // 1. Maintain call stack depth
            if current_depth < call_stack.len() {
                call_stack.truncate(current_depth);
            }
            while call_stack.len() < current_depth {
                call_stack.push("CALL".to_string());
            }

            // 2. Decode target address & function selector if this is a call opcode
            let (target_address, resolved_label) = decode_call_context(step, registry);

            // 3. Build stack path string
            let stack_str = if call_stack.is_empty() {
                operation.clone()
            } else {
                format!("{};{}", call_stack.join(";"), operation)
            };

            // 4. Accumulate into map
            let entry = stack_map.entry(stack_str).or_insert_with(|| AggregatedData {
                total_gas: 0,
                last_pc: step.pc,
                max_depth: step.depth,
                target_address: None,
                resolved_label: None,
                reverted: false,
                vm_kind: step.vm_kind.clone(),
            });

            entry.total_gas = entry.total_gas.saturating_add(step.gas_cost);
            entry.last_pc = step.pc;
            entry.max_depth = entry.max_depth.max(step.depth);

            if target_address.is_some() {
                entry.target_address = target_address;
            }
            if resolved_label.is_some() {
                entry.resolved_label = resolved_label;
            }
            if step.reverted {
                entry.reverted = true;
            }
            entry.vm_kind = step.vm_kind.clone();
        }

        let mut stacks: Vec<CollapsedStack> = stack_map
            .into_iter()
            .map(|(stack, data)| CollapsedStack {
                stack,
                weight: data.total_gas,
                last_pc: Some(data.last_pc),
                depth: data.max_depth,
                vm_kind: data.vm_kind,
                target_address: data.target_address,
                resolved_label: data.resolved_label,
                reverted: data.reverted,
            })
            .collect();

        stacks.sort_by_key(|b| std::cmp::Reverse(b.weight));
        debug!("Built {} unique collapsed stacks", stacks.len());

        stacks
    }
}

// ─── Private Helpers ──────────────────────────────────────────────────────────

struct AggregatedData {
    total_gas: u64,
    last_pc: u64,
    max_depth: u16,
    target_address: Option<String>,
    resolved_label: Option<String>,
    reverted: bool,
    vm_kind: VmKind,
}

fn is_call_op(op: &str) -> bool {
    matches!(op, "CALL" | "STATICCALL" | "DELEGATECALL" | "CALLCODE")
}

fn decode_call_context(
    step: &TraceStep,
    registry: &AdapterRegistry,
) -> (Option<String>, Option<String>) {
    if !is_call_op(&step.op) {
        return (None, None);
    }

    let Some(stack) = &step.stack else {
        return (None, None);
    };

    let target_address = extract_target_address(stack);
    let mut resolved_label = None;

    if let Some(mem) = &step.memory
        && let Some(selector) = extract_memory_selector(&step.op, stack, mem)
    {
        resolved_label = registry.resolve(target_address.as_deref(), Some(&selector));
    }

    (target_address, resolved_label)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use atupa_adapters::ProtocolAdapter;

    #[test]
    fn collapses_simple_call_hierarchy() {
        let steps = vec![
            TraceStep {
                op: "PUSH1".into(),
                gas: 100,
                gas_cost: 3,
                depth: 1,
                ..Default::default()
            },
            TraceStep {
                pc: 1,
                op: "CALL".into(),
                gas: 90,
                depth: 1,
                ..Default::default()
            },
            TraceStep {
                op: "SSTORE".into(),
                gas: 50,
                gas_cost: 20,
                depth: 2,
                ..Default::default()
            },
            TraceStep {
                pc: 1,
                op: "RETURN".into(),
                gas: 20,
                depth: 2,
                ..Default::default()
            },
            TraceStep {
                pc: 2,
                op: "STOP".into(),
                gas: 15,
                depth: 1,
                ..Default::default()
            },
        ];

        let stacks = Aggregator::build_collapsed_stacks(&steps);
        assert!(!stacks.is_empty());
        let sstore_stack = stacks
            .iter()
            .find(|s| s.stack == "CALL;CALL;SSTORE")
            .expect("Should find SSTORE");
        assert_eq!(sstore_stack.weight, 20);
    }

    #[test]
    fn handles_recursive_call_depths() {
        let steps = vec![
            TraceStep {
                op: "CALL".into(),
                gas: 1000,
                depth: 1,
                ..Default::default()
            },
            TraceStep {
                op: "CALL".into(),
                gas: 900,
                depth: 2,
                ..Default::default()
            },
            TraceStep {
                op: "SSTORE".into(),
                gas: 800,
                gas_cost: 5000,
                depth: 3,
                ..Default::default()
            },
            TraceStep {
                pc: 1,
                op: "RETURN".into(),
                gas: 700,
                depth: 3,
                ..Default::default()
            },
            TraceStep {
                pc: 1,
                op: "RETURN".into(),
                gas: 600,
                depth: 2,
                ..Default::default()
            },
        ];

        let stacks = Aggregator::build_collapsed_stacks(&steps);
        let sstore_stack = stacks
            .iter()
            .find(|s| s.stack == "CALL;CALL;CALL;SSTORE")
            .expect("Should find deep SSTORE");
        assert_eq!(sstore_stack.weight, 5000);
    }

    #[test]
    fn propagates_revert_status() {
        let steps = vec![
            TraceStep {
                op: "CALL".into(),
                gas: 1000,
                depth: 1,
                ..Default::default()
            },
            TraceStep {
                op: "REVERT".into(),
                gas: 900,
                gas_cost: 200,
                depth: 2,
                error: Some("Reverted".into()),
                reverted: true,
                ..Default::default()
            },
        ];

        let stacks = Aggregator::build_collapsed_stacks(&steps);
        let revert_stack = stacks
            .iter()
            .find(|s| s.stack == "CALL;CALL;REVERT")
            .expect("Should find REVERT");
        assert!(revert_stack.reverted);
        assert_eq!(revert_stack.weight, 200);
    }

    #[test]
    fn extracts_memory_selector_and_resolves_label() {
        let stack = vec![
            "0x0".to_string(),
            "0x0".to_string(),
            "0x4".to_string(),   // argsLength = 4
            "0x20".to_string(),  // argsOffset = 32
            "0x0".to_string(),
            "0x0000000000000000000000001111111111111111111111111111111111111111".to_string(),
            "0x1000".to_string(),
        ];

        let memory = vec![
            "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
            "18a9d38100000000000000000000000000000000000000000000000000000000".to_string(),
        ];

        let steps = vec![
            TraceStep {
                op: "CALL".into(),
                gas: 1000,
                gas_cost: 50,
                depth: 1,
                stack: Some(stack),
                memory: Some(memory),
                ..Default::default()
            },
            TraceStep {
                pc: 1,
                op: "STOP".into(),
                gas: 900,
                depth: 1,
                ..Default::default()
            },
        ];

        let stacks = Aggregator::build_collapsed_stacks(&steps);
        let call_stack = stacks
            .iter()
            .find(|s| s.stack == "CALL;CALL")
            .expect("Should find CALL");

        assert_eq!(
            call_stack.target_address.as_deref(),
            Some("0x1111111111111111111111111111111111111111")
        );
        assert_eq!(
            call_stack.resolved_label.as_deref(),
            Some("Uniswapv4: beforeInitialize")
        );
    }

    #[test]
    fn custom_registry_resolution() {
        struct MockCustomAdapter;
        impl ProtocolAdapter for MockCustomAdapter {
            fn name(&self) -> &str {
                "CustomProtocol"
            }
            fn resolve_label(&self, _address: Option<&str>, selector: Option<&str>) -> Option<String> {
                if selector == Some("0xab9c4b5d") {
                    Some("Custom::flashLoan".to_string())
                } else {
                    None
                }
            }
        }

        let stack = vec![
            "0x0".to_string(),
            "0x0".to_string(),
            "0x4".to_string(),
            "0x0".to_string(),
            "0x0".to_string(),
            "0x0000000000000000000000002222222222222222222222222222222222222222".to_string(),
            "0x1000".to_string(),
        ];

        let memory = vec![
            "ab9c4b5d00000000000000000000000000000000000000000000000000000000".to_string(),
        ];

        let steps = vec![TraceStep {
            op: "CALL".into(),
            gas: 1000,
            gas_cost: 80,
            depth: 1,
            stack: Some(stack),
            memory: Some(memory),
            ..Default::default()
        }];

        let mut registry = AdapterRegistry::empty();
        registry.register_typed(MockCustomAdapter);

        let stacks = Aggregator::build_collapsed_stacks_with_registry(&steps, &registry);
        let call_stack = stacks
            .iter()
            .find(|s| s.stack == "CALL;CALL")
            .expect("Should find CALL");

        assert_eq!(
            call_stack.resolved_label.as_deref(),
            Some("Custom::flashLoan")
        );
    }
}
