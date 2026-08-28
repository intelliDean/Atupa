//! [`VmKind`] — identifies which Virtual Machine produced a set of execution steps.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Identifies which Virtual Machine produced a set of execution trace steps.
///
/// Marked `#[non_exhaustive]` so that downstream crates handle future VM
/// additions gracefully (via wildcard arms) rather than failing to compile.
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default, Hash)]
pub enum VmKind {
    /// Standard Ethereum Virtual Machine (also used for Arbitrum EVM frames).
    #[default]
    Evm,
    /// Arbitrum Stylus WASM Host I/O frames.
    Stylus,
    /// Starknet Cairo VM execution frames.
    Starknet,
    /// Solana Sealevel VM (SVM) / Cross-Program Invocation frames.
    Solana,
    /// Stellar Soroban Host Function frames.
    Stellar,
}

impl fmt::Display for VmKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VmKind::Evm => write!(f, "EVM"),
            VmKind::Stylus => write!(f, "Stylus"),
            VmKind::Starknet => write!(f, "Starknet"),
            VmKind::Solana => write!(f, "Solana"),
            VmKind::Stellar => write!(f, "Stellar"),
        }
    }
}

// ── Parsing ───────────────────────────────────────────────────────────────────

/// Error returned when a string cannot be parsed into a [`VmKind`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseVmKindError(pub String);

impl fmt::Display for ParseVmKindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unknown VM kind {:?} — expected one of: evm, stylus, starknet, solana, stellar",
            self.0
        )
    }
}

impl std::error::Error for ParseVmKindError {}

impl TryFrom<&str> for VmKind {
    type Error = ParseVmKindError;

    /// Parse a VM kind from a string (case-insensitive).
    ///
    /// `"soroban"` is accepted as an alias for [`VmKind::Stellar`].
    ///
    /// ```
    /// use atupa_core::VmKind;
    ///
    /// assert_eq!(VmKind::try_from("evm").unwrap(), VmKind::Evm);
    /// assert_eq!(VmKind::try_from("Solana").unwrap(), VmKind::Solana);
    /// assert_eq!(VmKind::try_from("soroban").unwrap(), VmKind::Stellar);
    /// assert!(VmKind::try_from("cosmos").is_err());
    /// ```
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s.to_lowercase().as_str() {
            "evm" => Ok(VmKind::Evm),
            "stylus" => Ok(VmKind::Stylus),
            "starknet" => Ok(VmKind::Starknet),
            "solana" => Ok(VmKind::Solana),
            "stellar" | "soroban" => Ok(VmKind::Stellar),
            other => Err(ParseVmKindError(other.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_formats_correctly() {
        assert_eq!(VmKind::Evm.to_string(), "EVM");
        assert_eq!(VmKind::Stylus.to_string(), "Stylus");
        assert_eq!(VmKind::Starknet.to_string(), "Starknet");
        assert_eq!(VmKind::Solana.to_string(), "Solana");
        assert_eq!(VmKind::Stellar.to_string(), "Stellar");
    }

    #[test]
    fn try_from_is_case_insensitive() {
        assert_eq!(VmKind::try_from("evm").unwrap(), VmKind::Evm);
        assert_eq!(VmKind::try_from("EVM").unwrap(), VmKind::Evm);
        assert_eq!(VmKind::try_from("Stylus").unwrap(), VmKind::Stylus);
        assert_eq!(VmKind::try_from("STARKNET").unwrap(), VmKind::Starknet);
        assert_eq!(VmKind::try_from("Solana").unwrap(), VmKind::Solana);
    }

    #[test]
    fn soroban_alias_maps_to_stellar() {
        assert_eq!(VmKind::try_from("soroban").unwrap(), VmKind::Stellar);
        assert_eq!(VmKind::try_from("stellar").unwrap(), VmKind::Stellar);
    }

    #[test]
    fn unknown_vm_returns_error() {
        let err = VmKind::try_from("cosmos").unwrap_err();
        assert!(err.to_string().contains("cosmos"));
    }

    #[test]
    fn empty_string_returns_error() {
        assert!(VmKind::try_from("").is_err());
    }
}
