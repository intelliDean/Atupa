mod commands;

use clap::{Parser, Subcommand};
use colored::*;
use ethos_core::config::EthosConfig;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Profile a transaction from a JSON-RPC endpoint
    Profile {
        /// The transaction hash
        #[arg(short, long, default_value = "")]
        tx: String,

        /// RPC endpoint URL
        #[arg(short, long, default_value = "http://localhost:8545")]
        rpc: String,

        /// Run a local offline demo trace
        #[arg(long, default_value_t = false)]
        demo: bool,

        /// Optional output path for the SVG
        #[arg(short, long)]
        out: Option<String>,

        /// Etherscan API Key for contract name resolution
        #[arg(long, env = "ETHERSCAN_API_KEY")]
        etherscan_key: Option<String>,
    },
    /// Compare two transaction traces
    Diff {
        /// Base transaction hash
        #[arg(short, long)]
        base: String,

        /// Target transaction hash
        #[arg(short, long)]
        target: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let cli = Cli::parse();
    let config = EthosConfig::load();

    eprintln!(
        "{}",
        "Ethos: High-Fidelity Ethereum Tracing Suite".bold().cyan()
    );

    match cli.command {
        Commands::Profile { tx, rpc, demo, out, etherscan_key } => {
            let effective_rpc = if rpc != "http://localhost:8545" { rpc } else { config.rpc_url };
            let effective_key = etherscan_key.or(config.etherscan_key);

            if !demo && tx.is_empty() {
                eprintln!(
                    "\n{} You must provide a transaction hash (--tx) or run with --demo.",
                    "Error:".bold().red()
                );
                std::process::exit(1);
            }

            let display_tx = if demo { "demo" } else { &tx };
            eprintln!(
                "Profiling transaction: {} on {}",
                display_tx.green(),
                effective_rpc.yellow()
            );

            commands::profile::execute_profile(&tx, &effective_rpc, demo, out, effective_key).await?;
        }
        Commands::Diff { base, target } => {
            eprintln!("Comparing traces: {} and {}", base.green(), target.yellow());
        }
    }

    Ok(())
}
