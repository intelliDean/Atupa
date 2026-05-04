<p align="center">
  <img src="assets/logo.png" width="350" alt="Atupa Logo">
</p>

<h1 align="center">Atupa</h1>

<p align="center">
  <strong>High-Fidelity Ethereum Tracing, Profiling &amp; Visual Analysis Suite</strong>
</p>

<p align="center">
  <a href="https://github.com/One-Block-Org/Atupa/actions"><img src="https://github.com/One-Block-Org/Atupa/actions/workflows/rust.yml/badge.svg" alt="CI Status"></a>
  <a href="https://crates.io/crates/atupa-core"><img src="https://img.shields.io/crates/v/atupa-core.svg" alt="Crates.io"></a>
  <a href="https://docs.rs/atupa-core"><img src="https://docs.rs/atupa-core/badge.svg" alt="Documentation"></a>
  <a href="https://opensource.org/licenses/MIT"><img src="https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg" alt="License"></a>
</p>

---

**Atupa** is a professional-grade **Universal Multi-VM Execution Profiler**. It provides a unified observability layer for the modular execution landscape, including EVM, Arbitrum Stylus (WASM), Starknet (Cairo), Solana (SVM), and Stellar (Soroban), turning raw execution logs into actionable visual insights.

## ✨ Key Features

- **🌐 Universal Multi-VM Profiling**: Unified tracing for EVM, Arbitrum Stylus (WASM), Starknet (Cairo), Solana (SVM), and Stellar (Soroban).
- **🔥 Dual-VM Stitching**: Seamlessly reconstructs execution timelines across VM boundaries (e.g., EVM calling Stylus WASM).
- **📊 Protocol-Aware Gas Analysis**: Specialized cost mapping for non-EVM units, including Solana Compute Units (CU) and Soroban HostFn weights.
- **🏮 Atupa Studio**: A local-first web visualizer — drop a `report.json` to instantly render cross-chain metric cards and interactive flamegraphs.
- **🚨 Crisp Revert Identification**: Instantly identifies failing sub-calls or program errors with high-contrast highlights.
- **🔍 Smart Contract Resolution**: Automatically resolves addresses to verified contract names via Etherscan, Starkscan, and Solana Explorers.
- **🚀 Automated CI/CD Pipeline**: Built-in zero-config gas regression gating for GitHub Actions across all supported chains.
- **💉 Protocol-Specific Deep Auditing**: Built-in deep traces for **Lido stETH**, **Aave v3**, and upcoming Solana DeFi primitives.
- **🛠 Modular Library Architecture**: Pure Rust workspace with specialized crates for each VM adapter and execution environment.

## 🚀 Quick Start

### Installation

```bash
cargo install atupa
```

### 🏮 One-Click Initialization

Bootstrap your project with Atupa profiling and automated CI regression in one command.

```bash
# Detects Foundry/Hardhat and sets up atupa.toml + GitHub Action
atupa init
```

### Capturing a Unified Trace

```bash
# Capture an Arbitrum Stylus transaction (summary to terminal)
atupa capture --tx 0x... --rpc https://arb-mainnet.g.alchemy.com/v2/KEY

# Capture a Solana transaction (SVM Compute Unit breakdown)
atupa capture --tx 5Z9... --rpc https://api.mainnet-beta.solana.com

# Capture a Starknet transaction (Cairo execution steps)
atupa capture --tx 0x... --rpc https://starknet-mainnet.public.blastapi.io

# Capture a Stellar transaction (Soroban diagnostic events)
atupa capture --tx 0x... --rpc https://soroban-testnet.stellar.org

# Export as JSON for Atupa Studio
atupa capture --tx 0x... --rpc https://... --output json --file report.json

# Compare execution cost of two transactions (cross-chain diffing)
atupa diff --base 0x... --target 0x...
```

## 🛡 Automated Gas Regression (CI)

Atupa is designed to sit inside your CI/CD pipeline. Use `atupa init` to generate a `.github/workflows/atupa.yml` file that:
1. Runs your profile scripts on the base branch (baseline).
2. Runs your profile scripts on the pull request branch (target).
3. Compares results and fails the CI if gas regressions exceed your `atupa.toml` thresholds.

```bash
atupa diff --base 0xBASE_TX --target 0xPR_TX --protocol lido
```

### Run the Demo
```bash
atupa profile --demo
```

## 🏮 Atupa Studio

Atupa Studio is a local-first web visualizer for your execution traces.

```bash
# Start the Studio dev server
cd studio && npm install && npm run dev
```

Then open **http://localhost:5173**, generate a trace with `--output json --file report.json`, and drop the file into the Studio. The dashboard instantly renders:

- **Execution Metrics** — EVM Gas, Stylus Ink, HostIO call counts, VM boundary crossings
- **HostIO Hot Paths** — Ranked table with inline distribution bars
- **Trace Inspector** — Paginated, filterable, searchable step-by-step execution viewer

## 📦 Project Structure

Atupa is built as a highly modular monorepo:

| Crate / Directory | Description |
|---|---|
| [`bin/atupa`](bin/atupa) | The primary command-line interface. |
| [`studio/`](studio) | Atupa Studio — Vite + React web visualizer. |
| [`crates/atupa-sdk`](crates/atupa-sdk) | Public-facing SDK for programmatic tracing. |
| [`crates/atupa-core`](crates/atupa-core) | Shared types and core configuration logic. |
| [`crates/atupa-parser`](crates/atupa-parser) | Aggregation engine that collapses EVM traces. |
| [`crates/atupa-nitro`](crates/atupa-nitro) | Arbitrum Nitro dual-VM stitcher (EVM + Stylus). |
| [`crates/atupa-starknet`](crates/atupa-starknet) | Starknet (Cairo) VM adapter. |
| [`crates/atupa-solana`](crates/atupa-solana) | Solana (SVM) log-stitching profiler. |
| [`crates/atupa-stellar`](crates/atupa-stellar) | Stellar (Soroban) diagnostic event parser. |
| [`crates/atupa-rpc`](crates/atupa-rpc) | Async multi-chain RPC client & resolver. |
| [`crates/atupa-lido`](crates/atupa-lido) | Specialized adapter for Lido stETH. |
| [`crates/atupa-aave`](crates/atupa-aave) | Specialized adapter for Aave v3 & GHO. |

## 🤝 Contributing

We welcome contributions! Please see our [Contributing Guidelines](CONTRIBUTING.md) for more details.

## 📖 Documentation

For a deep dive into Atupa's internals and philosophy:
- [**The Atupa Vision**](docs/VISION.md) — Why we are building a universal profiler.
- [**System Architecture**](ARCHITECTURE.md) — How the engine, adapters, and Studio interact.
- [**Adapter Guide**](docs/ADAPTER_GUIDE.md) — A step-by-step guide to adding support for new VMs.

## 📄 License

Atupa is dual-licensed under the [MIT License](LICENSE-MIT) and the [Apache License, Version 2.0](LICENSE-APACHE).
You may use this software under either license, at your option.
