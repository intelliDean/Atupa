//! Error types for Stellar/Soroban RPC and event parsing.

use atupa_rpc::RpcError;
use thiserror::Error;

/// Errors that can occur during Stellar/Soroban RPC queries or diagnostic event extraction.
#[derive(Error, Debug)]
pub enum StellarError {
    /// HTTP or network layer failure.
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    /// Stellar JSON-RPC node error.
    #[error("RPC error: {0}")]
    Rpc(#[from] RpcError),

    /// Parsing or structure error in transaction diagnostic events.
    #[error("Parsing error: {0}")]
    Parse(String),

    /// Request timed out.
    #[error("Timeout error: {0}")]
    Timeout(String),
}

/// Convenience result alias for operations returning [`StellarError`].
pub type StellarResult<T> = Result<T, StellarError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_formatting() {
        let err = StellarError::Parse("malformed event payload".to_string());
        assert_eq!(err.to_string(), "Parsing error: malformed event payload");
    }
}
