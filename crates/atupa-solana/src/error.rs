//! Error types for Solana RPC and log parsing operations.

use atupa_rpc::RpcError;
use thiserror::Error;

/// Errors that can occur during Solana RPC calls or log reconstruction.
#[derive(Error, Debug)]
pub enum SolanaError {
    /// HTTP or network layer failure.
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    /// Solana JSON-RPC error.
    #[error("RPC error: {0}")]
    Rpc(#[from] RpcError),

    /// Parsing or structure error in transaction log response.
    #[error("Parsing error: {0}")]
    Parse(String),

    /// Request timed out.
    #[error("Timeout error: {0}")]
    Timeout(String),
}

/// Convenience result alias for operations returning [`SolanaError`].
pub type SolanaResult<T> = Result<T, SolanaError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_formatting() {
        let err = SolanaError::Parse("missing logMessages".to_string());
        assert_eq!(err.to_string(), "Parsing error: missing logMessages");

        let err_timeout = SolanaError::Timeout("node did not respond in 30s".to_string());
        assert_eq!(err_timeout.to_string(), "Timeout error: node did not respond in 30s");
    }
}
