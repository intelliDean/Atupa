//! # atupa-aave — DeepTracer
//!
//! Aave v3 & GHO protocol adapter for the Atupa EVM profiling engine.
//!
//! Provides deep trace analysis for liquidation flows, supply/borrow mechanics,
//! and GHO stablecoin risk monitoring.
//!
//! ## Modules
//!
//! | Module | Contents |
//! |---|---|
//! | [`selectors`] | Selector/address tables and shared lookup helpers |
//! | [`adapter`] | [`AaveV3Adapter`] — [`ProtocolAdapter`] implementation |
//! | [`gho`] | [`GhoSupplyMetrics`] and GHO label classifier |
//! | [`report`] | [`LiquidationReport`], [`LabeledCall`] |
//! | [`tracer`] | [`AaveDeepTracer`] — main analysis entry point |
//!
//! ## Re-exports
//!
//! All public types are re-exported from the crate root so downstream crates
//! can use `atupa_aave::AaveDeepTracer` etc. without knowing the module layout.

pub mod adapter;
pub mod gho;
pub mod report;
pub mod selectors;
pub mod tracer;

// ── Flat re-exports ───────────────────────────────────────────────────────────

pub use adapter::AaveV3Adapter;
pub use gho::GhoSupplyMetrics;
pub use report::{LabeledCall, LiquidationReport};
pub use tracer::AaveDeepTracer;
