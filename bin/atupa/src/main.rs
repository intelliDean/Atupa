//! # atupa CLI
//!
//! Universal Multi-VM Execution Profiler (EVM, Arbitrum Nitro/Stylus, Solana, Starknet, Stellar).
//!
//! ## Usage
//!
//! ```text
//! atupa profile  --tx <HASH> [--rpc <URL>] [--out trace.svg] [--demo]
//! atupa capture  --tx <HASH> [--rpc <URL>] [--output summary|json|metric] [--file report.json]
//!                [--profile] [--etherscan-key <KEY>] [--studio]
//! atupa audit    --tx <HASH> [--rpc <URL>] [--protocol aave|lido]
//! atupa diff     --base <HASH> --target <HASH> [--rpc <URL>]
//! atupa studio   [--port 5173] [--dir <PATH>]
//! atupa init     [--force]
//! ```

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

use atupa_core::config::AtupaConfig;

mod banner;
mod cli;
mod commands;
mod init;
mod studio;
mod thresholds;
mod utils;

use banner::print_banner;
use cli::{Cli, Commands};
use commands::{cmd_audit, cmd_capture, cmd_diff, cmd_profile, cmd_studio};

#[tokio::main]
async fn main() -> Result<()> {
    let args = std::env::args_os();

    env_logger::builder().filter_level(log::LevelFilter::Warn).parse_default_env().init();

    let cli = Cli::parse_from(args);
    let mut config = AtupaConfig::load();

    if let Some(r) = cli.rpc {
        config.rpc_url = r;
    }

    print_banner();

    match cli.command {
        Commands::Profile { tx, demo, out, etherscan_key, vm } => {
            if let Some(key) = etherscan_key {
                config.etherscan_key = Some(key);
            }
            cmd_profile(&config, &tx, demo, out, vm).await?;
        }
        Commands::Capture { tx, output, file, profile, etherscan_key, studio, vm } => {
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
                config.studio_dir = Some(PathBuf::from(d));
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
