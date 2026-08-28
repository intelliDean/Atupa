//! [`AaveV3Adapter`] — [`ProtocolAdapter`] implementation for Aave v3 & GHO.

use atupa_adapters::ProtocolAdapter;

use crate::selectors::{resolve_address, resolve_selector};

/// Aave v3 + GHO protocol adapter — maps contract addresses and 4-byte
/// selectors to human-readable labels for flamegraph annotation and deep-trace
/// audits.
///
/// Resolution priority (highest to lowest):
/// 1. GHO Facilitator address → `"Facilitator::*"`
/// 2. Aave Oracle address     → `"Oracle::*"`
/// 3. Pool selector           → `"AaveV3Pool::*"`
/// 4. GHO selector            → `"GHO::*"`
#[derive(Default)]
pub struct AaveV3Adapter;

impl ProtocolAdapter for AaveV3Adapter {
    fn name(&self) -> &str {
        "Aave v3 / GHO"
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

impl AaveV3Adapter {
    /// Resolve a 4-byte selector string to a human-readable label without
    /// requiring an adapter instance.
    ///
    /// Returns `None` if the selector is not found in either the Pool or GHO
    /// selector tables.
    pub fn resolve_selector_label(selector: &str) -> Option<String> {
        resolve_selector(selector)
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_pool_selector() {
        let adapter = AaveV3Adapter;
        assert_eq!(
            adapter.resolve_label(None, Some("0x00a718a9")),
            Some("AaveV3Pool::liquidationCall".to_string())
        );
    }

    #[test]
    fn resolves_gho_selector() {
        let adapter = AaveV3Adapter;
        assert_eq!(
            adapter.resolve_label(None, Some("0x40c10f19")),
            Some("GHO::mint".to_string())
        );
    }

    #[test]
    fn resolves_facilitator_address() {
        let adapter = AaveV3Adapter;
        assert_eq!(
            adapter.resolve_label(
                Some("0x5513224daaEABCa31af5280727878d52097afA05"),
                None
            ),
            Some("Facilitator::Direct Minter (Aave V3)".to_string())
        );
    }

    #[test]
    fn resolves_oracle_address() {
        let adapter = AaveV3Adapter;
        assert_eq!(
            adapter.resolve_label(
                Some("0x54586bE62E3c3580375aE3716C14bd2563060Ca0"),
                None
            ),
            Some("Oracle::Aave Price Oracle".to_string())
        );
    }

    #[test]
    fn address_takes_priority_over_selector() {
        // When both are provided, address resolution should win.
        let adapter = AaveV3Adapter;
        let label = adapter.resolve_label(
            Some("0x5513224daaeabca31af5280727878d52097afa05"),
            Some("0x00a718a9"),
        );
        assert!(label.as_deref().unwrap_or("").starts_with("Facilitator::"));
    }

    #[test]
    fn returns_none_for_unknown_inputs() {
        let adapter = AaveV3Adapter;
        assert!(adapter.resolve_label(None, Some("0xdeadbeef")).is_none());
        assert!(adapter.resolve_label(None, None).is_none());
    }

    #[test]
    fn static_resolve_selector_label() {
        assert_eq!(
            AaveV3Adapter::resolve_selector_label("0x9dc29fac"),
            Some("GHO::burn".to_string())
        );
        assert!(AaveV3Adapter::resolve_selector_label("0xdeadbeef").is_none());
    }
}
