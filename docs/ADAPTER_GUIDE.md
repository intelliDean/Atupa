# 🛠 Atupa Adapter Guide: Building for a New VM

Atupa is architected to be extensible to new execution environments (e.g. Move VM, Fuel VM, Aptos/Sui). This guide explains how to build a new VM adapter crate.

---

## 1. Anatomy of an Adapter

Every VM adapter should be an independent crate in the `crates/` directory (e.g., `crates/atupa-fuel`). An adapter's primary responsibility is to fetch raw RPC execution data and transform it into the unified `atupa_core::TraceStep` model.

### Key Components:
1. **Error Types (`error.rs`)**: Domain-specific error enum (`FuelError`) with standard `std::error::Error` and `Display` implementations.
2. **Data Models (`types.rs`)**: Serde-compatible models mapping the target chain's JSON-RPC structures.
3. **Trace Parser / Flattener (`parser.rs`)**: Logic that converts logs, diagnostic events, or raw invocation trees into `Vec<TraceStep>`.
4. **Client (`client.rs`)**: Async RPC client communicating with the target chain endpoint.
5. **Facade (`lib.rs`)**: Clean re-export of public client, parser, and error types.

---

## 2. Implementation Steps

### Step A: Define the `VmKind`
Add your new VM to the `VmKind` enum in `crates/atupa-core/src/vm.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VmKind {
    Evm,
    Stylus,
    Starknet,
    Solana,
    Stellar,
    Fuel, // New VM kind
}
```

### Step B: Create the Client and Parser
Implement the RPC fetching logic and map native execution events to `TraceStep`:

```rust
pub struct FuelClient {
    rpc_url: String,
}

impl FuelClient {
    pub fn new(rpc_url: String) -> Self {
        Self { rpc_url }
    }

    pub async fn get_transaction_trace(&self, tx: &str) -> FuelResult<Vec<TraceStep>> {
        // 1. Fetch raw transaction data from RPC
        // 2. Parse opcodes or receipt receipts into TraceStep
        // 3. Set vm_kind = VmKind::Fuel
    }
}
```

### Step C: Handle Call-Stack Depth
Atupa flamegraphs rely on the `depth` field:
- **Flat sequential logs** (like Solana): Implement a state machine tracking call/return markers.
- **Recursive trees** (like Starknet): Recursively flatten the tree while incrementing the depth counter at each level.

### Step D: Unit Normalization
Map native gas or resource units to a meaningful `gas_cost`:
- **Solana**: 1 Compute Unit (CU) = 1 `gas_cost`.
- **Soroban**: HostFn CPU/Memory weight = estimated `gas_cost`.
- **Starknet**: Cairo instruction steps + builtin resource weights = `gas_cost`.

---

## 3. Registering the Adapter

1. Add the new crate to the workspace `Cargo.toml`.
2. Add the dependency to `crates/atupa-sdk` and `bin/atupa`.
3. Add CLI hint handling in `bin/atupa/src/cli.rs` (`VmTarget`) and `crates/atupa-sdk/src/profile.rs` (`VmHint`).
4. Update `atupa-output` with chain-specific color schemes for SVG rendering.
5. Update `studio/src/types/trace.ts` with color tokens for Atupa Studio.

---

## 4. Testing Your Adapter

Add unit tests in your crate using fixture JSON files or mocked responses:
- Verify that total `gas_cost` matches expected weights.
- Verify that call depth increments and decrements correctly.
- Verify error display formatting and failure paths.
