//! # Atupa
//!
//! **Unified Multi-VM Blockchain Execution Profiler** — the top-level façade crate for the
//! Atupa SDK. This crate re-exports every layer of the suite so that external
//! integrators only need to depend on a single crate:
//!
//! ```toml
//! [dependencies]
//! atupa = "0.1"
//! ```
//!
//! ## Crate Architecture
//!
//! ```text
//! atupa (this façade)
//! ├── atupa-core      → Types: TraceStep, CollapsedStack, GasCategory, VmKind
//! ├── atupa-rpc       → JSON-RPC client (EthClient, EtherscanResolver)
//! ├── atupa-parser    → TraceStep normalization and Call Stack Aggregation
//! ├── atupa-adapters  → ProtocolAdapter trait (Uniswap v4, ERC-20, etc.)
//! ├── atupa-output    → SvgGenerator & differential flamegraph renderers
//! ├── atupa-aave      → AaveDeepTracer, GHO supply metrics
//! ├── atupa-lido      → LidoDeepTracer, stETH / wstETH tracing
//! ├── atupa-nitro     → Mixed EVM + Arbitrum Stylus WASM dual-tracing
//! ├── atupa-starknet  → Starknet Cairo VM trace flattening
//! ├── atupa-solana    → Solana Sealevel instruction log stitcher
//! └── atupa-stellar   → Stellar Soroban WASM diagnostic event parser
//! ```
//!
//! ## Modules
//!
//! | Module | Contents |
//! |---|---|
//! | [`registry`] | [`build_default_registry`] pre-configured with all protocol adapters |
//! | [`profile`] | [`execute_profile`] and [`VmHint`] high-level execution engine |

pub mod profile;
pub mod registry;

// ─── Public re-exports ────────────────────────────────────────────────────────

/// Core types shared across the entire Atupa suite.
pub use atupa_core as core;

/// JSON-RPC transport layer: `EthClient`, `EtherscanResolver`, `RawStructLog`.
pub use atupa_rpc as rpc;

/// Trace normalization and stack aggregation engine.
pub use atupa_parser as parser;

/// `ProtocolAdapter` trait and `AdapterRegistry` for pluggable protocol recognizers.
pub use atupa_adapters as adapters;

/// SVG flamegraph and diff visualization renderer.
pub use atupa_output as output;

/// Aave v3 + GHO protocol tracer.
pub use atupa_aave as aave;

/// Lido stETH protocol tracer.
pub use atupa_lido as lido;

/// Arbitrum Nitro & Stylus WASM dual-tracing client.
pub use atupa_nitro as nitro;

/// Starknet (Cairo VM) protocol tracer.
pub use atupa_starknet as starknet;

/// Solana (Sealevel VM) protocol tracer.
pub use atupa_solana as solana;

/// Stellar (Soroban WASM VM) protocol tracer.
pub use atupa_stellar as stellar;

// ─── High-level API Re-exports ────────────────────────────────────────────────

pub use profile::{VmHint, execute_profile};
pub use registry::build_default_registry;
