# 🛠 Atupa Adapter Guide: Building for a New VM

Atupa is designed to be easily extended to new execution environments. This guide explains how to build a new VM adapter crate.

---

## 1. Anatomy of an Adapter

Every VM adapter should be a separate crate in the `crates/` directory (e.g., `atupa-fvm`). An adapter's primary responsibility is to fetch raw RPC data and transform it into the unified `atupa_core::TraceStep` model.

### Key Components:
1. **The Client**: An async struct that wraps the target chain's JSON-RPC.
2. **The Parser/Stitcher**: Logic that converts logs, diagnostic events, or raw traces into `Vec<TraceStep>`.
3. **The Normalizer**: Mapping of native units (e.g., Compute Units) to gas-equivalent weights.

---

## 2. Implementation Steps

### Step A: Define the `VmKind`
Add your new VM to the `VmKind` enum in `crates/atupa-core/src/lib.rs`.

```rust
pub enum VmKind {
    Evm,
    Stylus,
    Starknet,
    Solana,
    Stellar,
    MyNewVM, // Add this
}
```

### Step B: Create the Client
Implement the RPC fetching logic. Ensure you handle common error cases like missing traces or invalid hashes.

```rust
pub struct MyVMClient { ... }

impl MyVMClient {
    pub async fn get_trace(&self, hash: &str) -> Result<Vec<TraceStep>, MyVMError> {
        // 1. Fetch raw data
        // 2. Map to TraceSteps
        // 3. Return
    }
}
```

### Step C: Handle Call-Stack Depth
Atupa flamegraphs rely on the `depth` field. 
- If your RPC provides a flat list (like Solana logs), you must implement a state machine to track `invoke` and `return` markers to calculate the current depth.
- If your RPC is recursive (like Starknet), you must flatten the tree while incrementing the depth at each level.

### Step D: Unit Normalization
Decide how to weight your native instructions. For example, in Solana, we use the `Compute Unit` directly as the `gas_cost`. In Starknet, we use the `steps` count.

---

## 3. Registering the Adapter

Once your crate is ready:
1. Add it to the workspace `Cargo.toml`.
2. Update the CLI router in `bin/atupa/src/main.rs`.
3. Add color tokens to `atupa-output` (SVG) and `Atupa Studio` (TypeScript).

### CLI Dispatch Pattern:
In `cmd_capture` and `cmd_diff`, use the RPC URL signature to auto-detect the correct adapter:

```rust
if config.rpc_url.contains("mynewvm") {
    let client = atupa_mynewvm::MyVMClient::new(config.rpc_url.clone());
    // ... logic
}
```

---

## 4. Testing Your Adapter
Create a small integration test in your crate using a mock or a saved JSON sample of a real transaction trace. Ensure that:
- Total `gas_cost` matches the expected value.
- Maximum `depth` is correctly calculated.
- All steps have the correct `vm_kind`.

---
🏮 *Build the future of observability with Atupa.*
