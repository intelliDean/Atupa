//! # atupa-nitro
//!
//! Arbitrum Nitro and Stylus execution trace stitcher and RPC client.
//!
//! Arbitrum Nitro executes dual-VM transactions where standard EVM contracts
//! interoperate seamlessly with WebAssembly (WASM) Stylus programs. This crate
//! provides:
//!
//! 1. [`MixedTraceStitcher`] — fuses asynchronous `structLogger` (EVM) and
//!    `stylusTracer` (WASM) streams into a single time-ordered [`StitchedReport`].
//! 2. [`NitroClient`] — concurrent dual-tracer RPC client with automatic Nitro
//!    chain detection and fallback handling.
//! 3. Cost normalisation between Stylus **Ink** and EVM **Gas** (`1 Gas = 10,000 Ink`).
//!
//! ## Modules
//!
//! | Module | Contents |
//! |---|---|
//! | [`error`] | [`NitroError`] and [`NitroResult`](error::NitroResult) |
//! | [`types`] | [`StylusHostIO`], [`VmKind`], [`UnifiedStep`], and [`StitchedReport`] |
//! | [`stitcher`] | [`MixedTraceStitcher`] and [`CALL_OPCODES`](stitcher::CALL_OPCODES) |
//! | [`client`] | [`NitroClient`] and [`is_nitro_chain`](client::is_nitro_chain) |
//!
//! ## Re-exports
//!
//! All primary types are re-exported at the crate root for downstream convenience.

pub mod client;
pub mod error;
pub mod stitcher;
pub mod types;

// ── Flat re-exports ───────────────────────────────────────────────────────────

pub use client::{is_nitro_chain, NitroClient};
pub use error::{NitroError, NitroResult};
pub use stitcher::{MixedTraceStitcher, CALL_OPCODES};
pub use types::{StitchedReport, StylusHostIO, UnifiedStep, VmKind};
