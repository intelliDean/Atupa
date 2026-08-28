//! The core [`ProtocolAdapter`] trait for translating low-level EVM execution
//! context (contract addresses and 4-byte function selectors) into
//! human-readable labels.

/// The shared interface that every protocol adapter must implement.
///
/// An adapter identifies whether an execution frame (defined by target address
/// and/or function selector) belongs to its protocol domain and returns a
/// structured, human-readable label (e.g. `"Uniswap v4: beforeSwap"` or
/// `"AaveV3Pool::liquidationCall"`).
pub trait ProtocolAdapter: Send + Sync {
    /// The human-readable name of the protocol (e.g., `"Uniswap v4"`).
    fn name(&self) -> &str;

    /// Resolves a combination of target address and function selector into a
    /// human-readable label.
    ///
    /// Returns `None` if this adapter does not recognise the combination.
    fn resolve_label(&self, address: Option<&str>, selector: Option<&str>) -> Option<String>;

    /// Convenience helper to resolve a label using only a 4-byte function selector.
    fn resolve_selector(&self, selector: &str) -> Option<String> {
        self.resolve_label(None, Some(selector))
    }

    /// Convenience helper to resolve a label using only a contract address.
    fn resolve_address(&self, address: &str) -> Option<String> {
        self.resolve_label(Some(address), None)
    }

    /// Returns `true` if this adapter recognises the given contract address.
    fn matches_address(&self, address: &str) -> bool {
        self.resolve_address(address).is_some()
    }

    /// Returns `true` if this adapter recognises the given 4-byte selector.
    fn matches_selector(&self, selector: &str) -> bool {
        self.resolve_selector(selector).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockAdapter;

    impl ProtocolAdapter for MockAdapter {
        fn name(&self) -> &str {
            "Mock Protocol"
        }

        fn resolve_label(&self, address: Option<&str>, selector: Option<&str>) -> Option<String> {
            if let Some(addr) = address
                && addr.eq_ignore_ascii_case("0x1111111111111111111111111111111111111111")
            {
                return Some("Mock::Vault".to_string());
            }
            if let Some(sel) = selector
                && sel == "0x12345678"
            {
                return Some("Mock::deposit".to_string());
            }
            None
        }
    }

    #[test]
    fn trait_defaults_resolve_selector() {
        let adapter = MockAdapter;
        assert_eq!(
            adapter.resolve_selector("0x12345678"),
            Some("Mock::deposit".to_string())
        );
        assert_eq!(adapter.resolve_selector("0xdeadbeef"), None);
        assert!(adapter.matches_selector("0x12345678"));
        assert!(!adapter.matches_selector("0xdeadbeef"));
    }

    #[test]
    fn trait_defaults_resolve_address() {
        let adapter = MockAdapter;
        assert_eq!(
            adapter.resolve_address("0x1111111111111111111111111111111111111111"),
            Some("Mock::Vault".to_string())
        );
        assert_eq!(
            adapter.resolve_address("0x2222222222222222222222222222222222222222"),
            None
        );
        assert!(adapter.matches_address("0x1111111111111111111111111111111111111111"));
        assert!(!adapter.matches_address("0x2222222222222222222222222222222222222222"));
    }

    #[test]
    fn adapter_name() {
        let adapter = MockAdapter;
        assert_eq!(adapter.name(), "Mock Protocol");
    }
}
