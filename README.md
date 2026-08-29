<p align="center">
  <img src="assets/logo.png" width="350" alt="Atupa Logo">
</p>

<h1 align="center">🏮 Atupa</h1>

<p align="center">
  <strong>Universal Multi-VM Execution Profiler &amp; Visual Analysis Suite</strong>
</p>

<p align="center">
  <a href="https://github.com/One-Block-Org/Atupa/actions"><img src="https://github.com/One-Block-Org/Atupa/actions/workflows/rust.yml/badge.svg" alt="CI Status"></a>
  <a href="https://crates.io/crates/atupa-core"><img src="https://img.shields.io/crates/v/atupa-core.svg" alt="Crates.io"></a>
  <a href="https://docs.rs/atupa-core"><img src="https://docs.rs/atupa-core/badge.svg" alt="Documentation"></a>
  <a href="https://opensource.org/licenses/MIT"><img src="https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg" alt="License"></a>
</p>

---

**Atupa** is a professional-grade **Universal Multi-VM Execution Profiler**. It provides a unified observability and regression analysis layer across diverse blockchain Virtual Machines — including **EVM**, **Arbitrum Stylus (WASM)**, **Starknet (Cairo)**, **Solana (SVM)**, and **Stellar (Soroban)** — turning raw execution traces into actionable visual insights and CI-ready differential reports.

## ✨ Key Features

- **🌐 Universal Multi-VM Profiling**: Unified tracing for EVM, Arbitrum Stylus (WASM), Starknet (Cairo), Solana (SVM), and Stellar (Soroban).
- **🔥 Dual-VM Stitching**: Seamlessly reconstructs execution timelines across VM boundaries (e.g. EVM calling Stylus WASM with HostIO breakdown).
- **📊 Protocol-Aware Gas Analysis**: Specialized cost mapping for non-EVM units, including Solana Compute Units (CU), Soroban HostFn weights, and Cairo execution steps.
- **🏮 Atupa Studio**: An embedded local-first web visualizer (`atupa studio`) — drop a `report.json` to instantly render cross-chain metric cards and interactive flamegraphs.
- **🔍 Smart Contract Resolution**: Automatically resolves addresses to verified contract names via Etherscan, Starkscan, and explorer resolvers.
- **🚀 Automated CI/CD Regression Gates**: Built-in zero-config gas regression gating for GitHub Actions with sticky PR commenting and SVG artifact generation.
- **🔬 Protocol-Specific Deep Auditing**: Built-in deep tracers for **Aave v3 / GHO** and **Lido stETH**.
- **🛠 Modular Architecture**: 13 pure Rust crates with zero-cost abstractions, strict type safety, and clean separation of concerns.

## 🚀 Quick Start

### Installation

```bash
cargo install atupa
```

### 🏮 One-Click Initialization

Bootstrap your project with Atupa profiling and automated CI regression in one command:

```bash
# Detects Foundry, Hardhat, or Stylus and generates atupa.toml + GitHub Action + profile script
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

# Explicitly specify VM runtime if ambiguous
atupa capture --tx 0x... --vm stylus --rpc https://arb-sepolia.g.alchemy.com/v2/KEY

# Export report as JSON and generate an SVG flamegraph simultaneously
atupa capture --tx 0x... --output json --file report.json --profile
```

### Comparing Transactions (Differential Profiling)

```bash
# Compare execution costs of two transactions
atupa diff --base 0xBASE_TX --target 0xTARGET_TX --rpc https://...

# Enforce a CI regression threshold (fail if gas increases by > 2%)
atupa diff --base 0xBASE_TX --target 0xTARGET_TX --threshold 2.0 --markdown --svg

# Run protocol deep diff (Aave v3 or Lido stETH)
atupa diff --base 0xBASE_TX --target 0xTARGET_TX --protocol aave
```

### Generating an Interactive SVG Flamegraph

```bash
# Offline demo trace (no RPC required)
atupa profile --demo --out profile_demo.svg

# Profile a live on-chain transaction
atupa profile --tx 0x... --rpc https://arb-mainnet.g.alchemy.com/v2/KEY
```

---

## 🏮 Atupa Studio

Atupa Studio is an embedded local web visualizer for interactive trace inspection and flamegraph exploration.

```bash
# Launch Studio and auto-open in browser
atupa studio

# Or capture a trace and launch Studio with the report automatically loaded
atupa capture --tx 0x... --rpc https://... --studio
```

---

## 🛡 Automated Gas Regression (GitHub Action)

Integrate Atupa into your repository's CI pipeline with [`One-Block-Org/Atupa`](action.yml):

```yaml
- name: Run Atupa Gas Regression Check
  uses: One-Block-Org/Atupa@main
  with:
    base_tx: ${{ steps.baseline.outputs.tx_hash }}
    target_tx: ${{ steps.target.outputs.tx_hash }}
    rpc_url: ${{ secrets.ATUPA_RPC_URL }}
    config: 'atupa.toml'
    post_comment: 'true'
    upload_svg: 'true'
    upload_json: 'true'
```

---

## 📦 Monorepo Architecture

The workspace is organized into modular crates:

| Crate / Directory | Description |
|---|---|
| [`bin/atupa`](bin/atupa) | The primary command-line interface (`profile`, `capture`, `audit`, `diff`, `studio`, `init`). |
| [`studio/`](studio) | Atupa Studio — Vite + React 19 + TypeScript web visualizer. |
| [`crates/atupa-sdk`](crates/atupa-sdk) | High-level SDK facade for programmatic profiling and multi-VM routing. |
| [`crates/atupa-core`](crates/atupa-core) | Core data models, `TraceStep`, `GasCategory`, `VmKind`, and configuration types. |
| [`crates/atupa-adapters`](crates/atupa-adapters) | Common adapter registry and standard protocol traits (Uniswap v4, ERC-20). |
| [`crates/atupa-parser`](crates/atupa-parser) | Selector decoders, trace normalizers, and stack aggregators. |
| [`crates/atupa-output`](crates/atupa-output) | Standalone SVG flamegraph and visual diff flamegraph rendering engines. |
| [`crates/atupa-nitro`](crates/atupa-nitro) | Arbitrum Nitro dual-VM clock stitcher (EVM + Stylus WASM HostIOs). |
| [`crates/atupa-starknet`](crates/atupa-starknet) | Starknet Cairo execution flattener and builtin weight calculators. |
| [`crates/atupa-solana`](crates/atupa-solana) | Solana Sealevel VM (SVM) log-stitching state machine. |
| [`crates/atupa-stellar`](crates/atupa-stellar) | Stellar Soroban diagnostic event tracer and HostFn cost estimator. |
| [`crates/atupa-rpc`](crates/atupa-rpc) | Async multi-chain RPC client, raw trace types, and Etherscan resolver. |
| [`crates/atupa-aave`](crates/atupa-aave) | Specialized deep tracer for Aave v3 and GHO stablecoin operations. |
| [`crates/atupa-lido`](crates/atupa-lido) | Specialized deep tracer for Lido stETH staking operations. |

---

## 🤝 Contributing

We welcome contributions! Please see our [Contributing Guidelines](CONTRIBUTING.md) and [Testing Guide](TESTING_GUIDE.md) for details.

## 📖 Further Reading

- [**The Atupa Vision**](docs/VISION.md) — Why universal multi-VM execution observability matters.
- [**System Architecture**](ARCHITECTURE.md) — Deep dive into normalization, dual-VM clock stitching, and pipeline design.
- [**Adapter Guide**](docs/ADAPTER_GUIDE.md) — Step-by-step guide to adding support for new Virtual Machines.

## 📄 License

Atupa is dual-licensed under the [MIT License](LICENSE-MIT) and the [Apache License, Version 2.0](LICENSE-APACHE).
