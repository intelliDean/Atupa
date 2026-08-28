//! Reconstructs hierarchical execution traces from raw Solana program log events.

use atupa_core::{TraceStep, VmKind};
use regex::Regex;
use std::sync::OnceLock;

static INVOKE_REGEX: OnceLock<Regex> = OnceLock::new();
static CONSUMED_REGEX: OnceLock<Regex> = OnceLock::new();
static RETURN_REGEX: OnceLock<Regex> = OnceLock::new();

fn get_invoke_regex() -> &'static Regex {
    INVOKE_REGEX.get_or_init(|| {
        Regex::new(r"Program (?P<addr>[1-9A-HJ-NP-Za-km-z]{32,44}) invoke \[(?P<depth>\d+)\]")
            .expect("Valid invoke regex")
    })
}

fn get_consumed_regex() -> &'static Regex {
    CONSUMED_REGEX.get_or_init(|| {
        Regex::new(r"Program (?P<addr>[1-9A-HJ-NP-Za-km-z]{32,44}) consumed (?P<cu>\d+) of (?P<total>\d+) compute units")
            .expect("Valid consumed regex")
    })
}

fn get_return_regex() -> &'static Regex {
    RETURN_REGEX.get_or_init(|| {
        Regex::new(r"Program (?P<addr>[1-9A-HJ-NP-Za-km-z]{32,44}) (?P<status>success|failed)")
            .expect("Valid return regex")
    })
}

/// Reconstructs linear/hierarchical [`TraceStep`] execution traces from raw Solana log strings.
pub struct SolanaLogStitcher;

impl SolanaLogStitcher {
    /// Reconstructs a trace timeline from raw Solana log strings.
    pub fn parse_logs(logs: &[String]) -> Vec<TraceStep> {
        let mut steps = Vec::new();
        let mut active_frames: Vec<ActiveFrame> = Vec::new();

        let invoke_re = get_invoke_regex();
        let consumed_re = get_consumed_regex();
        let return_re = get_return_regex();

        for log in logs {
            if let Some(caps) = invoke_re.captures(log) {
                handle_invoke(&caps, &mut steps, &mut active_frames);
            } else if let Some(caps) = consumed_re.captures(log) {
                handle_consumed(&caps, &mut active_frames);
            } else if let Some(caps) = return_re.captures(log) {
                handle_return(&caps, &mut steps, &mut active_frames);
            }
        }

        steps
    }
}

// ─── Internal Parsing Helpers ─────────────────────────────────────────────────

struct ActiveFrame {
    addr: String,
    start_idx: usize,
    total_cu: u64,
    children_cu: u64,
}

fn handle_invoke(
    caps: &regex::Captures<'_>,
    steps: &mut Vec<TraceStep>,
    active_frames: &mut Vec<ActiveFrame>,
) {
    let addr = caps["addr"].to_string();
    let depth: u16 = caps["depth"].parse().unwrap_or(1);

    let short_addr = if addr.len() > 8 {
        &addr[0..8]
    } else {
        &addr
    };

    steps.push(TraceStep {
        pc: 0,
        op: format!("INVOKE:{short_addr}"),
        gas: 0,
        gas_cost: 0, // Computed when the frame returns
        depth,
        stack: Some(vec![addr.clone()]),
        memory: None,
        error: None,
        reverted: false,
        vm_kind: VmKind::Solana,
    });

    active_frames.push(ActiveFrame {
        addr,
        start_idx: steps.len() - 1,
        total_cu: 0,
        children_cu: 0,
    });
}

fn handle_consumed(caps: &regex::Captures<'_>, active_frames: &mut [ActiveFrame]) {
    let addr = &caps["addr"];
    let cu: u64 = caps["cu"].parse().unwrap_or(0);

    // Match the consumed log to the topmost active frame for this address
    if let Some(frame) = active_frames.iter_mut().rev().find(|f| f.addr == addr) {
        frame.total_cu = cu;
    }
}

fn handle_return(
    caps: &regex::Captures<'_>,
    steps: &mut [TraceStep],
    active_frames: &mut Vec<ActiveFrame>,
) {
    let addr = &caps["addr"];
    let status = &caps["status"];

    // Pop frames until we find the matching address (handles intermediate uncaught failures)
    while let Some(frame) = active_frames.pop() {
        let is_match = frame.addr == addr;

        let exclusive_cu = frame.total_cu.saturating_sub(frame.children_cu);
        if let Some(step) = steps.get_mut(frame.start_idx) {
            step.gas_cost = exclusive_cu;
            if is_match && status == "failed" {
                step.reverted = true;
            }
        }

        if let Some(parent) = active_frames.last_mut() {
            parent.children_cu = parent.children_cu.saturating_add(frame.total_cu);
        }

        if is_match {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_program_invocations() {
        let logs = vec![
            "Program 11111111111111111111111111111111 invoke [1]".to_string(),
            "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA invoke [2]".to_string(),
            "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA consumed 4000 of 195000 compute units".to_string(),
            "Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA success".to_string(),
            "Program 11111111111111111111111111111111 consumed 5000 of 200000 compute units".to_string(),
            "Program 11111111111111111111111111111111 success".to_string(),
        ];

        let steps = SolanaLogStitcher::parse_logs(&logs);
        assert_eq!(steps.len(), 2);

        // Step 0 is the parent
        assert_eq!(steps[0].op, "INVOKE:11111111");
        assert_eq!(steps[0].depth, 1);
        assert_eq!(steps[0].gas_cost, 1000); // 5000 total - 4000 children
        assert!(!steps[0].reverted);

        // Step 1 is the child
        assert_eq!(steps[1].op, "INVOKE:Tokenkeg");
        assert_eq!(steps[1].depth, 2);
        assert_eq!(steps[1].gas_cost, 4000); // 4000 total - 0 children
        assert!(!steps[1].reverted);
    }

    #[test]
    fn parses_failed_invocation() {
        let logs = vec![
            "Program 11111111111111111111111111111111 invoke [1]".to_string(),
            "Program 11111111111111111111111111111111 consumed 2500 of 200000 compute units".to_string(),
            "Program 11111111111111111111111111111111 failed".to_string(),
        ];

        let steps = SolanaLogStitcher::parse_logs(&logs);
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].gas_cost, 2500);
        assert!(steps[0].reverted);
    }

    #[test]
    fn empty_logs_returns_empty_steps() {
        let steps = SolanaLogStitcher::parse_logs(&[]);
        assert!(steps.is_empty());
    }
}
