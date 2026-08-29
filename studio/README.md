# 🏮 Atupa Studio

**Atupa Studio** is the local-first visual execution profiler and interactive analysis dashboard for [Atupa](https://github.com/One-Block-Org/Atupa).

It provides execution flamegraphs, category cost breakdowns, paginated trace inspection, and side-by-side differential analysis across **EVM**, **Arbitrum Stylus (WASM)**, **Solana (SVM)**, **Starknet (Cairo)**, and **Stellar (Soroban)**.

---

## ⚡ Key Features

- **🔆 Interactive Flamegraph**: Pure React + SVG zoomable call tree with real-time opcode/function search and hover cost tooltips.
- **🌐 Chain-Adaptive Metrics**: Automatically detects the active runtime and adjusts badges, labels, and units (Gas, Ink, Compute Units (CU), Cairo Steps, or Soroban Resource Units).
- **🧩 Trace Inspector**: Paginated opcode, instruction, and HostIO explorer with search, address label resolution, and cross-VM / CPI boundary indicators.
- **⚖️ Differential Execution (Diff Mode)**: Side-by-side transaction comparison with category delta bars and percentage regression analysis.
- **🔥 Stylus HostIO Hot Paths**: Aggregated breakdown of Stylus WASM HostIO calls ranked by ink and gas-equivalent consumption.
- **🔒 100% Local & Private**: Runs entirely in your browser with zero telemetry or external network calls.

---

## 🏗️ Architecture & Integration

Atupa Studio is built with **React 19**, **TypeScript**, and **Vite**. 

During `npm run build`, Vite compiles the production SPA directly into `../bin/atupa/dist/`. The Rust CLI (`atupa`) embeds this directory at compile time using `rust-embed`, allowing users to launch the entire UI with a single command without needing Node.js installed:

```bash
# Launched automatically from the Rust CLI
atupa studio --file report.json
```

---

## 🛠️ Local Development

### 1. Install Dependencies
```bash
npm install
```

### 2. Start Vite Dev Server
```bash
npm run dev
```
Open [http://localhost:5173](http://localhost:5173) in your browser. You can click any of the 7 preloaded multi-VM presets on the landing page to load instant demo data.

### 3. Lint & Type Check
```bash
npm run lint
```

### 4. Build for Production
```bash
npm run build
```
*Outputs bundled HTML/CSS/JS assets to `../bin/atupa/dist/` for embedded distribution.*
