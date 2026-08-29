//! JSON-RPC network client for Arbitrum Nitro & Stylus tracing.

use atupa_rpc::{EthClient, RpcError};
use serde_json::json;

use crate::error::{NitroError, NitroResult};
use crate::stitcher::MixedTraceStitcher;
use crate::types::{StitchedReport, StylusHostIO};

/// Arbitrum Nitro RPC client — fetches and stitches dual-VM traces concurrently.
pub struct NitroClient {
    base_client: EthClient,
    rpc_url: String,
    client: reqwest::Client,
}

impl NitroClient {
    /// Creates a new [`NitroClient`] targeting the given JSON-RPC URL.
    pub fn new(rpc_url: impl Into<String>) -> Self {
        let rpc_url = rpc_url.into();
        Self {
            base_client: EthClient::new(rpc_url.clone()),
            rpc_url,
            client: reqwest::Client::new(),
        }
    }

    /// Fetches the Stylus HostIO trace for `tx_hash` using the `stylusTracer`.
    pub async fn get_stylus_trace(&self, tx_hash: &str) -> NitroResult<Vec<StylusHostIO>> {
        let payload = json!({
            "jsonrpc": "2.0",
            "method": "debug_traceTransaction",
            "params": [tx_hash, { "tracer": "stylusTracer" }],
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
            return Err(NitroError::Rpc(RpcError::Node(
                error["message"].as_str().unwrap_or("Unknown RPC error").to_string(),
            )));
        }

        let result = response.get("result").ok_or_else(|| {
            NitroError::Stitch("Missing 'result' in stylusTracer response".into())
        })?;

        Ok(serde_json::from_value(result.clone())?)
    }

    /// Fetches both EVM and Stylus traces **concurrently**, then stitches them
    /// into a single [`StitchedReport`].
    ///
    /// If the `stylusTracer` is unavailable (e.g. pure-EVM transaction on Arbitrum,
    /// or an older node version), the error is downgraded to a warning and the report
    /// will contain only EVM steps with `total_stylus_ink = 0`.
    pub async fn trace_transaction(&self, tx_hash: &str) -> NitroResult<StitchedReport> {
        let chain_id = self.base_client.get_chain_id().await.unwrap_or(0);
        let is_nitro = is_nitro_chain(chain_id);

        log::info!(
            "atupa-nitro: fetching trace for {} (chain_id: {}, nitro_aware: {})",
            tx_hash,
            chain_id,
            is_nitro
        );

        let (evm_result, stylus_result) = if is_nitro {
            tokio::join!(
                self.base_client.get_transaction_trace(tx_hash),
                self.get_stylus_trace(tx_hash),
            )
        } else {
            (self.base_client.get_transaction_trace(tx_hash).await, Ok(Vec::new()))
        };

        let evm_trace = evm_result?;
        let stylus_trace = stylus_result.unwrap_or_else(|e| {
            log::warn!(
                "atupa-nitro: stylusTracer unavailable for {} ({}); falling back to pure-EVM.",
                tx_hash,
                e,
            );
            Vec::new()
        });

        let report =
            MixedTraceStitcher::stitch(tx_hash, chain_id, evm_trace.struct_logs, stylus_trace);

        log::info!(
            "atupa-nitro: {} steps stitched | network: {} | EVM gas: {} | Stylus ink: {} ({:.2} gas-equiv) | boundaries: {}",
            report.steps.len(),
            chain_id,
            report.total_evm_gas,
            report.total_stylus_ink,
            report.total_stylus_gas_equiv,
            report.vm_boundary_count,
        );

        Ok(report)
    }
}

/// Identifies whether a given chain ID is known to support Nitro / Stylus tracing.
pub fn is_nitro_chain(chain_id: u64) -> bool {
    match chain_id {
        // Known Arbitrum / Nitro chains: One, Nova, Goerli, Sepolia, Stylus testnet, Orbit
        42161 | 42170 | 421611 | 421613 | 421614 | 23011913 => true,
        // Local devnets often used for Nitro (nitro-testnode, anvil)
        1337 | 31337 => true,
        // Known non-Nitro chains (Ethereum mainnet, Sepolia, Holesky, Base, Optimism, Polygon)
        1 | 11155111 | 17000 | 8453 | 84532 | 10 | 11155420 | 137 => false,
        // Unknown: assume potentially Nitro-enabled
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_nitro_chains() {
        assert!(is_nitro_chain(42161)); // Arbitrum One
        assert!(is_nitro_chain(42170)); // Arbitrum Nova
        assert!(is_nitro_chain(421614)); // Arbitrum Sepolia
        assert!(is_nitro_chain(1337)); // Local devnet
    }

    #[test]
    fn detects_non_nitro_chains() {
        assert!(!is_nitro_chain(1)); // Ethereum Mainnet
        assert!(!is_nitro_chain(11155111)); // Ethereum Sepolia
        assert!(!is_nitro_chain(8453)); // Base
        assert!(!is_nitro_chain(10)); // Optimism
    }
}
