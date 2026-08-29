//! CLI argument models and subcommands for the Atupa command-line interface.

use clap::{Parser, Subcommand, ValueEnum};

/// Top-level CLI configuration.
#[derive(Parser, Debug)]
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
pub struct Cli {
    /// Arbitrum / Ethereum / Multi-VM RPC endpoint (or set ATUPA_RPC_URL)
    #[arg(short, long, global = true, value_name = "URL")]
    pub rpc: Option<String>,

    #[command(subcommand)]
    pub command: Commands,
}

/// Available CLI subcommands.
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Generate a visual SVG flamegraph for any EVM/Stylus/Solana/Starknet/Stellar transaction
    Profile {
        /// Transaction hash (0x-prefixed or base58); omit when using --demo
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

    /// Capture a unified execution trace and export JSON/terminal metrics.
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

        /// Output format (summary | json | metric)
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

/// Supported report and output formats.
#[derive(Clone, Copy, ValueEnum, Debug, PartialEq, Eq)]
pub enum OutputFormat {
    /// Human-readable terminal summary (default)
    Summary,
    /// Full step-by-step JSON — suitable for CI assertions and tooling
    Json,
    /// Emit only the numeric unified cost (gas-equiv) — ideal for scripting
    Metric,
}

/// Explicitly selects which VM runtime the profiler should use when
/// auto-detection (based on RPC URL or tx-hash format) is ambiguous.
#[derive(Clone, Copy, ValueEnum, Debug, PartialEq, Eq)]
pub enum VmTarget {
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

impl VmTarget {
    /// Converts CLI target into the SDK's [`atupa::profile::VmHint`].
    pub fn to_sdk_hint(self) -> atupa::profile::VmHint {
        match self {
            Self::Evm => atupa::profile::VmHint::Evm,
            Self::Stylus => atupa::profile::VmHint::Stylus,
            Self::Starknet => atupa::profile::VmHint::Starknet,
            Self::Solana => atupa::profile::VmHint::Solana,
            Self::Stellar => atupa::profile::VmHint::Stellar,
        }
    }
}

/// Target DeFi protocol for specialized deep tracing and invariant checking.
#[derive(Clone, Copy, ValueEnum, Debug, PartialEq, Eq)]
pub enum Protocol {
    /// Aave v3 + GHO stablecoin protocol adapters
    Aave,
    /// Lido stETH execution resilience
    Lido,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vm_target_to_sdk_hint_conversions() {
        assert_eq!(VmTarget::Evm.to_sdk_hint(), atupa::profile::VmHint::Evm);
        assert_eq!(
            VmTarget::Stylus.to_sdk_hint(),
            atupa::profile::VmHint::Stylus
        );
        assert_eq!(
            VmTarget::Starknet.to_sdk_hint(),
            atupa::profile::VmHint::Starknet
        );
        assert_eq!(
            VmTarget::Solana.to_sdk_hint(),
            atupa::profile::VmHint::Solana
        );
        assert_eq!(
            VmTarget::Stellar.to_sdk_hint(),
            atupa::profile::VmHint::Stellar
        );
    }

    #[test]
    fn parses_profile_subcommand() {
        let cli = Cli::try_parse_from(["atupa", "profile", "--demo", "--vm", "stylus"]).unwrap();
        match cli.command {
            Commands::Profile { demo, vm, .. } => {
                assert!(demo);
                assert_eq!(vm, Some(VmTarget::Stylus));
            }
            _ => panic!("Expected Profile command"),
        }
    }

    #[test]
    fn parses_capture_subcommand_with_output_flags() {
        let cli = Cli::try_parse_from([
            "atupa",
            "capture",
            "--tx",
            "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
            "--output",
            "json",
            "--profile",
        ])
        .unwrap();

        match cli.command {
            Commands::Capture {
                tx,
                output,
                profile,
                ..
            } => {
                assert_eq!(
                    tx,
                    "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
                );
                assert_eq!(output, OutputFormat::Json);
                assert!(profile);
            }
            _ => panic!("Expected Capture command"),
        }
    }

    #[test]
    fn parses_diff_subcommand() {
        let cli = Cli::try_parse_from([
            "atupa",
            "diff",
            "--base",
            "0xaaaa",
            "--target",
            "0xbbbb",
            "--threshold",
            "5.5",
            "--markdown",
        ])
        .unwrap();

        match cli.command {
            Commands::Diff {
                base,
                target,
                threshold,
                markdown,
                ..
            } => {
                assert_eq!(base, "0xaaaa");
                assert_eq!(target, "0xbbbb");
                assert_eq!(threshold, Some(5.5));
                assert!(markdown);
            }
            _ => panic!("Expected Diff command"),
        }
    }
}
