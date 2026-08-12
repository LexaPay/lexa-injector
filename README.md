# LaxaFlow — Real-Time Payroll & Revenue Splitter

> An open-source, automated payroll and revenue-sharing platform for teams and DAOs, built natively on [Soroban](https://soroban.stellar.org) (Stellar's smart contract platform).

## Overview

LaxaFlow enables organisations to:

- **Stream salaries** to team members in real-time (per-second accrual).
- **Split revenue** across configurable pools (e.g. 60% Development, 40% Marketing).
- **Distribute funds** with a single on-chain transaction — no manual payroll runs.
- **Track earnings** with full transparency on the Stellar ledger.

All powered by Stellar's low-cost, high-throughput network.

---

## Architecture

```
┌────────────────────────────────────────────┐
│           LaxaFlow Smart Contract          │
│                                            │
│  ┌──────────┐  ┌────────────┐  ┌────────┐ │
│  │ Treasury │  │  Streaming │  │ Revenue│ │
│  │ Deposit  │  │  Payroll   │  │ Splits │ │
│  │ & Query  │  │  Engine    │  │ Matrix │ │
│  └──────────┘  └────────────┘  └────────┘ │
│                                            │
│  Token: Any Stellar Asset (XLM, USDC, …)  │
└────────────────────────────────────────────┘
```

### Key Components

| Component | Description |
|---|---|
| **Treasury** | Accepts token deposits, tracks contract balance |
| **Streaming Payroll** | Per-second salary accrual with on-demand claiming |
| **Revenue Splits** | Basis-point pools (10 000 = 100%) with equal per-member splits |
| **Admin Controls** | Add/remove members, configure pools, trigger distributions |

---

## Contract API

### Initialisation

| Function | Parameters | Description |
|---|---|---|
| `initialize` | `admin: Address, token: Address` | Set up the treasury admin and payment token |

### Treasury

| Function | Parameters | Description |
|---|---|---|
| `deposit` | `caller: Address, amount: i128` | Deposit tokens into the contract |
| `get_balance` | — | Query the contract's token balance |

### Streaming Payroll

| Function | Parameters | Description |
|---|---|---|
| `add_member` | `admin, member: Address, rate_per_second: i128` | Register a team member with a streaming salary |
| `remove_member` | `admin, member: Address` | Deactivate a member (pays out accrued balance) |
| `claim` | `member: Address` | Claim all accrued earnings → returns amount |
| `get_accrued` | `member: Address` | View unclaimed earnings (read-only) |
| `get_member` | `member: Address` | Get full stream configuration |

### Revenue Splits

| Function | Parameters | Description |
|---|---|---|
| `set_pools` | `admin, pools: Vec<PoolConfig>` | Configure split matrix (bps must sum to 10 000) |
| `distribute` | `admin, total_amount: i128` | One-click distribution across all pools |

### Data Types

```rust
pub struct StreamConfig {
    pub rate: i128,           // tokens per second
    pub start: u64,           // stream start timestamp
    pub last_claim: u64,      // last claim timestamp
    pub total_claimed: i128,  // cumulative claimed
}

pub struct PoolConfig {
    pub name: Symbol,         // pool identifier
    pub bps: u32,             // basis points (6000 = 60%)
    pub members: Vec<Address>,// pool members
}
```

---

## Project Layout

```
laxa-pay/
├── Cargo.toml                          # Workspace root
├── .gitignore
├── README.md
└── contracts/
    └── laxaflow/
        ├── Cargo.toml                  # Contract crate
        └── src/
            ├── lib.rs                  # Contract logic
            └── test.rs                 # Unit tests
```

---

## Quick Start

### Prerequisites

- [Rust](https://rustup.rs/) (stable)
- `wasm32-unknown-unknown` target: `rustup target add wasm32-unknown-unknown`
- [Stellar CLI](https://soroban.stellar.org/docs/getting-started/setup) (optional, for deployment)

### Build

```bash
cargo build --target wasm32-unknown-unknown --release
```

The compiled WASM binary will be at:
```
target/wasm32-unknown-unknown/release/laxaflow_contract.wasm
```

### Test

```bash
cargo test
```

### Deploy (Stellar Testnet)

```bash
stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/laxaflow_contract.wasm \
  --network testnet \
  --source <YOUR_SECRET_KEY>
```

---

## Example Usage

### 1. Stream a salary

```
# Admin initializes and funds the contract
initialize(admin, USDC_token)
deposit(admin, 1_000_000)         # 1M USDC

# Add team member at 0.01 USDC/second (~$864/day)
add_member(admin, alice, 10_000)  # 10 000 stroops/s

# After 1 hour, Alice claims
claim(alice)                       # → 36 000 000 stroops (36 USDC)
```

### 2. Split revenue across teams

```
# Configure: 60% dev, 40% marketing
set_pools(admin, [
  { name: "dev",       bps: 6000, members: [dev1, dev2] },
  { name: "marketing", bps: 4000, members: [mkt1] },
])

# Distribute 10 000 tokens
distribute(admin, 10_000)
# → dev1:  3 000
# → dev2:  3 000
# → mkt1:  4 000
```

---

## Security Considerations

- **Admin-only mutations**: `add_member`, `remove_member`, `set_pools`, and `distribute` all require admin authentication.
- **Member auth for claims**: Only the member themselves can call `claim`.
- **Overflow protection**: Release profile enables `overflow-checks = true`.
- **No reentrancy risk**: Soroban's execution model prevents reentrancy by design.
- **Graceful removal**: `remove_member` pays out accrued balance before zeroing the rate.

---

## License

MIT — see [LICENSE](LICENSE) for details.

---

## Contributing

Contributions welcome! Please open an issue or submit a PR.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'feat: add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request
