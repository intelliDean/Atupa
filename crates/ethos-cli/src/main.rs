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
        #[arg(short, long)]
        tx: String,

        /// RPC endpoint URL
        #[arg(short, long, default_value = "http://localhost:8545")]
        rpc: String,
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

use ethos_rpc::EthClient;
use ethos_parser::{Parser as EthosParser, aggregator::Aggregator};
use ethos_output::SvgGenerator;
use ethos_core::TraceStep;
use std::fs;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let cli = Cli::parse();

    println!("{}", "Ethos: High-Fidelity Ethereum Tracing Suite".bold().cyan());

    match &cli.command {
        Commands::Profile { tx, rpc } => {
            println!("Profiling transaction: {} on {}", tx.green(), rpc.yellow());
            
            let steps = if tx == "demo" {
                println!("{} Generating offline demo trace...", "[1/2]".bold().dimmed());
                vec![
                    TraceStep { pc: 0, op: "PUSH1".into(), gas: 1000, gas_cost: 3, depth: 1, stack: None, memory: None },
                    TraceStep { pc: 1, op: "CALL".into(), gas: 997, gas_cost: 0, depth: 1, stack: None, memory: None },
                    TraceStep { pc: 0, op: "SLOAD".into(), gas: 500, gas_cost: 2100, depth: 2, stack: None, memory: None },
                    TraceStep { pc: 1, op: "SSTORE".into(), gas: 480, gas_cost: 20000, depth: 2, stack: None, memory: None },
                    TraceStep { pc: 2, op: "RETURN".into(), gas: 400, gas_cost: 0, depth: 2, stack: None, memory: None },
                    TraceStep { pc: 2, op: "STOP".into(), gas: 300, gas_cost: 0, depth: 1, stack: None, memory: None },
                ]
            } else {
                // 1. Fetch
                let client = EthClient::new(rpc.to_string());
                println!("{} Fetching trace from node...", "[1/4]".bold().dimmed());
                
                let trace_res = match client.get_transaction_trace(tx).await {
                    Ok(res) => res,
                    Err(e) => {
                        eprintln!("\n{} Could not fetch trace from node.", "Error:".bold().red());
                        eprintln!("{} Is your node running at {}?", "Hint:".cyan(), rpc.yellow().bold());
                        eprintln!("{} {}", "Details:".dimmed(), e);
                        std::process::exit(1);
                    }
                };
                
                // 2. Parse
                println!("{} Normalizing {} structLogs...", "[2/4]".bold().dimmed(), trace_res.struct_logs.len());
                EthosParser::normalize(trace_res.struct_logs)
            };
            
            // 3. Aggregate
            let aggregate_step_msg = if tx == "demo" { "[2/2]" } else { "[3/4]" };
            println!("{} Aggregating execution metrics...", aggregate_step_msg.bold().dimmed());
            let stacks = Aggregator::build_collapsed_stacks(&steps);
            
            // 4. Output
            let output_step_msg = if tx == "demo" { "[Done]" } else { "[4/4]" };
            println!("{} Generating visual flamegraph...", output_step_msg.bold().dimmed());
            let svg = SvgGenerator::generate_flamegraph(&stacks)?;
            
            let out_file = format!("profile_{}.svg", tx);
            fs::write(&out_file, svg)?;
            
            println!("{} Profile saved to {}", "Success!".bold().green(), out_file.bold());
        }
        Commands::Diff { base, target } => {
            println!("Comparing traces: {} and {}", base.green(), target.yellow());
        }
    }

    Ok(())
}
