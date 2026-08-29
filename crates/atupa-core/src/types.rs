//! Core execution trace types: [`TraceStep`], [`CollapsedStack`], [`HotPath`],
//! [`Profile`], and the [`ProfileBuilder`].

use crate::{GasCategory, VmKind};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─── TraceStep ────────────────────────────────────────────────────────────────

/// A single normalized execution step, produced by any supported VM.
///
/// All VM-specific cost units (Compute Units, Cairo steps, Stylus Ink) are
/// mapped into [`gas_cost`](TraceStep::gas_cost) at the adapter layer so that
/// cross-chain comparison remains possible without further translation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TraceStep {
    /// Program counter or instruction index within the current call frame.
    pub pc: u64,
    /// Opcode name, HostFn label, or program identifier.
    pub op: String,
    /// Gas remaining at this step (EVM convention; may be `0` for non-EVM VMs).
    pub gas: u64,
    /// Normalized execution cost for this single step.
    pub gas_cost: u64,
    /// Call-stack depth at this step (`0` = root frame).
    pub depth: u16,
    /// EVM stack snapshot at this step, if available.
    pub stack: Option<Vec<String>>,
    /// EVM memory snapshot at this step, if available.
    pub memory: Option<Vec<String>>,
    /// Revert or error message emitted by this step, if any.
    #[serde(default)]
    pub error: Option<String>,
    /// Whether this step (or its enclosing call frame) was reverted.
    #[serde(default)]
    pub reverted: bool,
    /// Which Virtual Machine produced this step.
    #[serde(default)]
    pub vm_kind: VmKind,
}

impl TraceStep {
    /// Convenience constructor for a minimal EVM step.
    ///
    /// All fields not specified default to their zero values.
    /// Useful for building test fixtures without boilerplate.
    ///
    /// ```
    /// use atupa_core::{TraceStep, VmKind};
    ///
    /// let step = TraceStep::evm("SSTORE", 5_000);
    /// assert_eq!(step.vm_kind, VmKind::Evm);
    /// assert_eq!(step.gas_cost, 5_000);
    /// ```
    pub fn evm(op: impl Into<String>, gas_cost: u64) -> Self {
        Self { op: op.into(), gas_cost, vm_kind: VmKind::Evm, ..Default::default() }
    }

    /// Returns `true` if this step represents a cross-frame call boundary in the EVM.
    ///
    /// Useful for filtering steps that create a new call context (and thus a
    /// new depth level) during aggregation.
    pub fn is_call(&self) -> bool {
        matches!(
            self.op.as_str(),
            "CALL" | "STATICCALL" | "DELEGATECALL" | "CALLCODE" | "CREATE" | "CREATE2"
        )
    }
}

// ─── CollapsedStack ───────────────────────────────────────────────────────────

/// A depth-aggregated execution path with its total accumulated cost weight.
///
/// Stack paths use a semi-colon-delimited format, e.g.:
/// `"CALL;SSTORE;KECCAK256"`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollapsedStack {
    /// Semi-colon delimited opcode / label path.
    pub stack: String,
    /// Total normalized execution weight (gas / CU / Ink) for this path.
    pub weight: u64,
    /// Program counter of the last step folded into this entry.
    pub last_pc: Option<u64>,
    /// Maximum call depth seen across the steps in this path.
    #[serde(default)]
    pub depth: u16,
    /// The VM that produced the steps in this path.
    #[serde(default)]
    pub vm_kind: VmKind,
    /// Callee contract address extracted from the call boundary step, if any.
    #[serde(default)]
    pub target_address: Option<String>,
    /// Human-readable label resolved via a protocol adapter, if any.
    #[serde(default)]
    pub resolved_label: Option<String>,
    /// Whether the call frame represented by this stack was reverted.
    #[serde(default)]
    pub reverted: bool,
}

// ─── HotPath ──────────────────────────────────────────────────────────────────

/// An aggregated hot path — a collapsed stack ranked by its share of total gas.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotPath {
    /// Semi-colon delimited opcode / label path.
    pub stack: String,
    /// Total accumulated execution cost for this path.
    pub gas: u64,
    /// Share of the total transaction cost (0.0–100.0).
    pub percentage: f64,
    /// Dominant cost category for this path.
    pub category: GasCategory,
}

// ─── Profile ──────────────────────────────────────────────────────────────────

/// The top-level profiling report emitted by the Atupa engine.
///
/// Construct via [`Profile::new`] or, for deterministic testing, via
/// [`ProfileBuilder`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    /// Crate version that generated this report.
    pub version: String,
    /// Transaction hash (or trace identifier) that was profiled.
    pub transaction_hash: String,
    /// Total normalized execution cost across all steps.
    pub total_gas: u64,
    /// Per-[`GasCategory`] cost breakdown.
    pub categories: HashMap<GasCategory, u64>,
    /// Ranked list of execution hot paths.
    pub hot_paths: Vec<HotPath>,
    /// RFC-3339 timestamp at which this report was generated.
    pub generated_at: String,
}

impl Profile {
    /// Creates a new, empty profile for the given transaction hash.
    ///
    /// `generated_at` is set to the current UTC time. Use [`ProfileBuilder`]
    /// when you need a deterministic, injectable timestamp (e.g. in unit tests
    /// or snapshot testing).
    pub fn new(tx_hash: impl Into<String>) -> Self {
        ProfileBuilder::new(tx_hash).build()
    }
}

// ─── ProfileBuilder ───────────────────────────────────────────────────────────

/// A builder for [`Profile`] that allows injecting a custom `generated_at`
/// timestamp for deterministic unit testing.
///
/// ```
/// use atupa_core::ProfileBuilder;
///
/// let profile = ProfileBuilder::new("0xdeadbeef")
///     .generated_at("2026-01-01T00:00:00Z")
///     .build();
///
/// assert_eq!(profile.generated_at, "2026-01-01T00:00:00Z");
/// assert_eq!(profile.total_gas, 0);
/// ```
pub struct ProfileBuilder {
    tx_hash: String,
    generated_at: Option<String>,
}

impl ProfileBuilder {
    /// Start building a [`Profile`] for the given transaction hash.
    pub fn new(tx_hash: impl Into<String>) -> Self {
        Self { tx_hash: tx_hash.into(), generated_at: None }
    }

    /// Override the `generated_at` timestamp.
    ///
    /// If not called, defaults to [`chrono::Utc::now()`] at `.build()` time.
    pub fn generated_at(mut self, ts: impl Into<String>) -> Self {
        self.generated_at = Some(ts.into());
        self
    }

    /// Consume the builder and return the finished [`Profile`].
    pub fn build(self) -> Profile {
        Profile {
            version: env!("CARGO_PKG_VERSION").to_string(),
            transaction_hash: self.tx_hash,
            total_gas: 0,
            categories: HashMap::new(),
            hot_paths: Vec::new(),
            generated_at: self.generated_at.unwrap_or_else(|| chrono::Utc::now().to_rfc3339()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── ProfileBuilder ────────────────────────────────────────────────────────

    #[test]
    fn profile_builder_deterministic_timestamp() {
        let p = ProfileBuilder::new("0xabc").generated_at("2026-01-01T00:00:00Z").build();
        assert_eq!(p.transaction_hash, "0xabc");
        assert_eq!(p.generated_at, "2026-01-01T00:00:00Z");
        assert_eq!(p.total_gas, 0);
        assert!(p.categories.is_empty());
        assert!(p.hot_paths.is_empty());
    }

    #[test]
    fn profile_new_sets_version() {
        let p = Profile::new("0xbeef");
        assert!(!p.version.is_empty(), "version should be set from CARGO_PKG_VERSION");
    }

    // ── TraceStep ─────────────────────────────────────────────────────────────

    #[test]
    fn trace_step_evm_helper_sets_fields() {
        let step = TraceStep::evm("SSTORE", 5_000);
        assert_eq!(step.op, "SSTORE");
        assert_eq!(step.gas_cost, 5_000);
        assert_eq!(step.vm_kind, VmKind::Evm);
        assert!(!step.reverted);
        assert_eq!(step.depth, 0);
    }

    #[test]
    fn trace_step_is_call_detects_call_opcodes() {
        for op in &["CALL", "STATICCALL", "DELEGATECALL", "CALLCODE", "CREATE", "CREATE2"] {
            assert!(TraceStep::evm(*op, 0).is_call(), "{op} should be a call");
        }
    }

    #[test]
    fn trace_step_is_call_rejects_non_calls() {
        for op in &["SSTORE", "SLOAD", "ADD", "JUMPDEST"] {
            assert!(!TraceStep::evm(*op, 0).is_call(), "{op} should not be a call");
        }
    }
}
