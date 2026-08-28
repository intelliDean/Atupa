//! # atupa-parser
//!
//! Trace normalization, address/selector decoding, and stack aggregation engine.
//!
//! Converts raw RPC debug execution traces into normalized [`atupa_core::TraceStep`]s,
//! decodes call arguments/selectors from EVM memory, and aggregates linear execution steps
//! into hierarchical [`atupa_core::CollapsedStack`] profiles for flamegraph visualization.
//!
//! ## Modules
//!
//! | Module | Contents |
//! |---|---|
//! | [`normalize`] | [`Parser`] for converting RPC `structLog`s into [`atupa_core::TraceStep`]s |
//! | [`decoder`] | Memory selector extraction & target address decoders |
//! | [`aggregator`] | [`Aggregator`] for collapsing linear steps into tree call-stacks |
//!
//! ## Re-exports
//!
//! Primary types are re-exported at the crate root.

pub mod aggregator;
pub mod decoder;
pub mod normalize;

// ── Flat re-exports ───────────────────────────────────────────────────────────

pub use aggregator::Aggregator;
pub use decoder::{extract_memory_selector, extract_target_address};
pub use normalize::Parser;
