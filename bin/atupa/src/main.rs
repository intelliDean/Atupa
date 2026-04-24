//! # atupa CLI
//!
//! Unified Ethereum + Arbitrum Stylus execution profiler.
//!
//! ## Usage
//!
//! ```text
//! atupa profile  --tx <HASH> [--rpc <URL>] [--out trace.svg] [--demo]
//! atupa capture  --tx <HASH> [--rpc <URL>] [--output summary|json|metric] [--file report.json]
//!               [--profile] [--etherscan-key <KEY>] [--studio]
//! atupa audit    --tx <HASH> [--rpc <URL>] [--protocol aave|lido]
//! atupa diff     --base <HASH> --target <HASH> [--rpc <URL>]
//! ```
//!
//! ## Standalone Usage
//! Atupa is designed to be used as a standalone CLI tool.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

use atupa_aave::AaveDeepTracer;
use atupa_core::TraceStep;
use atupa_core::config::AtupaConfig;
use atupa_lido::LidoDeepTracer;
use atupa_nitro::{NitroClient, StitchedReport, VmKind};
use atupa_output::SvgGenerator;
use atupa_parser::Parser as TraceParser;
use atupa_parser::aggregator::Aggregator;
use atupa_rpc::{EthClient, RawStructLog};
 
 mod studio;


// ─── CLI Definition ────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "atupa",
    bin_name = "atupa",
    about = "🏮 Atupa — Unified Ethereum & Stylus Execution Profiler",
    long_about = "\
Inspect, profile, and audit transactions across the full Arbitrum Nitro\n\
dual-VM stack (EVM + Stylus WASM). Part of the One Block infrastructure suite.\n\
SOURCE: https://github.com/One-Block-Org/Atupa",
    version
)]
struct Cli {
    /// Arbitrum / Ethereum RPC endpoint (or set ATUPA_RPC_URL)
    #[arg(
        short,
        long,
        global = true,
        value_name = "URL"
    )]
    rpc: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate a visual SVG flamegraph for any EVM transaction
    Profile {
        /// Transaction hash (0x-prefixed); omit when using --demo
        #[arg(short, long, value_name = "TX_HASH", default_value = "")]
        tx: String,

        /// Run an offline demo trace (no RPC required)
        #[arg(long, default_value_t = false)]
        demo: bool,

        /// Output path for the SVG (default: profile_<tx>.svg)
        #[arg(short, long, value_name = "FILE")]
        out: Option<String>,

        /// Etherscan API key for contract name resolution
        #[arg(long, value_name = "KEY")]
        etherscan_key: Option<String>,
    },

    /// Capture a unified EVM + Stylus execution trace (Arbitrum Nitro).
    ///
    /// Add --profile to also generate an SVG flamegraph from the same RPC call.
    /// Add --studio  to automatically launch Atupa Studio with the report loaded.
    Capture {
        /// Transaction hash to profile (0x-prefixed)
        #[arg(short, long, value_name = "TX_HASH")]
        tx: String,

        /// Output format for the JSON/summary report
        #[arg(short, long, value_enum, default_value_t = OutputFormat::Summary)]
        output: OutputFormat,

        /// Write report to a file instead of stdout
        #[arg(short = 'f', long, value_name = "FILE")]
        file: Option<String>,

        /// Also generate an SVG flamegraph (reuses the same RPC trace)
        #[arg(long, default_value_t = false)]
        profile: bool,

        /// Etherscan API key for contract name resolution
        #[arg(long, value_name = "KEY")]
        etherscan_key: Option<String>,

        /// Launch Atupa Studio after capture and open it in the browser
        #[arg(long, default_value_t = false)]
        studio: bool,
    },

    /// Protocol-aware execution auditing (Aave v3/GHO, Lido)
    Audit {
        /// Transaction hash to audit (0x-prefixed)
        #[arg(short, long, value_name = "TX_HASH")]
        tx: String,

        /// Protocol adapter to apply
        #[arg(short, long, value_enum, default_value_t = Protocol::Aave)]
        protocol: Protocol,
    },

    /// Compare the execution cost of two transactions
    Diff {
        /// Base transaction hash (0x-prefixed)
        #[arg(short, long, value_name = "BASE_TX")]
        base: String,

        /// Target transaction hash (0x-prefixed)
        #[arg(short, long, value_name = "TARGET_TX")]
        target: String,
    },

    /// Launch Atupa Studio — the local web visualizer for trace reports
    Studio {
        /// Port for the dev server (default: 5173)
        #[arg(short, long, default_value_t = 5173)]
        port: u16,

        /// Path to the studio directory (overrides auto-detection)
        #[arg(long, value_name = "DIR")]
        dir: Option<String>,

        /// Open the browser automatically after the server starts
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        open: bool,
    },
}

#[derive(Clone, ValueEnum, Debug)]
enum OutputFormat {
    /// Human-readable terminal summary (default)
    Summary,
    /// Full step-by-step JSON — suitable for CI assertions and tooling
    Json,
    /// Emit only the numeric unified cost (gas-equiv) — ideal for scripting
    Metric,
}

#[derive(Clone, ValueEnum, Debug)]
enum Protocol {
    /// Aave v3 + GHO stablecoin protocol adapters
    Aave,
    /// Lido stETH execution resilience (Phase II roadmap)
    Lido,
}

// ─── Entry Point ──────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    let args = std::env::args_os();

    env_logger::builder()
        .filter_level(log::LevelFilter::Warn)
        .parse_default_env()
        .init();

    let cli = Cli::parse_from(args);
    let mut config = AtupaConfig::load();

    if let Some(r) = cli.rpc {
        config.rpc_url = r;
    }

    print_banner();

    match cli.command {
        Commands::Profile {
            tx,
            demo,
            out,
            etherscan_key,
        } => {
            if let Some(key) = etherscan_key {
                config.etherscan_key = Some(key);
            }
            cmd_profile(&config, &tx, demo, out).await?;
        }
        Commands::Capture {
            tx,
            output,
            file,
            profile,
            etherscan_key,
            studio,
        } => {
            if let Some(key) = etherscan_key {
                config.etherscan_key = Some(key);
            }
            let report_path = cmd_capture(&config, &tx, output, file, profile).await?;
            if studio {
                // Pass the generated report path to Studio for auto-load
                cmd_studio(&config, config.studio_port, true, report_path).await?;
            }
        }
        Commands::Audit { tx, protocol } => {
            cmd_audit(&config, &tx, protocol).await?;
        }
        Commands::Diff { base, target } => {
            cmd_diff(&config, &base, &target).await?;
        }
        Commands::Studio { port, dir, open } => {
            if let Some(d) = dir {
                config.studio_dir = Some(std::path::PathBuf::from(d));
            }
            config.studio_port = port;
            cmd_studio(&config, port, open, None).await?;
        }
    }

    Ok(())
}

// ─── Profile Command ──────────────────────────────────────────────────────────

async fn cmd_profile(
    config: &AtupaConfig,
    tx: &str,
    demo: bool,
    out: Option<String>,
) -> Result<()> {
    if !demo && tx.is_empty() {
        anyhow::bail!(
            "You must provide --tx <HASH> or run with --demo.\n\
             Example: atupa profile --demo"
        );
    }

    let display = if demo { "demo" } else { tx };
    eprintln!("{} {}", "→ Profiling:".bold(), display.cyan());
    eprintln!("{} {}\n", "→ Endpoint: ".bold(), config.rpc_url.dimmed());

    // Route output through the standard artifacts directory (same as capture)
    let svg_path = resolve_artifact_path(out, "profile", tx, "svg");

    let (out_path, network) = atupa::execute_profile(tx, &config.rpc_url, demo, Some(svg_path), config.etherscan_key.clone())
        .await
        .context("Profile command failed")?;

    eprintln!();
    eprintln!(
        "  {} ({})",
        "PROFILE COMPLETE".bold().underline(),
        network.cyan()
    );
    let div = "─".repeat(40).dimmed().to_string();
    eprintln!("{div}");
    eprintln!(
        "  {:<24} {}",
        "SVG saved to:".bold(),
        out_path.green().bold()
    );
    eprintln!("{div}");
    Ok(())
}


// ─── Capture Command ──────────────────────────────────────────────────────────

async fn cmd_capture(
    config: &AtupaConfig,
    tx: &str,
    format: OutputFormat,
    file: Option<String>,
    generate_profile: bool,
) -> Result<Option<String>> {
    let tx = normalise_hash(tx);
    eprintln!("{} {}", "→ Transaction:".bold(), tx.cyan());
    eprintln!("{} {}\n", "→ Endpoint:   ".bold(), config.rpc_url.dimmed());

    // Phase 1: fetch ──────────────────────────────────────────────────────────
    let pb = spinner("Detecting network and fetching execution trace…");
    let client = NitroClient::new(config.rpc_url.clone());
 
    let mut report = client
        .trace_transaction(&tx)
        .await
        .context("Failed to fetch trace — ensure the RPC endpoint is valid and accessible.")?;
 
    let network_name = get_network_name(report.chain_id);
    pb.finish_with_message(format!(
        "{} Captured trace from {} ({} EVM steps{} )",
        "✔".green().bold(),
        network_name.cyan().bold(),
        evm_count(&report).to_string().green(),
        if report.total_stylus_ink > 0 {
            format!(" + {} Stylus HostIOs", report.stylus_steps().len().to_string().yellow())
        } else {
            "".into()
        }
    ));

    // Phase 1b: fetch receipt for on-chain gasUsed (non-fatal) ──────────────────
    let eth_client = EthClient::new(config.rpc_url.clone());
    report.on_chain_gas_used = eth_client.get_gas_used(&tx).await;

    // Phase 1.5: resolve contract names ─────────────────────────────────────────
    if let Some(key) = config.etherscan_key.clone() {
        let pb_names = spinner("Resolving contract names via Etherscan…");
        let resolver = atupa_rpc::etherscan::EtherscanResolver::new(Some(key), report.chain_id);
        
        let mut addresses = std::collections::HashSet::new();
        for step in &report.steps {
            if let Some(evm) = &step.evm {
                if evm.op.contains("CALL") || evm.op.contains("CREATE") {
                    if let Some(stack) = &evm.stack {
                        if stack.len() >= 2 {
                            let hex_addr = &stack[stack.len() - 2];
                            let clean_hex = hex_addr.trim_start_matches("0x");
                            let padded = format!("{:0>40}", clean_hex);
                            let extracted = &padded[padded.len() - 40..];
                            addresses.insert(format!("0x{}", extracted));
                        }
                    }
                }
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
    }

    // Phase 2: optional Flamegraph SVG (built from already-fetched report — no second RPC call) ──
    let mut svg_path: Option<String> = None;
    if generate_profile {
        let pb_svg = spinner("Generating SVG flamegraph…");

        // Convert report steps → collapsed stacks → SVG (zero extra RPC calls)
        let trace_steps: Vec<atupa_core::TraceStep> = report.steps.iter().map(|s| s.to_trace_step()).collect();
        let normalized = TraceParser::normalize_raw(trace_steps);
        let stacks = Aggregator::build_collapsed_stacks(&normalized);
        let svg = SvgGenerator::generate_flamegraph(&stacks)
            .context("SVG flamegraph generation failed")?;

        let svg_suggestion = file.as_ref().map(|f| {
            if f.ends_with(".json") {
                f.trim_end_matches(".json").to_string() + ".svg"
            } else {
                f.to_string() + ".svg"
            }
        });
        let svg_out = resolve_artifact_path(svg_suggestion, "capture", &tx, "svg");
        std::fs::write(&svg_out, svg)
            .with_context(|| format!("Failed to write SVG to '{svg_out}'"))?;

        pb_svg.finish_with_message(format!("{} SVG saved → {}", "✔".green().bold(), svg_out.green().bold()));
        svg_path = Some(svg_out);
    }

    // Phase 3: render report ──────────────────────────────────────────────────
    let pb2 = spinner("Rendering report…");
    let summary_text = render_capture_summary(&report);
    
    let rendered = match format {
        OutputFormat::Summary => summary_text.clone(),
        OutputFormat::Json => serde_json::to_string_pretty(&report)?,
        OutputFormat::Metric => format!("{:.4}", report.total_unified_cost),
    };
    pb2.finish_with_message(format!("{} Report ready.", "✔".green().bold()));

    eprintln!();
    println!("{}", summary_text);
    eprintln!();

    // Phase 4: output ─────────────────────────────────────────────────────────
    let report_path = resolve_artifact_path(file, "capture", &tx, "json");

    std::fs::write(&report_path, &rendered)
        .with_context(|| format!("Failed to write report to '{report_path}'"))?;

    eprintln!(
        "{} Report saved to {}",
        "✔".green().bold(),
        report_path.cyan().bold()
    );

    if let Some(ref svg) = svg_path {
        eprintln!("{} SVG profile saved to {}", "✔".green().bold(), svg.cyan().bold());
    }

    Ok(Some(report_path))
}

// ─── Audit Command ────────────────────────────────────────────────────────────

async fn cmd_audit(config: &AtupaConfig, tx: &str, protocol: Protocol) -> Result<()> {
    let tx = normalise_hash(tx);
    let label = match protocol {
        Protocol::Aave => "Aave v3 + GHO",
        Protocol::Lido => "Lido stETH",
    };

    eprintln!(
        "{} {} audit for {}",
        "→".bold(),
        label.yellow().bold(),
        tx.cyan()
    );
    eprintln!("{} {}\n", "→ Endpoint:".bold(), config.rpc_url.dimmed());

    let pb = spinner(&format!("Fetching trace for {label} audit…"));
    let client = NitroClient::new(config.rpc_url.clone());

    let report = client
        .trace_transaction(&tx)
        .await
        .context("Failed to fetch trace — is the Arbitrum node running?")?;

    pb.finish_with_message(format!(
        "{} Trace captured ({} unified steps).",
        "✔".green().bold(),
        report.steps.len()
    ));

    match protocol {
        Protocol::Aave => {
            let pb2 = spinner("Applying Aave v3 + GHO protocol adapter…");

            let trace_steps: Vec<TraceStep> = report
                .steps
                .iter()
                .filter(|s| s.vm == VmKind::Evm)
                .filter_map(|s| s.evm.as_ref())
                .map(bridge_raw_to_trace_step)
                .collect();

            let tracer = AaveDeepTracer::new();
            let liq = tracer
                .analyze_liquidation(&tx, &trace_steps)
                .context("Aave adapter failed")?;

            pb2.finish_with_message(format!("{} Aave v3 adapter complete.", "✔".green().bold()));
            eprintln!();
            print_aave_report(&liq, &report);
        }
        Protocol::Lido => {
            let pb2 = spinner("Applying Lido stETH protocol adapter…");

            let trace_steps: Vec<TraceStep> = report
                .steps
                .iter()
                .filter(|s| s.vm == VmKind::Evm)
                .filter_map(|s| s.evm.as_ref())
                .map(bridge_raw_to_trace_step)
                .collect();

            let tracer = LidoDeepTracer::new();
            let res = tracer
                .analyze_staking(&tx, &trace_steps)
                .context("Lido adapter failed")?;

            pb2.finish_with_message(format!(
                "{} Lido stETH adapter complete.",
                "✔".green().bold()
            ));
            eprintln!();
            print_lido_report(&res, &report);
        }
    }

    Ok(())
}

// ─── Diff Command ─────────────────────────────────────────────────────────────

async fn cmd_diff(config: &AtupaConfig, base: &str, target: &str) -> Result<()> {
    let base = normalise_hash(base);
    let target = normalise_hash(target);

    eprintln!(
        "{} {} {} {}",
        "→ Base:  ".bold(),
        base.cyan(),
        "Target:".bold(),
        target.yellow()
    );
    eprintln!("{} {}\n", "→ Endpoint:".bold(), config.rpc_url.dimmed());

    let client = NitroClient::new(config.rpc_url.clone());

    let pb = spinner("Fetching both traces concurrently…");
    let (base_report, target_report) = tokio::try_join!(
        client.trace_transaction(&base),
        client.trace_transaction(&target),
    )
    .context("Failed to fetch one or both traces")?;
    pb.finish_with_message(format!("{} Both traces fetched.", "✔".green().bold()));

    eprintln!();

    // Cost delta
    let base_cost = base_report.total_unified_cost;
    let target_cost = target_report.total_unified_cost;
    let delta = target_cost - base_cost;
    let pct = if base_cost > 0.0 {
        delta / base_cost * 100.0
    } else {
        0.0
    };

    let div = "─".repeat(56).dimmed().to_string();
    println!("{}", "  EXECUTION DIFF".bold().underline());
    println!("{div}");
    println!(
        "  {:<30} {}",
        "Base unified cost (gas):".bold(),
        format!("{base_cost:.2}").green()
    );
    println!(
        "  {:<30} {}",
        "Target unified cost (gas):".bold(),
        format!("{target_cost:.2}").yellow()
    );
    println!("{div}");

    let sign = if delta >= 0.0 { "+" } else { "" };
    let color = if delta > 0.0 {
        format!("{sign}{delta:.2}").red().to_string()
    } else if delta < 0.0 {
        format!("{sign}{delta:.2}").green().to_string()
    } else {
        format!("{sign}{delta:.2}").dimmed().to_string()
    };
    println!(
        "  {:<30} {} ({sign}{pct:.1}%)",
        "Δ Unified Cost:".bold(),
        color
    );
    println!("{div}");

    // Step count comparison
    let base_evm = evm_count(&base_report);
    let tgt_evm = evm_count(&target_report);
    println!(
        "  {:<30} {} EVM | {} Stylus",
        "Base steps:".bold(),
        base_evm.to_string().green(),
        base_report.stylus_steps().len().to_string().yellow()
    );
    println!(
        "  {:<30} {} EVM | {} Stylus",
        "Target steps:".bold(),
        tgt_evm.to_string().green(),
        target_report.stylus_steps().len().to_string().yellow()
    );
    println!("{div}");

    Ok(())
}

// ─── Studio Command ───────────────────────────────────────────────────────────

async fn cmd_studio(
    _config: &AtupaConfig,
    port: u16,
    launch_browser: bool,
    report_path: Option<String>,
) -> Result<()> {
    // 1. Read report if provided
    let report_content = if let Some(path) = report_path.as_ref() {
        Some(std::fs::read_to_string(path).context("Failed to read report file for Studio")?)
    } else {
        None
    };

    // 2. Prepare the server
    let server = studio::StudioServer::new(report_content);
    let mut url = format!("http://localhost:{port}/");
    if report_path.is_some() {
        url += "?auto=true";
    }

    eprintln!("{} Launching Atupa Studio...", "→".bold().cyan());

    // Spawn server in background
    let server_handle = tokio::spawn(async move {
        if let Err(e) = server.start(port).await {
            eprintln!("\n{} Studio server error: {e}", "⚠".red().bold());
        }
    });

    // Wait for the port to be active
    let addr = format!("127.0.0.1:{port}");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while std::net::TcpStream::connect(&addr).is_err() {
        if std::time::Instant::now() > deadline {
            anyhow::bail!("Studio server failed to start on port {port} within 5s.");
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    eprintln!(
        "{} Studio ready at {}",
        "✔".green().bold(),
        url.cyan().bold()
    );

    // 3. Open browser
    if launch_browser {
        if let Err(e) = open::that(&url) {
            eprintln!("{} Could not open browser: {e}", "⚠".yellow());
        }
    }

    // 4. Footer info
    if let Some(path) = report_path {
        eprintln!(
            "\n  {} Report loaded: {}\n  The Studio has automatically opened this report.",
            "✔".green().bold(),
            path.cyan().bold(),
        );
    }
    eprintln!("{}\n", "  Press Ctrl+C to stop the Studio server.".dimmed());

    // Keep the main thread alive while the server runs
    let _ = server_handle.await;
    Ok(())
}

// ─── Banner & Rendering ───────────────────────────────────────────────────────



fn print_banner() {
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

fn hostio_category_color(label: &str) -> &'static str {
    match label {
        "storage_flush_cache" | "storage_store_bytes32" => "\x1b[31;1m",
        "storage_load_bytes32" | "storage_cache_bytes32" => "\x1b[33m",
        "native_keccak256" => "\x1b[35m",
        "read_args" | "write_result" | "pay_for_memory_grow" => "\x1b[32m",
        "msg_sender" | "msg_value" | "msg_reentrant"
        | "emit_log" | "account_balance" | "block_hash" => "\x1b[36m",
        "call" | "static_call" | "delegate_call" | "create" => "\x1b[34m",
        _ => "\x1b[90m",
    }
}

fn render_capture_summary(report: &StitchedReport) -> String {
    const RESET: &str = "\x1b[0m";
    let div = "─".repeat(56).dimmed().to_string();
    let wide_div = "━".repeat(72);
    let mut out = String::new();

    out += &format!(
        "  {} ({})\n",
        "UNIFIED EXECUTION SUMMARY".bold().underline(),
        get_network_name(report.chain_id).cyan()
    );
    out += &format!("{div}\n");

    // ── Gas totals with Execution vs Intrinsic split ───────────────────────────────
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

    out += &format!("{div}\n");
    out += &format!(
        "  {:<34} {}\n",
        "TOTAL UNIFIED COST:".bold().cyan(),
        format!("{:.2} gas", report.total_unified_cost).cyan().bold()
    );
    out += &format!("{div}\n");

    // EVM step count always shown
    out += &format!(
        "  {:<34} {}\n",
        "EVM Steps:".bold(),
        evm_count(report).to_string().green()
    );

    // Stylus section — only when HostIO steps exist
    let stylus = report.stylus_steps();
    if !stylus.is_empty() {
        // Aggregate ink cost by label
        let mut grouped: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
        for step in stylus.iter() {
            *grouped.entry(step.label.clone()).or_insert(0.0) += step.cost_equiv;
        }
        let mut aggregated: Vec<(String, f64)> = grouped.into_iter().collect();
        aggregated.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

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

        // ── Colour-coded hot-path table ────────────────────────────────────
        out += &format!("  {}\n", "🔥 STYLUS HOT PATHS".bold());
        out += &format!("  {wide_div}\n");
        out += &format!(
            "  ┃ {:<42} ┃ {:>10} ┃ {:>14} ┃ {:>7} ┃\n",
            "HostIO (Hottest First)", "GAS", "INK (raw)", "%"
        );
        out += &format!("  {wide_div}\n");
        for (label, cost_gas) in aggregated.iter().take(10) {
            let cost_ink = (cost_gas * 10_000.0) as u64;
            let pct = if total_ink_gas > 0.0 { cost_gas / total_ink_gas * 100.0 } else { 0.0 };
            let color = hostio_category_color(label);
            let gas_str = format!("{:.0}", cost_gas);
            out += &format!(
                "  ┃ {color}{:<42}{RESET} ┃ {gas_str:>10} ┃ {cost_ink:>14} ┃ {pct:>6.1}% ┃\n",
                label,
            );
        }
        out += &format!("  {wide_div}\n");

        // ── ASCII flamegraph ───────────────────────────────────────────────
        out += &format!("\n  {}\n", "📊 SIMPLIFIED FLAMEGRAPH".bold());
        out += "  root ██████████████████████████████████████████████████ 100%\n";
        for (label, cost_gas) in aggregated.iter().take(5) {
            let pct = if total_ink_gas > 0.0 { cost_gas / total_ink_gas * 100.0 } else { 0.0 };
            let bar_width = (pct / 2.0) as usize;
            let bar = "█".repeat(bar_width);
            let color = hostio_category_color(label);
            out += &format!(
                "  └─ {color}{:<20}{RESET} {color}{:<50}{RESET} {:>5.1}%\n",
                label, bar, pct
            );
        }
        if unique_paths > 10 {
            out += &format!("\n   ({} of {} unique paths shown)\n", 10, unique_paths);
        }

        out += &format!("{div}\n");
    }

    out += &format!("  tx  {}\n", report.tx_hash.dimmed());
    out
}

fn print_aave_report(aave: &atupa_aave::LiquidationReport, nitro: &StitchedReport) {
    let div = "─".repeat(56).dimmed().to_string();
    println!("{}", "  AAVE v3 PROTOCOL AUDIT".bold().underline());
    println!("{div}");

    let rows: &[(&str, String)] = &[
        ("Total Gas (Aave frame):", aave.total_gas.to_string()),
        ("Liquidation Gas:", aave.liquidation_gas.to_string()),
        ("Storage Reads (SLOAD):", aave.storage_reads.to_string()),
        ("Storage Writes (SSTORE):", aave.storage_writes.to_string()),
        ("External Calls:", aave.external_calls.to_string()),
        ("Oracle Calls:", aave.oracle_calls.to_string()),
        (
            "Cross-VM Calls (Stylus):",
            nitro.vm_boundary_count.to_string(),
        ),
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
        if aave.reverted {
            "YES".red().bold().to_string()
        } else {
            "NO".green().to_string()
        }
    );
    println!(
        "  {:<34} {:.4}",
        "Liquidation Efficiency:".bold(),
        aave.liquidation_efficiency
    );
    println!("{div}");
}

fn print_lido_report(lido: &atupa_lido::LidoReport, nitro: &StitchedReport) {
    let div = "─".repeat(56).dimmed().to_string();
    println!("{}", "  LIDO stETH PROTOCOL AUDIT".bold().underline());
    println!("{div}");

    let rows: &[(&str, String)] = &[
        ("Total Gas (Lido frame):", lido.total_gas.to_string()),
        ("Staking Operations Gas:", lido.staking_gas.to_string()),
        ("Shares Transfers:", lido.shares_transfers.to_string()),
        ("Token Transfers:", lido.token_transfers.to_string()),
        ("Oracle Updates:", lido.oracle_updates.to_string()),
        ("Wrapped TXs (wstETH):", lido.wrapped_txs.to_string()),
        (
            "Cross-VM Calls (Stylus):",
            nitro.vm_boundary_count.to_string(),
        ),
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
            println!(
                "    ... and {} more",
                (lido.labeled_calls.len() - 10).to_string().dimmed()
            );
        }
        println!("{div}");
    }

    println!(
        "  {:<34} {}",
        "Reverted:".bold(),
        if lido.reverted {
            "YES".red().bold().to_string()
        } else {
            "NO".green().to_string()
        }
    );
    println!("{div}");
}

// ─── Shared Utilities ─────────────────────────────────────────────────────────

/// Normalise a transaction hash to lowercase 0x-prefixed form.
fn normalise_hash(tx: &str) -> String {
    let t = tx.trim();
    if t.to_lowercase().starts_with("0x") {
        t.to_lowercase()
    } else {
        format!("0x{}", t.to_lowercase())
    }
}

fn evm_count(r: &StitchedReport) -> usize {
    r.steps.iter().filter(|s| s.vm == VmKind::Evm).count()
}

/// Bridge `RawStructLog` (atupa-rpc) → `TraceStep` (atupa-core) for adapters
/// that still operate on the lower-level type.
fn bridge_raw_to_trace_step(raw: &RawStructLog) -> TraceStep {
    TraceStep {
        pc: raw.pc,
        op: raw.op.clone(),
        gas: raw.gas,
        gas_cost: raw.gas_cost,
        depth: raw.depth,
        stack: raw.stack.clone(),
        memory: raw.memory.clone(),
        error: raw.error.clone(),
        reverted: raw.error.is_some(),
        vm_kind: atupa_core::VmKind::Evm,
    }
}

fn spinner(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("{spinner:.cyan} {msg}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    pb.enable_steady_tick(Duration::from_millis(80));
    pb.set_message(msg.to_string());
    pb
}

fn get_network_name(chain_id: u64) -> String {
    match chain_id {
        1 => "Ethereum Mainnet".to_string(),
        11155111 => "Sepolia Testnet".to_string(),
        17000 => "Holesky Testnet".to_string(),
        42161 => "Arbitrum One".to_string(),
        42170 => "Arbitrum Nova".to_string(),
        421614 => "Arbitrum Sepolia".to_string(),
        8453 => "Base Mainnet".to_string(),
        84532 => "Base Sepolia".to_string(),
        10 => "Optimism".to_string(),
        11155420 => "Optimism Sepolia".to_string(),
        137 => "Polygon POS".to_string(),
        1337 | 31337 => "Local Devnet".to_string(),
        412346 => "Nitro Local Devnet".to_string(),
        0 => "Unknown Network".to_string(),
        id => format!("Chain ID: {}", id),
    }
}

fn resolve_artifact_path(path: Option<String>, category: &str, tx_hash: &str, ext: &str) -> String {
    let filename = path.unwrap_or_else(|| {
        let short = tx_hash.trim_start_matches("0x").get(..10).unwrap_or(tx_hash);
        match ext {
            "json" => format!("report_{short}.json"),
            "svg" => format!("profile_{short}.svg"),
            _ => format!("artifact_{short}.{ext}"),
        }
    });

    let pb = std::path::PathBuf::from(&filename);
    // If it's a simple filename (no parent directory), move it to artifacts/<category>/
    if pb.parent().map(|p| p.as_os_str().is_empty()).unwrap_or(true) {
        let dir = format!("artifacts/{}", category);
        let _ = std::fs::create_dir_all(&dir);
        format!("{}/{}", dir, filename)
    } else {
        filename
    }
}

