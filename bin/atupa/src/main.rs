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

mod init;
mod studio;
mod thresholds;

use thresholds::AtupaConfigToml;
// ─── CLI Definition ────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "atupa",
    bin_name = "atupa",
    about = "🏮 Atupa — Universal Multi-VM Execution Profiler",
    long_about = "\
Inspect, profile, and audit transactions across Multi-VM\n\
Part of the One Block infrastructure suite.\n\
SOURCE: https://github.com/One-Block-Org/Atupa",
    version
)]
struct Cli {
    /// Arbitrum / Ethereum RPC endpoint (or set ATUPA_RPC_URL)
    #[arg(short, long, global = true, value_name = "URL")]
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

        /// Explicitly select which VM runtime to use (default: auto-detect from RPC/tx)
        #[arg(long, value_enum, value_name = "VM")]
        vm: Option<VmTarget>,
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

        /// Explicitly select which VM runtime to use (default: auto-detect from RPC URL)
        #[arg(long, value_enum, value_name = "VM")]
        vm: Option<VmTarget>,
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

        /// Simple mode override: Fail CI if gas increases by > X%
        #[arg(long, value_name = "PERCENT")]
        threshold: Option<f64>,

        /// Path to atupa.toml (defaults to looking in CWD)
        #[arg(long, value_name = "FILE")]
        config: Option<String>,

        /// Generate artifacts/diff/report.md for GitHub PRs
        #[arg(long, default_value_t = false)]
        markdown: bool,

        /// Generate visual diff flamegraph in artifacts/diff/
        #[arg(long, default_value_t = false)]
        svg: bool,

        /// Output format (summary | json | markdown)
        #[arg(short, long, value_enum, default_value_t = OutputFormat::Summary)]
        output: OutputFormat,

        /// Optional: Run DeepTracer on both and diff heuristics
        #[arg(short, long, value_enum)]
        protocol: Option<Protocol>,

        /// Explicitly select which VM runtime to use (default: auto-detect from RPC URL)
        #[arg(long, value_enum, value_name = "VM")]
        vm: Option<VmTarget>,
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

    /// Scaffold Atupa config, GitHub Actions workflow, and a profile script
    ///
    /// Run this once in a new repository to get started.
    /// Detects Foundry, Hardhat, or Stylus projects automatically.
    Init {
        /// Overwrite existing files
        #[arg(long, default_value_t = false)]
        force: bool,
    },
}

#[derive(Clone, ValueEnum, Debug, PartialEq, Eq)]
enum OutputFormat {
    /// Human-readable terminal summary (default)
    Summary,
    /// Full step-by-step JSON — suitable for CI assertions and tooling
    Json,
    /// Emit only the numeric unified cost (gas-equiv) — ideal for scripting
    Metric,
}

/// Explicitly selects which VM runtime the profiler should use when
/// auto-detection (based on RPC URL or tx-hash format) is ambiguous.
#[derive(Clone, ValueEnum, Debug, PartialEq, Eq)]
enum VmTarget {
    /// Standard EVM / Arbitrum Nitro (default)
    Evm,
    /// Arbitrum Stylus / WASM (EVM + HostIO stitching)
    Stylus,
    /// Starknet Cairo VM
    Starknet,
    /// Solana Sealevel VM
    Solana,
    /// Stellar Soroban WASM VM
    Stellar,
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
            vm,
        } => {
            if let Some(key) = etherscan_key {
                config.etherscan_key = Some(key);
            }
            cmd_profile(&config, &tx, demo, out, vm).await?;
        }
        Commands::Capture {
            tx,
            output,
            file,
            profile,
            etherscan_key,
            studio,
            vm,
        } => {
            if let Some(key) = etherscan_key {
                config.etherscan_key = Some(key);
            }
            let report_path = cmd_capture(&config, &tx, output, file, profile, vm).await?;
            if studio {
                // Pass the generated report path to Studio for auto-load
                cmd_studio(&config, config.studio_port, true, report_path).await?;
            }
        }
        Commands::Audit { tx, protocol } => {
            cmd_audit(&config, &tx, protocol).await?;
        }
        Commands::Diff {
            base,
            target,
            threshold,
            config: diff_config,
            markdown,
            svg,
            protocol,
            output,
            vm,
        } => {
            cmd_diff(
                &config,
                &base,
                &target,
                threshold,
                diff_config,
                markdown,
                svg,
                output,
                protocol,
                vm,
            )
            .await?;
        }
        Commands::Studio { port, dir, open } => {
            if let Some(d) = dir {
                config.studio_dir = Some(std::path::PathBuf::from(d));
            }
            config.studio_port = port;
            cmd_studio(&config, port, open, None).await?;
        }
        Commands::Init { force } => {
            init::execute_init(init::InitArgs { force })?;
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
    vm: Option<VmTarget>,
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

    // Convert CLI VmTarget into the SDK's VmHint
    let vm_hint = vm.map(|v| match v {
        VmTarget::Evm => atupa::profile::VmHint::Evm,
        VmTarget::Stylus => atupa::profile::VmHint::Stylus,
        VmTarget::Starknet => atupa::profile::VmHint::Starknet,
        VmTarget::Solana => atupa::profile::VmHint::Solana,
        VmTarget::Stellar => atupa::profile::VmHint::Stellar,
    });

    // Route output through the standard artifacts directory (same as capture)
    let svg_path = resolve_artifact_path(out, "profile", tx, "svg");

    let (out_path, network) = atupa::execute_profile(
        tx,
        &config.rpc_url,
        demo,
        Some(svg_path),
        config.etherscan_key.clone(),
        vm_hint,
    )
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
    vm: Option<VmTarget>,
) -> Result<Option<String>> {
    let tx = normalise_hash(tx);
    eprintln!("{} {}", "→ Transaction:".bold(), tx.cyan());
    eprintln!("{} {}\n", "→ Endpoint:   ".bold(), config.rpc_url.dimmed());

    // Hint-or-heuristic routing (same priority logic as execute_profile)
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

    let eth_client = EthClient::new(config.rpc_url.clone());
    let client = NitroClient::new(config.rpc_url.clone());

    // Fetch the top-level calldata selector (non-fatal) — gives us the real function being called
    let top_level_selector = eth_client
        .get_transaction_input(&tx)
        .await
        .and_then(|input| EthClient::selector_from_input(&input));

    let pb = spinner(&format!("Fetching trace for {label} audit…"));

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
            print_aave_report(&liq, &report, top_level_selector.as_deref());
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
            print_lido_report(&res, &report, top_level_selector.as_deref());
        }
    }

    Ok(())
}

// ─── Diff Command ─────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
#[allow(clippy::collapsible_if)]
async fn cmd_diff(
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

    eprintln!(
        "{} {} {} {}",
        "→ Base:  ".bold(),
        base.cyan(),
        "Target:".bold(),
        target.yellow()
    );
    eprintln!("{} {}\n", "→ Endpoint:".bold(), config.rpc_url.dimmed());

    // Hint-or-heuristic routing
    let use_solana = matches!(vm, Some(VmTarget::Solana))
        || (vm.is_none() && config.rpc_url.contains("solana"));
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
    let cost_pct = if base_cost > 0 {
        cost_delta / base_cost as f64 * 100.0
    } else {
        0.0
    };

    let base_count = args.base_steps.len();
    let target_count = args.target_steps.len();
    let count_delta = target_count as f64 - base_count as f64;
    let count_pct = if base_count > 0 {
        count_delta / base_count as f64 * 100.0
    } else {
        0.0
    };

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

    println!(
        "{}",
        format!("  {} EXECUTION DIFF", args.network_name)
            .bold()
            .underline()
    );
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
            format!("{sign}{delta:.0} ({sign}{pct:.1}%)")
                .red()
                .to_string()
        } else if delta < 0.0 {
            format!("{sign}{delta:.0} ({sign}{pct:.1}%)")
                .green()
                .to_string()
        } else {
            format!("{sign}{delta:.0} ({sign}{pct:.1}%)")
                .dimmed()
                .to_string()
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
    if let Some(t) = args.threshold.filter(|&t| data.cost_pct > t) {
        failures.push(format!(
            "Total {} increased by {:.1}% (limit: {:.1}%)",
            args.unit_name, data.cost_pct, t
        ));
    }
    failures
}

fn generate_generic_diff_svg(args: &GenericDiffArgs) -> Result<()> {
    let pb_svg = spinner("Generating diff flamegraph…");
    let base_norm = TraceParser::normalize_raw(args.base_steps.clone());
    let target_norm = TraceParser::normalize_raw(args.target_steps.clone());
    let registry = atupa::build_default_registry();
    let base_stacks = Aggregator::build_collapsed_stacks_with_registry(&base_norm, &registry);
    let target_stacks = Aggregator::build_collapsed_stacks_with_registry(&target_norm, &registry);

    let svg_out = atupa_output::generate_diff_flamegraph(&base_stacks, &target_stacks)
        .context("SVG diff generation failed")?;
    let out_path = format!(
        "artifacts/diff/{}_vs_{}.svg",
        &args.base_tx[..10],
        &args.target_tx[..10]
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
        for f in failures.iter() {
            println!("     - {}", f.red());
        }
        return Err(anyhow::anyhow!(
            "{} regression thresholds exceeded",
            args.network_name
        ));
    } else if args.threshold.is_some() {
        println!(
            "\n  {} Execution cost within acceptable limits.",
            "✅ [PASSED]".green().bold()
        );
    }

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
    if launch_browser && let Err(e) = open::that(&url) {
        eprintln!("{} Could not open browser: {e}", "⚠".yellow());
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
        "msg_sender" | "msg_value" | "msg_reentrant" | "emit_log" | "account_balance"
        | "block_hash" => "\x1b[36m",
        "call" | "static_call" | "delegate_call" | "create" => "\x1b[34m",
        _ => "\x1b[90m",
    }
}

fn render_capture_summary(report: &StitchedReport) -> String {
    let div = "─".repeat(56).dimmed().to_string();
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

fn render_stylus_summary(
    report: &StitchedReport,
    stylus: &[&atupa_nitro::UnifiedStep],
    div: &str,
) -> String {
    let mut out = String::new();
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

fn print_aave_report(
    aave: &atupa_aave::LiquidationReport,
    nitro: &StitchedReport,
    top_selector: Option<&str>,
) {
    let div = "─".repeat(56).dimmed().to_string();
    println!("{}", "  AAVE v3 PROTOCOL AUDIT".bold().underline());
    println!("{div}");

    // Show the actual top-level function called, resolved from calldata
    if let Some(sel) = top_selector {
        let fn_name = atupa_aave::AaveV3Adapter::resolve_selector_label(sel)
            .unwrap_or_else(|| format!("unknown ({})", sel));
        println!(
            "  {:<34} {}",
            "Top-Level Call:".bold(),
            fn_name.yellow().bold()
        );
    }

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

fn print_lido_report(
    lido: &atupa_lido::LidoReport,
    nitro: &StitchedReport,
    top_selector: Option<&str>,
) {
    let div = "─".repeat(56).dimmed().to_string();
    println!("{}", "  LIDO stETH PROTOCOL AUDIT".bold().underline());
    println!("{div}");

    // Show the actual top-level function called, resolved from calldata
    if let Some(sel) = top_selector {
        let fn_name = atupa_lido::LidoAdapter::resolve_selector_label(sel)
            .unwrap_or_else(|| format!("unknown fn ({})", sel));
        println!(
            "  {:<34} {}",
            "Top-Level Call:".bold(),
            fn_name.yellow().bold()
        );
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
/// Unified helper to generate an SVG flamegraph and save it.
fn generate_and_save_svg(
    steps: &[atupa_core::TraceStep],
    tx: &str,
    file_option: &Option<String>,
) -> Result<String> {
    let pb_svg = spinner("Generating SVG flamegraph…");
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

/// Helper to save the report to disk and print the final summary.
///
/// `rendered`     — the terminal-facing string (may contain ANSI escape codes for
///                  Summary format). Printed to stdout; never written to disk.
/// `json_for_disk` — always a clean, machine-readable JSON payload that is written
///                  to the `.json` artifact file regardless of the `--output` flag.
///                  Studio, CI diffing, and any downstream tooling read this file.
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
        OutputFormat::Summary => println!("{}", rendered),
        OutputFormat::Json => println!("{}", rendered),
        OutputFormat::Metric => println!("{}", rendered),
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

/// Generic handler for Starknet traces
async fn handle_starknet_capture(
    rpc_url: &str,
    tx: &str,
    format: OutputFormat,
    file: Option<String>,
    generate_profile: bool,
) -> Result<String> {
    let pb = spinner("Detecting Starknet network and fetching execution trace…");
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

    let pb_render = spinner("Rendering report…");
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

/// Generic handler for Solana traces
async fn handle_solana_capture(
    rpc_url: &str,
    tx: &str,
    format: OutputFormat,
    file: Option<String>,
    generate_profile: bool,
) -> Result<String> {
    let pb = spinner("Detecting Solana network and fetching execution trace…");
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

    let pb_render = spinner("Rendering report…");
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

/// Generic handler for Soroban (Stellar) traces
async fn handle_stellar_capture(
    rpc_url: &str,
    tx: &str,
    format: OutputFormat,
    file: Option<String>,
    generate_profile: bool,
) -> Result<String> {
    let pb = spinner("Detecting Stellar network and fetching diagnostic events…");
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

    let pb_render = spinner("Rendering report…");
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

/// Orchestrates the multi-VM capture for Arbitrum Nitro / EVM
async fn handle_nitro_capture(
    config: &AtupaConfig,
    tx: &str,
    format: OutputFormat,
    file: Option<String>,
    generate_profile: bool,
) -> Result<String> {
    let pb = spinner("Detecting network and fetching execution trace…");
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

    // Phase 1b: fetch receipt for on-chain gasUsed (non-fatal)
    let eth_client = EthClient::new(config.rpc_url.clone());
    report.on_chain_gas_used = eth_client.get_gas_used(tx).await;

    // Phase 1.5: resolve contract names
    if let Some(key) = config.etherscan_key.clone() {
        resolve_names_via_etherscan(&mut report, &key).await?;
    }

    // Phase 2: optional Flamegraph SVG
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
    let pb_names = spinner("Resolving contract names via Etherscan…");
    let resolver = atupa_rpc::etherscan::EtherscanResolver::new(
        Some(etherscan_key.to_string()),
        report.chain_id,
    );

    let mut addresses = std::collections::HashSet::new();
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
            addresses.insert(format!("0x{}", extracted));
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

/// Returns `(terminal_rendered, json_for_disk)`.
///
/// `terminal_rendered` may contain ANSI escape codes and is only for stdout.
/// `json_for_disk` is always the full `StitchedReport` JSON — clean and
/// machine-readable regardless of the user's `--output` flag.
fn render_nitro_report(report: &StitchedReport, format: &OutputFormat) -> Result<(String, String)> {
    let pb_render = spinner("Rendering report…");
    let json_for_disk = serde_json::to_string_pretty(report)?;

    let rendered = match format {
        OutputFormat::Summary => render_capture_summary(report),
        OutputFormat::Json => json_for_disk.clone(),
        OutputFormat::Metric => format!("{:.4}", report.total_unified_cost),
    };
    pb_render.finish_with_message(format!("{} Report ready.", "✔".green().bold()));
    Ok((rendered, json_for_disk))
}

/// Helper data for Nitro/EVM diff calculation
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

/// Handler for Solana execution diffing
async fn handle_solana_diff(
    rpc_url: &str,
    base: &str,
    target: &str,
    threshold: Option<f64>,
    svg: bool,
) -> Result<()> {
    let solana_client = atupa_solana::SolanaClient::new(rpc_url.to_string());
    let pb = spinner("Fetching both Solana logs concurrently…");
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

/// Handler for Starknet execution diffing
async fn handle_starknet_diff(
    rpc_url: &str,
    base: &str,
    target: &str,
    threshold: Option<f64>,
    svg: bool,
) -> Result<()> {
    let starknet_client = atupa_starknet::StarknetClient::new(rpc_url.to_string());
    let pb = spinner("Fetching both Starknet traces concurrently…");
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

/// Handler for Stellar/Soroban execution diffing
async fn handle_stellar_diff(
    rpc_url: &str,
    base: &str,
    target: &str,
    threshold: Option<f64>,
    svg: bool,
) -> Result<()> {
    let stellar_client = atupa_stellar::StellarClient::new(rpc_url.to_string());
    let pb = spinner("Fetching both Stellar diagnostic events concurrently…");
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

/// Orchestrates the Nitro/EVM diffing process
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

    let pb = spinner("Fetching both traces and receipts concurrently…");
    let (base_report, target_report) = tokio::try_join!(
        client.trace_transaction(base),
        client.trace_transaction(target),
    )
    .context("Failed to fetch one or both traces")?;

    let (base_receipt_gas, target_receipt_gas) = tokio::join!(
        eth_client.get_gas_used(base),
        eth_client.get_gas_used(target),
    );
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
            for f in failures.iter() {
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
    let unified_pct = if base_unified_cost > 0.0 {
        unified_delta / base_unified_cost * 100.0
    } else {
        0.0
    };

    let base_total_gas = base_receipt_gas.unwrap_or(base_unified_cost as u64);
    let target_total_gas = target_receipt_gas.unwrap_or(target_unified_cost as u64);
    let total_gas_delta = target_total_gas as f64 - base_total_gas as f64;
    let total_gas_pct = if base_total_gas > 0 {
        total_gas_delta / base_total_gas as f64 * 100.0
    } else {
        0.0
    };

    let base_intrinsic = base_total_gas.saturating_sub(base_unified_cost as u64);
    let target_intrinsic = target_total_gas.saturating_sub(target_unified_cost as u64);

    let base_evm = evm_count(&base_report);
    let tgt_evm = evm_count(&target_report);
    let evm_delta = tgt_evm as f64 - base_evm as f64;
    let evm_pct = if base_evm > 0 {
        evm_delta / base_evm as f64 * 100.0
    } else {
        0.0
    };

    let base_stylus = base_report.stylus_steps().len();
    let tgt_stylus = target_report.stylus_steps().len();
    let stylus_delta = tgt_stylus as f64 - base_stylus as f64;
    let stylus_pct = if base_stylus > 0 {
        stylus_delta / base_stylus as f64 * 100.0
    } else {
        0.0
    };

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
            format!("{sign}{delta:.0} ({sign}{pct:.1}%)")
                .red()
                .to_string()
        } else if delta < 0.0 {
            format!("{sign}{delta:.0} ({sign}{pct:.1}%)")
                .green()
                .to_string()
        } else {
            format!("{sign}{delta:.0} ({sign}{pct:.1}%)")
                .dimmed()
                .to_string()
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
) -> Result<(String, Vec<atupa_core::DiffRow>)> {
    let base_steps: Vec<TraceStep> = base_report
        .steps
        .iter()
        .map(|s| s.to_trace_step())
        .collect();
    let target_steps: Vec<TraceStep> = target_report
        .steps
        .iter()
        .map(|s| s.to_trace_step())
        .collect();

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
            println!(
                "\n  {} DEEP DIFF",
                r.protocol.to_uppercase().bold().underline()
            );
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
    proto_rows: &[atupa_core::DiffRow],
) -> Result<()> {
    let mut md = String::from("## 🏮 Atupa Gas Regression Report\n\n");
    md.push_str("| Metric | Base | Target | Delta |\n");
    md.push_str("|--------|------|--------|-------|\n");

    md.push_str(&generate_summary_table_rows(data));
    md.push_str("\n*Profiled via Atupa Unified Tracer*\n");

    if !proto_rows.is_empty() {
        md.push_str(&format!("\n### 🔬 {} Protocol Deep Diff\n\n", proto_name));
        md.push_str("| Metric | Base | Target | Delta |\n");
        md.push_str("|--------|------|--------|-------|\n");
        md.push_str(&generate_protocol_deep_diff_rows(proto_rows));
    }

    let out_path = format!(
        "artifacts/diff/{}_vs_{}.md",
        &data.base_tx[..10],
        &data.target_tx[..10]
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
        (
            "EVM Steps",
            data.base_evm as f64,
            data.tgt_evm as f64,
            data.evm_delta,
            data.evm_pct,
        ),
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

fn generate_protocol_deep_diff_rows(proto_rows: &[atupa_core::DiffRow]) -> String {
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
    let base_steps: Vec<atupa_core::TraceStep> = data
        .base_report
        .steps
        .iter()
        .map(|s| s.to_trace_step())
        .collect();
    let registry = atupa::build_default_registry();
    let base_stacks = Aggregator::build_collapsed_stacks_with_registry(&TraceParser::normalize_raw(base_steps), &registry);

    let target_steps: Vec<atupa_core::TraceStep> = data
        .target_report
        .steps
        .iter()
        .map(|s| s.to_trace_step())
        .collect();
    let target_stacks =
        Aggregator::build_collapsed_stacks_with_registry(&TraceParser::normalize_raw(target_steps), &registry);

    let svg_content = atupa_output::generate_diff_flamegraph(&base_stacks, &target_stacks)?;
    let out_path = format!(
        "artifacts/diff/{}_vs_{}.svg",
        &data.base_tx[..10],
        &data.target_tx[..10]
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
    let config_toml = if let Some(path) = diff_config {
        AtupaConfigToml::load(std::path::Path::new(&path)).ok()
    } else {
        AtupaConfigToml::auto_load()
    };

    if let Some(t) = threshold {
        if data.total_gas_pct > t {
            failures.push(format!(
                "Total Gas increased by {:.1}% (limit: {:.1}%)",
                data.total_gas_pct, t
            ));
        }
    } else if let Some(ref cfg) = config_toml
        && let Some(diff_cfg) = &cfg.diff
    {
        if let Some(max_total) = diff_cfg.max_total_gas_increase_percent
            && data.total_gas_pct > max_total
        {
            failures.push(format!(
                "Total Gas increased by {:.1}% (limit: {:.1}%)",
                data.total_gas_pct, max_total
            ));
        }
        if let Some(max_exec) = diff_cfg.max_execution_gas_increase_percent
            && data.unified_pct > max_exec
        {
            failures.push(format!(
                "Execution Gas increased by {:.1}% (limit: {:.1}%)",
                data.unified_pct, max_exec
            ));
        }
        if let Some(max_evm) = diff_cfg.max_evm_steps_increase
            && data.evm_delta > max_evm as f64
        {
            failures.push(format!(
                "EVM Steps increased by {:.0} (limit: {})",
                data.evm_delta, max_evm
            ));
        }
        if let Some(max_stylus) = diff_cfg.max_stylus_calls_increase
            && data.stylus_delta > max_stylus as f64
        {
            failures.push(format!(
                "Stylus Calls increased by {:.0} (limit: {})",
                data.stylus_delta, max_stylus
            ));
        }
    }
    failures
}

// ─── Shared Utilities ─────────────────────────────────────────────────────────

/// Normalise a transaction hash or signature.
/// EVM hashes get lowercased and `0x`-prefixed.
/// Solana signatures (Base58, >70 chars) are preserved exactly as provided.
fn normalise_hash(tx: &str) -> String {
    let t = tx.trim();
    
    // Solana signatures are Base58 and much longer than standard 64-char hex hashes
    if t.len() > 70 {
        return t.to_string();
    }
    
    if t.to_lowercase().starts_with("0x") {
        t.to_lowercase()
    } else {
        // EVM / Starknet typically expect 0x prefix.
        // If it's exactly 64 chars, we'll prefix it.
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

/// Converts a flat `Vec<TraceStep>` (from Starknet/Solana/Stellar adapters)
/// into a `StitchedReport` that the Studio and downstream tooling can consume.
///
/// All steps are assigned the given `chain_vm` kind so the Studio flame graph
/// renders them with the correct chain-specific colour palette.
fn trace_steps_to_report(
    tx: &str,
    steps: Vec<atupa_core::TraceStep>,
    chain_vm: VmKind,
) -> StitchedReport {
    let mut total_gas: u64 = 0;
    let mut category_costs: std::collections::HashMap<atupa_core::GasCategory, f64> =
        std::collections::HashMap::new();

    let unified: Vec<atupa_nitro::UnifiedStep> = steps
        .into_iter()
        .enumerate()
        .map(|(i, s)| {
            let cost = s.gas_cost as f64;
            total_gas = total_gas.saturating_add(s.gas_cost);
            let category =
                atupa_core::GasCategory::from_step(&s.op, &s.vm_kind);
            *category_costs.entry(category.clone()).or_insert(0.0) += cost;
            atupa_nitro::UnifiedStep {
                index: i,
                vm: chain_vm.clone(),
                label: s.op,
                gas_cost: s.gas_cost,
                cost_equiv: cost,
                depth: s.depth,
                is_vm_boundary: false,
                category,
                target_address: None,
                evm: None,
                stylus: None,
            }
        })
        .collect();

    StitchedReport {
        tx_hash: tx.to_string(),
        chain_id: 0,
        steps: unified,
        total_evm_gas: total_gas,
        total_stylus_ink: 0,
        vm_boundary_count: 0,
        total_stylus_gas_equiv: 0.0,
        total_unified_cost: total_gas as f64,
        category_costs,
        resolved_names: std::collections::HashMap::new(),
        on_chain_gas_used: None,
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
        let short = tx_hash
            .trim_start_matches("0x")
            .get(..10)
            .unwrap_or(tx_hash);
        match ext {
            "json" => format!("report_{short}.json"),
            "svg" => format!("profile_{short}.svg"),
            _ => format!("artifact_{short}.{ext}"),
        }
    });

    let pb = std::path::PathBuf::from(&filename);
    // If it's a simple filename (no parent directory), move it to artifacts/<category>/
    if pb
        .parent()
        .map(|p| p.as_os_str().is_empty())
        .unwrap_or(true)
    {
        let dir = format!("artifacts/{}", category);
        let _ = std::fs::create_dir_all(&dir);
        format!("{}/{}", dir, filename)
    } else {
        filename
    }
}
