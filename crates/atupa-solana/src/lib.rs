//! # atupa-solana
//!
//! Solana Sealevel VM program log parser and trace reconstructor for the Atupa engine.
//!
//! Solana does not expose opcode-level step traces by default. This crate reconstructs
//! execution trees and computes exclusive compute unit (CU) costs per frame by parsing
//! standard Solana `Program ... invoke`, `consumed ... compute units`, and `success/failed`
//! transaction log messages.
//!
//! ## Modules
//!
//! | Module | Contents |
//! |---|---|
//! | [`error`] | [`SolanaError`] and [`SolanaResult`](error::SolanaResult) |
//! | [`client`] | [`SolanaClient`] for querying validator RPCs via `getTransaction` |
//! | [`parser`] | [`SolanaLogStitcher`] for reconstructing [`atupa_core::TraceStep`] trees |
//!
//! ## Re-exports
//!
//! Primary types are re-exported at the crate root.

pub mod client;
pub mod error;
pub mod parser;

// ── Flat re-exports ───────────────────────────────────────────────────────────

pub use client::{SolanaClient, SolanaMeta, SolanaTransactionResponse};
pub use error::{SolanaError, SolanaResult};
pub use parser::SolanaLogStitcher;
