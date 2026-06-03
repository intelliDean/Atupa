/// The shared trait every protocol adapter must implement.
pub trait ProtocolAdapter {
    /// The name of the protocol (e.g., "Uniswap v4").
    fn name(&self) -> &str;

    /// Resolves a combination of target address and function selector into a
    /// human-readable label. Returns `None` if this adapter does not recognise
    /// the combination.
    fn resolve_label(&self, address: Option<&str>, selector: Option<&str>) -> Option<String>;
}

// ─── Built-in adapters ────────────────────────────────────────────────────────

/// Identifies Uniswap v4 Hook interface calls by their 4-byte selectors.
///
/// This adapter lives here because Uniswap v4 has no dedicated `atupa-*` crate.
/// Protocol-specific adapters (Aave, Lido, …) live in their own crates and
/// register themselves into the [`AdapterRegistry`] at the call site.
pub struct UniswapV4Adapter;

impl ProtocolAdapter for UniswapV4Adapter {
    fn name(&self) -> &str {
        "Uniswap v4"
    }

    fn resolve_label(&self, _address: Option<&str>, selector: Option<&str>) -> Option<String> {
        let sel = selector?;
        // Uniswap v4 Hook standard interface selectors
        let label = match sel {
            "0x18a9d381" => "beforeInitialize",
            "0x999dea5d" => "afterInitialize",
            "0x910746f2" => "beforeAddLiquidity",
            "0xefd81287" => "afterAddLiquidity",
            "0xd7386be3" => "beforeRemoveLiquidity",
            "0x1efe5f9e" => "afterRemoveLiquidity",
            "0xe82c3b75" => "beforeSwap",
            "0x14d6eaec" => "afterSwap",
            "0xa3d03227" => "beforeDonate",
            "0x0df2d576" => "afterDonate",
            _ => return None,
        };

        Some(format!("Uniswapv4: {}", label))
    }
}

// ─── Registry ─────────────────────────────────────────────────────────────────

/// A runtime registry of [`ProtocolAdapter`]s.
///
/// Create an empty registry and register exactly the adapters you need:
///
/// ```rust,no_run
/// use atupa_adapters::{AdapterRegistry, UniswapV4Adapter};
///
/// let mut registry = AdapterRegistry::new();
/// registry.register(Box::new(UniswapV4Adapter));
/// // registry.register(Box::new(atupa_aave::AaveV3Adapter::default()));
/// // registry.register(Box::new(atupa_lido::LidoAdapter::default()));
/// ```
///
/// `AdapterRegistry::default()` pre-loads only `UniswapV4Adapter` so that
/// the adapters crate stays free of dependencies on the deep-tracer crates.
pub struct AdapterRegistry {
    adapters: Vec<Box<dyn ProtocolAdapter>>,
}

impl AdapterRegistry {
    /// Creates an empty registry.  Use [`AdapterRegistry::default()`] to get
    /// one pre-loaded with the built-in Uniswap v4 adapter.
    pub fn empty() -> Self {
        Self {
            adapters: Vec::new(),
        }
    }

    /// Register a protocol adapter.
    pub fn register(&mut self, adapter: Box<dyn ProtocolAdapter>) {
        self.adapters.push(adapter);
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

    /// Returns the names of all currently registered adapters.
    pub fn adapter_names(&self) -> Vec<&str> {
        self.adapters.iter().map(|a| a.name()).collect()
    }
}

impl Default for AdapterRegistry {
    /// Pre-loads `UniswapV4Adapter`.  Protocol-specific adapters (Aave, Lido)
    /// must be registered separately to avoid pulling in their crates as
    /// transitive dependencies.
    fn default() -> Self {
        let mut registry = Self::empty();
        registry.register(Box::new(UniswapV4Adapter));
        registry
    }
}

// Preserve the old `new()` alias so existing call-sites don't break.
impl AdapterRegistry {
    /// Alias for [`AdapterRegistry::default()`].
    pub fn new() -> Self {
        Self::default()
    }
}
