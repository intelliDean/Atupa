//! [`GasCategory`] — logical cost-driver classification for execution steps.

use crate::VmKind;
use serde::{Deserialize, Serialize};

/// Logical grouping of an execution step by its dominant cost driver.
///
/// This categorisation is VM-agnostic — the same category names are used
/// whether the step came from EVM, Stylus, Starknet, Solana, or Stellar.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
pub enum GasCategory {
    /// Persistent state writes (e.g. `SSTORE`, `storage_store`, `storage_write`).
    StorageWrite,
    /// Persistent state reads (e.g. `SLOAD`, `storage_load`, `storage_read`).
    StorageRead,
    /// Memory allocation and access (e.g. `MLOAD`, `MSTORE`, `memory_grow`).
    Memory,
    /// Cryptographic operations (e.g. `KECCAK256`, `pedersen`, `secp256k1`).
    Crypto,
    /// Cross-frame calls and contract deployments (e.g. `CALL`, `invoke_signed`).
    Call,
    /// Arithmetic, logic, stack management, and control-flow opcodes.
    Execution,
    /// Precompiled contract calls.
    Precompile,
    /// The root execution frame itself.
    Root,
    /// Any step that does not match a more specific category.
    #[default]
    Other,
}

impl GasCategory {
    /// Classify a single execution step given its opcode/label and the VM that produced it.
    ///
    /// Each VM has its own naming conventions, so classification is delegated
    /// to a VM-specific function. For EVM, classification is exhaustive over
    /// known opcodes; for all other VMs a keyword-matching strategy is used.
    pub fn from_step(op: &str, vm: &VmKind) -> Self {
        let op = op.trim();
        match vm {
            VmKind::Evm => Self::from_evm(op),
            VmKind::Stylus => Self::from_stylus(op),
            VmKind::Starknet => Self::from_starknet(op),
            VmKind::Solana => Self::from_solana(op),
            VmKind::Stellar => Self::from_stellar(op),
        }
    }

    // ─── EVM (exhaustive over all known opcodes) ──────────────────────────────

    fn from_evm(op: &str) -> Self {
        match op {
            // Storage
            "SSTORE" | "TSTORE" => Self::StorageWrite,
            "SLOAD" | "TLOAD" => Self::StorageRead,
            // Memory
            "MLOAD" | "MSTORE" | "MSTORE8" | "MCOPY" | "MSIZE" => Self::Memory,
            // Cryptography
            "KECCAK256" | "SHA3" => Self::Crypto,
            // Calls & deployment
            "CALL" | "STATICCALL" | "DELEGATECALL" | "CALLCODE" | "CREATE" | "CREATE2"
            | "RETURN" | "REVERT" | "STOP" | "INVALID" | "SELFDESTRUCT" => Self::Call,
            // Arithmetic, logic, stack & control flow
            "ADD" | "SUB" | "MUL" | "DIV" | "SDIV" | "MOD" | "SMOD" | "ADDMOD" | "MULMOD"
            | "EXP" | "SIGNEXTEND" | "LT" | "GT" | "SLT" | "SGT" | "EQ" | "ISZERO" | "AND"
            | "OR" | "XOR" | "NOT" | "BYTE" | "SHL" | "SHR" | "SAR" | "POP" | "PUSH1" | "PUSH2"
            | "PUSH3" | "PUSH4" | "PUSH5" | "PUSH6" | "PUSH7" | "PUSH8" | "PUSH9" | "PUSH10"
            | "PUSH11" | "PUSH12" | "PUSH13" | "PUSH14" | "PUSH15" | "PUSH16" | "PUSH17"
            | "PUSH18" | "PUSH19" | "PUSH20" | "PUSH21" | "PUSH22" | "PUSH23" | "PUSH24"
            | "PUSH25" | "PUSH26" | "PUSH27" | "PUSH28" | "PUSH29" | "PUSH30" | "PUSH31"
            | "PUSH32" | "DUP1" | "DUP2" | "DUP3" | "DUP4" | "DUP5" | "DUP6" | "DUP7" | "DUP8"
            | "DUP9" | "DUP10" | "DUP11" | "DUP12" | "DUP13" | "DUP14" | "DUP15" | "DUP16"
            | "SWAP1" | "SWAP2" | "SWAP3" | "SWAP4" | "SWAP5" | "SWAP6" | "SWAP7" | "SWAP8"
            | "SWAP9" | "SWAP10" | "SWAP11" | "SWAP12" | "SWAP13" | "SWAP14" | "SWAP15"
            | "SWAP16" | "JUMP" | "JUMPI" | "PC" | "GAS" | "JUMPDEST" => Self::Execution,
            _ => Self::Other,
        }
    }

    // ─── Stylus WASM HostIO ───────────────────────────────────────────────────

    fn from_stylus(hostio: &str) -> Self {
        classify_by_keyword(
            hostio,
            &[
                (&["flush", "storage_store"], Self::StorageWrite),
                (&["storage_load", "storage_cache"], Self::StorageRead),
                (&["keccak", "sha2"], Self::Crypto),
                (&["call", "create"], Self::Call),
                (&["memory", "args", "return_data"], Self::Memory),
                (&["msg", "block", "tx", "evm", "user"], Self::Execution),
            ],
        )
    }

    // ─── Starknet Cairo builtins & syscalls ───────────────────────────────────

    fn from_starknet(op: &str) -> Self {
        classify_by_keyword(
            op,
            &[
                (&["storage_write"], Self::StorageWrite),
                (&["storage_read"], Self::StorageRead),
                (&["keccak", "pedersen", "poseidon", "ec_op"], Self::Crypto),
                (&["call", "deploy", "invoke"], Self::Call),
                (&["range_check", "bitwise", "steps"], Self::Execution),
            ],
        )
    }

    // ─── Solana CPI & Compute Budget ──────────────────────────────────────────

    fn from_solana(op: &str) -> Self {
        classify_by_keyword(
            op,
            &[
                (&["write", "store", "set_account"], Self::StorageWrite),
                (&["read", "load", "get_account"], Self::StorageRead),
                (&["hash", "keccak", "secp256k1", "ed25519"], Self::Crypto),
                (&["invoke", "cpi", "call"], Self::Call),
                (&["compute", "log", "instruction", "syscall"], Self::Execution),
            ],
        )
    }

    // ─── Stellar Soroban HostFn ───────────────────────────────────────────────

    fn from_stellar(op: &str) -> Self {
        classify_by_keyword(
            op,
            &[
                (&["put_contract_data", "write"], Self::StorageWrite),
                (&["get_contract_data", "read"], Self::StorageRead),
                (&["hash", "verify", "crypto", "recover"], Self::Crypto),
                (&["call", "invoke", "create_contract"], Self::Call),
                (&["value", "obj", "vec", "map", "bytes"], Self::Memory),
                (&["log", "ledger", "meta"], Self::Execution),
            ],
        )
    }
}

// ─── Private helpers ──────────────────────────────────────────────────────────

/// Walk a prioritised list of `(keywords, category)` rules and return the first
/// [`GasCategory`] whose keyword appears (case-insensitively) within `op`.
///
/// Falls through to [`GasCategory::Other`] if no rule matches.
fn classify_by_keyword(op: &str, rules: &[(&[&str], GasCategory)]) -> GasCategory {
    let lower = op.to_lowercase();
    for (keywords, category) in rules {
        if keywords.iter().any(|kw| lower.contains(kw)) {
            return category.clone();
        }
    }
    GasCategory::Other
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── EVM ───────────────────────────────────────────────────────────────────

    #[test]
    fn evm_storage_ops() {
        assert_eq!(GasCategory::from_step("SSTORE", &VmKind::Evm), GasCategory::StorageWrite);
        assert_eq!(GasCategory::from_step("TSTORE", &VmKind::Evm), GasCategory::StorageWrite);
        assert_eq!(GasCategory::from_step("SLOAD", &VmKind::Evm), GasCategory::StorageRead);
        assert_eq!(GasCategory::from_step("TLOAD", &VmKind::Evm), GasCategory::StorageRead);
    }

    #[test]
    fn evm_memory_ops() {
        assert_eq!(GasCategory::from_step("MLOAD", &VmKind::Evm), GasCategory::Memory);
        assert_eq!(GasCategory::from_step("MSTORE", &VmKind::Evm), GasCategory::Memory);
        assert_eq!(GasCategory::from_step("MCOPY", &VmKind::Evm), GasCategory::Memory);
    }

    #[test]
    fn evm_crypto_ops() {
        assert_eq!(GasCategory::from_step("KECCAK256", &VmKind::Evm), GasCategory::Crypto);
        assert_eq!(GasCategory::from_step("SHA3", &VmKind::Evm), GasCategory::Crypto);
    }

    #[test]
    fn evm_call_ops() {
        assert_eq!(GasCategory::from_step("CALL", &VmKind::Evm), GasCategory::Call);
        assert_eq!(GasCategory::from_step("DELEGATECALL", &VmKind::Evm), GasCategory::Call);
        assert_eq!(GasCategory::from_step("STATICCALL", &VmKind::Evm), GasCategory::Call);
        assert_eq!(GasCategory::from_step("CREATE", &VmKind::Evm), GasCategory::Call);
        assert_eq!(GasCategory::from_step("CREATE2", &VmKind::Evm), GasCategory::Call);
    }

    #[test]
    fn evm_execution_ops() {
        assert_eq!(GasCategory::from_step("ADD", &VmKind::Evm), GasCategory::Execution);
        assert_eq!(GasCategory::from_step("JUMPDEST", &VmKind::Evm), GasCategory::Execution);
        assert_eq!(GasCategory::from_step("PUSH32", &VmKind::Evm), GasCategory::Execution);
        assert_eq!(GasCategory::from_step("DUP1", &VmKind::Evm), GasCategory::Execution);
        assert_eq!(GasCategory::from_step("SWAP16", &VmKind::Evm), GasCategory::Execution);
    }

    #[test]
    fn evm_unknown_op_falls_to_other() {
        assert_eq!(GasCategory::from_step("CUSTOMOP", &VmKind::Evm), GasCategory::Other);
        assert_eq!(GasCategory::from_step("", &VmKind::Evm), GasCategory::Other);
    }

    // ── Stylus ────────────────────────────────────────────────────────────────

    #[test]
    fn stylus_storage_ops() {
        assert_eq!(
            GasCategory::from_step("storage_store_bytes32", &VmKind::Stylus),
            GasCategory::StorageWrite
        );
        assert_eq!(
            GasCategory::from_step("storage_load_bytes32", &VmKind::Stylus),
            GasCategory::StorageRead
        );
        assert_eq!(
            GasCategory::from_step("flush_cache", &VmKind::Stylus),
            GasCategory::StorageWrite
        );
    }

    #[test]
    fn stylus_call_ops() {
        assert_eq!(GasCategory::from_step("call_contract", &VmKind::Stylus), GasCategory::Call);
    }

    // ── Starknet ──────────────────────────────────────────────────────────────

    #[test]
    fn starknet_crypto_and_storage() {
        assert_eq!(GasCategory::from_step("pedersen_hash", &VmKind::Starknet), GasCategory::Crypto);
        assert_eq!(
            GasCategory::from_step("poseidon_hash_many", &VmKind::Starknet),
            GasCategory::Crypto
        );
        assert_eq!(
            GasCategory::from_step("storage_write", &VmKind::Starknet),
            GasCategory::StorageWrite
        );
        assert_eq!(
            GasCategory::from_step("storage_read", &VmKind::Starknet),
            GasCategory::StorageRead
        );
    }

    // ── Solana ────────────────────────────────────────────────────────────────

    #[test]
    fn solana_cpi_is_call() {
        assert_eq!(GasCategory::from_step("invoke_signed_cpi", &VmKind::Solana), GasCategory::Call);
    }

    #[test]
    fn solana_secp256k1_is_crypto() {
        assert_eq!(
            GasCategory::from_step("secp256k1_recover", &VmKind::Solana),
            GasCategory::Crypto
        );
    }

    // ── Stellar ───────────────────────────────────────────────────────────────

    #[test]
    fn stellar_hostfn_storage() {
        assert_eq!(
            GasCategory::from_step("put_contract_data", &VmKind::Stellar),
            GasCategory::StorageWrite
        );
        assert_eq!(
            GasCategory::from_step("get_contract_data", &VmKind::Stellar),
            GasCategory::StorageRead
        );
    }

    // ── Leading/trailing whitespace handling ──────────────────────────────────

    #[test]
    fn whitespace_is_trimmed() {
        assert_eq!(GasCategory::from_step("  SSTORE  ", &VmKind::Evm), GasCategory::StorageWrite);
    }
}
