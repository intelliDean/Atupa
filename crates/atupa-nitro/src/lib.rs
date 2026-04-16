use atupa_rpc::{EthClient, RawStructLog, RpcError};
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

// ─── Error Type ──────────────────────────────────────────────────────────────

#[derive(Error, Debug)]
pub enum NitroError {
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("RPC error: {0}")]
    Rpc(#[from] RpcError),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Stitching error: {0}")]
    Stitch(String),
}

// ─── Stylus Types ─────────────────────────────────────────────────────────────

/// A single HostIO event emitted by Arbitrum's `stylusTracer`.
///
/// HostIOs represent cross-VM system calls from WASM back into the Nitro host
/// (e.g. reading storage, emitting logs). Each event tracks its Ink budget.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Ink is a monotonically-decreasing budget; this will always be >= 0.
    pub fn ink_consumed(&self) -> u64 {
        self.start_ink.saturating_sub(self.end_ink)
    }

    /// Converts Ink consumed to an equivalent Gas unit.
    ///
    /// Arbitrum Nitro defines the canonical ratio: **1 Gas = 10,000 Ink**.
    /// This allows unified cost reporting across both VMs.
    pub fn ink_as_gas_equiv(&self) -> f64 {
        self.ink_consumed() as f64 / 10_000.0
    }
}

// ─── Unified Step ─────────────────────────────────────────────────────────────

/// Identifies which virtual machine produced a step.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum VmKind {
    Evm,
    Stylus,
}

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
    /// Raw EVM structLog, present only for EVM steps.
    pub evm: Option<RawStructLog>,
    /// Raw Stylus HostIO, present only for Stylus steps.
    pub stylus: Option<StylusHostIO>,
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
}

impl StitchedReport {
    /// Returns references to only the Stylus/WASM steps.
    pub fn stylus_steps(&self) -> Vec<&UnifiedStep> {
        self.steps
            .iter()
            .filter(|s| s.vm == VmKind::Stylus)
            .collect()
    }

    /// Returns references to the VM boundary (EVM→WASM crossing) steps.
    pub fn boundary_steps(&self) -> Vec<&UnifiedStep> {
        self.steps.iter().filter(|s| s.is_vm_boundary).collect()
    }
}

// ─── Stitcher Engine ──────────────────────────────────────────────────────────

/// EVM opcodes that dispatch execution into a Stylus (WASM) contract.
/// These mark the EVM→WASM transition boundary.
const CALL_OPCODES: &[&str] = &["CALL", "STATICCALL", "DELEGATECALL", "CALLCODE"];

/// The core engine for merging EVM and WASM execution paths into a unified timeline.
///
/// ## Background: How Arbitrum Nitro executes hybrid transactions
///
/// When an EVM contract calls a Stylus contract, Nitro's `debug_traceTransaction`
/// with the default tracer (`structLogger`) records the CALL opcode and then continues
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
/// After each CALL, we drain the next batch of Stylus HostIOs and interleave them
/// into the unified timeline at the same call depth. This preserves temporal ordering
/// while clearly annotating which steps belong to which VM.
pub struct MixedTraceStitcher;

impl MixedTraceStitcher {
    /// Stitches EVM structLogs with Stylus HostIO events into a `StitchedReport`.
    ///
    /// ## Algorithm
    /// 1. Stream EVM steps in program-counter order.
    /// 2. On a `CALL`/`STATICCALL`/`DELEGATECALL` opcode, mark it as a VM boundary.
    /// 3. Drain HostIOs from the Stylus stream, grouping them into the current boundary
    ///    frame. Stop when a `user_entrypoint` HostIO (signals a fresh Stylus invocation)
    ///    is encountered AND we have already ingested at least one HostIO in this window.
    /// 4. Continue streaming EVM steps from the point immediately after the CALL.
    /// 5. Drain any remaining Stylus steps (handles the case where the outer frame itself
    ///    is a Stylus contract — no preceding EVM CALL will exist).
    /// 6. Aggregate totals and build the `StitchedReport`.
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

            steps.push(UnifiedStep {
                index,
                vm: VmKind::Evm,
                label: log.op.clone(),
                gas_cost,
                cost_equiv: gas_cost as f64,
                depth,
                is_vm_boundary: false, // Will be set to true if subsequent HostIOs are found
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
            let mut window_host_io_count: usize = 0;

            loop {
                // Peek first — we may need to keep the next HostIO for the next window.
                let should_break = match stylus_iter.peek() {
                    None => true,
                    Some(next) => {
                        // A second `user_entrypoint` signals a new Stylus invocation frame.
                        // Break so the NEXT CALL boundary picks it up.
                        next.name == "user_entrypoint" && window_host_io_count > 0
                    }
                };
                if should_break {
                    break;
                }

                let host_io = stylus_iter.next().unwrap();
                let ink_used = host_io.ink_consumed();
                total_stylus_ink = total_stylus_ink.saturating_add(ink_used);
                window_host_io_count += 1;

                steps.push(UnifiedStep {
                    index,
                    vm: VmKind::Stylus,
                    label: host_io.name.clone(),
                    gas_cost: 0,
                    cost_equiv: host_io.ink_as_gas_equiv(),
                    depth, // Inherit depth from the owning CALL frame.
                    is_vm_boundary: false,
                    evm: None,
                    stylus: Some(host_io),
                });
                index += 1;
            }

            if window_host_io_count > 0 {
                vm_boundary_count += 1;
                steps[call_step_index].is_vm_boundary = true;
            }
        }

        // ── Trailing Stylus Steps ────────────────────────────────────────────
        // Drain any Stylus steps that had no matching EVM CALL preceding them.
        // This handles transactions where the TOP-LEVEL entrypoint is itself Stylus.
        for host_io in stylus_iter {
            let ink_used = host_io.ink_consumed();
            total_stylus_ink = total_stylus_ink.saturating_add(ink_used);

            steps.push(UnifiedStep {
                index,
                vm: VmKind::Stylus,
                label: host_io.name.clone(),
                gas_cost: 0,
                cost_equiv: host_io.ink_as_gas_equiv(),
                depth: 0,
                is_vm_boundary: false,
                evm: None,
                stylus: Some(host_io),
            });
            index += 1;
        }

        let total_stylus_gas_equiv = total_stylus_ink as f64 / 10_000.0;
        let total_unified_cost = total_evm_gas as f64 + total_stylus_gas_equiv;

        StitchedReport {
            tx_hash,
            chain_id,
            steps,
            total_evm_gas,
            total_stylus_ink,
            vm_boundary_count,
            total_stylus_gas_equiv,
            total_unified_cost,
        }
    }
}

// ─── Network Client ───────────────────────────────────────────────────────────

/// Arbitrum Nitro RPC client — fetches and stitches dual-VM traces concurrently.
pub struct NitroClient {
    base_client: EthClient,
    rpc_url: String,
    client: reqwest::Client,
}

impl NitroClient {
    pub fn new(rpc_url: String) -> Self {
        Self {
            base_client: EthClient::new(rpc_url.clone()),
            rpc_url,
            client: reqwest::Client::new(),
        }
    }

    /// Fetches the Stylus HostIO trace for `tx_hash` using the `stylusTracer`.
    pub async fn get_stylus_trace(&self, tx_hash: &str) -> Result<Vec<StylusHostIO>, NitroError> {
        let payload = json!({
            "jsonrpc": "2.0",
            "method": "debug_traceTransaction",
            "params": [tx_hash, { "tracer": "stylusTracer" }],
            "id": 1
        });

        let response = self
            .client
            .post(&self.rpc_url)
            .json(&payload)
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;

        if let Some(error) = response.get("error") {
            return Err(NitroError::Rpc(RpcError::Node(
                error["message"]
                    .as_str()
                    .unwrap_or("Unknown RPC error")
                    .to_string(),
            )));
        }

        let result = response.get("result").ok_or_else(|| {
            NitroError::Stitch("Missing 'result' in stylusTracer response".into())
        })?;

        Ok(serde_json::from_value(result.clone())?)
    }

    /// Fetches both EVM and Stylus traces **concurrently**, then stitches them
    /// into a single `StitchedReport`.
    ///
    /// If the `stylusTracer` is unavailable (e.g. pure-EVM transaction on Arbitrum,
    /// or an older node version), the error is silently downgraded and the report
    /// will contain only EVM steps with `total_stylus_ink = 0`.
    pub async fn trace_transaction(&self, tx_hash: &str) -> Result<StitchedReport, NitroError> {
        let chain_id = self.base_client.get_chain_id().await.unwrap_or(0);
        
        let is_nitro = match chain_id {
            // Known Arbitrum / Nitro chains
            42161 | 42170 | 421611 | 421613 | 421614 | 23011913 => true,
            // Local devnets often used for Nitro
            1337 | 31337 => true,
            // Known non-Nitro chains (skip tracer)
            1 | 11155111 | 17000 | 8453 | 84532 | 10 | 11155420 | 137 => false,
            // Unknown – try it but don't fail hard
            _ => true,
        };

        log::info!("atupa-nitro: fetching trace for {} (chain_id: {}, nitro_aware: {})", tx_hash, chain_id, is_nitro);

        let (evm_result, stylus_result) = if is_nitro {
            tokio::join!(
                self.base_client.get_transaction_trace(tx_hash),
                self.get_stylus_trace(tx_hash),
            )
        } else {
            (self.base_client.get_transaction_trace(tx_hash).await, Ok(Vec::new()))
        };

        let evm_trace = evm_result?;
        let stylus_trace = stylus_result.unwrap_or_else(|e| {
            log::warn!(
                "atupa-nitro: stylusTracer unavailable for {} ({}); falling back to pure-EVM.",
                tx_hash,
                e,
            );
            Vec::new()
        });

        let report = MixedTraceStitcher::stitch(tx_hash, chain_id, evm_trace.struct_logs, stylus_trace);

        log::info!(
            "atupa-nitro: {} steps stitched | network: {} | EVM gas: {} | Stylus ink: {} ({:.2} gas-equiv) | boundaries: {}",
            report.steps.len(),
            chain_id,
            report.total_evm_gas,
            report.total_stylus_ink,
            report.total_stylus_gas_equiv,
            report.vm_boundary_count,
        );

        Ok(report)
    }
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

        // 3 EVM + 2 Stylus = 5 total
        assert_eq!(report.steps.len(), 5);
        assert_eq!(report.vm_boundary_count, 1);
        assert_eq!(report.total_evm_gas, 103);
        assert_eq!(report.total_stylus_ink, 200_000);
        // Unified cost: 103 gas + 200_000/10_000 gas-equiv = 103 + 20 = 123
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
        // Each user_entrypoint should be in a separate window (depth preserved).
        // First window entry is at index 1, second at index 3.
        assert_eq!(report.steps[1].label, "user_entrypoint");
        assert_eq!(report.steps[3].label, "user_entrypoint");
    }

    #[test]
    fn top_level_stylus_tx_drains_trailing_host_ios() {
        // No EVM CALL — the outer frame IS the Stylus contract.
        let report = MixedTraceStitcher::stitch(
            "0x999",
            42161,
            vec![],
            vec![host_io("user_entrypoint", 1_000_000, 900_000)],
        );
        assert_eq!(report.stylus_steps().len(), 1);
        assert_eq!(report.steps[0].depth, 0); // no EVM depth to inherit
    }

    #[test]
    fn ink_gas_equiv_ratio_is_correct() {
        // 1 Gas = 10,000 Ink.
        let h = host_io("test", 20_000, 10_000);
        assert_eq!(h.ink_consumed(), 10_000);
        assert!((h.ink_as_gas_equiv() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn boundary_steps_filter_returns_only_calls() {
        // Without any HostIO steps, a CALL must NOT be marked as a VM boundary.
        // (Pure-EVM transactions have no Stylus crossing — no false positives.)
        let evm_logs = vec![evm("ADD", 3, 1), evm("CALL", 100, 1)];
        let report = MixedTraceStitcher::stitch("0xfff", 42161, evm_logs, vec![]);
        assert_eq!(
            report.boundary_steps().len(),
            0,
            "CALL without Stylus steps should not be a boundary"
        );

        // With HostIO steps present, the CALL that precedes them IS a boundary.
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
}
