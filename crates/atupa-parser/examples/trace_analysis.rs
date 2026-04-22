use atupa_core::TraceStep;
use atupa_parser::aggregator::Aggregator;

/// An example showing how to use the Atupa Aggregator library to collapse EVM traces.
fn main() {
    // 1. Create mock trace steps (in a real app, these come from atupa-rpc)
    let steps = vec![
        TraceStep {
            op: "PUSH1".into(),
            gas: 100,
            gas_cost: 3,
            depth: 1,
            ..Default::default()
        },
        TraceStep {
            pc: 1,
            op: "CALL".into(),
            gas: 90,
            gas_cost: 2600,
            depth: 1,
            stack: Some(vec![
                "0x0".into(),
                "0x0".into(),
                "0x0".into(),
                "0x0".into(),
                "0x0".into(),
                "0x000000000000000000000000a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".into(), // USDC
                "0xFFFF".into(),
            ]),
            ..Default::default()
        },
        TraceStep {
            op: "SSTORE".into(),
            gas: 50,
            gas_cost: 20000,
            depth: 2,
            ..Default::default()
        },
    ];

    // 2. Initialize logger for the example
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // 3. Collapse the steps into weighted execution stacks
    let stacks = Aggregator::build_collapsed_stacks(&steps);

    // 4. Log the results
    log::info!(
        "Collapsed {} execution steps into {} paths.",
        steps.len(),
        stacks.len()
    );

    for stack in stacks {
        let status = if stack.reverted {
            "[REVERTED]"
        } else {
            "[SUCCESS]"
        };
        log::info!("{} {} (weight: {} gas)", status, stack.stack, stack.weight);

        if let Some(addr) = stack.target_address {
            log::info!("   └─ Target: {}", addr);
        }
    }
}
