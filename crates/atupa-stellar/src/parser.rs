//! Parser for mapping Stellar / Soroban diagnostic event streams to [`TraceStep`]s.

use crate::types::SorobanDiagnosticEvent;
use atupa_core::{TraceStep, VmKind};

/// Estimated gas costs for common Soroban host functions.
pub const COST_PUT_CONTRACT_DATA: u64 = 5_000;
pub const COST_GET_CONTRACT_DATA: u64 = 2_100;
pub const COST_CRYPTO_HASH: u64 = 3_000;
pub const COST_INVOKE_CONTRACT: u64 = 1_500;
pub const COST_GENERIC_HOST_FN: u64 = 100;

/// Reconstructs hierarchical execution traces from Soroban diagnostic events.
pub struct StellarTraceParser;

impl StellarTraceParser {
    /// Maps Stellar diagnostic events to Atupa [`TraceStep`]s.
    pub fn parse_diagnostic_events(events: &[SorobanDiagnosticEvent]) -> Vec<TraceStep> {
        let mut steps = Vec::new();
        let mut depth: u16 = 1;

        for event in events {
            if event.event_type != "diagnostic" {
                continue;
            }

            let event_action = event.topics.first().map(|s| s.as_str()).unwrap_or("");
            let fn_name = event.topics.get(1).map(|s| s.as_str()).unwrap_or("unknown");

            // Handle depth adjustments for returning contract calls
            if event_action.contains("return") {
                depth = depth.saturating_sub(1);
                continue;
            }

            let gas_cost = estimate_host_fn_gas_cost(fn_name);

            steps.push(TraceStep {
                pc: 0,
                op: fn_name.to_string(),
                gas: 0,
                gas_cost,
                depth,
                stack: None,
                memory: None,
                error: None,
                reverted: false,
                vm_kind: VmKind::Stellar,
            });

            // If this was an invocation call, nested events happen at deeper frame level
            if fn_name.contains("invoke_contract") && event_action.contains("call") {
                depth = depth.saturating_add(1);
            }
        }

        steps
    }
}

/// Estimates the gas-equivalent cost for a given Soroban host function name.
pub fn estimate_host_fn_gas_cost(fn_name: &str) -> u64 {
    if fn_name.contains("put_contract_data") {
        COST_PUT_CONTRACT_DATA
    } else if fn_name.contains("get_contract_data") {
        COST_GET_CONTRACT_DATA
    } else if fn_name.contains("crypto") || fn_name.contains("hash") {
        COST_CRYPTO_HASH
    } else if fn_name.contains("invoke") {
        COST_INVOKE_CONTRACT
    } else {
        COST_GENERIC_HOST_FN
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_events_with_depth_tracking() {
        let events = vec![
            SorobanDiagnosticEvent {
                event_type: "diagnostic".into(),
                topics: vec!["fn_call".into(), "invoke_contract".into()],
                value: "AAAAA...".into(),
            },
            SorobanDiagnosticEvent {
                event_type: "diagnostic".into(),
                topics: vec!["fn_call".into(), "put_contract_data".into()],
                value: "AAAAA...".into(),
            },
            SorobanDiagnosticEvent {
                event_type: "diagnostic".into(),
                topics: vec!["fn_return".into(), "invoke_contract".into()],
                value: "AAAAA...".into(),
            },
        ];

        let steps = StellarTraceParser::parse_diagnostic_events(&events);
        assert_eq!(steps.len(), 2);

        assert_eq!(steps[0].op, "invoke_contract");
        assert_eq!(steps[0].depth, 1);
        assert_eq!(steps[0].gas_cost, COST_INVOKE_CONTRACT);

        assert_eq!(steps[1].op, "put_contract_data");
        assert_eq!(steps[1].depth, 2); // Depth increased after invoke_contract
        assert_eq!(steps[1].gas_cost, COST_PUT_CONTRACT_DATA);
    }

    #[test]
    fn skips_non_diagnostic_events() {
        let events = vec![SorobanDiagnosticEvent {
            event_type: "contract".into(),
            topics: vec!["transfer".into()],
            value: "123".into(),
        }];

        let steps = StellarTraceParser::parse_diagnostic_events(&events);
        assert!(steps.is_empty());
    }

    #[test]
    fn estimates_gas_costs_accurately() {
        assert_eq!(estimate_host_fn_gas_cost("put_contract_data"), 5000);
        assert_eq!(estimate_host_fn_gas_cost("get_contract_data"), 2100);
        assert_eq!(estimate_host_fn_gas_cost("crypto_keccak256"), 3000);
        assert_eq!(estimate_host_fn_gas_cost("custom_host_fn"), 100);
    }
}
