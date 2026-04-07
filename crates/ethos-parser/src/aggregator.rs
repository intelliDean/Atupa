use ethos_core::{CollapsedStack, TraceStep};
use std::collections::HashMap;
use log::debug;

pub struct Aggregator;

impl Aggregator {
    /// Build collapsed stacks from a sequence of raw trace steps (structLogs style).
    ///
    /// # Algorithm
    /// 1. Walk through execution steps
    /// 2. Track call stack depth
    /// 3. Build stack strings for each gas-consuming operation
    /// 4. Aggregate by unique stack (sum gas weights)
    pub fn build_collapsed_stacks(steps: &[TraceStep]) -> Vec<CollapsedStack> {
        debug!("Building collapsed stacks from {} execution steps", steps.len());

        // Map to aggregate stacks: stack_string -> (total_gas, last_pc, target_address, reverted)
        let mut stack_map: HashMap<String, (u64, u64, Option<String>, bool)> = HashMap::new();

        // Current call stack
        let mut call_stack: Vec<String> = Vec::new();

        for step in steps {
            let operation = step.op.clone();
            let current_depth = step.depth as usize;

            // If depth decreased, we returned from function calls
            if current_depth < call_stack.len() {
                call_stack.truncate(current_depth);
            }

            // If depth increased, we entered a new call
            while call_stack.len() < current_depth {
                call_stack.push("CALL".to_string());
            }

            // Extract Target Address if this is a Call opcode
            let mut target_address = None;
            if operation == "CALL" || operation == "STATICCALL" || operation == "DELEGATECALL" || operation == "CALLCODE" {
                if let Some(stack) = &step.stack {
                    if stack.len() >= 2 {
                        // In Geth/Anvil trace stack array, the end of the array is the top of the stack.
                        // CALL takes: gas, address, value, argsOffset, argsLength, retOffset, retLength
                        let hex_addr = &stack[stack.len() - 2];
                        let clean_hex = hex_addr.trim_start_matches("0x");
                        // EVM addresses are exactly 40 chars, padded to 64 chars in stack elements
                        if clean_hex.len() >= 40 {
                            let extracted = &clean_hex[clean_hex.len() - 40..];
                            target_address = Some(format!("0x{}", extracted));
                        }
                    }
                }
            }

            // Build the full stack string with current operation
            let stack_str = if call_stack.is_empty() {
                operation.clone()
            } else {
                format!("{};{}", call_stack.join(";"), operation)
            };

            // Accumulate gas cost and flags
            let entry = stack_map.entry(stack_str).or_insert((0, 0, None, false));
            entry.0 += step.gas_cost;
            entry.1 = step.pc;
            if target_address.is_some() {
                entry.2 = target_address;
            }
            if step.reverted {
                entry.3 = true;
            }
            
            // NOTE: Reverts naturally bubble up visually because if an internal call hits REVERT, 
            // the specific reverting stack path gets the `entry.3 = true` flag. 
        }

        let mut stacks: Vec<CollapsedStack> = stack_map
            .into_iter()
            .map(|(stack, (weight, pc, target_address, reverted))| CollapsedStack {
                stack,
                weight,
                last_pc: Some(pc),
                target_address,
                reverted,
            })
            .collect();

        stacks.sort_by(|a, b| b.weight.cmp(&a.weight));
        debug!("Built {} unique collapsed stacks", stacks.len());

        stacks
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethos_core::TraceStep;

    #[test]
    fn test_aggregator_collapses_simple_call() {
        let steps = vec![
            // Root context opcodes (Depth 1)
            TraceStep { pc: 0, op: "PUSH1".into(), gas: 100, gas_cost: 3, depth: 1, stack: None, memory: None, error: None, reverted: false },
            TraceStep { pc: 1, op: "CALL".into(), gas: 90, gas_cost: 0, depth: 1, stack: None, memory: None, error: None, reverted: false },
            // Sub-context opcodes (Depth 2)
            TraceStep { pc: 0, op: "SSTORE".into(), gas: 50, gas_cost: 20, depth: 2, stack: None, memory: None, error: None, reverted: false },
            TraceStep { pc: 1, op: "RETURN".into(), gas: 20, gas_cost: 0, depth: 2, stack: None, memory: None, error: None, reverted: false },
            // Back to root (Depth 1)
            TraceStep { pc: 2, op: "STOP".into(), gas: 15, gas_cost: 0, depth: 1, stack: None, memory: None, error: None, reverted: false },
        ];

        let stacks = Aggregator::build_collapsed_stacks(&steps);
        assert!(!stacks.is_empty(), "Stacks should not be empty");
        let sstore_stack = stacks.iter().find(|s| s.stack == "CALL;CALL;SSTORE").expect("Should find SSTORE");
        assert_eq!(sstore_stack.weight, 20);
    }

    #[test]
    fn test_aggregator_recursive_calls() {
        let steps = vec![
            TraceStep { pc: 0, op: "CALL".into(), gas: 1000, gas_cost: 0, depth: 1, stack: None, memory: None, error: None, reverted: false },
            TraceStep { pc: 0, op: "CALL".into(), gas: 900, gas_cost: 0, depth: 2, stack: None, memory: None, error: None, reverted: false },
            TraceStep { pc: 0, op: "SSTORE".into(), gas: 800, gas_cost: 5000, depth: 3, stack: None, memory: None, error: None, reverted: false },
            TraceStep { pc: 1, op: "RETURN".into(), gas: 700, gas_cost: 0, depth: 3, stack: None, memory: None, error: None, reverted: false },
            TraceStep { pc: 1, op: "RETURN".into(), gas: 600, gas_cost: 0, depth: 2, stack: None, memory: None, error: None, reverted: false },
        ];

        let stacks = Aggregator::build_collapsed_stacks(&steps);
        let sstore_stack = stacks.iter().find(|s| s.stack == "CALL;CALL;CALL;SSTORE").expect("Should find deep SSTORE");
        assert_eq!(sstore_stack.weight, 5000);
    }

    #[test]
    fn test_aggregator_revert_propagation() {
        let steps = vec![
            TraceStep { pc: 0, op: "CALL".into(), gas: 1000, gas_cost: 0, depth: 1, stack: None, memory: None, error: None, reverted: false },
            TraceStep { pc: 0, op: "REVERT".into(), gas: 900, gas_cost: 200, depth: 2, stack: None, memory: None, error: Some("Reverted".into()), reverted: true },
        ];

        let stacks = Aggregator::build_collapsed_stacks(&steps);
        let revert_stack = stacks.iter().find(|s| s.stack == "CALL;CALL;REVERT").expect("Should find REVERT");
        assert!(revert_stack.reverted);
        assert_eq!(revert_stack.weight, 200);
    }
}

