//! Handler for `atupa capture` command and multi-VM trace capture routines.

use anyhow::{Context, Result};
use colored::*;
use std::collections::{HashMap, HashSet};

use atupa_core::config::AtupaConfig;
use atupa_nitro::{NitroClient, StitchedReport, UnifiedStep, VmKind};
use atupa_output::SvgGenerator;
use atupa_parser::Parser as TraceParser;
use atupa_parser::aggregator::Aggregator;
use atupa_rpc::EthClient;

use crate::banner::hostio_category_color;
use crate::cli::{OutputFormat, VmTarget};
use crate::utils::{
    divider, evm_count, get_network_name, make_spinner, normalise_hash, resolve_artifact_path,
    trace_steps_to_report,
};

/// Executes the `capture` command across EVM / Arbitrum Stylus / Solana / Starknet / Stellar.
pub async fn cmd_capture(
    config: &AtupaConfig,
    tx: &str,
    format: OutputFormat,
    file: Option<String>,
    generate_profile: bool,
    vm: Option<VmTarget>,
) -> Result<Option<String>> {
    let tx = normalise_hash(tx);
    eprintln!("{} {}", "→ Transaction:".bold(), tx.cyan());
    eprintln!("{} {}\n", "→ Endpoint:   ".bold(), config.rpc_url.dimmed());

    // Hint-or-heuristic routing
    let use_starknet = matches!(vm, Some(VmTarget::Starknet))
        || (vm.is_none() && config.rpc_url.contains("starknet"));
    let use_solana = !use_starknet
        && (matches!(vm, Some(VmTarget::Solana))
            || (vm.is_none() && config.rpc_url.contains("solana")));
    let use_stellar = !use_starknet
        && !use_solana
        && (matches!(vm, Some(VmTarget::Stellar))
            || (vm.is_none()
                && (config.rpc_url.contains("stellar") || config.rpc_url.contains("soroban"))));

    let report_path = if use_starknet {
        handle_starknet_capture(&config.rpc_url, &tx, format, file, generate_profile).await?
    } else if use_solana {
        handle_solana_capture(&config.rpc_url, &tx, format, file, generate_profile).await?
    } else if use_stellar {
        handle_stellar_capture(&config.rpc_url, &tx, format, file, generate_profile).await?
    } else {
        handle_nitro_capture(config, &tx, format, file, generate_profile).await?
    };

    Ok(Some(report_path))
}

// ─── Multi-VM Handlers ────────────────────────────────────────────────────────

async fn handle_starknet_capture(
    rpc_url: &str,
    tx: &str,
    format: OutputFormat,
    file: Option<String>,
    generate_profile: bool,
) -> Result<String> {
    let pb = make_spinner("Detecting Starknet network and fetching execution trace…");
    let client = atupa_starknet::StarknetClient::new(rpc_url.to_string());
    let steps = client.profile_transaction(tx).await.context(
        "Failed to fetch Starknet trace — ensure the RPC endpoint is valid and accessible.",
    )?;

    pb.finish_with_message(format!(
        "{} Captured Starknet trace ({} steps)",
        "✔".green().bold(),
        steps.len().to_string().cyan().bold()
    ));

    let svg_path = if generate_profile {
        Some(generate_and_save_svg(&steps, tx, &file)?)
    } else {
        None
    };

    let pb_render = make_spinner("Rendering report…");
    let report = trace_steps_to_report(tx, steps, VmKind::Starknet);
    let json_for_disk = serde_json::to_string_pretty(&report)?;
    let rendered = match format {
        OutputFormat::Summary => format!(
            "Starknet trace: {} steps · {:.2} gas-equiv",
            report.steps.len(),
            report.total_unified_cost
        ),
        OutputFormat::Json => json_for_disk.clone(),
        OutputFormat::Metric => format!("{:.4}", report.total_unified_cost),
    };
    pb_render.finish_with_message(format!("{} Report ready.", "✔".green().bold()));

    finalize_report(&rendered, &format, &json_for_disk, file, tx, svg_path)
}

async fn handle_solana_capture(
    rpc_url: &str,
    tx: &str,
    format: OutputFormat,
    file: Option<String>,
    generate_profile: bool,
) -> Result<String> {
    let pb = make_spinner("Detecting Solana network and fetching execution trace…");
    let client = atupa_solana::SolanaClient::new(rpc_url.to_string());
    let logs = client.get_transaction_logs(tx).await.context(
        "Failed to fetch Solana logs — ensure the RPC endpoint is valid and accessible.",
    )?;

    let steps = atupa_solana::SolanaLogStitcher::parse_logs(&logs);

    pb.finish_with_message(format!(
        "{} Reconstructed Solana trace ({} steps)",
        "✔".green().bold(),
        steps.len().to_string().cyan().bold()
    ));

    let svg_path = if generate_profile {
        Some(generate_and_save_svg(&steps, tx, &file)?)
    } else {
        None
    };

    let pb_render = make_spinner("Rendering report…");
    let report = trace_steps_to_report(tx, steps, VmKind::Solana);
    let json_for_disk = serde_json::to_string_pretty(&report)?;
    let rendered = match format {
        OutputFormat::Summary => format!(
            "Solana trace: {} steps · {} compute units",
            report.steps.len(),
            report.total_evm_gas
        ),
        OutputFormat::Json => json_for_disk.clone(),
        OutputFormat::Metric => format!("{:.4}", report.total_unified_cost),
    };
    pb_render.finish_with_message(format!("{} Report ready.", "✔".green().bold()));

    finalize_report(&rendered, &format, &json_for_disk, file, tx, svg_path)
}

async fn handle_stellar_capture(
    rpc_url: &str,
    tx: &str,
    format: OutputFormat,
    file: Option<String>,
    generate_profile: bool,
) -> Result<String> {
    let pb = make_spinner("Detecting Stellar network and fetching diagnostic events…");
    let client = atupa_stellar::StellarClient::new(rpc_url.to_string());
    let steps = client
        .get_transaction_trace(tx)
        .await
        .context("Failed to fetch Stellar diagnostic events — ensure the RPC endpoint supports Soroban traces.")?;

    pb.finish_with_message(format!(
        "{} Reconstructed Soroban trace ({} steps)",
        "✔".green().bold(),
        steps.len().to_string().cyan().bold()
    ));

    let svg_path = if generate_profile {
        Some(generate_and_save_svg(&steps, tx, &file)?)
    } else {
        None
    };

    let pb_render = make_spinner("Rendering report…");
    let report = trace_steps_to_report(tx, steps, VmKind::Stellar);
    let json_for_disk = serde_json::to_string_pretty(&report)?;
    let rendered = match format {
        OutputFormat::Summary => format!(
            "Stellar/Soroban trace: {} host function calls · {} resource units",
            report.steps.len(),
            report.total_evm_gas
        ),
        OutputFormat::Json => json_for_disk.clone(),
        OutputFormat::Metric => format!("{:.4}", report.total_unified_cost),
    };
    pb_render.finish_with_message(format!("{} Report ready.", "✔".green().bold()));

    finalize_report(&rendered, &format, &json_for_disk, file, tx, svg_path)
}

async fn handle_nitro_capture(
    config: &AtupaConfig,
    tx: &str,
    format: OutputFormat,
    file: Option<String>,
    generate_profile: bool,
) -> Result<String> {
    let pb = make_spinner("Detecting network and fetching execution trace…");
    let client = NitroClient::new(config.rpc_url.clone());

    let mut report = client
        .trace_transaction(tx)
        .await
        .context("Failed to fetch trace — ensure the RPC endpoint is valid and accessible.")?;

    let network_name = get_network_name(report.chain_id);
    pb.finish_with_message(format!(
        "{} Captured trace from {} ({} EVM steps{} )",
        "✔".green().bold(),
        network_name.cyan().bold(),
        evm_count(&report).to_string().green(),
        if report.total_stylus_ink > 0 {
            format!(
                " + {} Stylus HostIOs",
                report.stylus_steps().len().to_string().yellow()
            )
        } else {
            "".into()
        }
    ));

    // Fetch on-chain gasUsed from receipt (non-fatal)
    let eth_client = EthClient::new(config.rpc_url.clone());
    report.on_chain_gas_used = eth_client.get_gas_used(tx).await;

    // Resolve contract names via Etherscan if configured
    if let Some(key) = config.etherscan_key.clone() {
        resolve_names_via_etherscan(&mut report, &key).await?;
    }

    // Optional flamegraph SVG
    let svg_path = if generate_profile {
        let trace_steps: Vec<atupa_core::TraceStep> =
            report.steps.iter().map(|s| s.to_trace_step()).collect();
        Some(generate_and_save_svg(&trace_steps, tx, &file)?)
    } else {
        None
    };

    let (rendered, json_for_disk) = render_nitro_report(&report, &format)?;
    finalize_report(&rendered, &format, &json_for_disk, file, tx, svg_path)
}

async fn resolve_names_via_etherscan(
    report: &mut StitchedReport,
    etherscan_key: &str,
) -> Result<()> {
    let pb_names = make_spinner("Resolving contract names via Etherscan…");
    let resolver = atupa_rpc::etherscan::EtherscanResolver::new(
        Some(etherscan_key.to_string()),
        report.chain_id,
    );

    let mut addresses = HashSet::new();
    for step in &report.steps {
        if let Some(evm) = &step.evm
            && (evm.op.contains("CALL") || evm.op.contains("CREATE"))
            && let Some(stack) = &evm.stack
            && stack.len() >= 2
        {
            let hex_addr = &stack[stack.len() - 2];
            let clean_hex = hex_addr.trim_start_matches("0x");
            let padded = format!("{:0>40}", clean_hex);
            let extracted = &padded[padded.len() - 40..];
            addresses.insert(format!("0x{extracted}"));
        }
    }

    for addr in addresses {
        if let Some(name) = resolver.resolve_contract_name(&addr).await {
            report.resolved_names.insert(addr, name);
        }
    }
    pb_names.finish_with_message(format!(
        "{} Resolved {} contract name(s) via Etherscan.",
        "✔".green().bold(),
        report.resolved_names.len().to_string().cyan().bold()
    ));
    Ok(())
}

// ─── Rendering Helpers ────────────────────────────────────────────────────────

fn render_nitro_report(report: &StitchedReport, format: &OutputFormat) -> Result<(String, String)> {
    let pb_render = make_spinner("Rendering report…");
    let json_for_disk = serde_json::to_string_pretty(report)?;

    let rendered = match format {
        OutputFormat::Summary => render_capture_summary(report),
        OutputFormat::Json => json_for_disk.clone(),
        OutputFormat::Metric => format!("{:.4}", report.total_unified_cost),
    };
    pb_render.finish_with_message(format!("{} Report ready.", "✔".green().bold()));
    Ok((rendered, json_for_disk))
}

fn render_capture_summary(report: &StitchedReport) -> String {
    let div = divider(56);
    let mut out = String::new();

    out += &format!(
        "  {} ({})\n{}\n",
        "UNIFIED EXECUTION SUMMARY".bold().underline(),
        get_network_name(report.chain_id).cyan(),
        div
    );

    out += &render_gas_totals(report);
    out += &format!("{div}\n");
    out += &format!(
        "  {:<34} {}\n{}\n",
        "TOTAL UNIFIED COST:".bold().cyan(),
        format!("{:.2} gas", report.total_unified_cost)
            .cyan()
            .bold(),
        div
    );

    out += &format!(
        "  {:<34} {}\n",
        "EVM Steps:".bold(),
        evm_count(report).to_string().green()
    );

    let stylus = report.stylus_steps();
    if !stylus.is_empty() {
        out += &render_stylus_summary(report, &stylus, &div);
    }

    out += &format!("  tx  {}\n", report.tx_hash.dimmed());
    out
}

fn render_gas_totals(report: &StitchedReport) -> String {
    let mut out = String::new();
    if let Some(on_chain) = report.on_chain_gas_used {
        let execution_gas = report.total_evm_gas;
        let intrinsic_gas = on_chain.saturating_sub(execution_gas);
        out += &format!(
            "  {:<34} {}\n",
            "Total Gas Used (on-chain):".bold(),
            on_chain.to_string().green().bold()
        );
        out += &format!(
            "  {:<34} {}\n",
            "  ├─ Execution:".dimmed(),
            execution_gas.to_string().green()
        );
        out += &format!(
            "  {:<34} {}\n",
            "  └─ Intrinsic (base + calldata):".dimmed(),
            intrinsic_gas.to_string().yellow()
        );
    } else {
        out += &format!(
            "  {:<34} {}\n",
            "EVM Trace Gas (Total):".bold(),
            report.total_evm_gas.to_string().green()
        );
    }

    if report.total_stylus_ink > 0 {
        out += &format!(
            "  {:<34} {}\n",
            "Stylus Ink (raw):".bold(),
            report.total_stylus_ink.to_string().yellow()
        );
        out += &format!(
            "  {:<34} {}\n",
            "  → Gas-equivalent (÷10,000):".dimmed(),
            format!("{:.2}", report.total_stylus_gas_equiv).yellow()
        );
    }

    if report.vm_boundary_count > 0 {
        out += &format!(
            "  {:<34} {}\n",
            "VM Boundaries (EVM ↔ WASM):".bold(),
            report.vm_boundary_count.to_string().magenta()
        );
    }
    out
}

fn render_stylus_summary(report: &StitchedReport, stylus: &[&UnifiedStep], div: &str) -> String {
    let mut out = String::new();
    let mut grouped: HashMap<String, f64> = HashMap::new();
    for step in stylus.iter() {
        *grouped.entry(step.label.clone()).or_insert(0.0) += step.cost_equiv;
    }
    let mut aggregated: Vec<(String, f64)> = grouped.into_iter().collect();
    aggregated.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let total_ink_gas: f64 = aggregated.iter().map(|(_, c)| c).sum();
    let unique_paths = aggregated.len();

    out += &format!(
        "  {:<34} {}\n",
        "Stylus HostIO Calls:".bold(),
        stylus.len().to_string().yellow()
    );
    out += &format!(
        "  {:<34} {}\n",
        "Unique HostIO Paths:".bold(),
        unique_paths.to_string().yellow()
    );

    if report.vm_boundary_count > 0 {
        out += &format!("  {}\n", "EVM→WASM Boundary Details:".bold());
        for (i, step) in report.boundary_steps().iter().take(5).enumerate() {
            out += &format!(
                "    {}  {} at depth {}\n",
                format!("[{}]", i + 1).cyan(),
                step.label.bold(),
                step.depth.to_string().dimmed()
            );
        }
        if report.vm_boundary_count > 5 {
            out += &format!(
                "    … and {} more\n",
                (report.vm_boundary_count - 5).to_string().dimmed()
            );
        }
    }

    out += &format!("{div}\n");
    out += &render_hot_paths(&aggregated, total_ink_gas);
    out += &render_ascii_flamegraph(&aggregated, total_ink_gas, unique_paths);
    out += &format!("{div}\n");
    out
}

fn render_hot_paths(aggregated: &[(String, f64)], total_ink_gas: f64) -> String {
    let wide_div = "━".repeat(72);
    let reset = "\x1b[0m";
    let mut out = format!("  {}\n  {wide_div}\n", "🔥 STYLUS HOT PATHS".bold());
    out += &format!(
        "  ┃ {:<42} ┃ {:>10} ┃ {:>14} ┃ {:>7} ┃\n",
        "HostIO (Hottest First)", "GAS", "INK (raw)", "%"
    );
    out += &format!("  {wide_div}\n");

    for (label, cost_gas) in aggregated.iter().take(10) {
        let cost_ink = (cost_gas * 10_000.0) as u64;
        let pct = if total_ink_gas > 0.0 {
            cost_gas / total_ink_gas * 100.0
        } else {
            0.0
        };
        let color = hostio_category_color(label);
        let gas_str = format!("{:.0}", cost_gas);
        out += &format!(
            "  ┃ {color}{:<42}{reset} ┃ {gas_str:>10} ┃ {cost_ink:>14} ┃ {pct:>6.1}% ┃\n",
            label
        );
    }
    out += &format!("  {wide_div}\n");
    out
}

fn render_ascii_flamegraph(
    aggregated: &[(String, f64)],
    total_ink_gas: f64,
    unique_paths: usize,
) -> String {
    let reset = "\x1b[0m";
    let mut out = format!("\n  {}\n", "📊 SIMPLIFIED FLAMEGRAPH".bold());
    out += "  root ██████████████████████████████████████████████████ 100%\n";

    for (label, cost_gas) in aggregated.iter().take(5) {
        let pct = if total_ink_gas > 0.0 {
            cost_gas / total_ink_gas * 100.0
        } else {
            0.0
        };
        let bar_width = (pct / 2.0) as usize;
        let bar = "█".repeat(bar_width);
        let color = hostio_category_color(label);
        out += &format!(
            "  └─ {color}{:<20}{reset} {color}{:<50}{reset} {:>5.1}%\n",
            label, bar, pct
        );
    }
    if unique_paths > 10 {
        out += &format!("\n   ({} of {} unique paths shown)\n", 10, unique_paths);
    }
    out
}

fn generate_and_save_svg(
    steps: &[atupa_core::TraceStep],
    tx: &str,
    file_option: &Option<String>,
) -> Result<String> {
    let pb_svg = make_spinner("Generating SVG flamegraph…");
    let normalized = TraceParser::normalize_raw(steps.to_vec());
    let registry = atupa::build_default_registry();
    let stacks = Aggregator::build_collapsed_stacks_with_registry(&normalized, &registry);
    let svg =
        SvgGenerator::generate_flamegraph(&stacks).context("SVG flamegraph generation failed")?;

    let svg_suggestion = file_option.as_ref().map(|f| {
        if f.ends_with(".json") {
            f.trim_end_matches(".json").to_string() + ".svg"
        } else {
            f.to_string() + ".svg"
        }
    });
    let svg_out = resolve_artifact_path(svg_suggestion, "capture", tx, "svg");
    std::fs::write(&svg_out, svg).with_context(|| format!("Failed to write SVG to '{svg_out}'"))?;

    pb_svg.finish_with_message(format!(
        "{} SVG saved → {}",
        "✔".green().bold(),
        svg_out.green().bold()
    ));
    Ok(svg_out)
}

fn finalize_report(
    rendered: &str,
    format: &OutputFormat,
    json_for_disk: &str,
    file_option: Option<String>,
    tx: &str,
    svg_path: Option<String>,
) -> Result<String> {
    eprintln!();
    match format {
        OutputFormat::Summary => println!("{rendered}"),
        OutputFormat::Json => println!("{rendered}"),
        OutputFormat::Metric => println!("{rendered}"),
    }
    eprintln!();

    let report_path = resolve_artifact_path(file_option, "capture", tx, "json");
    std::fs::write(&report_path, json_for_disk)
        .with_context(|| format!("Failed to write report to '{report_path}'"))?;

    eprintln!(
        "{} Report saved to {}",
        "✔".green().bold(),
        report_path.cyan().bold()
    );

    if let Some(svg) = svg_path {
        eprintln!(
            "{} SVG profile saved to {}",
            "✔".green().bold(),
            svg.cyan().bold()
        );
    }

    Ok(report_path)
}
