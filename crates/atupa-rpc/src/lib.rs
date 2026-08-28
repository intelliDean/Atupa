//! # atupa-rpc
//!
//! JSON-RPC client and Etherscan metadata resolver for the Atupa execution tracer.
//!
//! Provides:
//! - [`EthClient`] — HTTP client for `debug_traceTransaction`, `eth_chainId`,
//!   `eth_getTransactionReceipt`, and `eth_getTransactionByHash`.
//! - [`EtherscanResolver`] — Verified contract name resolution with persistent disk caching.
//! - [`RawStructLog`] & [`TraceResult`] — Universal debug trace payload models.
//!
//! ## Modules
//!
//! | Module | Contents |
//! |---|---|
//! | [`error`] | [`RpcError`] and [`RpcResult`](error::RpcResult) |
//! | [`types`] | [`RawStructLog`] and [`TraceResult`] |
//! | [`client`] | [`EthClient`] JSON-RPC client |
//! | [`etherscan`] | [`EtherscanResolver`] contract metadata resolver |
//!
//! ## Re-exports
//!
//! Primary types are re-exported at the crate root.

pub mod client;
pub mod error;
pub mod etherscan;
pub mod types;

// ── Flat re-exports ───────────────────────────────────────────────────────────

pub use client::EthClient;
pub use error::{RpcError, RpcResult};
pub use etherscan::EtherscanResolver;
pub use types::{RawStructLog, TraceResult};
