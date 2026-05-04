use atupa_core::{TraceStep, VmKind};
use atupa_rpc::RpcError;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::OnceLock;
use thiserror::Error;

static INVOKE_REGEX: OnceLock<Regex> = OnceLock::new();
static CONSUMED_REGEX: OnceLock<Regex> = OnceLock::new();

fn get_invoke_regex() -> &'static Regex {
    INVOKE_REGEX.get_or_init(|| {
        Regex::new(r"Program (?P<addr>[1-9A-HJ-NP-Za-km-z]{32,44}) invoke \[(?P<depth>\d+)\]")
            .unwrap()
    })
}

fn get_consumed_regex() -> &'static Regex {
    CONSUMED_REGEX.get_or_init(|| Regex::new(r"Program (?P<addr>[1-9A-HJ-NP-Za-km-z]{32,44}) consumed (?P<cu>\d+) of (?P<total>\d+) compute units").unwrap())
}

static RETURN_REGEX: OnceLock<Regex> = OnceLock::new();
fn get_return_regex() -> &'static Regex {
    RETURN_REGEX.get_or_init(|| {
        Regex::new(r"Program (?P<addr>[1-9A-HJ-NP-Za-km-z]{32,44}) (?P<status>success|failed)")
            .unwrap()
    })
}

// ─── Solana RPC Types ────────────────────────────────────────────────────────

#[derive(Error, Debug)]
pub enum SolanaError {
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("RPC error: {0}")]
    Rpc(#[from] RpcError),
    #[error("Parsing error: {0}")]
    Parse(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaTransactionResponse {
    pub meta: Option<SolanaMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolanaMeta {
    #[serde(rename = "logMessages")]
    pub log_messages: Option<Vec<String>>,
    pub fee: u64,
}

// ─── Solana Log Parser ────────────────────────────────────────────────────────

pub struct SolanaLogStitcher;

impl SolanaLogStitcher {
    /// Reconstructs a trace timeline from raw Solana log strings.
    pub fn parse_logs(logs: &[String]) -> Vec<TraceStep> {
        let mut steps = Vec::new();

        struct ActiveFrame {
            addr: String,
            start_idx: usize,
            total_cu: u64,
            children_cu: u64,
        }

        let mut active_frames: Vec<ActiveFrame> = Vec::new();

        let invoke_re = get_invoke_regex();
        let consumed_re = get_consumed_regex();
        let return_re = get_return_regex();

        for log in logs {
            if let Some(caps) = invoke_re.captures(log) {
                let addr = caps["addr"].to_string();
                let depth: u16 = caps["depth"].parse().unwrap_or(1);

                let short_addr = if addr.len() > 8 { &addr[0..8] } else { &addr };

                steps.push(TraceStep {
                    pc: 0,
                    op: format!("INVOKE:{}", short_addr),
                    gas: 0,
                    gas_cost: 0, // Computed at return
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
            } else if let Some(caps) = consumed_re.captures(log) {
                let addr = caps["addr"].to_string();
                let cu: u64 = caps["cu"].parse().unwrap_or(0);

                // Match the consumed log to the current active frame for this address
                if let Some(frame) = active_frames.iter_mut().rev().find(|f| f.addr == addr) {
                    frame.total_cu = cu;
                }
            } else if let Some(caps) = return_re.captures(log) {
                let addr = caps["addr"].to_string();
                let status = &caps["status"];

                // Pop frames until we find the matching address
                // This handles cases where intermediate frames failed without a clear return log
                while let Some(frame) = active_frames.pop() {
                    let is_match = frame.addr == addr;

                    let exclusive_cu = frame.total_cu.saturating_sub(frame.children_cu);
                    steps[frame.start_idx].gas_cost = exclusive_cu;

                    if is_match && status == "failed" {
                        steps[frame.start_idx].reverted = true;
                    }

                    if let Some(parent) = active_frames.last_mut() {
                        parent.children_cu += frame.total_cu;
                    }

                    if is_match {
                        break;
                    }
                }
            }
        }

        steps
    }
}

// ─── Solana Client ────────────────────────────────────────────────────────────

pub struct SolanaClient {
    rpc_url: String,
    client: reqwest::Client,
}

impl SolanaClient {
    pub fn new(rpc_url: String) -> Self {
        Self {
            rpc_url,
            client: reqwest::Client::new(),
        }
    }

    pub async fn get_transaction_logs(&self, tx_sig: &str) -> Result<Vec<String>, SolanaError> {
        let payload = json!({
            "jsonrpc": "2.0",
            "method": "getTransaction",
            "params": [tx_sig, { "encoding": "json", "maxSupportedTransactionVersion": 0 }],
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
            return Err(SolanaError::Rpc(RpcError::Node(
                error["message"]
                    .as_str()
                    .unwrap_or("Unknown RPC error")
                    .to_string(),
            )));
        }

        let result: SolanaTransactionResponse = serde_json::from_value(response["result"].clone())
            .map_err(|e| SolanaError::Parse(e.to_string()))?;

        result
            .meta
            .and_then(|m| m.log_messages)
            .ok_or_else(|| SolanaError::Parse("No log messages found in transaction".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_solana_log_parsing() {
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

        // Step 1 is the child
        assert_eq!(steps[1].op, "INVOKE:Tokenkeg");
        assert_eq!(steps[1].depth, 2);
        assert_eq!(steps[1].gas_cost, 4000); // 4000 total - 0 children
    }
}
