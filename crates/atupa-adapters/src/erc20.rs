//! Built-in [`Erc20Adapter`] for identifying standard ERC-20 / ERC-721 token calls.

use crate::traits::ProtocolAdapter;

/// Common ERC-20, ERC-721, and ERC-2612 4-byte function selectors.
pub const ERC20_SELECTORS: &[(&str, &str)] = &[
    ("0xa9059cbb", "transfer"),
    ("0x23b872dd", "transferFrom"),
    ("0x095ea7b3", "approve"),
    ("0x70a08231", "balanceOf"),
    ("0xdd62ed3e", "allowance"),
    ("0x18160ddd", "totalSupply"),
    ("0x313ce567", "decimals"),
    ("0x06fdde03", "name"),
    ("0x95d89b41", "symbol"),
    ("0x40c10f19", "mint"),
    ("0x42966c68", "burn"),
    ("0xd505accf", "permit"),
];

/// Identifies standard ERC-20, ERC-721, and permit calls in EVM execution traces.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Erc20Adapter;

impl Erc20Adapter {
    /// Create a new [`Erc20Adapter`].
    pub fn new() -> Self {
        Self
    }

    /// Resolve a 4-byte selector string directly to an ERC-20 method name.
    pub fn resolve_erc20_selector(selector: &str) -> Option<&'static str> {
        let selector = selector.trim();
        for &(known_sel, label) in ERC20_SELECTORS {
            if selector.eq_ignore_ascii_case(known_sel) {
                return Some(label);
            }
        }
        None
    }
}

impl ProtocolAdapter for Erc20Adapter {
    fn name(&self) -> &str {
        "ERC-20"
    }

    fn resolve_label(&self, _address: Option<&str>, selector: Option<&str>) -> Option<String> {
        let sel = selector?;
        let label = Self::resolve_erc20_selector(sel)?;
        Some(format!("ERC20::{label}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_name() {
        let adapter = Erc20Adapter::new();
        assert_eq!(adapter.name(), "ERC-20");
    }

    #[test]
    fn resolves_erc20_selectors() {
        let adapter = Erc20Adapter;
        assert_eq!(
            adapter.resolve_label(None, Some("0xa9059cbb")),
            Some("ERC20::transfer".to_string())
        );
        assert_eq!(
            adapter.resolve_label(None, Some("0x23b872dd")),
            Some("ERC20::transferFrom".to_string())
        );
        assert_eq!(
            adapter.resolve_label(None, Some("0x095ea7b3")),
            Some("ERC20::approve".to_string())
        );
        assert_eq!(
            adapter.resolve_label(None, Some("0x70a08231")),
            Some("ERC20::balanceOf".to_string())
        );
    }

    #[test]
    fn static_resolver_is_case_insensitive() {
        assert_eq!(Erc20Adapter::resolve_erc20_selector("0xA9059CBB"), Some("transfer"));
        assert_eq!(Erc20Adapter::resolve_erc20_selector("  0x095ea7b3  "), Some("approve"));
    }

    #[test]
    fn returns_none_for_unknown_selector() {
        let adapter = Erc20Adapter;
        assert_eq!(adapter.resolve_label(None, Some("0xdeadbeef")), None);
        assert_eq!(adapter.resolve_label(None, None), None);
    }
}
