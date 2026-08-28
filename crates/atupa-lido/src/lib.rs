//! # atupa-lido — DeepTracer
//!
//! Lido stETH protocol adapter and deep trace analysis for the Atupa EVM
//! profiling engine.
//!
//! Tracks liquid staking mechanics across submitting ETH, rebase oracle reports,
//! shares transfers, and withdrawal queue lifecycle.
//!
//! ## Modules
//!
//! | Module | Contents |
//! |---|---|
//! | [`selectors`] | Lido selectors, contract addresses, and lookup helpers |
//! | [`adapter`] | [`LidoAdapter`] implementing [`atupa_adapters::ProtocolAdapter`] |
//! | [`report`] | [`LidoReport`], [`LabeledCall`], and [`LidoAccumulator`](report::LidoAccumulator) |
//! | [`tracer`] | [`LidoDeepTracer`] analysis engine and diff reporting |
//!
//! ## Re-exports
//!
//! All public types are re-exported from the crate root so downstream crates
//! can use `atupa_lido::LidoAdapter`, `atupa_lido::LidoReport`, and
//! `atupa_lido::LidoDeepTracer` directly.

pub mod adapter;
pub mod report;
pub mod selectors;
pub mod tracer;

// ── Flat re-exports ───────────────────────────────────────────────────────────

pub use adapter::LidoAdapter;
pub use report::{LabeledCall, LidoReport};
pub use tracer::LidoDeepTracer;
