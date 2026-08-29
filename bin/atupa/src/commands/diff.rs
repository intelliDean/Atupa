//! Handler for `atupa diff` command across Multi-VM and protocol deep tracers.

use anyhow::{Context, Result};
use colored::*;

use atupa_aave::AaveDeepTracer;
use atupa_core::config::AtupaConfig;
use atupa_core::{DiffRow, TraceStep};
use atupa_lido::LidoDeepTracer;
use atupa_nitro::{NitroClient, StitchedReport};
use atupa_parser::aggregator::Aggregator;
use atupa_parser::Parser as TraceParser;
use atupa_rpc::EthClient;

use crate::cli::{OutputFormat, Protocol, VmTarget};
use crate::thresholds::AtupaConfigToml;
use crate::utils::{evm_count, make_spinner, normalise_hash};

/// Executes the `diff` command, comparing base and target transactions.
#[allow(clippy::too_many_arguments)]
pub async fn cmd_diff(
    config: &AtupaConfig,
    base: &str,
    target: &str,
    threshold: Option<f64>,
    diff_config: Option<String>,
    markdown: bool,
    svg: bool,
    output_format: OutputFormat,
    protocol: Option<Protocol>,
    vm: Option<VmTarget>,
) -> Result<()> {
    let base = normalise_hash(base);
    let target = normalise_hash(target);

    eprintln!("{} {} {} {}", "→ Base:  ".bold(), base.cyan(), "Target:".bold(), target.yellow());
    eprintln!("{} {}\n", "→ Endpoint:".bold(), config.rpc_url.dimmed());

    let use_solana =
        matches!(vm, Some(VmTarget::Solana)) || (vm.is_none() && config.rpc_url.contains("solana"));
    let use_starknet = !use_solana
        && (matches!(vm, Some(VmTarget::Starknet))
            || (vm.is_none() && config.rpc_url.contains("starknet")));
    let use_stellar = !use_solana
        && !use_starknet
        && (matches!(vm, Some(VmTarget::Stellar))
            || (vm.is_none()
                && (config.rpc_url.contains("stellar") || config.rpc_url.contains("soroban"))));

    if use_solana {
        handle_solana_diff(&config.rpc_url, &base, &target, threshold, svg).await?;
    } else if use_starknet {
        handle_starknet_diff(&config.rpc_url, &base, &target, threshold, svg).await?;
    } else if use_stellar {
        handle_stellar_diff(&config.rpc_url, &base, &target, threshold, svg).await?;
    } else {
        handle_nitro_diff(
            config,
            &base,
            &target,
            threshold,
            diff_config,
            markdown,
            svg,
            output_format,
            protocol,
        )
        .await?;
    }

    Ok(())
}

// ─── Multi-VM Diff Handlers ───────────────────────────────────────────────────

struct GenericDiffArgs<'a> {
    network_name: &'a str,
    unit_name: &'a str,
    base_tx: &'a str,
    target_tx: &'a str,
    base_steps: Vec<TraceStep>,
    target_steps: Vec<TraceStep>,
    svg: bool,
    threshold: Option<f64>,
}

struct GenericDiffData {
    base_cost: u64,
    target_cost: u64,
    cost_delta: f64,
    cost_pct: f64,
    base_count: usize,
    target_count: usize,
    count_delta: f64,
    count_pct: f64,
}

fn calculate_generic_diff_data(args: &GenericDiffArgs) -> GenericDiffData {
    let base_cost = args.base_steps.iter().map(|s| s.gas_cost).sum::<u64>();
    let target_cost = args.target_steps.iter().map(|s| s.gas_cost).sum::<u64>();
    let cost_delta = target_cost as f64 - base_cost as f64;
    let cost_pct = if base_cost > 0 { cost_delta / base_cost as f64 * 100.0 } else { 0.0 };

    let base_count = args.base_steps.len();
    let target_count = args.target_steps.len();
    let count_delta = target_count as f64 - base_count as f64;
    let count_pct = if base_count > 0 { count_delta / base_count as f64 * 100.0 } else { 0.0 };

    GenericDiffData {
        base_cost,
        target_cost,
        cost_delta,
        cost_pct,
        base_count,
        target_count,
        count_delta,
        count_pct,
    }
}

fn print_generic_diff_summary(args: &GenericDiffArgs, data: &GenericDiffData) {
    let div = "─".repeat(70).dimmed().to_string();

    println!("{}", format!("  {} EXECUTION DIFF", args.network_name).bold().underline());
    println!("{div}");
    println!(
        "  {:<25} {:<15} {:<15} {}",
        "Metric".bold(),
        "Base".bold(),
        "Target".bold(),
        "Delta".bold()
    );
    println!("{div}");

    let colorize_delta = |delta: f64, pct: f64| -> String {
        let sign = if delta >= 0.0 { "+" } else { "" };
        if delta > 0.0 {
            format!("{sign}{delta:.0} ({sign}{pct:.1}%)").red().to_string()
        } else if delta < 0.0 {
            format!("{sign}{delta:.0} ({sign}{pct:.1}%)").green().to_string()
        } else {
            format!("{sign}{delta:.0} ({sign}{pct:.1}%)").dimmed().to_string()
        }
    };

    println!(
        "  {:<25} {:<15} {:<15} {}",
        format!("Total {}:", args.unit_name),
        data.base_cost.to_string().cyan(),
        data.target_cost.to_string().cyan(),
        colorize_delta(data.cost_delta, data.cost_pct)
    );

    println!(
        "  {:<25} {:<15} {:<15} {}",
        "Execution Steps:",
        data.base_count.to_string().green(),
        data.target_count.to_string().yellow(),
        colorize_delta(data.count_delta, data.count_pct)
    );
    println!("{div}\n");
}

fn evaluate_generic_thresholds(args: &GenericDiffArgs, data: &GenericDiffData) -> Vec<String> {
    let mut failures = Vec::new();
    if let Some(t) = args.threshold
        && let Some(err) = crate::thresholds::DiffConfig::evaluate_simple_threshold(
            args.unit_name,
            data.cost_pct,
            t,
        )
    {
        failures.push(err);
    }
    failures
}

fn generate_generic_diff_svg(args: &GenericDiffArgs) -> Result<()> {
    let pb_svg = make_spinner("Generating diff flamegraph…");
    let base_norm = TraceParser::normalize_raw(args.base_steps.clone());
    let target_norm = TraceParser::normalize_raw(args.target_steps.clone());
    let registry = atupa::build_default_registry();
    let base_stacks = Aggregator::build_collapsed_stacks_with_registry(&base_norm, &registry);
    let target_stacks = Aggregator::build_collapsed_stacks_with_registry(&target_norm, &registry);

    let svg_out = atupa_output::generate_diff_flamegraph(&base_stacks, &target_stacks)
        .context("SVG diff generation failed")?;
    let out_path = format!(
        "artifacts/diff/{}_vs_{}.svg",
        &args.base_tx[..10.min(args.base_tx.len())],
        &args.target_tx[..10.min(args.target_tx.len())]
    );
    std::fs::create_dir_all("artifacts/diff").ok();
    std::fs::write(&out_path, svg_out).context("Failed to write diff SVG")?;
    pb_svg.finish_with_message(format!(
        "{} Diff SVG saved → {}",
        "✔".green().bold(),
        out_path.cyan()
    ));
    Ok(())
}

fn process_generic_diff(args: GenericDiffArgs) -> Result<()> {
    let data = calculate_generic_diff_data(&args);
    print_generic_diff_summary(&args, &data);

    let failures = evaluate_generic_thresholds(&args, &data);

    if args.svg {
        generate_generic_diff_svg(&args)?;
    }

    if !failures.is_empty() {
        println!("\n  {}", "❌ [FAILED] Regression detected:".red().bold());
        for f in &failures {
            println!("     - {}", f.red());
        }
        return Err(anyhow::anyhow!("{} regression thresholds exceeded", args.network_name));
    } else if args.threshold.is_some() {
        println!("\n  {} Execution cost within acceptable limits.", "✅ [PASSED]".green().bold());
    }

    Ok(())
}

async fn handle_solana_diff(
    rpc_url: &str,
    base: &str,
    target: &str,
    threshold: Option<f64>,
    svg: bool,
) -> Result<()> {
    let solana_client = atupa_solana::SolanaClient::new(rpc_url.to_string());
    let pb = make_spinner("Fetching both Solana logs concurrently…");
    let (base_logs, target_logs) = tokio::try_join!(
        solana_client.get_transaction_logs(base),
        solana_client.get_transaction_logs(target),
    )
    .context("Failed to fetch Solana logs")?;
    pb.finish_with_message(format!("{} Both traces fetched.", "✔".green().bold()));
    eprintln!();

    let base_steps = atupa_solana::SolanaLogStitcher::parse_logs(&base_logs);
    let target_steps = atupa_solana::SolanaLogStitcher::parse_logs(&target_logs);

    process_generic_diff(GenericDiffArgs {
        network_name: "Solana",
        unit_name: "Compute Units",
        base_tx: base,
        target_tx: target,
        base_steps,
        target_steps,
        svg,
        threshold,
    })
}

async fn handle_starknet_diff(
    rpc_url: &str,
    base: &str,
    target: &str,
    threshold: Option<f64>,
    svg: bool,
) -> Result<()> {
    let starknet_client = atupa_starknet::StarknetClient::new(rpc_url.to_string());
    let pb = make_spinner("Fetching both Starknet traces concurrently…");
    let (base_steps, target_steps) = tokio::try_join!(
        starknet_client.profile_transaction(base),
        starknet_client.profile_transaction(target),
    )
    .context("Failed to fetch Starknet traces")?;
    pb.finish_with_message(format!("{} Both traces fetched.", "✔".green().bold()));
    eprintln!();

    process_generic_diff(GenericDiffArgs {
        network_name: "Starknet Cairo",
        unit_name: "Gas-Equivalent Steps",
        base_tx: base,
        target_tx: target,
        base_steps,
        target_steps,
        svg,
        threshold,
    })
}

async fn handle_stellar_diff(
    rpc_url: &str,
    base: &str,
    target: &str,
    threshold: Option<f64>,
    svg: bool,
) -> Result<()> {
    let stellar_client = atupa_stellar::StellarClient::new(rpc_url.to_string());
    let pb = make_spinner("Fetching both Stellar diagnostic events concurrently…");
    let (base_steps, target_steps) = tokio::try_join!(
        stellar_client.get_transaction_trace(base),
        stellar_client.get_transaction_trace(target),
    )
    .context("Failed to fetch Stellar traces")?;
    pb.finish_with_message(format!("{} Both traces fetched.", "✔".green().bold()));
    eprintln!();

    process_generic_diff(GenericDiffArgs {
        network_name: "Stellar Soroban",
        unit_name: "HostFn Weight",
        base_tx: base,
        target_tx: target,
        base_steps,
        target_steps,
        svg,
        threshold,
    })
}

// ─── Nitro / EVM Diff Handlers ────────────────────────────────────────────────

struct NitroDiffData<'a> {
    base_tx: &'a str,
    target_tx: &'a str,
    base_report: StitchedReport,
    target_report: StitchedReport,
    base_total_gas: u64,
    target_total_gas: u64,
    total_gas_delta: f64,
    total_gas_pct: f64,
    base_unified_cost: f64,
    target_unified_cost: f64,
    unified_delta: f64,
    unified_pct: f64,
    base_intrinsic: u64,
    target_intrinsic: u64,
    base_evm: usize,
    tgt_evm: usize,
    evm_delta: f64,
    evm_pct: f64,
    base_stylus: usize,
    tgt_stylus: usize,
    stylus_delta: f64,
    stylus_pct: f64,
}

#[allow(clippy::too_many_arguments)]
async fn handle_nitro_diff(
    config: &AtupaConfig,
    base: &str,
    target: &str,
    threshold: Option<f64>,
    diff_config: Option<String>,
    markdown: bool,
    svg: bool,
    output_format: OutputFormat,
    protocol: Option<Protocol>,
) -> Result<()> {
    let client = NitroClient::new(config.rpc_url.clone());
    let eth_client = EthClient::new(config.rpc_url.clone());

    let pb = make_spinner("Fetching both traces and receipts concurrently…");
    let (base_report, target_report) =
        tokio::try_join!(client.trace_transaction(base), client.trace_transaction(target),)
            .context("Failed to fetch one or both traces")?;

    let (base_receipt_gas, target_receipt_gas) =
        tokio::join!(eth_client.get_gas_used(base), eth_client.get_gas_used(target),);
    pb.finish_with_message(format!("{} Both traces fetched.", "✔".green().bold()));
    eprintln!();

    let data = calculate_nitro_diff_data(
        base,
        target,
        base_report,
        target_report,
        base_receipt_gas,
        target_receipt_gas,
    );

    print_nitro_diff_summary(&data);

    let (proto_name, proto_rows) = if let Some(ref proto) = protocol {
        handle_protocol_deep_diff(proto, base, target, &data.base_report, &data.target_report)
            .await?
    } else {
        (String::new(), Vec::new())
    };

    if markdown {
        generate_diff_markdown(&data, &proto_name, &proto_rows)?;
    }

    if svg {
        generate_diff_svg(&data)?;
    }

    let failures = evaluate_thresholds(&data, threshold, diff_config);

    if output_format == OutputFormat::Json {
        let diff_report = serde_json::json!({
            "type": "diff",
            "protocol": protocol.map(|p| format!("{:?}", p)),
            "base": { "tx_hash": base, "report": data.base_report },
            "target": { "tx_hash": target, "report": data.target_report },
            "metrics": {
                "base_total_gas": data.base_total_gas,
                "target_total_gas": data.target_total_gas,
                "gas_delta": data.total_gas_delta,
                "gas_pct": data.total_gas_pct,
                "base_unified_cost": data.base_unified_cost,
                "target_unified_cost": data.target_unified_cost,
                "unified_delta": data.unified_delta,
                "unified_pct": data.unified_pct,
            }
        });
        println!("{}", serde_json::to_string_pretty(&diff_report)?);
    } else {
        if !failures.is_empty() {
            println!("\n  {}", "❌ [FAILED] Regression detected:".red().bold());
            for f in &failures {
                println!("     - {}", f.red());
            }
        } else if threshold.is_some() || AtupaConfigToml::auto_load().is_some() {
            println!(
                "\n  {} Execution cost within acceptable limits.",
                "✅ [PASSED]".green().bold()
            );
        }
    }

    if !failures.is_empty() {
        return Err(anyhow::anyhow!("Gas regression thresholds exceeded"));
    }

    Ok(())
}

fn calculate_nitro_diff_data<'a>(
    base_tx: &'a str,
    target_tx: &'a str,
    base_report: StitchedReport,
    target_report: StitchedReport,
    base_receipt_gas: Option<u64>,
    target_receipt_gas: Option<u64>,
) -> NitroDiffData<'a> {
    let base_unified_cost = base_report.total_unified_cost;
    let target_unified_cost = target_report.total_unified_cost;
    let unified_delta = target_unified_cost - base_unified_cost;
    let unified_pct =
        if base_unified_cost > 0.0 { unified_delta / base_unified_cost * 100.0 } else { 0.0 };

    let base_total_gas = base_receipt_gas.unwrap_or(base_unified_cost as u64);
    let target_total_gas = target_receipt_gas.unwrap_or(target_unified_cost as u64);
    let total_gas_delta = target_total_gas as f64 - base_total_gas as f64;
    let total_gas_pct =
        if base_total_gas > 0 { total_gas_delta / base_total_gas as f64 * 100.0 } else { 0.0 };

    let base_intrinsic = base_total_gas.saturating_sub(base_unified_cost as u64);
    let target_intrinsic = target_total_gas.saturating_sub(target_unified_cost as u64);

    let base_evm = evm_count(&base_report);
    let tgt_evm = evm_count(&target_report);
    let evm_delta = tgt_evm as f64 - base_evm as f64;
    let evm_pct = if base_evm > 0 { evm_delta / base_evm as f64 * 100.0 } else { 0.0 };

    let base_stylus = base_report.stylus_steps().len();
    let tgt_stylus = target_report.stylus_steps().len();
    let stylus_delta = tgt_stylus as f64 - base_stylus as f64;
    let stylus_pct = if base_stylus > 0 { stylus_delta / base_stylus as f64 * 100.0 } else { 0.0 };

    NitroDiffData {
        base_tx,
        target_tx,
        base_report,
        target_report,
        base_total_gas,
        target_total_gas,
        total_gas_delta,
        total_gas_pct,
        base_unified_cost,
        target_unified_cost,
        unified_delta,
        unified_pct,
        base_intrinsic,
        target_intrinsic,
        base_evm,
        tgt_evm,
        evm_delta,
        evm_pct,
        base_stylus,
        tgt_stylus,
        stylus_delta,
        stylus_pct,
    }
}

fn print_nitro_diff_summary(data: &NitroDiffData) {
    let div = "─".repeat(70).dimmed().to_string();
    println!("{}", "  EXECUTION DIFF".bold().underline());
    println!("{div}");
    println!(
        "  {:<25} {:<15} {:<15} {}",
        "Metric".bold(),
        "Base".bold(),
        "Target".bold(),
        "Delta".bold()
    );
    println!("{div}");

    let colorize_delta = |delta: f64, pct: f64| -> String {
        let sign = if delta >= 0.0 { "+" } else { "" };
        if delta > 0.0 {
            format!("{sign}{delta:.0} ({sign}{pct:.1}%)").red().to_string()
        } else if delta < 0.0 {
            format!("{sign}{delta:.0} ({sign}{pct:.1}%)").green().to_string()
        } else {
            format!("{sign}{delta:.0} ({sign}{pct:.1}%)").dimmed().to_string()
        }
    };

    println!(
        "  {:<25} {:<15} {:<15} {}",
        "Total On-Chain Gas:",
        data.base_total_gas.to_string().green(),
        data.target_total_gas.to_string().yellow(),
        colorize_delta(data.total_gas_delta, data.total_gas_pct)
    );

    println!(
        "  {:<25} {:<15} {:<15} {}",
        "↳ Execution Gas (EVM):",
        data.base_unified_cost.to_string().cyan(),
        data.target_unified_cost.to_string().cyan(),
        colorize_delta(data.unified_delta, data.unified_pct)
    );

    let intrinsic_delta = data.target_intrinsic as f64 - data.base_intrinsic as f64;
    let intrinsic_pct = if data.base_intrinsic > 0 {
        intrinsic_delta / data.base_intrinsic as f64 * 100.0
    } else {
        0.0
    };
    println!(
        "  {:<25} {:<15} {:<15} {}",
        "↳ Intrinsic Gas:",
        data.base_intrinsic.to_string().dimmed(),
        data.target_intrinsic.to_string().dimmed(),
        colorize_delta(intrinsic_delta, intrinsic_pct)
    );

    println!("{div}");

    println!(
        "  {:<25} {:<15} {:<15} {}",
        "EVM Steps:",
        data.base_evm.to_string().green(),
        data.tgt_evm.to_string().yellow(),
        colorize_delta(data.evm_delta, data.evm_pct)
    );

    println!(
        "  {:<25} {:<15} {:<15} {}",
        "Stylus Cross-VM Calls:",
        data.base_stylus.to_string().green(),
        data.tgt_stylus.to_string().yellow(),
        colorize_delta(data.stylus_delta, data.stylus_pct)
    );
    println!("{div}");
}

async fn handle_protocol_deep_diff(
    proto: &Protocol,
    base: &str,
    target: &str,
    base_report: &StitchedReport,
    target_report: &StitchedReport,
) -> Result<(String, Vec<DiffRow>)> {
    let base_steps: Vec<TraceStep> = base_report.steps.iter().map(|s| s.to_trace_step()).collect();
    let target_steps: Vec<TraceStep> =
        target_report.steps.iter().map(|s| s.to_trace_step()).collect();

    let report = match proto {
        Protocol::Aave => {
            AaveDeepTracer::new().diff_reports(base, &base_steps, target, &target_steps)
        }
        Protocol::Lido => {
            LidoDeepTracer::new().diff_reports(base, &base_steps, target, &target_steps)
        }
    };

    match report {
        Ok(r) => {
            let proto_div = "─".repeat(70).dimmed().to_string();
            println!("\n  {} DEEP DIFF", r.protocol.to_uppercase().bold().underline());
            println!("{proto_div}");
            println!(
                "  {:<28} {:<15} {:<15} {}",
                "Metric".bold(),
                "Base".bold(),
                "Target".bold(),
                "Delta".bold()
            );
            println!("{proto_div}");

            for row in &r.rows {
                let sign = if row.delta >= 0.0 { "+" } else { "" };
                let delta_str = format!("{sign}{:.0} ({sign}{:.1}%)", row.delta, row.pct);
                let delta_colored = if row.delta == 0.0 {
                    delta_str.dimmed().to_string()
                } else if (row.delta > 0.0) == row.higher_is_worse {
                    delta_str.red().to_string()
                } else {
                    delta_str.green().to_string()
                };
                println!(
                    "  {:<28} {:<15} {:<15} {}",
                    row.metric,
                    row.base.to_string().dimmed(),
                    row.target.to_string().dimmed(),
                    delta_colored
                );
            }
            println!("{proto_div}");
            Ok((r.protocol, r.rows))
        }
        Err(e) => {
            eprintln!("  ⚠ Protocol deep diff skipped: {e}");
            Ok((String::new(), Vec::new()))
        }
    }
}

fn generate_diff_markdown(
    data: &NitroDiffData,
    proto_name: &str,
    proto_rows: &[DiffRow],
) -> Result<()> {
    let mut md = String::from("## 🏮 Atupa Gas Regression Report\n\n");
    md.push_str("| Metric | Base | Target | Delta |\n");
    md.push_str("|--------|------|--------|-------|\n");

    md.push_str(&generate_summary_table_rows(data));
    md.push_str("\n*Profiled via Atupa Unified Tracer*\n");

    if !proto_rows.is_empty() {
        md.push_str(&format!("\n### 🔬 {proto_name} Protocol Deep Diff\n\n"));
        md.push_str("| Metric | Base | Target | Delta |\n");
        md.push_str("|--------|------|--------|-------|\n");
        md.push_str(&generate_protocol_deep_diff_rows(proto_rows));
    }

    let out_path = format!(
        "artifacts/diff/{}_vs_{}.md",
        &data.base_tx[..10.min(data.base_tx.len())],
        &data.target_tx[..10.min(data.target_tx.len())]
    );
    std::fs::create_dir_all("artifacts/diff").ok();
    std::fs::write(&out_path, md).context("Failed to write markdown diff")?;
    println!("  📝 Markdown report written to {}", out_path.cyan());
    Ok(())
}

fn generate_summary_table_rows(data: &NitroDiffData) -> String {
    let format_plain_delta = |delta: f64, pct: f64| -> String {
        let sign = if delta >= 0.0 { "+" } else { "" };
        format!("{sign}{delta:.0} ({sign}{pct:.1}%)")
    };

    let mut rows = String::new();
    let entries = [
        (
            "Total Gas",
            data.base_total_gas as f64,
            data.target_total_gas as f64,
            data.total_gas_delta,
            data.total_gas_pct,
        ),
        (
            "Execution Gas",
            data.base_unified_cost,
            data.target_unified_cost,
            data.unified_delta,
            data.unified_pct,
        ),
        ("EVM Steps", data.base_evm as f64, data.tgt_evm as f64, data.evm_delta, data.evm_pct),
        (
            "Stylus Calls",
            data.base_stylus as f64,
            data.tgt_stylus as f64,
            data.stylus_delta,
            data.stylus_pct,
        ),
    ];

    for (metric, base, target, delta, pct) in entries {
        rows.push_str(&format!(
            "| **{}** | {} | {} | {} |\n",
            metric,
            base,
            target,
            format_plain_delta(delta, pct)
        ));
    }
    rows
}

fn generate_protocol_deep_diff_rows(proto_rows: &[DiffRow]) -> String {
    let mut rows = String::new();
    for row in proto_rows {
        let sign = if row.delta >= 0.0 { "+" } else { "" };
        let emoji = if row.delta == 0.0 {
            ""
        } else if (row.delta > 0.0) == row.higher_is_worse {
            "🔴 "
        } else {
            "🟢 "
        };
        rows.push_str(&format!(
            "| **{}** | {} | {} | {}{}{:.0} ({}{:.1}%) |\n",
            row.metric, row.base, row.target, emoji, sign, row.delta, sign, row.pct
        ));
    }
    rows
}

fn generate_diff_svg(data: &NitroDiffData) -> Result<()> {
    let base_steps: Vec<TraceStep> =
        data.base_report.steps.iter().map(|s| s.to_trace_step()).collect();
    let registry = atupa::build_default_registry();
    let base_stacks = Aggregator::build_collapsed_stacks_with_registry(
        &TraceParser::normalize_raw(base_steps),
        &registry,
    );

    let target_steps: Vec<TraceStep> =
        data.target_report.steps.iter().map(|s| s.to_trace_step()).collect();
    let target_stacks = Aggregator::build_collapsed_stacks_with_registry(
        &TraceParser::normalize_raw(target_steps),
        &registry,
    );

    let svg_content = atupa_output::generate_diff_flamegraph(&base_stacks, &target_stacks)?;
    let out_path = format!(
        "artifacts/diff/{}_vs_{}.svg",
        &data.base_tx[..10.min(data.base_tx.len())],
        &data.target_tx[..10.min(data.target_tx.len())]
    );
    std::fs::create_dir_all("artifacts/diff").ok();
    std::fs::write(&out_path, svg_content).context("Failed to write diff flamegraph SVG")?;
    println!("  🔥 Visual diff flamegraph written to {}", out_path.cyan());
    Ok(())
}

fn evaluate_thresholds(
    data: &NitroDiffData,
    threshold: Option<f64>,
    diff_config: Option<String>,
) -> Vec<String> {
    let mut failures = Vec::new();
    let config_toml = AtupaConfigToml::resolve(diff_config.as_deref());

    if let Some(t) = threshold {
        if let Some(err) =
            crate::thresholds::DiffConfig::evaluate_simple_threshold("Gas", data.total_gas_pct, t)
        {
            failures.push(err);
        }
    } else if let Some(ref cfg) = config_toml
        && let Some(diff_cfg) = &cfg.diff
    {
        failures.extend(diff_cfg.evaluate_nitro(
            data.total_gas_pct,
            data.unified_pct,
            data.evm_delta,
            data.stylus_delta,
        ));
    }
    failures
}
