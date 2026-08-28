//! Error types for Starknet RPC and trace processing.

use atupa_rpc::RpcError;
use thiserror::Error;

/// Errors that can occur during Starknet RPC communication or trace extraction.
#[derive(Error, Debug)]
pub enum StarknetError {
    /// HTTP or network layer failure.
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    /// Starknet JSON-RPC node error.
    #[error("RPC error: {0}")]
    Rpc(#[from] RpcError),

    /// JSON serialization or deserialization failure.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Trace processing or normalization error.
    #[error("Processing error: {0}")]
    Process(String),
}

/// Convenience result alias for operations returning [`StarknetError`].
pub type StarknetResult<T> = Result<T, StarknetError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_formatting() {
        let err = StarknetError::Process("missing result field".to_string());
        assert_eq!(err.to_string(), "Processing error: missing result field");
    }
}
