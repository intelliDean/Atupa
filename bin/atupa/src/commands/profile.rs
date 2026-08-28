//! Handler for `atupa profile` command.

use anyhow::{Context, Result};
use colored::*;

use atupa_core::config::AtupaConfig;
use crate::cli::VmTarget;
use crate::utils::{divider, resolve_artifact_path};

/// Executes the `profile` command, generating an SVG flamegraph.
pub async fn cmd_profile(
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

    let vm_hint = vm.map(|v| v.to_sdk_hint());
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
    let div = divider(40);
    eprintln!("{div}");
    eprintln!(
        "  {:<24} {}",
        "SVG saved to:".bold(),
        out_path.green().bold()
    );
    eprintln!("{div}");
    Ok(())
}
