//! ASCII banner and terminal styling helpers for the Atupa CLI.

use colored::*;

/// Prints the Atupa terminal banner.
pub fn print_banner() {
    eprintln!(
        "{}",
        "╔════════════════════════════════════════════╗".dimmed()
    );
    eprintln!(
        "{} {} {}",
        "║".dimmed(),
        " 🏮  ATUPA  ·  Unified Execution Profiler  ".bold(),
        "║".dimmed()
    );
    eprintln!(
        "{}",
        "╚════════════════════════════════════════════╝".dimmed()
    );
    eprintln!();
}

/// Returns ANSI color escape code for HostIO call labels.
pub fn hostio_category_color(label: &str) -> &'static str {
    match label {
        "storage_flush_cache" | "storage_store_bytes32" => "\x1b[31;1m",
        "storage_load_bytes32" | "storage_cache_bytes32" => "\x1b[33m",
        "native_keccak256" => "\x1b[35m",
        "read_args" | "write_result" | "pay_for_memory_grow" => "\x1b[32m",
        "msg_sender" | "msg_value" | "msg_reentrant" | "emit_log" | "account_balance"
        | "block_hash" => "\x1b[36m",
        "call" | "static_call" | "delegate_call" | "create" => "\x1b[34m",
        _ => "\x1b[90m",
    }
}
