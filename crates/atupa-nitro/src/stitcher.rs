//! Dual-VM execution timeline stitching engine for Arbitrum Nitro & Stylus traces.

use atupa_core::GasCategory;
use atupa_core::VmKind as CoreVmKind;
use atupa_rpc::RawStructLog;
use std::collections::HashMap;

use crate::types::{StitchedReport, StylusHostIO, UnifiedStep, VmKind};

/// EVM opcodes that dispatch execution into a Stylus (WASM) contract.
/// These mark the EVM→WASM transition boundary.
pub const CALL_OPCODES: &[&str] = &["CALL", "STATICCALL", "DELEGATECALL", "CALLCODE"];

/// The core engine for merging EVM and WASM execution paths into a unified timeline.
///
/// ## Background: How Arbitrum Nitro executes hybrid transactions
///
/// When an EVM contract calls a Stylus contract, Nitro's `debug_traceTransaction`
/// with the default tracer (`structLogger`) records the `CALL` opcode and then continues
/// as if execution returned immediately. The WASM portion is opaque to the EVM tracer.
///
/// The `stylusTracer` records only the Stylus side: a sequence of `StylusHostIO`
/// events representing every cross-VM system call made by the WASM code.
///
/// `MixedTraceStitcher` fuses these two independent traces into a single timeline
/// using the following heuristic:
///
/// > **"Every CALL opcode in the EVM trace is a potential WASM entry point."**
///
/// After each `CALL`, we drain the next batch of Stylus HostIOs and interleave them
/// into the unified timeline at the nested call depth (`depth + 1`). This preserves
/// temporal ordering while clearly annotating which steps belong to which VM.
pub struct MixedTraceStitcher;

impl MixedTraceStitcher {
    /// Stitches EVM structLogs with Stylus HostIO events into a [`StitchedReport`].
    ///
    /// ## Algorithm
    /// 1. Stream EVM steps in program-counter order.
    /// 2. On a `CALL`/`STATICCALL`/`DELEGATECALL`/`CALLCODE` opcode, mark it as a VM boundary.
    /// 3. Drain HostIOs from the Stylus stream, grouping them into the current boundary
    ///    frame. Stop when a `user_entrypoint` HostIO (signals a fresh Stylus invocation)
    ///    is encountered AND we have already ingested at least one HostIO in this window.
    /// 4. Continue streaming EVM steps from the point immediately after the `CALL`.
    /// 5. Drain any remaining Stylus steps (handles the case where the outer frame itself
    ///    is a Stylus contract — no preceding EVM CALL will exist).
    /// 6. Aggregate totals and build the finished [`StitchedReport`].
    pub fn stitch(
        tx_hash: impl Into<String>,
        chain_id: u64,
        evm_logs: Vec<RawStructLog>,
        stylus_logs: Vec<StylusHostIO>,
    ) -> StitchedReport {
        let tx_hash = tx_hash.into();
        let mut steps: Vec<UnifiedStep> = Vec::with_capacity(evm_logs.len() + stylus_logs.len());
        let mut stylus_iter = stylus_logs.into_iter().peekable();

        let mut total_evm_gas: u64 = 0;
        let mut total_stylus_ink: u64 = 0;
        let mut vm_boundary_count: usize = 0;
        let mut index: usize = 0;

        for log in evm_logs {
            let is_boundary = CALL_OPCODES.contains(&log.op.as_str());
            let gas_cost = log.gas_cost;
            let depth = log.depth;

            total_evm_gas = total_evm_gas.saturating_add(gas_cost);
            let category = GasCategory::from_step(&log.op, &CoreVmKind::Evm);
            let target_address = extract_target_address(&log);

            steps.push(UnifiedStep {
                index,
                vm: VmKind::Evm,
                label: log.op.clone(),
                gas_cost,
                cost_equiv: gas_cost as f64,
                depth,
                is_vm_boundary: false,
                category,
                target_address,
                evm: Some(log),
                stylus: None,
            });
            let call_step_index = index;
            index += 1;

            if !is_boundary {
                continue;
            }

            // ── WASM Window ──────────────────────────────────────────────────
            // Drain Stylus HostIOs that belong to this boundary frame.
            let window_ink = drain_wasm_window(&mut stylus_iter, &mut steps, &mut index, depth + 1);

            if window_ink > 0 {
                total_stylus_ink = total_stylus_ink.saturating_add(window_ink);
                vm_boundary_count += 1;
                steps[call_step_index].is_vm_boundary = true;
            }
        }

        // ── Trailing Stylus Steps ────────────────────────────────────────────
        // Drain any Stylus steps that had no matching EVM CALL preceding them.
        // Handles transactions where the TOP-LEVEL entrypoint is itself Stylus.
        let trailing_ink = drain_trailing_stylus_steps(&mut stylus_iter, &mut steps, &mut index);
        total_stylus_ink = total_stylus_ink.saturating_add(trailing_ink);

        let total_stylus_gas_equiv = total_stylus_ink as f64 / 10_000.0;
        let total_unified_cost = total_evm_gas as f64 + total_stylus_gas_equiv;
        let category_costs = aggregate_category_costs(&steps);

        StitchedReport {
            tx_hash,
            chain_id,
            steps,
            total_evm_gas,
            total_stylus_ink,
            vm_boundary_count,
            total_stylus_gas_equiv,
            total_unified_cost,
            category_costs,
            resolved_names: HashMap::new(),
            on_chain_gas_used: None,
        }
    }
}

// ─── Private helpers ──────────────────────────────────────────────────────────

/// Extract target contract address from stack for CALL/CREATE operations.
pub(crate) fn extract_target_address(log: &RawStructLog) -> Option<String> {
    if (log.op.contains("CALL") || log.op.contains("CREATE"))
        && let Some(stack) = &log.stack
        && stack.len() >= 2
    {
        let hex_addr = &stack[stack.len() - 2];
        let clean_hex = hex_addr.trim_start_matches("0x");
        let padded = format!("{:0>40}", clean_hex);
        let extracted = &padded[padded.len() - 40..];
        Some(format!("0x{}", extracted.to_lowercase()))
    } else {
        None
    }
}

/// Drain Stylus HostIO events belonging to a specific EVM call boundary window.
fn drain_wasm_window<I>(
    stylus_iter: &mut std::iter::Peekable<I>,
    steps: &mut Vec<UnifiedStep>,
    index: &mut usize,
    depth: u16,
) -> u64
where
    I: Iterator<Item = StylusHostIO>,
{
    let mut window_ink: u64 = 0;
    let mut window_count: usize = 0;

    loop {
        let should_break = match stylus_iter.peek() {
            None => true,
            Some(next) => next.name == "user_entrypoint" && window_count > 0,
        };
        if should_break {
            break;
        }

        let host_io = stylus_iter.next().unwrap();
        let ink_used = host_io.ink_consumed();
        window_ink = window_ink.saturating_add(ink_used);
        window_count += 1;

        let cost_equiv = host_io.ink_as_gas_equiv();
        let category = GasCategory::from_step(&host_io.name, &CoreVmKind::Stylus);
        steps.push(UnifiedStep {
            index: *index,
            vm: VmKind::Stylus,
            label: host_io.name.clone(),
            gas_cost: 0,
            cost_equiv,
            depth,
            is_vm_boundary: false,
            category,
            target_address: None,
            evm: None,
            stylus: Some(host_io),
        });
        *index += 1;
    }

    window_ink
}

/// Drain any remaining Stylus HostIO events at depth 0.
fn drain_trailing_stylus_steps<I>(
    stylus_iter: &mut std::iter::Peekable<I>,
    steps: &mut Vec<UnifiedStep>,
    index: &mut usize,
) -> u64
where
    I: Iterator<Item = StylusHostIO>,
{
    let mut trailing_ink: u64 = 0;
    for host_io in stylus_iter.by_ref() {
        let ink_used = host_io.ink_consumed();
        trailing_ink = trailing_ink.saturating_add(ink_used);

        let cost_equiv = host_io.ink_as_gas_equiv();
        let category = GasCategory::from_step(&host_io.name, &CoreVmKind::Stylus);
        steps.push(UnifiedStep {
            index: *index,
            vm: VmKind::Stylus,
            label: host_io.name.clone(),
            gas_cost: 0,
            cost_equiv,
            depth: 0,
            is_vm_boundary: false,
            category,
            target_address: None,
            evm: None,
            stylus: Some(host_io),
        });
        *index += 1;
    }
    trailing_ink
}

/// Aggregate step costs by their GasCategory.
fn aggregate_category_costs(steps: &[UnifiedStep]) -> HashMap<GasCategory, f64> {
    let mut category_costs = HashMap::new();
    for step in steps {
        *category_costs.entry(step.category.clone()).or_insert(0.0) += step.cost_equiv;
    }
    category_costs
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn evm(op: &str, gas_cost: u64, depth: u16) -> RawStructLog {
        RawStructLog {
            pc: 0,
            op: op.to_string(),
            gas: 1_000_000,
            gas_cost,
            depth,
            error: None,
            stack: None,
            memory: None,
            storage: None,
        }
    }

    fn host_io(name: &str, start_ink: u64, end_ink: u64) -> StylusHostIO {
        StylusHostIO {
            name: name.to_string(),
            args: String::new(),
            outs: String::new(),
            start_ink,
            end_ink,
            address: None,
        }
    }

    #[test]
    fn pure_evm_produces_no_stylus_steps() {
        let logs = vec![evm("PUSH1", 3, 1), evm("ADD", 3, 1), evm("RETURN", 0, 1)];
        let report = MixedTraceStitcher::stitch("0xabc", 1, logs, vec![]);

        assert_eq!(report.steps.len(), 3);
        assert_eq!(report.vm_boundary_count, 0);
        assert_eq!(report.total_evm_gas, 6);
        assert_eq!(report.total_stylus_ink, 0);
        assert!(report.stylus_steps().is_empty());
    }

    #[test]
    fn hybrid_tx_stitches_wasm_window_after_call() {
        let evm_logs = vec![
            evm("PUSH1", 3, 1),
            evm("CALL", 100, 1), // ← VM boundary
            evm("RETURN", 0, 1),
        ];
        let stylus_logs = vec![
            host_io("user_entrypoint", 1_000_000, 900_000), // 100k ink
            host_io("storage_load_bytes32", 900_000, 800_000), // 100k ink
        ];

        let report = MixedTraceStitcher::stitch("0xdef", 42161, evm_logs, stylus_logs);

        assert_eq!(report.steps.len(), 5);
        assert_eq!(report.vm_boundary_count, 1);
        assert_eq!(report.total_evm_gas, 103);
        assert_eq!(report.total_stylus_ink, 200_000);
        assert!((report.total_unified_cost - 123.0).abs() < f64::EPSILON);
    }

    #[test]
    fn multiple_call_boundaries_each_get_a_wasm_window() {
        let evm_logs = vec![
            evm("CALL", 50, 1),       // boundary 1
            evm("STATICCALL", 30, 1), // boundary 2
        ];
        let stylus_logs = vec![
            host_io("user_entrypoint", 500_000, 400_000), // window 1: 100k ink
            host_io("user_entrypoint", 300_000, 200_000), // window 2: 100k ink
        ];

        let report = MixedTraceStitcher::stitch("0x111", 42161, evm_logs, stylus_logs);

        assert_eq!(report.vm_boundary_count, 2);
        assert_eq!(report.stylus_steps().len(), 2);
        assert_eq!(report.steps[1].label, "user_entrypoint");
        assert_eq!(report.steps[1].category, GasCategory::Execution);
        assert_eq!(report.steps[3].label, "user_entrypoint");
        assert_eq!(report.steps[3].category, GasCategory::Execution);
        assert!(report.category_costs.get(&GasCategory::Call).unwrap() > &0.0);
        assert!(report.category_costs.get(&GasCategory::Execution).unwrap() > &0.0);
    }

    #[test]
    fn top_level_stylus_tx_drains_trailing_host_ios() {
        let report = MixedTraceStitcher::stitch(
            "0x999",
            42161,
            vec![],
            vec![host_io("user_entrypoint", 1_000_000, 900_000)],
        );
        assert_eq!(report.stylus_steps().len(), 1);
        assert_eq!(report.steps[0].depth, 0);
    }

    #[test]
    fn boundary_steps_filter_returns_only_calls() {
        let evm_logs = vec![evm("ADD", 3, 1), evm("CALL", 100, 1)];
        let report = MixedTraceStitcher::stitch("0xfff", 42161, evm_logs, vec![]);
        assert_eq!(
            report.boundary_steps().len(),
            0,
            "CALL without Stylus steps should not be a boundary"
        );

        let evm_logs2 = vec![evm("ADD", 3, 1), evm("CALL", 100, 1)];
        let stylus_steps = vec![host_io("user_entrypoint", 100_000, 90_000)];
        let report2 = MixedTraceStitcher::stitch("0xfff", 42161, evm_logs2, stylus_steps);
        assert_eq!(
            report2.boundary_steps().len(),
            1,
            "CALL before Stylus steps should be a boundary"
        );
        assert_eq!(report2.boundary_steps()[0].label, "CALL");
    }

    #[test]
    fn target_address_is_extracted_from_evm_stack() {
        let mut log = evm("CALL", 100, 1);
        log.stack = Some(vec![
            "0x0".into(),
            "0x0".into(),
            "0x4".into(),
            "0x20".into(),
            "0x0".into(),
            "0x00000000000000000000000071C7656EC7ab88b098defB751B7401B5f6d8976F".into(),
            "0x1000".into(),
        ]);

        let report = MixedTraceStitcher::stitch("0xabc", 1, vec![log], vec![]);
        assert_eq!(
            report.steps[0].target_address.as_deref(),
            Some("0x71c7656ec7ab88b098defb751b7401b5f6d8976f")
        );
    }
}
