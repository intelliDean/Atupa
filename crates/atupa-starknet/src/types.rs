//! Data models for Starknet (Cairo VM) transaction execution traces and resource counters.

use serde::{Deserialize, Serialize};

/// Cairo VM execution resources consumed by a single Starknet call frame.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ExecutionResources {
    /// Number of Cairo steps executed.
    pub steps: u64,
    /// Number of Pedersen hash builtin invocations.
    #[serde(default)]
    pub pedersen_builtin: u64,
    /// Number of Range Check builtin invocations.
    #[serde(default)]
    pub range_check_builtin: u64,
    /// Number of Bitwise builtin invocations.
    #[serde(default)]
    pub bitwise_builtin: u64,
    /// Number of Poseidon hash builtin invocations.
    #[serde(default)]
    pub poseidon_builtin: u64,
    /// Number of Elliptic Curve operations builtin invocations.
    #[serde(default)]
    pub ec_op_builtin: u64,
    /// Number of ECDSA signature verification builtin invocations.
    #[serde(default)]
    pub ecdsa_builtin: u64,
}

impl ExecutionResources {
    /// Returns `true` if any builtins were utilized in this call frame.
    pub fn has_builtins(&self) -> bool {
        self.pedersen_builtin > 0
            || self.range_check_builtin > 0
            || self.bitwise_builtin > 0
            || self.poseidon_builtin > 0
            || self.ec_op_builtin > 0
            || self.ecdsa_builtin > 0
    }
}

/// A recursive function call frame in a Starknet transaction trace.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct FunctionInvocation {
    /// Target contract address in hex string format.
    pub contract_address: String,
    /// 4-byte / felt entry point selector.
    pub entry_point_selector: String,
    /// Raw calldata felts.
    #[serde(default)]
    pub calldata: Vec<String>,
    /// Execution resources consumed directly by this frame.
    #[serde(default)]
    pub execution_resources: ExecutionResources,
    /// Nested child function calls invoked by this frame.
    #[serde(default)]
    pub calls: Vec<FunctionInvocation>,
}

/// Top-level transaction trace payload returned by `starknet_traceTransaction`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct StarknetTransactionTrace {
    /// Account contract validation phase invocation.
    pub validate_invocation: Option<FunctionInvocation>,
    /// Main execution phase invocation.
    pub execute_invocation: Option<FunctionInvocation>,
    /// Fee transfer phase invocation.
    pub fee_transfer_invocation: Option<FunctionInvocation>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execution_resources_builtin_check() {
        let empty = ExecutionResources::default();
        assert!(!empty.has_builtins());

        let with_poseidon = ExecutionResources {
            poseidon_builtin: 5,
            ..Default::default()
        };
        assert!(with_poseidon.has_builtins());
    }

    #[test]
    fn deserializes_function_invocation() {
        let json_str = r#"{
            "contract_address": "0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7",
            "entry_point_selector": "0x0361458367e696363fbcc70777d07ebbd23fef3c80269b0083d675d7b050f679",
            "calldata": [],
            "execution_resources": {
                "steps": 120,
                "pedersen_builtin": 2
            },
            "calls": []
        }"#;

        let inv: FunctionInvocation = serde_json::from_str(json_str).unwrap();
        assert_eq!(inv.execution_resources.steps, 120);
        assert_eq!(inv.execution_resources.pedersen_builtin, 2);
        assert!(inv.execution_resources.has_builtins());
    }
}
