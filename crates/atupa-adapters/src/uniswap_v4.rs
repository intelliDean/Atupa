//! Built-in [`UniswapV4Adapter`] for identifying Uniswap v4 Hook interface calls.

use crate::traits::ProtocolAdapter;

/// Known Uniswap v4 Hook standard interface 4-byte selectors.
pub const HOOK_SELECTORS: &[(&str, &str)] = &[
    ("0x18a9d381", "beforeInitialize"),
    ("0x999dea5d", "afterInitialize"),
    ("0x910746f2", "beforeAddLiquidity"),
    ("0xefd81287", "afterAddLiquidity"),
    ("0xd7386be3", "beforeRemoveLiquidity"),
    ("0x1efe5f9e", "afterRemoveLiquidity"),
    ("0xe82c3b75", "beforeSwap"),
    ("0x14d6eaec", "afterSwap"),
    ("0xa3d03227", "beforeDonate"),
    ("0x0df2d576", "afterDonate"),
];

/// Identifies Uniswap v4 Hook interface calls by their 4-byte selectors.
///
/// This adapter is included directly in `atupa-adapters` because Uniswap v4 hook
/// monitoring is part of the base profiler functionality without requiring a
/// separate heavy crate dependency.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct UniswapV4Adapter;

impl UniswapV4Adapter {
    /// Create a new [`UniswapV4Adapter`].
    pub fn new() -> Self {
        Self
    }

    /// Resolve a 4-byte selector string directly to a hook label name (e.g. `"beforeSwap"`).
    pub fn resolve_hook_selector(selector: &str) -> Option<&'static str> {
        let selector = selector.trim();
        for &(known_sel, label) in HOOK_SELECTORS {
            if selector.eq_ignore_ascii_case(known_sel) {
                return Some(label);
            }
        }
        None
    }
}

impl ProtocolAdapter for UniswapV4Adapter {
    fn name(&self) -> &str {
        "Uniswap v4"
    }

    fn resolve_label(&self, _address: Option<&str>, selector: Option<&str>) -> Option<String> {
        let sel = selector?;
        let label = Self::resolve_hook_selector(sel)?;
        Some(format!("Uniswapv4: {label}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_name() {
        let adapter = UniswapV4Adapter::new();
        assert_eq!(adapter.name(), "Uniswap v4");
    }

    #[test]
    fn resolves_all_known_hook_selectors() {
        let adapter = UniswapV4Adapter;
        for &(sel, expected_label) in HOOK_SELECTORS {
            let resolved = adapter.resolve_label(None, Some(sel));
            assert_eq!(
                resolved,
                Some(format!("Uniswapv4: {expected_label}")),
                "Failed to resolve hook selector {sel}"
            );
        }
    }

    #[test]
    fn static_resolver_is_case_insensitive() {
        assert_eq!(
            UniswapV4Adapter::resolve_hook_selector("0x18A9D381"),
            Some("beforeInitialize")
        );
        assert_eq!(
            UniswapV4Adapter::resolve_hook_selector("0x18a9d381"),
            Some("beforeInitialize")
        );
        assert_eq!(
            UniswapV4Adapter::resolve_hook_selector("  0xe82c3b75  "),
            Some("beforeSwap")
        );
    }

    #[test]
    fn returns_none_for_unknown_selector() {
        let adapter = UniswapV4Adapter;
        assert_eq!(adapter.resolve_label(None, Some("0xdeadbeef")), None);
        assert_eq!(adapter.resolve_label(None, None), None);
        assert_eq!(UniswapV4Adapter::resolve_hook_selector("0xdeadbeef"), None);
    }
}
