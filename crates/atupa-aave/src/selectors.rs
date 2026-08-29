//! Selector and address tables for Aave v3 & GHO, with shared lookup helpers.

use atupa_core::TraceStep;

// ─── Constants ────────────────────────────────────────────────────────────────

/// Gas cost baseline used to normalise the liquidation efficiency score.
pub(crate) const LIQUIDATION_EFFICIENCY_BASE: f64 = 100_000.0;

/// Known Aave v3 Pool function selectors → human-readable labels.
pub(crate) const POOL_SELECTORS: &[(&str, &str)] = &[
    ("0x617ba037", "supply"),
    ("0x69328dec", "withdraw"),
    ("0xa415bcad", "borrow"),
    ("0x573ade81", "repay"),
    ("0x563dd613", "repayWithPermit"),
    ("0x2dad97d4", "repayWithATokens"),
    ("0x00a718a9", "liquidationCall"),
    ("0xab9c4b5d", "flashLoan"),
    ("0x42b0b77c", "flashLoanSimple"),
    ("0xe8eda9df", "deposit"),      // v2 compatibility alias
    ("0xa9059cbb", "transfer"),     // ERC-20 — common inside traces
    ("0x23b872dd", "transferFrom"), // ERC-20
    ("0x095ea7b3", "approve"),      // ERC-20
    ("0x1e9a6950", "setUserUseReserveAsCollateral"),
    ("0x02c205f0", "swapBorrowRateMode"),
    ("0x1e9d0e2e", "claimRewards"),
];

/// Known GHO-specific function selectors → human-readable labels.
pub(crate) const GHO_SELECTORS: &[(&str, &str)] = &[
    ("0x40c10f19", "mint"),
    ("0x9dc29fac", "burn"),
    ("0xd73dd623", "increaseAllowance"),
    ("0x5d3a1f9b", "distributeFeesToTreasury"),
    ("0x2e0f2625", "updateFacilitatorBucketCapacity"),
    ("0xdb5a3c5e", "setVariableDebtToken"),
];

/// Known GHO Facilitator addresses (Ethereum Mainnet, stored lowercase).
pub(crate) const GHO_FACILITATORS: &[(&str, &str)] = &[
    ("0x5513224daaeabca31af5280727878d52097afa05", "Direct Minter (Aave V3)"),
    ("0xbc65ad17c5c0a2a4d159fa5a503f4992c7b545fe", "Spark (Sky) Facilitator"),
];

/// Known Aave oracle addresses (Ethereum Mainnet, stored lowercase).
pub(crate) const AAVE_ORACLES: &[(&str, &str)] = &[
    ("0x54586be62e3c3580375ae3716c14bd2563060ca0", "Aave Price Oracle"),
    ("0x3f12643d3f6f874d39c2a4c9f2cd6f2dbac877f", "GHO Price Oracle"),
];

// ─── Lookup helpers ───────────────────────────────────────────────────────────

/// Look up a contract address in the facilitator and oracle tables.
///
/// The comparison is case-insensitive; all stored entries are already lowercase.
pub(crate) fn resolve_address(addr: &str) -> Option<String> {
    let lower = addr.to_lowercase();

    for &(known, name) in GHO_FACILITATORS {
        if lower == known {
            return Some(format!("Facilitator::{name}"));
        }
    }
    for &(known, name) in AAVE_ORACLES {
        if lower == known {
            return Some(format!("Oracle::{name}"));
        }
    }
    None
}

/// Look up a 4-byte selector in the Pool and GHO selector tables.
pub(crate) fn resolve_selector(selector: &str) -> Option<String> {
    for &(known, label) in POOL_SELECTORS {
        if selector == known {
            return Some(format!("AaveV3Pool::{label}"));
        }
    }
    for &(known, label) in GHO_SELECTORS {
        if selector == known {
            return Some(format!("GHO::{label}"));
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
    fn resolve_selector_pool() {
        assert_eq!(resolve_selector("0x617ba037"), Some("AaveV3Pool::supply".to_string()));
    }

    #[test]
    fn resolve_selector_gho() {
        assert_eq!(resolve_selector("0x40c10f19"), Some("GHO::mint".to_string()));
    }

    #[test]
    fn resolve_selector_unknown_returns_none() {
        assert!(resolve_selector("0xdeadbeef").is_none());
    }

    #[test]
    fn resolve_address_facilitator_case_insensitive() {
        let mixed = "0x5513224daaEABCa31af5280727878d52097afA05";
        assert_eq!(
            resolve_address(mixed),
            Some("Facilitator::Direct Minter (Aave V3)".to_string())
        );
    }

    #[test]
    fn resolve_address_oracle() {
        let addr = "0x54586bE62E3c3580375aE3716C14bd2563060Ca0";
        assert_eq!(resolve_address(addr), Some("Oracle::Aave Price Oracle".to_string()));
    }

    #[test]
    fn resolve_address_unknown_returns_none() {
        assert!(resolve_address("0xdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef").is_none());
    }

    #[test]
    fn is_call_opcode_detects_all_variants() {
        for op in &["CALL", "STATICCALL", "DELEGATECALL", "CALLCODE"] {
            assert!(is_call_opcode(op), "{op} should be a call opcode");
        }
    }

    #[test]
    fn is_call_opcode_rejects_non_calls() {
        for op in &["SLOAD", "SSTORE", "ADD", "CREATE", "JUMPDEST"] {
            assert!(!is_call_opcode(op), "{op} should not be a call opcode");
        }
    }

    #[test]
    fn selector_from_stack_returns_last_element() {
        let step = atupa_core::TraceStep {
            op: "CALL".to_string(),
            stack: Some(vec!["0xaaaa".to_string(), "0x617ba037".to_string()]),
            ..Default::default()
        };
        assert_eq!(selector_from_stack(&step), Some("0x617ba037"));
    }

    #[test]
    fn selector_from_stack_returns_none_for_empty_stack() {
        let step = atupa_core::TraceStep { stack: Some(vec![]), ..Default::default() };
        assert!(selector_from_stack(&step).is_none());
    }

    #[test]
    fn selector_from_stack_returns_none_when_no_stack() {
        let step = atupa_core::TraceStep::default();
        assert!(selector_from_stack(&step).is_none());
    }
}
