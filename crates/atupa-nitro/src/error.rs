//! Error types for Arbitrum Nitro and Stylus trace processing.

use atupa_rpc::RpcError;
use thiserror::Error;

/// Errors that can occur when querying, parsing, or stitching Arbitrum Nitro traces.
#[derive(Error, Debug)]
pub enum NitroError {
    /// HTTP or connection failure when communicating with the node.
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    /// JSON-RPC node error (e.g. method not found, execution reverted).
    #[error("RPC error: {0}")]
    Rpc(#[from] RpcError),

    /// JSON serialization or deserialization failure.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Trace stitching or alignment inconsistency.
    #[error("Stitching error: {0}")]
    Stitch(String),
}

/// Convenience result alias for operations returning [`NitroError`].
pub type NitroResult<T> = Result<T, NitroError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stitch_error_display() {
        let err = NitroError::Stitch("unaligned step".to_string());
        assert_eq!(err.to_string(), "Stitching error: unaligned step");
    }

    #[test]
    fn rpc_error_conversion() {
        let rpc_err = RpcError::Node("method not supported".to_string());
        let nitro_err: NitroError = rpc_err.into();
        assert!(nitro_err.to_string().contains("RPC error"));
    }
}
