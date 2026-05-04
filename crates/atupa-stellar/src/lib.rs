use atupa_core::{TraceStep, VmKind};
use atupa_rpc::RpcError;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

// ─── Stellar/Soroban RPC Types ──────────────────────────────────────────────

#[derive(Error, Debug)]
pub enum StellarError {
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("RPC error: {0}")]
    Rpc(#[from] RpcError),
    #[error("Parsing error: {0}")]
    Parse(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SorobanDiagnosticEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub topics: Vec<String>,
    pub value: String, // Base64 XDR or simplified JSON
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StellarTransactionResponse {
    pub status: String,
    pub tx_hash: String,
    pub diagnostic_events: Option<Vec<SorobanDiagnosticEvent>>,
}

// ─── Stellar Trace Parser ─────────────────────────────────────────────────────

pub struct StellarTraceParser;

impl StellarTraceParser {
    /// Maps Stellar diagnostic events to Atupa TraceSteps.
    ///
    /// In a full implementation, this would involve decoding XDR topics
    /// to identify host function calls and their resource consumption.
    pub fn parse_diagnostic_events(events: &[SorobanDiagnosticEvent]) -> Vec<TraceStep> {
        let mut steps = Vec::new();
        let mut depth: u16 = 1;

        for event in events {
            if event.event_type != "diagnostic" {
                continue;
            }

            // In Soroban, diagnostic events for host calls often look like:
            // topics: ["fn_call", "invoke_contract"]
            // or ["fn_return", "invoke_contract"]
            
            let event_action = event.topics.first().map(|s| s.as_str()).unwrap_or("");
            let fn_name = event.topics.get(1).map(|s| s.as_str()).unwrap_or("unknown");

            // Handle depth adjustments for nested contract calls
            if event_action.contains("return") {
                depth = depth.saturating_sub(1);
                continue; // Don't create a step for the return event itself
            }

            let gas_cost = match fn_name {
                name if name.contains("put_contract_data") => 5000,
                name if name.contains("get_contract_data") => 2100,
                name if name.contains("crypto") || name.contains("hash") => 3000,
                name if name.contains("invoke") => 1500,
                _ => 100, // base cost for generic host functions
            };

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

            // If it was an invocation, subsequent events happen at a deeper level
            if fn_name.contains("invoke_contract") && event_action.contains("call") {
                depth += 1;
            }
        }

        steps
    }
}

// ─── Stellar Client ───────────────────────────────────────────────────────────

pub struct StellarClient {
    rpc_url: String,
    client: reqwest::Client,
}

impl StellarClient {
    pub fn new(rpc_url: String) -> Self {
        Self {
            rpc_url,
            client: reqwest::Client::new(),
        }
    }

    pub async fn get_transaction_trace(&self, tx_hash: &str) -> Result<Vec<TraceStep>, StellarError> {
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
                error["message"].as_str().unwrap_or("Unknown RPC error").to_string(),
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
    fn test_stellar_event_parsing() {
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
        assert_eq!(steps[0].gas_cost, 1500);

        assert_eq!(steps[1].op, "put_contract_data");
        assert_eq!(steps[1].depth, 2); // Depth increased after invoke_contract
        assert_eq!(steps[1].gas_cost, 5000);
    }
}
