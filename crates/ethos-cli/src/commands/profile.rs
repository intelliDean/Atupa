use ethos_rpc::EthClient;
use ethos_parser::{Parser as EthosParser, aggregator::Aggregator};
use ethos_output::SvgGenerator;
use ethos_core::TraceStep;
use std::fs;
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;
use colored::*;

/// Executes the profile command: fetching, parsing, aggregating and visualizing the trace.
pub async fn execute_profile(tx: &str, rpc: &str, is_demo: bool, out: Option<String>) -> anyhow::Result<()> {
    let spinner = initialize_spinner()?;

    // 1. Fetch
    let steps = if is_demo {
        get_demo_trace(&spinner)
    } else {
        fetch_live_trace(tx, rpc, &spinner).await
    };

    // 2. Aggregate & Output
    render_and_save_trace(steps, tx, is_demo, out, &spinner)?;

    Ok(())
}

fn initialize_spinner() -> anyhow::Result<ProgressBar> {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ")
            .template("{spinner:.cyan} {msg}")?,
    );
    spinner.enable_steady_tick(Duration::from_millis(100));
    Ok(spinner)
}

fn get_demo_trace(spinner: &ProgressBar) -> Vec<TraceStep> {
    spinner.set_message("Generating offline demo trace... [1/2]");
    vec![
        TraceStep { pc: 0, op: "PUSH1".into(), gas: 1000, gas_cost: 3, depth: 1, stack: None, memory: None },
        TraceStep { pc: 1, op: "CALL".into(), gas: 997, gas_cost: 0, depth: 1, stack: None, memory: None },
        TraceStep { pc: 0, op: "SLOAD".into(), gas: 500, gas_cost: 2100, depth: 2, stack: None, memory: None },
        TraceStep { pc: 1, op: "SSTORE".into(), gas: 480, gas_cost: 20000, depth: 2, stack: None, memory: None },
        TraceStep { pc: 2, op: "RETURN".into(), gas: 400, gas_cost: 0, depth: 2, stack: None, memory: None },
        TraceStep { pc: 2, op: "STOP".into(), gas: 300, gas_cost: 0, depth: 1, stack: None, memory: None },
    ]
}

async fn fetch_live_trace(tx: &str, rpc: &str, spinner: &ProgressBar) -> Vec<TraceStep> {
    spinner.set_message("Connecting to EVM Node via JSON-RPC... [1/4]");
    let client = EthClient::new(rpc.to_string());
    
    let trace_res = match tokio::time::timeout(Duration::from_secs(15), client.get_transaction_trace(tx)).await {
        Ok(Ok(res)) => res,
        Ok(Err(e)) => {
            spinner.finish_and_clear();
            eprintln!("\n{} Could not fetch trace from node.", "Error:".bold().red());
            eprintln!("{} Verify your RPC node is running at {}?", "Hint:".cyan(), rpc.yellow().bold());
            eprintln!("{} {}", "Details:".dimmed(), e);
            std::process::exit(1);
        }
        Err(_) => {
            spinner.finish_and_clear();
            eprintln!("\n{} Connection to RPC node timed out after 15 seconds.", "Timeout:".bold().red());
            eprintln!("{} Is the node fully synced and responding at {}?", "Hint:".cyan(), rpc.yellow().bold());
            std::process::exit(1);
        }
    };
    
    spinner.set_message(format!("Normalizing {} structLogs... [2/4]", trace_res.struct_logs.len()));
    EthosParser::normalize(trace_res.struct_logs)
}

fn render_and_save_trace(
    steps: Vec<TraceStep>,
    tx: &str,
    is_demo: bool,
    out: Option<String>,
    spinner: &ProgressBar,
) -> anyhow::Result<()> {
    // 3. Aggregate
    let aggregate_step_msg = if is_demo { "[2/2]" } else { "[3/4]" };
    spinner.set_message(format!("Aggregating execution metrics... {}", aggregate_step_msg));
    let stacks = Aggregator::build_collapsed_stacks(&steps);
    
    // 4. Render
    let output_step_msg = if is_demo { "[Done]" } else { "[4/4]" };
    spinner.set_message(format!("Generating visual flamegraph... {}", output_step_msg));
    let svg = SvgGenerator::generate_flamegraph(&stacks)?;
    
    // 5. Output
    let out_file = out.unwrap_or_else(|| format!("profile_{}.svg", if is_demo { "demo" } else { tx }));
    fs::write(&out_file, svg)?;
    
    spinner.finish_with_message(format!("{} Profile saved to {}", "Success!".bold().green(), out_file.bold()));
    
    Ok(())
}
