//! Default protocol adapter registry for the Atupa SDK.

use atupa_adapters::{AdapterRegistry, Erc20Adapter};

/// Builds the default adapter registry for the Atupa SDK, pre-loaded with
/// all supported protocol adapters (Uniswap v4, ERC-20, Aave v3 / GHO, Lido stETH).
pub fn build_default_registry() -> AdapterRegistry {
    let mut registry = AdapterRegistry::new();
    registry.register_typed(Erc20Adapter);
    registry.register_typed(atupa_aave::AaveV3Adapter);
    registry.register_typed(atupa_lido::LidoAdapter);
    registry
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_registry_contains_core_adapters() {
        let registry = build_default_registry();
        assert!(!registry.is_empty());
        assert!(registry.contains("Aave v3 / GHO"));
        assert!(registry.contains("Lido stETH"));
        assert!(registry.contains("Uniswap v4"));
        assert!(registry.contains("ERC-20"));
    }
}
