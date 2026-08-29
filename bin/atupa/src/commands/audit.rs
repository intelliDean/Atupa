//! Handler for `atupa audit` command (Aave v3/GHO, Lido stETH).

use anyhow::{Context, Result};
use colored::*;

use atupa_aave::AaveDeepTracer;
use atupa_core::config::AtupaConfig;
use atupa_core::TraceStep;
use atupa_lido::LidoDeepTracer;
use atupa_nitro::{NitroClient, StitchedReport, VmKind};
use atupa_rpc::EthClient;

use crate::cli::Protocol;
use crate::utils::{bridge_raw_to_trace_step, divider, make_spinner, normalise_hash};

/// Executes the `audit` command against specialized protocol deep tracers.
pub async fn cmd_audit(config: &AtupaConfig, tx: &str, protocol: Protocol) -> Result<()> {
    let tx = normalise_hash(tx);
    let label = match protocol {
        Protocol::Aave => "Aave v3 + GHO",
        Protocol::Lido => "Lido stETH",
    };

    eprintln!("{} {} audit for {}", "→".bold(), label.yellow().bold(), tx.cyan());
    eprintln!("{} {}\n", "→ Endpoint:".bold(), config.rpc_url.dimmed());

    let eth_client = EthClient::new(config.rpc_url.clone());
    let client = NitroClient::new(config.rpc_url.clone());

    // Fetch the top-level calldata selector (non-fatal)
    let top_level_selector = eth_client
        .get_transaction_input(&tx)
        .await
        .and_then(|input| EthClient::selector_from_input(&input));

    let pb = make_spinner(&format!("Fetching trace for {label} audit…"));

    let report = client
        .trace_transaction(&tx)
        .await
        .context("Failed to fetch trace — is the Arbitrum / EVM node reachable?")?;

    pb.finish_with_message(format!(
        "{} Trace captured ({} unified steps).",
        "✔".green().bold(),
        report.steps.len()
    ));

    match protocol {
        Protocol::Aave => {
            let pb2 = make_spinner("Applying Aave v3 + GHO protocol adapter…");

            let trace_steps: Vec<TraceStep> = report
                .steps
                .iter()
                .filter(|s| s.vm == VmKind::Evm)
                .filter_map(|s| s.evm.as_ref())
                .map(bridge_raw_to_trace_step)
                .collect();

            let tracer = AaveDeepTracer::new();
            let liq =
                tracer.analyze_liquidation(&tx, &trace_steps).context("Aave adapter failed")?;

            pb2.finish_with_message(format!("{} Aave v3 adapter complete.", "✔".green().bold()));
            eprintln!();
            print_aave_report(&liq, &report, top_level_selector.as_deref());
        }
        Protocol::Lido => {
            let pb2 = make_spinner("Applying Lido stETH protocol adapter…");

            let trace_steps: Vec<TraceStep> = report
                .steps
                .iter()
                .filter(|s| s.vm == VmKind::Evm)
                .filter_map(|s| s.evm.as_ref())
                .map(bridge_raw_to_trace_step)
                .collect();

            let tracer = LidoDeepTracer::new();
            let res = tracer.analyze_staking(&tx, &trace_steps).context("Lido adapter failed")?;

            pb2.finish_with_message(format!("{} Lido stETH adapter complete.", "✔".green().bold()));
            eprintln!();
            print_lido_report(&res, &report, top_level_selector.as_deref());
        }
    }

    Ok(())
}

fn print_aave_report(
    aave: &atupa_aave::LiquidationReport,
    nitro: &StitchedReport,
    top_selector: Option<&str>,
) {
    let div = divider(56);
    println!("{}", "  AAVE v3 PROTOCOL AUDIT".bold().underline());
    println!("{div}");

    if let Some(sel) = top_selector {
        let fn_name = atupa_aave::AaveV3Adapter::resolve_selector_label(sel)
            .unwrap_or_else(|| format!("unknown ({sel})"));
        println!("  {:<34} {}", "Top-Level Call:".bold(), fn_name.yellow().bold());
    }

    let rows: &[(&str, String)] = &[
        ("Total Gas (Aave frame):", aave.total_gas.to_string()),
        ("Liquidation Gas:", aave.liquidation_gas.to_string()),
        ("Storage Reads (SLOAD):", aave.storage_reads.to_string()),
        ("Storage Writes (SSTORE):", aave.storage_writes.to_string()),
        ("External Calls:", aave.external_calls.to_string()),
        ("Oracle Calls:", aave.oracle_calls.to_string()),
        ("Cross-VM Calls (Stylus):", nitro.vm_boundary_count.to_string()),
        ("Max Call Depth:", aave.max_depth.to_string()),
    ];
    for (label, val) in rows {
        println!("  {:<34} {}", label.bold(), val.cyan());
    }
    println!("{div}");

    if !aave.labeled_calls.is_empty() {
        println!("  {}", "Protocol Calls Detected:".bold());
        for call in aave.labeled_calls.iter().take(10) {
            println!(
                "    {} {} {}",
                format!("[depth={:>2}]", call.depth).dimmed(),
                call.label.yellow(),
                format!("({} gas)", call.gas_cost).dimmed()
            );
        }
        println!("{div}");
    }

    println!(
        "  {:<34} {}",
        "Reverted:".bold(),
        if aave.reverted { "YES".red().bold().to_string() } else { "NO".green().to_string() }
    );
    println!("  {:<34} {:.4}", "Liquidation Efficiency:".bold(), aave.liquidation_efficiency);
    println!("{div}");
}

fn print_lido_report(
    lido: &atupa_lido::LidoReport,
    nitro: &StitchedReport,
    top_selector: Option<&str>,
) {
    let div = divider(56);
    println!("{}", "  LIDO stETH PROTOCOL AUDIT".bold().underline());
    println!("{div}");

    if let Some(sel) = top_selector {
        let fn_name = atupa_lido::LidoAdapter::resolve_selector_label(sel)
            .unwrap_or_else(|| format!("unknown fn ({sel})"));
        println!("  {:<34} {}", "Top-Level Call:".bold(), fn_name.yellow().bold());
    }

    let rows: &[(&str, String)] = &[
        ("Total Gas (Lido frame):", lido.total_gas.to_string()),
        ("Storage Reads (SLOAD):", lido.storage_reads.to_string()),
        ("Storage Writes (SSTORE):", lido.storage_writes.to_string()),
        ("External Calls:", lido.external_calls.to_string()),
        ("Shares Transfers:", lido.shares_transfers.to_string()),
        ("Oracle Reports:", lido.oracle_reports.to_string()),
        ("Withdrawal Requests:", lido.withdrawal_requests.to_string()),
        ("Withdrawal Claims:", lido.withdrawal_claims.to_string()),
        ("Wrapped Ops (wstETH):", lido.wrapped_ops.to_string()),
        ("Cross-VM Calls (Stylus):", nitro.vm_boundary_count.to_string()),
        ("Max Call Depth:", lido.max_depth.to_string()),
    ];
    for (label, val) in rows {
        println!("  {:<34} {}", label.bold(), val.cyan());
    }
    println!("{div}");

    if !lido.labeled_calls.is_empty() {
        println!("  {}", "Protocol Calls Detected:".bold());
        for call in lido.labeled_calls.iter().take(10) {
            println!(
                "    {} {} {}",
                format!("[depth={:>2}]", call.depth).dimmed(),
                call.label.yellow(),
                format!("({} gas)", call.gas_cost).dimmed()
            );
        }
        if lido.labeled_calls.len() > 10 {
            println!("    ... and {} more", (lido.labeled_calls.len() - 10).to_string().dimmed());
        }
        println!("{div}");
    }

    println!(
        "  {:<34} {}",
        "Reverted:".bold(),
        if lido.reverted { "YES".red().bold().to_string() } else { "NO".green().to_string() }
    );
    println!("{div}");
}
