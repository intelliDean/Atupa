//! Handler for `atupa studio` command.

use anyhow::{Context, Result};
use colored::*;
use std::time::{Duration, Instant};

use atupa_core::config::AtupaConfig;
use crate::studio::StudioServer;

/// Executes the `studio` command, launching the local embedded web UI.
pub async fn cmd_studio(
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
    let server = StudioServer::new(report_content);
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

    // Wait for the port to become active
    let addr = format!("127.0.0.1:{port}");
    let deadline = Instant::now() + Duration::from_secs(5);
    while std::net::TcpStream::connect(&addr).is_err() {
        if Instant::now() > deadline {
            anyhow::bail!("Studio server failed to start on port {port} within 5s.");
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
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

    // Keep the main task active while the server runs
    let _ = server_handle.await;
    Ok(())
}
