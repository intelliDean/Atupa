//! Domain types for Arbitrum Nitro EVM and Stylus WASM trace modeling.

use atupa_core::GasCategory;
use atupa_core::VmKind as CoreVmKind;
use atupa_rpc::RawStructLog;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

// ─── Stylus HostIO ────────────────────────────────────────────────────────────

/// A single HostIO event emitted by Arbitrum's `stylusTracer`.
///
/// HostIOs represent cross-VM system calls from WASM back into the Nitro host
/// (e.g. reading storage, emitting logs). Each event tracks its Ink budget.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StylusHostIO {
    /// The HostIO function name (e.g. `storage_load_bytes32`, `user_entrypoint`).
    pub name: String,
    /// Hex-encoded input arguments.
    pub args: String,
    /// Hex-encoded output values.
    pub outs: String,
    /// Ink remaining at the START of this HostIO call.
    pub start_ink: u64,
    /// Ink remaining at the END of this HostIO call.
    pub end_ink: u64,
    /// Optional: the Stylus contract address that made this call.
    #[serde(default)]
    pub address: Option<String>,
}

impl StylusHostIO {
    /// Net Ink consumed by this single HostIO event.
    ///
    /// Ink is a monotonically-decreasing budget; this will always be `>= 0`.
    #[inline]
    pub fn ink_consumed(&self) -> u64 {
        self.start_ink.saturating_sub(self.end_ink)
    }

    /// Converts Ink consumed to an equivalent Gas unit.
    ///
    /// Arbitrum Nitro defines the canonical ratio: **1 Gas = 10,000 Ink**.
    /// This allows unified cost reporting across both VMs.
    #[inline]
    pub fn ink_as_gas_equiv(&self) -> f64 {
        self.ink_consumed() as f64 / 10_000.0
    }
}

// ─── VM Kind ──────────────────────────────────────────────────────────────────

/// Identifies which virtual machine produced a trace step in the unified timeline.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default, Hash)]
pub enum VmKind {
    /// Standard EVM execution step.
    #[default]
    Evm,
    /// Arbitrum Stylus WASM HostIO step.
    Stylus,
    /// Starknet Cairo VM step.
    Starknet,
    /// Solana Sealevel VM step.
    Solana,
    /// Stellar Soroban HostFn step.
    Stellar,
}

impl fmt::Display for VmKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VmKind::Evm => write!(f, "EVM"),
            VmKind::Stylus => write!(f, "Stylus"),
            VmKind::Starknet => write!(f, "Starknet"),
            VmKind::Solana => write!(f, "Solana"),
            VmKind::Stellar => write!(f, "Stellar"),
        }
    }
}

impl From<VmKind> for CoreVmKind {
    fn from(v: VmKind) -> Self {
        match v {
            VmKind::Evm => CoreVmKind::Evm,
            VmKind::Stylus => CoreVmKind::Stylus,
            VmKind::Starknet => CoreVmKind::Starknet,
            VmKind::Solana => CoreVmKind::Solana,
            VmKind::Stellar => CoreVmKind::Stellar,
        }
    }
}

// ─── Unified Step ─────────────────────────────────────────────────────────────

/// A single step in the merged, time-ordered execution timeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedStep {
    /// Sequential index in the merged timeline.
    pub index: usize,
    /// The VM that produced this step.
    pub vm: VmKind,
    /// The primary opcode (EVM) or HostIO name (Stylus).
    pub label: String,
    /// Gas cost for EVM steps; 0 for Stylus steps.
    pub gas_cost: u64,
    /// Normalised cost-of-execution (Gas for EVM, Ink-as-Gas for Stylus).
    pub cost_equiv: f64,
    /// Call depth in the EVM frame at this point in execution.
    pub depth: u16,
    /// True when this is the EVM `CALL` opcode that dispatches into a WASM contract.
    pub is_vm_boundary: bool,
    /// The logical category of this execution step.
    pub category: GasCategory,
    /// Target address for CALL/CREATE operations.
    pub target_address: Option<String>,
    /// Raw EVM structLog, present only for EVM steps.
    pub evm: Option<RawStructLog>,
    /// Raw Stylus HostIO, present only for Stylus steps.
    pub stylus: Option<StylusHostIO>,
}

impl UnifiedStep {
    /// Returns `true` if this step originated from the EVM.
    pub fn is_evm(&self) -> bool {
        self.vm == VmKind::Evm
    }

    /// Returns `true` if this step originated from Stylus WASM.
    pub fn is_stylus(&self) -> bool {
        self.vm == VmKind::Stylus
    }

    /// Converts a unified step back to a core [`atupa_core::TraceStep`], preserving
    /// VM identity, depth, and normalized costs.
    pub fn to_trace_step(&self) -> atupa_core::TraceStep {
        if let Some(evm) = &self.evm {
            let reverted = evm.error.is_some() || evm.op == "REVERT" || evm.op == "INVALID";
            atupa_core::TraceStep {
                pc: evm.pc,
                op: evm.op.clone(),
                gas: evm.gas,
                gas_cost: evm.gas_cost,
                depth: evm.depth,
                stack: evm.stack.clone(),
                memory: evm.memory.clone(),
                error: evm.error.clone(),
                reverted,
                vm_kind: atupa_core::VmKind::Evm,
            }
        } else if let Some(stylus) = &self.stylus {
            atupa_core::TraceStep {
                pc: 0,
                op: stylus.name.clone(),
                gas: 0,
                gas_cost: self.cost_equiv.round() as u64,
                depth: self.depth,
                stack: None,
                memory: None,
                error: None,
                reverted: false,
                vm_kind: atupa_core::VmKind::Stylus,
            }
        } else {
            // Fallback for synthetic/label-only steps
            atupa_core::TraceStep {
                pc: 0,
                op: self.label.clone(),
                gas: 0,
                gas_cost: self.gas_cost,
                depth: self.depth,
                stack: None,
                memory: None,
                error: None,
                reverted: false,
                vm_kind: self.vm.clone().into(),
            }
        }
    }
}

// ─── Stitched Report ──────────────────────────────────────────────────────────

/// The complete output of the stitching engine for a single transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StitchedReport {
    /// The transaction hash that was traced.
    pub tx_hash: String,
    /// The chain ID of the network being traced.
    pub chain_id: u64,
    /// Merged, time-ordered execution steps across both VMs.
    pub steps: Vec<UnifiedStep>,
    /// Total EVM gas consumed across all steps.
    pub total_evm_gas: u64,
    /// Total Stylus Ink consumed (absolute Ink units).
    pub total_stylus_ink: u64,
    /// Number of EVM→WASM VM boundary crossings detected.
    pub vm_boundary_count: usize,
    /// Stylus Ink normalised to Gas-equivalent units.
    pub total_stylus_gas_equiv: f64,
    /// Combined cost: `total_evm_gas` + `total_stylus_gas_equiv`.
    pub total_unified_cost: f64,
    /// Aggregated costs by gas category.
    pub category_costs: HashMap<GasCategory, f64>,
    /// Address labels resolved via contract registry or Etherscan.
    pub resolved_names: HashMap<String, String>,
    /// Actual gas used on-chain from receipt (if available).
    pub on_chain_gas_used: Option<u64>,
}

impl StitchedReport {
    /// Returns references to only the Stylus/WASM steps.
    pub fn stylus_steps(&self) -> Vec<&UnifiedStep> {
        self.steps
            .iter()
            .filter(|s| s.vm == VmKind::Stylus)
            .collect()
    }

    /// Returns references to only the EVM steps.
    pub fn evm_steps(&self) -> Vec<&UnifiedStep> {
        self.steps
            .iter()
            .filter(|s| s.vm == VmKind::Evm)
            .collect()
    }

    /// Returns references to the VM boundary (EVM→WASM crossing) steps.
    pub fn boundary_steps(&self) -> Vec<&UnifiedStep> {
        self.steps.iter().filter(|s| s.is_vm_boundary).collect()
    }

    /// Returns a one-line summary string of this report.
    pub fn summary(&self) -> String {
        let short_hash = self.tx_hash.get(..10).unwrap_or(&self.tx_hash);
        format!(
            "[NitroReport] tx={} steps={} evm_gas={} stylus_ink={} ({:.1} gas-equiv) boundaries={}",
            short_hash,
            self.steps.len(),
            self.total_evm_gas,
            self.total_stylus_ink,
            self.total_stylus_gas_equiv,
            self.vm_boundary_count,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stylus_host_io_cost_calculations() {
        let io = StylusHostIO {
            name: "storage_load_bytes32".to_string(),
            args: "".to_string(),
            outs: "".to_string(),
            start_ink: 1_000_000,
            end_ink: 900_000,
            address: None,
        };
        assert_eq!(io.ink_consumed(), 100_000);
        assert!((io.ink_as_gas_equiv() - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn vm_kind_display_and_conversion() {
        assert_eq!(VmKind::Evm.to_string(), "EVM");
        assert_eq!(VmKind::Stylus.to_string(), "Stylus");
        assert_eq!(VmKind::Starknet.to_string(), "Starknet");
        assert_eq!(VmKind::Solana.to_string(), "Solana");
        assert_eq!(VmKind::Stellar.to_string(), "Stellar");

        let core_vm: CoreVmKind = VmKind::Stylus.into();
        assert_eq!(core_vm, CoreVmKind::Stylus);
    }

    #[test]
    fn unified_step_to_trace_step_evm() {
        let step = UnifiedStep {
            index: 0,
            vm: VmKind::Evm,
            label: "SSTORE".to_string(),
            gas_cost: 20_000,
            cost_equiv: 20_000.0,
            depth: 1,
            is_vm_boundary: false,
            category: GasCategory::StorageWrite,
            target_address: None,
            evm: Some(RawStructLog {
                pc: 10,
                op: "SSTORE".to_string(),
                gas: 500_000,
                gas_cost: 20_000,
                depth: 1,
                error: None,
                stack: None,
                memory: None,
                storage: None,
            }),
            stylus: None,
        };

        assert!(step.is_evm());
        assert!(!step.is_stylus());
        let trace_step = step.to_trace_step();
        assert_eq!(trace_step.op, "SSTORE");
        assert_eq!(trace_step.gas_cost, 20_000);
        assert_eq!(trace_step.vm_kind, CoreVmKind::Evm);
    }

    #[test]
    fn unified_step_to_trace_step_stylus() {
        let step = UnifiedStep {
            index: 1,
            vm: VmKind::Stylus,
            label: "storage_load_bytes32".to_string(),
            gas_cost: 0,
            cost_equiv: 10.0,
            depth: 2,
            is_vm_boundary: false,
            category: GasCategory::StorageRead,
            target_address: None,
            evm: None,
            stylus: Some(StylusHostIO {
                name: "storage_load_bytes32".to_string(),
                args: "".to_string(),
                outs: "".to_string(),
                start_ink: 100_000,
                end_ink: 0,
                address: None,
            }),
        };

        assert!(step.is_stylus());
        let trace_step = step.to_trace_step();
        assert_eq!(trace_step.op, "storage_load_bytes32");
        assert_eq!(trace_step.gas_cost, 10);
        assert_eq!(trace_step.vm_kind, CoreVmKind::Stylus);
    }

    #[test]
    fn report_summary_safe_on_short_hash() {
        let report = StitchedReport {
            tx_hash: "0x12".to_string(),
            chain_id: 42161,
            steps: Vec::new(),
            total_evm_gas: 0,
            total_stylus_ink: 0,
            vm_boundary_count: 0,
            total_stylus_gas_equiv: 0.0,
            total_unified_cost: 0.0,
            category_costs: HashMap::new(),
            resolved_names: HashMap::new(),
            on_chain_gas_used: None,
        };
        let summary = report.summary();
        assert!(summary.contains("0x12"));
    }
}
