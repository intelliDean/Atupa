pub trait ProtocolAdapter {
    /// The name of the protocol (e.g., "Uniswap v4").
    fn name(&self) -> &str;
    
    /// Resolves a combination of target address and function selector into a human-readable label.
    fn resolve_label(&self, address: Option<&str>, selector: Option<&str>) -> Option<String>;
}

/// Adapter specifically for identifying Uniswap v4 Hooks
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

/// The registry holding all known protocol adapters.
pub struct AdapterRegistry {
    adapters: Vec<Box<dyn ProtocolAdapter>>,
}

impl AdapterRegistry {
    /// Initialize a new registry pre-loaded with all supported adapters.
    pub fn new() -> Self {
        let mut registry = Self { adapters: Vec::new() };
        registry.register(Box::new(UniswapV4Adapter));
        registry
    }

    /// Register a custom adapter
    pub fn register(&mut self, adapter: Box<dyn ProtocolAdapter>) {
        self.adapters.push(adapter);
    }

    /// Iterates through adapters to find a descriptive label for the call.
    pub fn resolve(&self, address: Option<&str>, selector: Option<&str>) -> Option<String> {
        for adapter in &self.adapters {
            if let Some(label) = adapter.resolve_label(address, selector) {
                return Some(label);
            }
        }
        None
    }
}

impl Default for AdapterRegistry {
    fn default() -> Self {
        Self::new()
    }
}
