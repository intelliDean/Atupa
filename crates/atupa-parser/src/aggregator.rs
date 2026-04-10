use atupa_core::{CollapsedStack, TraceStep};
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

        let registry = atupa_adapters::AdapterRegistry::new();

        // Map to aggregate stacks: stack_string -> (total_gas, last_pc, target_address, resolved_label, reverted)
        let mut stack_map: HashMap<String, (u64, u64, Option<String>, Option<String>, bool)> = HashMap::new();

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

            // Extract Target Address & Parse Function Selector if this is a Call opcode
            let mut target_address = None;
            let mut resolved_label = None;
            
            if operation == "CALL" || operation == "STATICCALL" || operation == "DELEGATECALL" || operation == "CALLCODE" {
                if let Some(stack) = &step.stack {
                    if stack.len() >= 2 {
                        // Extract target address (second item from top)
                        let hex_addr = &stack[stack.len() - 2];
                        let clean_hex = hex_addr.trim_start_matches("0x");
                        if clean_hex.len() >= 40 {
                            let extracted = &clean_hex[clean_hex.len() - 40..];
                            target_address = Some(format!("0x{}", extracted));
                        }
                    }

                    // Attempt to extract the 4-byte selector from Memory using Offset & Length
                    let mut args_offset_idx = None;
                    let mut args_length_idx = None;

                    if operation == "CALL" || operation == "CALLCODE" {
                        if stack.len() >= 5 {
                            args_offset_idx = Some(stack.len() - 4);
                            args_length_idx = Some(stack.len() - 5);
                        }
                    } else if (operation == "DELEGATECALL" || operation == "STATICCALL") && stack.len() >= 4 {
                        args_offset_idx = Some(stack.len() - 3);
                        args_length_idx = Some(stack.len() - 4);
                    }

                    if let (Some(off_idx), Some(len_idx)) = (args_offset_idx, args_length_idx) {
                        let offset_str = stack[off_idx].trim_start_matches("0x");
                        let len_str = stack[len_idx].trim_start_matches("0x");

                        if let (Ok(offset), Ok(length)) = (
                            usize::from_str_radix(offset_str, 16),
                            usize::from_str_radix(len_str, 16)
                        ) {
                            if length >= 4 {
                                if let Some(mem) = &step.memory {
                                    let word_idx = offset / 32;
                                    let byte_offset = offset % 32;
                                    let hex_offset = byte_offset * 2; // Each byte is 2 hex chars

                                    if let Some(word) = mem.get(word_idx) {
                                        let clean_word = word.trim_start_matches("0x");
                                        let selector_opt = if clean_word.len() >= hex_offset + 8 {
                                            let selector = &clean_word[hex_offset..hex_offset + 8];
                                            Some(format!("0x{}", selector))
                                        } else if word_idx + 1 < mem.len() {
                                            // The 4-byte selector spans across two memory boundary words
                                            let p1 = &clean_word[hex_offset..];
                                            let needed = 8 - p1.len();
                                            let next_word = mem[word_idx + 1].trim_start_matches("0x");
                                            if next_word.len() >= needed {
                                                let p2 = &next_word[..needed];
                                                Some(format!("0x{}{}", p1, p2))
                                            } else { None }
                                        } else { None };

                                        // Try resolving the label
                                        if let Some(sel) = selector_opt {
                                            resolved_label = registry.resolve(target_address.as_deref(), Some(&sel));
                                        }
                                    }
                                }
                            }
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
            let entry = stack_map.entry(stack_str).or_insert((0, 0, None, None, false));
            entry.0 += step.gas_cost;
            entry.1 = step.pc;
            if target_address.is_some() {
                entry.2 = target_address;
            }
            if resolved_label.is_some() {
                entry.3 = resolved_label;
            }
            if step.reverted {
                entry.4 = true;
            }
        }

        let mut stacks: Vec<CollapsedStack> = stack_map
            .into_iter()
            .map(|(stack, (weight, pc, target_address, resolved_label, reverted))| CollapsedStack {
                stack,
                weight,
                last_pc: Some(pc),
                target_address,
                resolved_label,
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
    use atupa_core::TraceStep;

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

    #[test]
    fn test_aggregator_memory_selector_extraction() {
        // Stack for CALL:
        // gas, address, value, argsOffset, argsLength, retOffset, retLength
        // Top of stack is at the end.
        // We want argsOffset to be "0x20" (32 bytes), argsLength to be "0x04" (4 bytes)
        // stack[len-4] = argsOffset
        // stack[len-5] = argsLength
        
        let stack = vec![
            "0x0".to_string(), // retLength
            "0x0".to_string(), // retOffset
            "0x4".to_string(), // argsLength
            "0x20".to_string(), // argsOffset (byte 32)
            "0x0".to_string(), // value
            "0x0000000000000000000000001111111111111111111111111111111111111111".to_string(), // target address
            "0x1000".to_string(), // gas
        ];

        // Memory array (32-byte chunks as 64-char hex strings)
        // We set argsOffset = 32, so it looks in mem[1].
        // "beforeInitialize" selector is 0x18a9d381. We'll pad the rest with zeroes.
        let memory = vec![
            "0000000000000000000000000000000000000000000000000000000000000000".to_string(), // word 0
            "18a9d38100000000000000000000000000000000000000000000000000000000".to_string(), // word 1
        ];

        let steps = vec![
            TraceStep { pc: 0, op: "CALL".into(), gas: 1000, gas_cost: 50, depth: 1, stack: Some(stack), memory: Some(memory), error: None, reverted: false },
            TraceStep { pc: 1, op: "STOP".into(), gas: 900, gas_cost: 0, depth: 1, stack: None, memory: None, error: None, reverted: false },
        ];

        let stacks = Aggregator::build_collapsed_stacks(&steps);
        let call_stack = stacks.iter().find(|s| s.stack == "CALL;CALL").expect("Should find CALL");
        
        // Ensure that the target address was resolved successfully
        assert_eq!(call_stack.target_address.as_deref(), Some("0x1111111111111111111111111111111111111111"));
        
        // Ensure that the specific Uniswap v4 Hook was decoded
        assert_eq!(call_stack.resolved_label.as_deref(), Some("Uniswapv4: beforeInitialize"));
    }
}
