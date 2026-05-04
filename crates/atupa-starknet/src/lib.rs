use atupa_core::{TraceStep, VmKind};
use atupa_rpc::RpcError;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

// ─── Starknet RPC Types ──────────────────────────────────────────────────────

#[derive(Error, Debug)]
pub enum StarknetError {
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("RPC error: {0}")]
    Rpc(#[from] RpcError),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Processing error: {0}")]
    Process(String),
}

/// Execution resources consumed by a Starknet call.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExecutionResources {
    pub steps: u64,
    #[serde(default)]
    pub pedersen_builtin: u64,
    #[serde(default)]
    pub range_check_builtin: u64,
    #[serde(default)]
    pub bitwise_builtin: u64,
    #[serde(default)]
    pub poseidon_builtin: u64,
    #[serde(default)]
    pub ec_op_builtin: u64,
    #[serde(default)]
    pub ecdsa_builtin: u64,
}

/// A recursive call in a Starknet transaction trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionInvocation {
    pub contract_address: String,
    pub entry_point_selector: String,
    pub calldata: Vec<String>,
    pub execution_resources: ExecutionResources,
    #[serde(default)]
    pub calls: Vec<FunctionInvocation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StarknetTransactionTrace {
    pub validate_invocation: Option<FunctionInvocation>,
    pub execute_invocation: Option<FunctionInvocation>,
    pub fee_transfer_invocation: Option<FunctionInvocation>,
}

// ─── Starknet Client ──────────────────────────────────────────────────────────

pub struct StarknetClient {
    rpc_url: String,
    client: reqwest::Client,
}

impl StarknetClient {
    pub fn new(rpc_url: String) -> Self {
        Self {
            rpc_url,
            client: reqwest::Client::new(),
        }
    }

    pub async fn get_transaction_trace(
        &self,
        tx_hash: &str,
    ) -> Result<StarknetTransactionTrace, StarknetError> {
        let payload = json!({
            "jsonrpc": "2.0",
            "method": "starknet_traceTransaction",
            "params": [tx_hash],
            "id": 1
        });

        let response = self
            .client
            .post(&self.rpc_url)
            .json(&payload)
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        if let Some(error) = response.get("error") {
            return Err(StarknetError::Rpc(RpcError::Node(
                error["message"]
                    .as_str()
                    .unwrap_or("Unknown RPC error")
                    .to_string(),
            )));
        }

        let result = response.get("result").ok_or_else(|| {
            StarknetError::Process("Missing 'result' in starknet_traceTransaction response".into())
        })?;

        Ok(serde_json::from_value(result.clone())?)
    }

    /// Recursively flattens a Starknet trace into Atupa-compatible TraceSteps.
    pub fn flatten_trace(&self, invocation: &FunctionInvocation, depth: u16) -> Vec<TraceStep> {
        let mut steps = Vec::new();

        // 1. Map execution resources to virtual "opcodes" for Atupa aggregation
        // In Starknet, we don't have individual opcodes in the RPC trace (usually),
        // but we have aggregated resources per call frame.

        // Root step for this call frame
        let selector_label = if invocation.entry_point_selector.len() > 12 {
            &invocation.entry_point_selector[0..12]
        } else {
            &invocation.entry_point_selector
        };

        // For target resolution, we can add the contract_address to the stack
        let stack_info = vec![invocation.contract_address.clone()];

        steps.push(TraceStep {
            pc: 0,
            op: format!("CALL:{}", selector_label),
            gas: 0,
            gas_cost: invocation.execution_resources.steps, // Use steps as base weight
            depth,
            stack: Some(stack_info),
            memory: None,
            error: None,
            reverted: false,
            vm_kind: VmKind::Starknet,
        });

        // Add virtual steps for builtins if they were used
        let mut add_builtin = |op: &str, count: u64, weight: u64| {
            if count > 0 {
                steps.push(TraceStep {
                    pc: 0,
                    op: op.to_string(),
                    gas: 0,
                    gas_cost: count * weight,
                    depth: depth + 1,
                    stack: None,
                    memory: None,
                    error: None,
                    reverted: false,
                    vm_kind: VmKind::Starknet,
                });
            }
        };

        add_builtin(
            "PEDERSEN",
            invocation.execution_resources.pedersen_builtin,
            32,
        );
        add_builtin(
            "RANGE_CHECK",
            invocation.execution_resources.range_check_builtin,
            16,
        );
        add_builtin(
            "BITWISE",
            invocation.execution_resources.bitwise_builtin,
            64,
        );
        add_builtin(
            "POSEIDON",
            invocation.execution_resources.poseidon_builtin,
            32,
        );
        add_builtin("EC_OP", invocation.execution_resources.ec_op_builtin, 1024);
        add_builtin("ECDSA", invocation.execution_resources.ecdsa_builtin, 2048);

        // 2. Recursively process nested calls
        for sub_call in &invocation.calls {
            steps.extend(self.flatten_trace(sub_call, depth + 1));
        }

        steps
    }

    pub async fn profile_transaction(
        &self,
        tx_hash: &str,
    ) -> Result<Vec<TraceStep>, StarknetError> {
        let trace = self.get_transaction_trace(tx_hash).await?;
        let mut all_steps = Vec::new();

        if let Some(invoke) = trace.validate_invocation {
            all_steps.extend(self.flatten_trace(&invoke, 1));
        }
        if let Some(invoke) = trace.execute_invocation {
            all_steps.extend(self.flatten_trace(&invoke, 1));
        }
        if let Some(invoke) = trace.fee_transfer_invocation {
            all_steps.extend(self.flatten_trace(&invoke, 1));
        }

        Ok(all_steps)
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flatten_recursive_trace() {
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

        let client = StarknetClient::new("http://localhost".to_string());
        let steps = client.flatten_trace(&invocation, 1);

        // 1 (root) + 1 (pedersen) + 1 (range_check) + 1 (sub-call) = 4 steps
        assert_eq!(steps.len(), 4);
        assert_eq!(steps[0].op, "CALL:0xabcdef1234");
        assert_eq!(steps[0].depth, 1);
        assert_eq!(steps[1].op, "PEDERSEN");
        assert_eq!(steps[1].depth, 2);
        assert_eq!(steps[2].op, "RANGE_CHECK");
        assert_eq!(steps[2].depth, 2);
        assert_eq!(steps[3].op, "CALL:0xdeadbeef");
        assert_eq!(steps[3].depth, 2);
    }
}
