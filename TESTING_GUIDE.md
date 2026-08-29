# 🧪 Atupa Testing Guide

This document provides a comprehensive framework for testing the entire Atupa monorepo, covering automated Rust workspace tests, frontend Studio testing, local CLI verification, and CI regression checks.

---

## 1. Automated Workspace Testing (Rust & Frontend)

### Rust Test Suite
The project leverages Cargo's test runner across all 13 crates:

```bash
# 1. Format Check
cargo fmt --all -- --check

# 2. Strict Linting
cargo clippy --workspace --all-targets -- -D warnings

# 3. All Unit & Integration Tests
cargo test --workspace
```

### Studio Frontend Testing & Linting
Verify the embedded React 19 visualizer:

```bash
cd studio

# 1. Lint TypeScript and React Components
npm run lint

# 2. Build Static Bundle for Binary Embedding
npm run build
```

---

## 2. CLI End-to-End Verification

Compile the CLI in release mode:

```bash
cargo build --release -p atupa
alias atupa="./target/release/atupa"
```

### Flow 1: Offline / Demo Flamegraph
Verify SVG generation without network dependency:

```bash
atupa profile --demo --out profile_demo.svg
```
> **Expected Output:** Profile banner, terminal confirmation, and a valid `profile_demo.svg` file created.

### Flow 2: Multi-VM Live Trace Captures
Test trace capture across supported VM runtimes:

```bash
# Arbitrum Nitro / EVM
atupa capture --tx 0x8a923... --rpc https://arb-mainnet.g.alchemy.com/v2/KEY

# Solana Sealevel VM (SVM)
atupa capture --tx 5Z9... --rpc https://api.mainnet-beta.solana.com

# Starknet Cairo VM
atupa capture --tx 0x... --rpc https://starknet-mainnet.public.blastapi.io

# Stellar Soroban WASM VM
atupa capture --tx 0x... --rpc https://soroban-testnet.stellar.org
```

### Flow 3: Protocol Deep Auditing
Verify specialized semantic decoding for DeFi protocols:

```bash
# Aave v3 + GHO Stablecoin Audit
atupa audit --protocol aave --tx 0x...

# Lido stETH Audit
atupa audit --protocol lido --tx 0x...
```

### Flow 4: Differential Profiling & CI Regression
Verify differential execution analysis:

```bash
# Basic comparison
atupa diff --base 0xBASE_HASH --target 0xTARGET_HASH --rpc https://...

# Enforce regression threshold with Markdown report & SVG generation
atupa diff --base 0xBASE_HASH --target 0xTARGET_HASH --threshold 2.0 --markdown --svg
```

### Flow 5: Local Studio Server
Launch the embedded web server and verify browser loading:

```bash
atupa studio --port 5173
```
> **Expected Output:** Local server starts on `http://localhost:5173` and automatically opens the visualizer in the default browser.

---

## 3. Security & Code Safety Rules

- **Zero Clippy Warnings**: All commits must pass `cargo clippy --workspace --all-targets -- -D warnings`.
- **Clean stdout**: CLI diagnostic messages use `eprintln!`; structured JSON or metric payloads use `stdout` so output can be safely piped into files or downstream tools.
