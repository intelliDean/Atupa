//! JSON-RPC client for querying Stellar / Soroban node diagnostic events.

use atupa_core::TraceStep;
use atupa_rpc::RpcError;
use reqwest::Client;
use serde_json::json;

use crate::error::{StellarError, StellarResult};
use crate::parser::StellarTraceParser;
use crate::types::StellarTransactionResponse;

/// JSON-RPC client for querying Stellar diagnostic transaction logs.
pub struct StellarClient {
    rpc_url: String,
    client: Client,
}

impl StellarClient {
    /// Creates a new [`StellarClient`] pointing to the specified Stellar/Soroban RPC endpoint.
    pub fn new(rpc_url: impl Into<String>) -> Self {
        Self {
            rpc_url: rpc_url.into(),
            client: Client::new(),
        }
    }

    /// Returns the target RPC URL.
    pub fn rpc_url(&self) -> &str {
        &self.rpc_url
    }

    /// Retrieves diagnostic events for a confirmed transaction and reconstructs [`TraceStep`]s.
    pub async fn get_transaction_trace(&self, tx_hash: &str) -> StellarResult<Vec<TraceStep>> {
        let payload = json!({
            "jsonrpc": "2.0",
            "method": "getTransaction",
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
            return Err(StellarError::Rpc(RpcError::Node(
                error["message"]
                    .as_str()
                    .unwrap_or("Unknown RPC error")
                    .to_string(),
            )));
        }

        let result: StellarTransactionResponse = serde_json::from_value(response["result"].clone())
            .map_err(|e| StellarError::Parse(e.to_string()))?;

        let events = result.diagnostic_events.unwrap_or_default();
        Ok(StellarTraceParser::parse_diagnostic_events(&events))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_constructor_and_getter() {
        let client = StellarClient::new("https://soroban-rpc.mainnet.stellar.org");
        assert_eq!(client.rpc_url(), "https://soroban-rpc.mainnet.stellar.org");
    }
}
