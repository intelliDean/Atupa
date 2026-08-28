//! # atupa-core
//!
//! Foundational types, configuration, and domain models shared across the
//! entire Atupa workspace.
//!
//! ## Modules
//!
//! | Module | Contents |
//! |---|---|
//! | [`vm`] | [`VmKind`] — identifies the source Virtual Machine |
//! | [`gas`] | [`GasCategory`] — classifies execution steps by cost driver |
//! | [`types`] | [`TraceStep`], [`CollapsedStack`], [`HotPath`], [`Profile`], [`ProfileBuilder`] |
//! | [`diff`] | [`ProtocolDiffReport`], [`DiffRow`] — protocol-level regression comparison |
//! | [`config`] | [`AtupaConfig`] — multi-source configuration loading |
//!
//! ## Re-exports
//!
//! All public types are re-exported from the crate root so that downstream
//! crates can use `atupa_core::TraceStep` etc. without knowing the module layout.

pub mod config;
pub mod diff;
pub mod gas;
pub mod types;
pub mod vm;

// ── Flat re-exports ───────────────────────────────────────────────────────────

pub use diff::{DiffRow, ProtocolDiffReport};
pub use gas::GasCategory;
pub use types::{CollapsedStack, HotPath, Profile, ProfileBuilder, TraceStep};
pub use vm::{ParseVmKindError, VmKind};
