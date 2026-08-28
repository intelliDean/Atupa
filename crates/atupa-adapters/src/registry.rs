//! Runtime registry for managing and querying active [`ProtocolAdapter`] instances.

use crate::traits::ProtocolAdapter;
use crate::uniswap_v4::UniswapV4Adapter;

/// A runtime registry of [`ProtocolAdapter`] instances.
///
/// Adapters are checked sequentially in the order they were registered. The
/// first adapter to return a `Some(label)` for a given address and/or selector
/// provides the resolved label.
///
/// # Examples
///
/// ```rust
/// use atupa_adapters::{AdapterRegistry, UniswapV4Adapter, Erc20Adapter};
///
/// // Create a default registry (pre-loaded with UniswapV4Adapter)
/// let mut registry = AdapterRegistry::default();
/// registry.register_typed(Erc20Adapter);
///
/// // Resolve a Uniswap v4 Hook selector
/// let label = registry.resolve(None, Some("0xe82c3b75"));
/// assert_eq!(label, Some("Uniswapv4: beforeSwap".to_string()));
///
/// // Resolve an ERC-20 transfer
/// let label = registry.resolve(None, Some("0xa9059cbb"));
/// assert_eq!(label, Some("ERC20::transfer".to_string()));
/// ```
pub struct AdapterRegistry {
    adapters: Vec<Box<dyn ProtocolAdapter>>,
}

impl Default for AdapterRegistry {
    /// Pre-loads [`UniswapV4Adapter`]. Protocol-specific adapters (Aave, Lido)
    /// must be registered separately to avoid pulling in their crates as
    /// transitive dependencies.
    fn default() -> Self {
        let mut registry = Self::empty();
        registry.register_typed(UniswapV4Adapter);
        registry
    }
}

impl AdapterRegistry {
    /// Creates an empty registry with no adapters loaded.
    pub fn empty() -> Self {
        Self {
            adapters: Vec::new(),
        }
    }

    /// Creates a new registry with default built-in adapters loaded.
    ///
    /// Alias for [`AdapterRegistry::default()`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a boxed [`ProtocolAdapter`].
    pub fn register(&mut self, adapter: Box<dyn ProtocolAdapter>) {
        self.adapters.push(adapter);
    }

    /// Register a typed [`ProtocolAdapter`] without needing explicit `Box::new`.
    pub fn register_typed<T: ProtocolAdapter + 'static>(&mut self, adapter: T) {
        self.adapters.push(Box::new(adapter));
    }

    /// Builder pattern: attach an adapter and return `self`.
    pub fn with_adapter<T: ProtocolAdapter + 'static>(mut self, adapter: T) -> Self {
        self.register_typed(adapter);
        self
    }

    /// Builder pattern: attach a boxed adapter and return `self`.
    pub fn with_boxed_adapter(mut self, adapter: Box<dyn ProtocolAdapter>) -> Self {
        self.register(adapter);
        self
    }

    /// Walk every registered adapter and return the first label match found.
    pub fn resolve(&self, address: Option<&str>, selector: Option<&str>) -> Option<String> {
        for adapter in &self.adapters {
            if let Some(label) = adapter.resolve_label(address, selector) {
                return Some(label);
            }
        }
        None
    }

    /// Resolve a label using only a 4-byte function selector.
    pub fn resolve_selector(&self, selector: &str) -> Option<String> {
        self.resolve(None, Some(selector))
    }

    /// Resolve a label using only a contract address.
    pub fn resolve_address(&self, address: &str) -> Option<String> {
        self.resolve(Some(address), None)
    }

    /// Returns the number of currently registered adapters.
    pub fn len(&self) -> usize {
        self.adapters.len()
    }

    /// Returns `true` if no adapters are registered.
    pub fn is_empty(&self) -> bool {
        self.adapters.is_empty()
    }

    /// Clear all registered adapters from the registry.
    pub fn clear(&mut self) {
        self.adapters.clear();
    }

    /// Returns `true` if an adapter with the specified name is currently registered.
    pub fn contains(&self, name: &str) -> bool {
        self.adapters.iter().any(|a| a.name() == name)
    }

    /// Returns an iterator over references to all registered adapters.
    pub fn iter(&self) -> impl Iterator<Item = &dyn ProtocolAdapter> {
        self.adapters.iter().map(|b| b.as_ref())
    }

    /// Returns the names of all currently registered adapters in order.
    pub fn adapter_names(&self) -> Vec<&str> {
        self.adapters.iter().map(|a| a.name()).collect()
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::erc20::Erc20Adapter;

    struct MockCustomAdapter;

    impl ProtocolAdapter for MockCustomAdapter {
        fn name(&self) -> &str {
            "CustomProtocol"
        }

        fn resolve_label(&self, address: Option<&str>, selector: Option<&str>) -> Option<String> {
            if let Some(addr) = address
                && addr == "0x1234"
            {
                return Some("Custom::Target".to_string());
            }
            if let Some(sel) = selector
                && sel == "0x9999"
            {
                return Some("Custom::action".to_string());
            }
            None
        }
    }

    #[test]
    fn default_registry_contains_uniswap_v4() {
        let registry = AdapterRegistry::default();
        assert_eq!(registry.len(), 1);
        assert!(!registry.is_empty());
        assert!(registry.contains("Uniswap v4"));
        assert_eq!(registry.adapter_names(), vec!["Uniswap v4"]);
    }

    #[test]
    fn empty_registry() {
        let registry = AdapterRegistry::empty();
        assert_eq!(registry.len(), 0);
        assert!(registry.is_empty());
        assert_eq!(registry.adapter_names().len(), 0);
        assert_eq!(registry.resolve(None, Some("0x18a9d381")), None);
    }

    #[test]
    fn register_typed_and_boxed() {
        let mut registry = AdapterRegistry::empty();
        registry.register_typed(UniswapV4Adapter);
        registry.register(Box::new(MockCustomAdapter));

        assert_eq!(registry.len(), 2);
        assert_eq!(registry.adapter_names(), vec!["Uniswap v4", "CustomProtocol"]);
        assert_eq!(
            registry.resolve_selector("0x9999"),
            Some("Custom::action".to_string())
        );
        assert_eq!(
            registry.resolve_address("0x1234"),
            Some("Custom::Target".to_string())
        );
    }

    #[test]
    fn builder_pattern_chaining() {
        let registry = AdapterRegistry::empty()
            .with_adapter(UniswapV4Adapter)
            .with_adapter(Erc20Adapter)
            .with_boxed_adapter(Box::new(MockCustomAdapter));

        assert_eq!(registry.len(), 3);
        assert_eq!(
            registry.adapter_names(),
            vec!["Uniswap v4", "ERC-20", "CustomProtocol"]
        );
    }

    #[test]
    fn resolution_order_priority() {
        struct OverrideAdapter;
        impl ProtocolAdapter for OverrideAdapter {
            fn name(&self) -> &str {
                "Override"
            }
            fn resolve_label(&self, _address: Option<&str>, selector: Option<&str>) -> Option<String> {
                if selector == Some("0x18a9d381") {
                    Some("Overridden!".to_string())
                } else {
                    None
                }
            }
        }

        // Register OverrideAdapter BEFORE UniswapV4Adapter
        let mut registry = AdapterRegistry::empty();
        registry.register_typed(OverrideAdapter);
        registry.register_typed(UniswapV4Adapter);

        assert_eq!(
            registry.resolve_selector("0x18a9d381"),
            Some("Overridden!".to_string())
        );
    }

    #[test]
    fn clear_registry() {
        let mut registry = AdapterRegistry::default();
        assert!(!registry.is_empty());
        registry.clear();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn iter_registered_adapters() {
        let registry = AdapterRegistry::empty()
            .with_adapter(UniswapV4Adapter)
            .with_adapter(Erc20Adapter);

        let names: Vec<&str> = registry.iter().map(|a| a.name()).collect();
        assert_eq!(names, vec!["Uniswap v4", "ERC-20"]);
    }
}
