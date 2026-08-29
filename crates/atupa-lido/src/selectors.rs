//! Selectors, contract addresses, and lookup helpers for Lido stETH protocol analysis.

use atupa_core::TraceStep;

// ─── Constants ────────────────────────────────────────────────────────────────

/// Selectors for major Lido stETH and wstETH protocol operations.
pub(crate) const LIDO_SELECTORS: &[(&str, &str)] = &[
    ("0xa1903eab", "submit"),             // stETH.submit(address _referral)
    ("0xea598cb0", "requestWithdrawals"), // Legacy request withdrawals
    ("0x826a73d6", "requestWithdrawalsWithPermit"),
    ("0xe35ea9a5", "claimWithdrawals"),
    ("0x8b6ca260", "handleOracleReport"), // Rebase oracle consensus
    ("0x39ba163b", "transferShares"),
    ("0x4dbcaef1", "transferSharesFrom"),
    ("0xa9059cbb", "transfer"), // ERC-20 generic
    ("0x095ea7b3", "approve"),  // ERC-20 generic
    ("0x0a19ea81", "wrap"),     // wstETH wrap
    ("0x1dfab2e1", "unwrap"),   // wstETH unwrap
];

/// Known Lido protocol contract addresses (Ethereum Mainnet, stored lowercase).
pub(crate) const LIDO_ADDRESSES: &[(&str, &str)] = &[
    ("0xae7ab96520de3a18e5e111b5eaab095312d7fe84", "stETH (Lido Core)"),
    ("0x55032650b14df07b85bf18a3a3ec8e0af2e028d5", "NodeOperatorsRegistry"),
    ("0x442af752419395f27ed54a848524a30028962bb2", "LidoOracle"),
    ("0x889edc2bf57978ed079b851d273218ee42a2b349", "WithdrawalQueue"),
    ("0x852f970761d74367f33b6c2e309a29d681e2f16a", "LegacyOracle"),
    ("0x7f39c581f595b53c5cb19bd0b3f8da6c935e2ca0", "wstETH"),
];

// ─── Lookup helpers ───────────────────────────────────────────────────────────

/// Look up a contract address in the known Lido addresses table.
///
/// Comparison is case-insensitive.
pub(crate) fn resolve_address(addr: &str) -> Option<String> {
    let lower = addr.to_lowercase();
    for &(known, name) in LIDO_ADDRESSES {
        if lower == known {
            return Some(format!("Lido::{name}"));
        }
    }
    None
}

/// Look up a 4-byte function selector in the Lido selectors table.
///
/// Accepts selectors with or without a `0x` prefix and performs case-insensitive
/// matching.
pub(crate) fn resolve_selector(selector: &str) -> Option<String> {
    let clean_sel = selector.trim().trim_start_matches("0x").to_lowercase();
    for &(known_sel, label) in LIDO_SELECTORS {
        let known_clean = known_sel.trim_start_matches("0x");
        if clean_sel == known_clean || clean_sel.starts_with(known_clean) {
            return Some(format!("stETH::{label}"));
        }
    }
    None
}

/// Returns `true` for EVM opcodes that initiate a new call frame.
#[inline]
pub(crate) fn is_call_opcode(op: &str) -> bool {
    matches!(op, "CALL" | "STATICCALL" | "DELEGATECALL" | "CALLCODE")
}

/// Extract the top-of-stack value from a [`TraceStep`] as a selector string.
pub(crate) fn selector_from_stack(step: &TraceStep) -> Option<&str> {
    step.stack.as_ref()?.last().map(String::as_str)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_selector_exact_and_prefixed() {
        assert_eq!(resolve_selector("0xa1903eab"), Some("stETH::submit".to_string()));
        assert_eq!(resolve_selector("a1903eab"), Some("stETH::submit".to_string()));
        assert_eq!(resolve_selector("0xA1903EAB"), Some("stETH::submit".to_string()));
        assert_eq!(resolve_selector("0x0a19ea81"), Some("stETH::wrap".to_string()));
        assert_eq!(resolve_selector("0x1dfab2e1"), Some("stETH::unwrap".to_string()));
    }

    #[test]
    fn resolve_selector_unknown_returns_none() {
        assert!(resolve_selector("0xdeadbeef").is_none());
        assert!(resolve_selector("").is_none());
    }

    #[test]
    fn resolve_address_case_insensitive() {
        assert_eq!(
            resolve_address("0xae7ab96520DE3A18E5e111B5EaAb095312D7fE84"),
            Some("Lido::stETH (Lido Core)".to_string())
        );
        assert_eq!(
            resolve_address("0x7f39C581F595B53c5cb19bD0b3f8dA6c935E2Ca0"),
            Some("Lido::wstETH".to_string())
        );
    }

    #[test]
    fn resolve_address_unknown_returns_none() {
        assert!(resolve_address("0x0000000000000000000000000000000000000000").is_none());
    }

    #[test]
    fn is_call_opcode_detects_call_variants() {
        for op in &["CALL", "STATICCALL", "DELEGATECALL", "CALLCODE"] {
            assert!(is_call_opcode(op));
        }
        for op in &["SLOAD", "SSTORE", "REVERT", "JUMP"] {
            assert!(!is_call_opcode(op));
        }
    }

    #[test]
    fn selector_from_stack_extracts_last_item() {
        let step = TraceStep {
            op: "CALL".to_string(),
            stack: Some(vec!["0x1111".to_string(), "0xa1903eab".to_string()]),
            ..Default::default()
        };
        assert_eq!(selector_from_stack(&step), Some("0xa1903eab"));
    }
}
