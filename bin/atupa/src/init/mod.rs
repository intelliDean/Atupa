//! # `atupa init`
//!
//! Scaffolds all files required to integrate Atupa Gas Regression checking
//! into the current repository. Detects the project type (Foundry / Hardhat /
//! Stylus-only) and generates tailored configuration.

use anyhow::{Context, Result};
use colored::*;
use std::fs;
use std::path::Path;

pub mod detector;
pub mod templates;

pub use detector::{detect_project, detect_protocol, ProjectKind};
pub use templates::*;

/// Command-line arguments for the `init` subcommand.
pub struct InitArgs {
    /// Overwrite existing scaffolding files if true.
    pub force: bool,
}

/// Executes repository initialization and scaffolding.
pub fn execute_init(args: InitArgs) -> Result<()> {
    println!();
    println!("{}", "🏮  Atupa — Initializing project integration".bold());
    println!("{}", "─".repeat(55).dimmed());
    println!();

    // ── Detect Project ────────────────────────────────────────────────────────
    let kind = detect_project();
    println!(
        "  {} {}",
        "🔍 Detected project type:".bold(),
        kind.label().cyan().bold()
    );

    // Attempt to detect protocol
    let protocol = detect_protocol();
    if let Some(p) = &protocol {
        println!(
            "  {} {}",
            "💉 Detected protocol adapter:".bold(),
            p.cyan().bold()
        );
    }
    println!();

    let mut created: Vec<String> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();

    // ── 1. atupa.toml ─────────────────────────────────────────────────────────
    let toml_content = match kind {
        ProjectKind::Foundry => ATUPA_TOML_FOUNDRY,
        ProjectKind::Hardhat => ATUPA_TOML_HARDHAT,
        ProjectKind::StylusOnly => ATUPA_TOML_STYLUS,
        ProjectKind::Unknown => ATUPA_TOML_FOUNDRY,
    };

    scaffold_file(
        "atupa.toml",
        toml_content,
        args.force,
        &mut created,
        &mut skipped,
    )?;

    // ── 2. .github/workflows/atupa.yml ───────────────────────────────────────
    let workflow_dir = Path::new(".github/workflows");
    fs::create_dir_all(workflow_dir).context("Failed to create .github/workflows directory")?;

    scaffold_file(
        ".github/workflows/atupa.yml",
        WORKFLOW_YAML,
        args.force,
        &mut created,
        &mut skipped,
    )?;

    // ── 3. Profile Script (project-specific) ─────────────────────────────────
    match kind {
        ProjectKind::Foundry | ProjectKind::StylusOnly => {
            fs::create_dir_all("script").context("Failed to create script/ directory")?;
            scaffold_file(
                "script/AtupaProfile.s.sol",
                FORGE_PROFILE_SCRIPT,
                args.force,
                &mut created,
                &mut skipped,
            )?;
        }
        ProjectKind::Hardhat => {
            fs::create_dir_all("scripts").context("Failed to create scripts/ directory")?;
            scaffold_file(
                "scripts/AtupaProfile.js",
                HARDHAT_PROFILE_SCRIPT,
                args.force,
                &mut created,
                &mut skipped,
            )?;
        }
        ProjectKind::Unknown => {
            fs::create_dir_all("script").ok();
            scaffold_file(
                "script/AtupaProfile.s.sol",
                FORGE_PROFILE_SCRIPT,
                args.force,
                &mut created,
                &mut skipped,
            )?;
        }
    }

    // ── Print Summary ─────────────────────────────────────────────────────────
    println!();
    for path in &created {
        println!("  {}  {}", "✅ Created".green().bold(), path.cyan());
    }
    for path in &skipped {
        println!(
            "  {}  {} {}",
            "⚠️  Skipped".yellow(),
            path.dimmed(),
            "(already exists — use --force to overwrite)".dimmed()
        );
    }

    println!();
    println!("{}", "─".repeat(55).dimmed());
    println!("{}", "  🚀  Next Steps".bold().underline());
    println!("{}", "─".repeat(55).dimmed());
    println!();

    match kind {
        ProjectKind::Foundry | ProjectKind::StylusOnly | ProjectKind::Unknown => {
            println!(
                "  {}  Edit {} to add your contract call.",
                "1.".bold(),
                "script/AtupaProfile.s.sol".cyan()
            );
        }
        ProjectKind::Hardhat => {
            println!(
                "  {}  Edit {} to add your contract call.",
                "1.".bold(),
                "scripts/AtupaProfile.js".cyan()
            );
        }
    }

    println!(
        "  {}  Add {} to your GitHub Repository Secrets.",
        "2.".bold(),
        "ATUPA_RPC_URL".cyan()
    );
    println!(
        "  {}  Open a Pull Request — Atupa will automatically comment with a gas diff.",
        "3.".bold()
    );
    println!();
    println!(
        "  {}  {}",
        "Docs:".dimmed(),
        "https://github.com/One-Block-Org/Atupa".dimmed()
    );
    println!();

    Ok(())
}

/// Helper function to write a scaffolded file or record it as skipped if already present.
pub fn scaffold_file(
    path: &str,
    content: &str,
    force: bool,
    created: &mut Vec<String>,
    skipped: &mut Vec<String>,
) -> Result<()> {
    if Path::new(path).exists() && !force {
        skipped.push(path.to_string());
        return Ok(());
    }
    fs::write(path, content).with_context(|| format!("Failed to write {path}"))?;
    created.push(path.to_string());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaffold_file_creates_and_skips() {
        let temp_dir = std::env::temp_dir().join(format!("atupa_test_scaffold_{}", std::process::id()));
        let _ = fs::create_dir_all(&temp_dir);

        let test_file = temp_dir.join("test_file.txt");
        let test_path = test_file.to_str().unwrap();

        let mut created = Vec::new();
        let mut skipped = Vec::new();

        // 1. Initial creation
        scaffold_file(test_path, "hello world", false, &mut created, &mut skipped).unwrap();
        assert_eq!(created.len(), 1);
        assert_eq!(skipped.len(), 0);
        assert_eq!(fs::read_to_string(&test_file).unwrap(), "hello world");

        // 2. Second time without force -> skipped
        scaffold_file(test_path, "new content", false, &mut created, &mut skipped).unwrap();
        assert_eq!(created.len(), 1);
        assert_eq!(skipped.len(), 1);
        assert_eq!(fs::read_to_string(&test_file).unwrap(), "hello world");

        // 3. Third time with force -> overwritten
        scaffold_file(test_path, "new content", true, &mut created, &mut skipped).unwrap();
        assert_eq!(created.len(), 2);
        assert_eq!(skipped.len(), 1);
        assert_eq!(fs::read_to_string(&test_file).unwrap(), "new content");

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
