//! # atupa-adapters
//!
//! Protocol adapter framework and registry for the Atupa execution tracer.
//!
//! Protocol adapters resolve raw contract addresses and 4-byte EVM function
//! selectors into human-readable labels (e.g. `"Uniswap v4: beforeSwap"`,
//! `"ERC20::transfer"`, `"AaveV3Pool::liquidationCall"`).
//!
//! ## Modules
//!
//! | Module | Contents |
//! |---|---|
//! | [`traits`] | The core [`ProtocolAdapter`] trait and default helpers |
//! | [`registry`] | The dynamic [`AdapterRegistry`] for runtime adapter resolution |
//! | [`uniswap_v4`] | Built-in [`UniswapV4Adapter`] for Uniswap v4 hook interfaces |
//! | [`erc20`] | Built-in [`Erc20Adapter`] for standard token operations |
//!
//! ## Re-exports
//!
//! All public types are re-exported from the crate root so downstream crates
//! can use `atupa_adapters::ProtocolAdapter` and `atupa_adapters::AdapterRegistry`
//! directly.

pub mod erc20;
pub mod registry;
pub mod traits;
pub mod uniswap_v4;

// ── Flat re-exports ───────────────────────────────────────────────────────────

pub use erc20::Erc20Adapter;
pub use registry::AdapterRegistry;
pub use traits::ProtocolAdapter;
pub use uniswap_v4::UniswapV4Adapter;
