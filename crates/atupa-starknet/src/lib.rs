//! # atupa-starknet
//!
//! Starknet (Cairo VM) execution trace adapter and flattener for the Atupa engine.
//!
//! Queries `starknet_traceTransaction` to retrieve hierarchical execution traces,
//! decomposes Cairo execution resources (steps, Pedersen, Range Check, Bitwise, Poseidon,
//! EC OP, and ECDSA builtins), and flattens recursive call frames into linear
//! [`atupa_core::TraceStep`] timelines for unified flamegraph profiling.
//!
//! ## Modules
//!
//! | Module | Contents |
//! |---|---|
//! | [`error`] | [`StarknetError`] and [`StarknetResult`](error::StarknetResult) |
//! | [`types`] | [`ExecutionResources`], [`FunctionInvocation`], [`StarknetTransactionTrace`] |
//! | [`flattener`] | [`flatten_invocation`] and [`flatten_trace`] recursive tree traversers |
//! | [`client`] | [`StarknetClient`] JSON-RPC client |
//!
//! ## Re-exports
//!
//! Primary types are re-exported at the crate root.

pub mod client;
pub mod error;
pub mod flattener;
pub mod types;

// ── Flat re-exports ───────────────────────────────────────────────────────────

pub use client::StarknetClient;
pub use error::{StarknetError, StarknetResult};
pub use flattener::{flatten_invocation, flatten_trace};
pub use types::{ExecutionResources, FunctionInvocation, StarknetTransactionTrace};
