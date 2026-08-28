//! [`LidoAdapter`] — [`ProtocolAdapter`] implementation for Lido stETH.

use atupa_adapters::ProtocolAdapter;

use crate::selectors::{resolve_address, resolve_selector};

/// Lido stETH protocol adapter — maps contract addresses and function selectors
/// to human-readable protocol labels.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct LidoAdapter;

impl LidoAdapter {
    /// Create a new [`LidoAdapter`].
    pub fn new() -> Self {
        Self
    }

    /// Resolve a 4-byte selector string to a human-readable label without
    /// requiring an adapter instance.
    pub fn resolve_selector_label(selector: &str) -> Option<String> {
        resolve_selector(selector)
    }
}

impl ProtocolAdapter for LidoAdapter {
    fn name(&self) -> &str {
        "Lido stETH"
    }

    fn resolve_label(&self, address: Option<&str>, selector: Option<&str>) -> Option<String> {
        if let Some(addr) = address
            && let Some(label) = resolve_address(addr)
        {
            return Some(label);
        }
        selector.and_then(resolve_selector)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_name() {
        let adapter = LidoAdapter::new();
        assert_eq!(adapter.name(), "Lido stETH");
    }

    #[test]
    fn resolves_submit_selector() {
        let adapter = LidoAdapter;
        assert_eq!(
            adapter.resolve_label(None, Some("0xa1903eab")),
            Some("stETH::submit".to_string())
        );
    }

    #[test]
    fn resolves_contract_address() {
        let adapter = LidoAdapter;
        assert_eq!(
            adapter.resolve_label(
                Some("0xae7ab96520de3a18e5e111b5eaab095312d7fe84"),
                None
            ),
            Some("Lido::stETH (Lido Core)".to_string())
        );
    }

    #[test]
    fn address_takes_precedence_over_selector() {
        let adapter = LidoAdapter;
        assert_eq!(
            adapter.resolve_label(
                Some("0x7f39c581f595b53c5cb19bd0b3f8da6c935e2ca0"),
                Some("0xa1903eab")
            ),
            Some("Lido::wstETH".to_string())
        );
    }

    #[test]
    fn static_resolver_helper() {
        assert_eq!(
            LidoAdapter::resolve_selector_label("0x8b6ca260"),
            Some("stETH::handleOracleReport".to_string())
        );
        assert_eq!(LidoAdapter::resolve_selector_label("0xdeadbeef"), None);
    }

    #[test]
    fn returns_none_for_unknown_input() {
        let adapter = LidoAdapter;
        assert_eq!(adapter.resolve_label(None, Some("0xdeadbeef")), None);
        assert_eq!(adapter.resolve_label(None, None), None);
    }
}
