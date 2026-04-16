pub mod etherscan;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RpcError {
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("RPC error: {0}")]
    Node(String),
}

/// Raw structLog from the EVM tracer
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawStructLog {
    pub pc: u64,
    pub op: String,
    pub gas: u64,
    pub gas_cost: u64,
    pub depth: u16,
    pub error: Option<String>,
    pub stack: Option<Vec<String>>,
    pub memory: Option<Vec<String>>,
    pub storage: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceResult {
    pub gas: u64,
    pub return_value: String,
    pub struct_logs: Vec<RawStructLog>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RpcResponse {
    jsonrpc: String,
    id: u64,
    result: Option<TraceResult>,
    error: Option<RpcErrorBody>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RpcErrorBody {
    code: i64,
    message: String,
}

pub struct EthClient {
    rpc_url: String,
    client: Client,
}

impl EthClient {
    pub fn new(rpc_url: String) -> Self {
        Self {
            rpc_url,
            client: Client::new(),
        }
    }

    /// Fetch a raw debug_traceTransaction structLog response from the node
    pub async fn get_transaction_trace(&self, tx_hash: &str) -> Result<TraceResult, RpcError> {
        // debug_traceTransaction parameters
        let params = json!([
            tx_hash,
            {
                "enableMemory": false,
                "disableStack": false,
                "disableStorage": true
            }
        ]);

        let payload = json!({
            "jsonrpc": "2.0",
            "method": "debug_traceTransaction",
            "params": params,
            "id": 1
        });

        let response = self
            .client
            .post(&self.rpc_url)
            .json(&payload)
            .send()
            .await?;

        let rpc_res: RpcResponse = response.json().await?;

        if let Some(err) = rpc_res.error {
            return Err(RpcError::Node(err.message));
        }

        rpc_res
            .result
            .ok_or_else(|| RpcError::Node("Missing result in RPC response".to_string()))
    }

    /// Fetch the chain ID from the node
    pub async fn get_chain_id(&self) -> Result<u64, RpcError> {
        let payload = json!({
            "jsonrpc": "2.0",
            "method": "eth_chainId",
            "params": [],
            "id": 1
        });

        let response = self
            .client
            .post(&self.rpc_url)
            .json(&payload)
            .send()
            .await?;

        let rpc_res: serde_json::Value = response.json().await?;

        if let Some(err) = rpc_res.get("error") {
            return Err(RpcError::Node(
                err["message"].as_str().unwrap_or("Unknown").to_string(),
            ));
        }

        let result = rpc_res["result"]
            .as_str()
            .ok_or_else(|| RpcError::Node("Missing result in eth_chainId response".to_string()))?;

        u64::from_str_radix(result.trim_start_matches("0x"), 16)
            .map_err(|e| RpcError::Node(format!("Invalid chainId hex: {}", e)))
    }
}

