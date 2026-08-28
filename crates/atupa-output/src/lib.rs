//! # atupa-output
//!
//! Visual SVG flamegraph rendering engine for single and differential EVM/Stylus traces.
//!
//! Provides two primary generators:
//! 1. [`SvgGenerator`] — renders depth-lane, multi-VM single-transaction flamegraphs.
//! 2. [`generate_diff_flamegraph`] — renders visual differential flamegraphs highlighting
//!    regressions, improvements, and changes between two executions.
//!
//! ## Modules
//!
//! | Module | Contents |
//! |---|---|
//! | [`common`] | Layout constants, label truncation, and SVG placeholders |
//! | [`flamegraph`] | [`SvgGenerator`] for single execution traces |
//! | [`diff`] | [`generate_diff_flamegraph`] for differential trace analysis |
//!
//! ## Re-exports
//!
//! Primary entry points are re-exported at the crate root.

pub mod common;
pub mod diff;
pub mod flamegraph;

// ── Flat re-exports ───────────────────────────────────────────────────────────

pub use diff::generate_diff_flamegraph;
pub use flamegraph::SvgGenerator;
