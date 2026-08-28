//! JSON-RPC client for querying Starknet node execution traces.

use atupa_core::TraceStep;
use atupa_rpc::RpcError;
use reqwest::Client;
use serde_json::json;

use crate::error::{StarknetError, StarknetResult};
use crate::flattener::{flatten_invocation, flatten_trace};
use crate::types::{FunctionInvocation, StarknetTransactionTrace};

/// JSON-RPC client for querying Starknet execution traces and Cairo resource counters.
pub struct StarknetClient {
    rpc_url: String,
    client: Client,
}

impl StarknetClient {
    /// Creates a new [`StarknetClient`] pointing to the specified Starknet RPC endpoint.
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

    /// Retrieves the execution trace of a transaction via `starknet_traceTransaction`.
    pub async fn get_transaction_trace(
        &self,
        tx_hash: &str,
    ) -> StarknetResult<StarknetTransactionTrace> {
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

    /// Recursively flattens a Starknet function invocation into [`TraceStep`]s.
    pub fn flatten_trace(&self, invocation: &FunctionInvocation, depth: u16) -> Vec<TraceStep> {
        flatten_invocation(invocation, depth)
    }

    /// Profiles a transaction by fetching its trace and flattening all execution phases into [`TraceStep`]s.
    pub async fn profile_transaction(&self, tx_hash: &str) -> StarknetResult<Vec<TraceStep>> {
        let trace = self.get_transaction_trace(tx_hash).await?;
        Ok(flatten_trace(&trace))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_constructor_and_getter() {
        let client = StarknetClient::new("https://starknet-mainnet.public.blastapi.io");
        assert_eq!(
            client.rpc_url(),
            "https://starknet-mainnet.public.blastapi.io"
        );
    }
}
