//! JSON-RPC client for interacting with Solana validator RPC nodes.

use atupa_rpc::RpcError;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::error::{SolanaError, SolanaResult};

/// JSON-RPC client for querying Solana transaction metadata and program execution logs.
pub struct SolanaClient {
    rpc_url: String,
    client: Client,
}

impl SolanaClient {
    /// Creates a new [`SolanaClient`] pointing to the specified Solana RPC endpoint.
    pub fn new(rpc_url: impl Into<String>) -> Self {
        Self { rpc_url: rpc_url.into(), client: Client::new() }
    }

    /// Returns the target RPC URL.
    pub fn rpc_url(&self) -> &str {
        &self.rpc_url
    }

    /// Retrieves the program log messages for a confirmed transaction signature via `getTransaction`.
    pub async fn get_transaction_logs(&self, tx_sig: &str) -> SolanaResult<Vec<String>> {
        let payload = json!({
            "jsonrpc": "2.0",
            "method": "getTransaction",
            "params": [tx_sig, { "encoding": "json", "maxSupportedTransactionVersion": 0 }],
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
            return Err(SolanaError::Rpc(RpcError::Node(
                error["message"].as_str().unwrap_or("Unknown RPC error").to_string(),
            )));
        }

        let result: SolanaTransactionResponse = serde_json::from_value(response["result"].clone())
            .map_err(|e| SolanaError::Parse(e.to_string()))?;

        result
            .meta
            .and_then(|m| m.log_messages)
            .ok_or_else(|| SolanaError::Parse("No log messages found in transaction".into()))
    }
}

// ─── Response Data Models ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SolanaTransactionResponse {
    pub meta: Option<SolanaMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SolanaMeta {
    #[serde(rename = "logMessages")]
    pub log_messages: Option<Vec<String>>,
    pub fee: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_constructor_and_getter() {
        let client = SolanaClient::new("https://api.mainnet-beta.solana.com");
        assert_eq!(client.rpc_url(), "https://api.mainnet-beta.solana.com");
    }

    #[test]
    fn deserializes_solana_response() {
        let json_str = r#"{
            "meta": {
                "logMessages": ["Program 1111 invoke [1]"],
                "fee": 5000
            }
        }"#;

        let res: SolanaTransactionResponse = serde_json::from_str(json_str).unwrap();
        assert_eq!(res.meta.as_ref().unwrap().fee, 5000);
        assert_eq!(
            res.meta.as_ref().unwrap().log_messages,
            Some(vec!["Program 1111 invoke [1]".to_string()])
        );
    }
}
