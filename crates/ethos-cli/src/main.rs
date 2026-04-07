mod commands;

use clap::{Parser, Subcommand};
use colored::*;

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

    eprintln!(
        "{}",
        "Ethos: High-Fidelity Ethereum Tracing Suite".bold().cyan()
    );

    match cli.command {
        Commands::Profile { tx, rpc, demo, out } => {
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
                rpc.yellow()
            );

            commands::profile::execute_profile(&tx, &rpc, demo, out).await?;
        }
        Commands::Diff { base, target } => {
            eprintln!("Comparing traces: {} and {}", base.green(), target.yellow());
        }
    }

    Ok(())
}
