//! Data models for EVM debug tracer JSON-RPC payloads.

use serde::{Deserialize, Serialize};

/// Raw execution step (`structLog`) emitted by Geth / Anvil / Nitro debug tracers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct RawStructLog {
    /// Program counter.
    pub pc: u64,
    /// Opcode mnemonic (e.g. `PUSH1`, `CALL`, `SSTORE`).
    pub op: String,
    /// Cumulative remaining gas before executing this opcode.
    pub gas: u64,
    /// Gas consumed by this specific execution step.
    pub gas_cost: u64,
    /// Call frame depth.
    pub depth: u16,
    /// Optional execution error (e.g. `execution reverted`, `out of gas`).
    pub error: Option<String>,
    /// EVM stack contents prior to opcode execution.
    pub stack: Option<Vec<String>>,
    /// EVM memory words (32-byte chunks as 64-character hex strings).
    pub memory: Option<Vec<String>>,
    /// EVM storage slots.
    pub storage: Option<serde_json::Value>,
}

impl RawStructLog {
    /// Returns `true` if this step resulted in an error or was a reverting opcode.
    pub fn is_reverted(&self) -> bool {
        self.error.is_some() || self.op == "REVERT" || self.op == "INVALID"
    }

    /// Returns `true` if this step represents a cross-contract call.
    pub fn is_call(&self) -> bool {
        matches!(self.op.as_str(), "CALL" | "STATICCALL" | "DELEGATECALL" | "CALLCODE")
    }
}

/// The top-level result payload returned by `debug_traceTransaction`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct TraceResult {
    /// Total gas used reported by the tracer.
    pub gas: u64,
    /// Hex-encoded return value of the top-level call.
    pub return_value: String,
    /// Sequential list of all execution step logs.
    pub struct_logs: Vec<RawStructLog>,
}

// ─── Internal JSON-RPC Envelope ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RpcResponse<T> {
    pub jsonrpc: String,
    pub id: u64,
    pub result: Option<T>,
    pub error: Option<RpcErrorBody>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RpcErrorBody {
    pub code: i64,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_raw_struct_log() {
        let json_data = r#"{
            "pc": 12,
            "op": "CALL",
            "gas": 950000,
            "gasCost": 2600,
            "depth": 1,
            "error": null,
            "stack": ["0x1000", "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"],
            "memory": ["0000000000000000000000000000000000000000000000000000000000000000"],
            "storage": null
        }"#;

        let log: RawStructLog = serde_json::from_str(json_data).unwrap();
        assert_eq!(log.pc, 12);
        assert_eq!(log.op, "CALL");
        assert_eq!(log.gas_cost, 2600);
        assert_eq!(log.depth, 1);
        assert!(log.is_call());
        assert!(!log.is_reverted());
        assert_eq!(log.stack.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn detects_revert_status() {
        let log = RawStructLog { op: "REVERT".to_string(), ..Default::default() };
        assert!(log.is_reverted());

        let log_err = RawStructLog {
            op: "PUSH1".to_string(),
            error: Some("out of gas".to_string()),
            ..Default::default()
        };
        assert!(log_err.is_reverted());
    }

    #[test]
    fn deserializes_trace_result() {
        let json_data = r#"{
            "gas": 21000,
            "returnValue": "0x",
            "structLogs": []
        }"#;

        let result: TraceResult = serde_json::from_str(json_data).unwrap();
        assert_eq!(result.gas, 21000);
        assert_eq!(result.return_value, "0x");
        assert!(result.struct_logs.is_empty());
    }
}
