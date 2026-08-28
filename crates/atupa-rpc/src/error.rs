//! Error types for JSON-RPC communication and node responses.

use thiserror::Error;

/// Errors that can occur when executing JSON-RPC calls against an EVM/L2 node.
#[derive(Error, Debug)]
pub enum RpcError {
    /// HTTP or network layer failure.
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    /// JSON serialization or deserialization failure.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Node returned a JSON-RPC error response.
    #[error("RPC error: {0}")]
    Node(String),
}

/// Convenience result alias for operations returning [`RpcError`].
pub type RpcResult<T> = Result<T, RpcError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rpc_error_node_display() {
        let err = RpcError::Node("method debug_traceTransaction not found".to_string());
        assert_eq!(
            err.to_string(),
            "RPC error: method debug_traceTransaction not found"
        );
    }
}
