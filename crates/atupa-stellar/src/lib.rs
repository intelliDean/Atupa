//! # atupa-stellar
//!
//! Stellar Soroban (WASM) execution event parser and diagnostic trace adapter for the Atupa engine.
//!
//! Reconstructs hierarchical execution call frames from Soroban diagnostic events emitted
//! during smart contract execution and assigns gas costs according to Soroban host function
//! resource models (contract data storage, cryptography/hashing, sub-invocations).
//!
//! ## Modules
//!
//! | Module | Contents |
//! |---|---|
//! | [`error`] | [`StellarError`] and [`StellarResult`](error::StellarResult) |
//! | [`types`] | [`SorobanDiagnosticEvent`], [`StellarTransactionResponse`] |
//! | [`parser`] | [`StellarTraceParser`] diagnostic event reconstructor |
//! | [`client`] | [`StellarClient`] JSON-RPC client |
//!
//! ## Re-exports
//!
//! Primary types are re-exported at the crate root.

pub mod client;
pub mod error;
pub mod parser;
pub mod types;

// ── Flat re-exports ───────────────────────────────────────────────────────────

pub use client::StellarClient;
pub use error::{StellarError, StellarResult};
pub use parser::StellarTraceParser;
pub use types::{SorobanDiagnosticEvent, StellarTransactionResponse};
