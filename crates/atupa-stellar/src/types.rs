//! Data models for Stellar / Soroban diagnostic event RPC payloads.

use serde::{Deserialize, Serialize};

/// A single Soroban diagnostic or contract event emitted during transaction execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SorobanDiagnosticEvent {
    /// Event category (e.g. `"diagnostic"`, `"contract"`).
    #[serde(rename = "type")]
    pub event_type: String,
    /// Event topic strings representing the function action and target.
    pub topics: Vec<String>,
    /// Base64 XDR or JSON value payload.
    pub value: String,
}

/// JSON-RPC response envelope from Stellar `getTransaction`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct StellarTransactionResponse {
    /// Transaction status string (e.g. `"SUCCESS"`, `"FAILED"`).
    pub status: String,
    /// Transaction hash identifier.
    pub tx_hash: String,
    /// Optional list of diagnostic event logs emitted during Soroban execution.
    pub diagnostic_events: Option<Vec<SorobanDiagnosticEvent>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_diagnostic_event() {
        let json_str = r#"{
            "type": "diagnostic",
            "topics": ["fn_call", "put_contract_data"],
            "value": "AAAAA..."
        }"#;

        let event: SorobanDiagnosticEvent = serde_json::from_str(json_str).unwrap();
        assert_eq!(event.event_type, "diagnostic");
        assert_eq!(event.topics.len(), 2);
        assert_eq!(event.topics[1], "put_contract_data");
    }

    #[test]
    fn deserializes_stellar_response() {
        let json_str = r#"{
            "status": "SUCCESS",
            "tx_hash": "abc123",
            "diagnostic_events": []
        }"#;

        let res: StellarTransactionResponse = serde_json::from_str(json_str).unwrap();
        assert_eq!(res.status, "SUCCESS");
        assert_eq!(res.tx_hash, "abc123");
        assert!(res.diagnostic_events.unwrap().is_empty());
    }
}
