//! JSON-RPC client implementation for interacting with Ethereum / EVM nodes.

use reqwest::Client;
use serde_json::json;

use crate::error::{RpcError, RpcResult};
use crate::types::{RpcResponse, TraceResult};

/// Lightweight HTTP JSON-RPC client for EVM node querying and debug trace retrieval.
pub struct EthClient {
    rpc_url: String,
    client: Client,
}

impl EthClient {
    /// Creates a new [`EthClient`] connected to the given JSON-RPC URL.
    pub fn new(rpc_url: impl Into<String>) -> Self {
        Self { rpc_url: rpc_url.into(), client: Client::new() }
    }

    /// Returns the target RPC URL.
    pub fn rpc_url(&self) -> &str {
        &self.rpc_url
    }

    /// Fetch a raw `debug_traceTransaction` structLog response from the node.
    pub async fn get_transaction_trace(&self, tx_hash: &str) -> RpcResult<TraceResult> {
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

        let response = self.client.post(&self.rpc_url).json(&payload).send().await?;

        let rpc_res: RpcResponse<TraceResult> = response.json().await?;

        if let Some(err) = rpc_res.error {
            return Err(RpcError::Node(err.message));
        }

        rpc_res.result.ok_or_else(|| RpcError::Node("Missing result in RPC response".to_string()))
    }

    /// Fetch the chain ID from the node (`eth_chainId`).
    pub async fn get_chain_id(&self) -> RpcResult<u64> {
        let payload = json!({
            "jsonrpc": "2.0",
            "method": "eth_chainId",
            "params": [],
            "id": 1
        });

        let response = self.client.post(&self.rpc_url).json(&payload).send().await?;

        let rpc_res: serde_json::Value = response.json().await?;

        if let Some(err) = rpc_res.get("error") {
            return Err(RpcError::Node(err["message"].as_str().unwrap_or("Unknown").to_string()));
        }

        let result = rpc_res["result"]
            .as_str()
            .ok_or_else(|| RpcError::Node("Missing result in eth_chainId response".to_string()))?;

        u64::from_str_radix(result.trim_start_matches("0x"), 16)
            .map_err(|e| RpcError::Node(format!("Invalid chainId hex: {e}")))
    }

    /// Fetch the actual on-chain `gasUsed` from `eth_getTransactionReceipt`.
    ///
    /// Returns `None` if the receipt is unavailable or the call fails (non-fatal).
    pub async fn get_gas_used(&self, tx_hash: &str) -> Option<u64> {
        let payload = json!({
            "jsonrpc": "2.0",
            "method": "eth_getTransactionReceipt",
            "params": [tx_hash],
            "id": 1
        });

        let response = self.client.post(&self.rpc_url).json(&payload).send().await.ok()?;

        let rpc_res: serde_json::Value = response.json().await.ok()?;
        let gas_hex = rpc_res["result"]["gasUsed"].as_str()?;
        u64::from_str_radix(gas_hex.trim_start_matches("0x"), 16).ok()
    }

    /// Fetch the raw `input` (calldata) of a transaction via `eth_getTransactionByHash`.
    ///
    /// Returns `None` if the transaction is not found or the call fails (non-fatal).
    pub async fn get_transaction_input(&self, tx_hash: &str) -> Option<String> {
        let payload = json!({
            "jsonrpc": "2.0",
            "method": "eth_getTransactionByHash",
            "params": [tx_hash],
            "id": 1
        });

        let response = self.client.post(&self.rpc_url).json(&payload).send().await.ok()?;

        let rpc_res: serde_json::Value = response.json().await.ok()?;
        rpc_res["result"]["input"].as_str().map(|s| s.to_string())
    }

    /// Extract the 4-byte function selector from raw transaction calldata.
    ///
    /// Returns a lowercase hex string like `"0xa9059cbb"`, or `None` if calldata is too short.
    pub fn selector_from_input(input: &str) -> Option<String> {
        let stripped = input.trim_start_matches("0x");
        if stripped.len() < 8 {
            return None;
        }
        Some(format!("0x{}", &stripped[..8].to_lowercase()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selector_from_input_extraction() {
        assert_eq!(
            EthClient::selector_from_input("0xa9059cbb0000000000000000000000001234"),
            Some("0xa9059cbb".to_string())
        );
        assert_eq!(
            EthClient::selector_from_input("A9059CBB0000000000000000000000001234"),
            Some("0xa9059cbb".to_string())
        );
        assert_eq!(EthClient::selector_from_input("0x123"), None);
        assert_eq!(EthClient::selector_from_input(""), None);
    }

    #[test]
    fn client_constructor_and_getter() {
        let client = EthClient::new("http://localhost:8545");
        assert_eq!(client.rpc_url(), "http://localhost:8545");
    }
}
