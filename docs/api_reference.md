# LaxaFlow Smart Contract API Reference

Comprehensive technical reference for the **LaxaFlow** Soroban smart contract. This guide documents all entry points, data types, authorization requirements, emitted events, expected error states, and integration code snippets (Soroban CLI & TypeScript SDK) to assist frontend developers, backend services, and integrators.

---

## Table of Contents

1. [Overview & Access Control](#overview--access-control)
2. [Data Types & Storage Keys](#data-types--storage-keys)
3. [Contract Endpoints](#contract-endpoints)
   - [Initialisation & Administration](#1-initialisation--administration)
     - [`initialize`](#initialize)
     - [`get_token`](#get_token)
     - [`check_admin`](#check_admin)
     - [`change_admin`](#change_admin)
     - [`upgrade`](#upgrade)
   - [Treasury Management](#2-treasury-management)
     - [`deposit`](#deposit)
     - [`get_balance`](#get_balance)
   - [Streaming Payroll Engine](#3-streaming-payroll-engine)
     - [`add_member`](#add_member)
     - [`remove_member`](#remove_member)
     - [`claim`](#claim)
     - [`get_accrued`](#get_accrued)
     - [`get_member`](#get_member)
     - [`pause_stream`](#pause_stream)
     - [`resume_stream`](#resume_stream)
     - [`update_stream_rate`](#update_stream_rate)
   - [Revenue-Split Matrix](#4-revenue-split-matrix)
     - [`set_pools`](#set_pools)
     - [`distribute`](#distribute)
   - [Security & Circuit Breaker](#5-security--circuit-breaker)
     - [`set_paused`](#set_paused)
     - [`is_paused`](#is_paused)
4. [Contract Events](#contract-events)
5. [Error Handling & Panic States](#error-handling--panic-states)
6. [Frontend & SDK Integration Guide](#frontend--sdk-integration-guide)

---

## Overview & Access Control

LaxaFlow operates with three tiers of access:

| Role | Permissions | Auth Required |
|---|---|---|
| **Admin** | Initialise contract, manage members, update rates, pause individual streams, configure split pools, trigger revenue distributions, trigger emergency global pause, change admin, and upgrade contract. | `admin.require_auth()` |
| **Member** | Claim accrued real-time streaming earnings. | `member.require_auth()` |
| **Public / Caller** | Deposit funds into treasury, query balances, view accrued streams, read pool configs, check pause & admin status. | `caller.require_auth()` for transfers, none for queries |

---

## Data Types & Storage Keys

### Data Keys (`DataKey`)
Soroban persistent storage keys used by LaxaFlow:

```rust
pub enum DataKey {
    Admin,           // Address of contract admin
    Token,           // Address of payment token (SAC or Custom Token)
    Stream(Address), // StreamConfig for a specific member
    Pool(Symbol),    // PoolConfig for a specific revenue pool
    PoolList,        // Vec<Symbol> of all configured pool names
    Paused,          // bool - global emergency circuit breaker status
}
```

### Structs

#### `StreamConfig`
Represents an individual employee or contractor salary stream:

```rust
pub struct StreamConfig {
    pub rate: i128,          // Tokens accrued per second (smallest unit, e.g. stroops)
    pub start: u64,          // Ledger timestamp when the stream was created
    pub last_claim: u64,     // Ledger timestamp of the most recent claim/rate update
    pub total_claimed: i128, // Cumulative lifetime tokens claimed
    pub cliff: u64,          // Ledger timestamp before which no claims are permitted
    pub paused: bool,        // Individual stream pause status
    pub paused_at: u64,      // Ledger timestamp when the stream was paused
}
```

#### `PoolConfig`
Represents a percentage-based revenue sharing pool:

```rust
pub struct PoolConfig {
    pub name: Symbol,         // Unique pool identifier (e.g. "dev", "marketing")
    pub bps: u32,             // Allocation in basis points (1 bps = 0.01%, 10,000 = 100%)
    pub members: Vec<Address>,// Addresses sharing this pool's payout equally
}
```

---

## Contract Endpoints

### 1. Initialisation & Administration

#### `initialize`
Initialises the contract treasury with an admin address and the designated payment asset contract.

- **Signature**: `initialize(env: Env, admin: Address, token: Address)`
- **Access**: `admin.require_auth()`
- **Parameters**:
  - `admin` (`Address`): The administrative account for the contract.
  - `token` (`Address`): The Stellar Asset Contract (SAC) or custom token address.
- **Panics / Errors**:
  - `"Already initialized"`: If the contract has already been initialised.
- **CLI Example**:
  ```bash
  soroban contract invoke \
    --id <CONTRACT_ID> \
    --source admin-identity \
    --network testnet \
    -- initialize \
    --admin <ADMIN_ADDRESS> \
    --token <TOKEN_CONTRACT_ADDRESS>
  ```

#### `get_token`
Returns the address of the asset contract configured for salary and payouts.

- **Signature**: `get_token(env: Env) -> Address`
- **Access**: Public read-only
- **Panics / Errors**:
  - `"Not initialized"`: If contract has not been initialised.

#### `check_admin`
Returns whether a specified address is the active contract admin.

- **Signature**: `check_admin(env: Env, admin: Address) -> bool`
- **Access**: Public read-only

#### `change_admin`
Transfers administrative ownership to a new account.

- **Signature**: `change_admin(env: Env, admin: Address, new_admin: Address)`
- **Access**: `admin.require_auth()`
- **CLI Example**:
  ```bash
  soroban contract invoke \
    --id <CONTRACT_ID> \
    --source admin-identity \
    -- change_admin \
    --admin <CURRENT_ADMIN> \
    --new_admin <NEW_ADMIN_ADDRESS>
  ```

#### `upgrade`
Updates the contract WASM bytecode to a newly deployed WASM hash.

- **Signature**: `upgrade(env: Env, admin: Address, new_wasm_hash: BytesN<32>)`
- **Access**: `admin.require_auth()`

---

### 2. Treasury Management

#### `deposit`
Transfers tokens from the caller into the contract treasury to fund payroll and pools.

- **Signature**: `deposit(env: Env, caller: Address, amount: i128)`
- **Access**: `caller.require_auth()`
- **Parameters**:
  - `caller` (`Address`): Account transferring tokens.
  - `amount` (`i128`): Amount in smallest token units (e.g., stroops). Must be `> 0`.
- **Panics / Errors**:
  - `"Amount must be positive"`: If `amount <= 0`.
  - `"Contract is paused"`: If the global circuit breaker is active.
- **CLI Example**:
  ```bash
  soroban contract invoke \
    --id <CONTRACT_ID> \
    --source funder-identity \
    -- deposit \
    --caller <CALLER_ADDRESS> \
    --amount 5000000000
  ```

#### `get_balance`
Queries the contract's current token balance held in treasury.

- **Signature**: `get_balance(env: Env) -> i128`
- **Access**: Public read-only

---

### 3. Streaming Payroll Engine

#### `add_member`
Enrolls an employee or contractor into streaming payroll with a per-second salary rate and optional cliff.

- **Signature**: `add_member(env: Env, admin: Address, member: Address, rate_per_second: i128, cliff: u64)`
- **Access**: `admin.require_auth()`
- **Parameters**:
  - `admin` (`Address`): Admin account.
  - `member` (`Address`): Recipient address.
  - `rate_per_second` (`i128`): Salary accrued every second in stroops (must be `> 0`).
  - `cliff` (`u64`): UNIX timestamp in seconds. Set `0` for no cliff.
- **Events Emitted**: `("add_str", member) -> (rate_per_second, cliff)`
- **Panics / Errors**:
  - `"Unauthorized"`: Caller is not the admin.
  - `"Contract is paused"`: Circuit breaker active.
  - `"Rate must be positive"`: If `rate_per_second <= 0`.

#### `remove_member`
Deactivates an active stream (sets rate to 0). Any unclaimed earnings accrued up to the current timestamp are automatically calculated and paid out directly to the member.

- **Signature**: `remove_member(env: Env, admin: Address, member: Address)`
- **Access**: `admin.require_auth()`
- **Events Emitted**: `("rem_str", member) -> accrued_paid`

#### `claim`
Transfers all accrued streaming salary from contract treasury to the calling member.

- **Signature**: `claim(env: Env, member: Address) -> i128`
- **Access**: `member.require_auth()`
- **Returns**: `i128` — Total amount transferred.
- **Events Emitted**: `("claim", member) -> accrued_amount`
- **Panics / Errors**:
  - `"Contract is paused"`: Circuit breaker active.
  - `"Not a registered member"`: Member is not in contract storage.
  - `"Cliff period not met"`: Current ledger timestamp is before `cliff`.
- **CLI Example**:
  ```bash
  soroban contract invoke \
    --id <CONTRACT_ID> \
    --source alice-identity \
    -- claim \
    --member <ALICE_ADDRESS>
  ```

#### `get_accrued`
Calculates and returns the amount of unclaimed tokens accrued by a member up to the current ledger timestamp.

- **Signature**: `get_accrued(env: Env, member: Address) -> i128`
- **Access**: Public read-only
- **Note**: Returns `0` if `cliff` is not yet reached or if member is not registered.

#### `get_member`
Retrieves the full `StreamConfig` struct for a member.

- **Signature**: `get_member(env: Env, member: Address) -> StreamConfig`
- **Access**: Public read-only

#### `pause_stream`
Freezes salary accrual for a specific member. Accrued balance up to pause time is auto-settled to the member to ensure no funds are trapped.

- **Signature**: `pause_stream(env: Env, admin: Address, member: Address)`
- **Access**: `admin.require_auth()`
- **Events Emitted**: `("pause_st", member) -> settled_balance`

#### `resume_stream`
Resumes accrual for a previously paused member stream from the current ledger timestamp forward.

- **Signature**: `resume_stream(env: Env, admin: Address, member: Address)`
- **Access**: `admin.require_auth()`
- **Events Emitted**: `("resum_st", member) -> timestamp`

#### `update_stream_rate`
Updates the per-second rate of an existing member stream. Automatically settles accrued salary up to this moment at the old rate before applying the new rate.

- **Signature**: `update_stream_rate(env: Env, admin: Address, member: Address, new_rate: i128)`
- **Access**: `admin.require_auth()`
- **Events Emitted**: `("upd_rate", member) -> new_rate`
- **Panics / Errors**:
  - `"Rate must be positive"`: If `new_rate <= 0`.
  - `"Not a registered member"`: If member does not exist.

---

### 4. Revenue-Split Matrix

#### `set_pools`
Configures the basis-point split matrix. The sum of basis points across all pools must equal **10,000** (100.00%).

- **Signature**: `set_pools(env: Env, admin: Address, pools: Vec<PoolConfig>)`
- **Access**: `admin.require_auth()`
- **Panics / Errors**:
  - `"Invalid basis points"`: If any pool has `bps == 0` or `bps > 10000`.
  - `"Basis points must sum to 10000"`: If total `bps != 10000`.
  - `"Contract is paused"`: Circuit breaker active.

#### `distribute`
Distributes `total_amount` across all configured pools according to their basis points, dividing each pool's share equally among its members.

- **Signature**: `distribute(env: Env, admin: Address, total_amount: i128)`
- **Access**: `admin.require_auth()`
- **Events Emitted**: `("distrib", admin) -> total_amount`
- **Panics / Errors**:
  - `"Amount must be positive"`: If `total_amount <= 0`.
  - `"No pools configured"`: If `set_pools` has not been called.
  - `"Contract is paused"`: Circuit breaker active.
- **CLI Example**:
  ```bash
  soroban contract invoke \
    --id <CONTRACT_ID> \
    --source admin-identity \
    -- distribute \
    --admin <ADMIN_ADDRESS> \
    --total_amount 10000000000
  ```

---

### 5. Security & Circuit Breaker

#### `set_paused`
Emergency toggle to globally pause or unpause contract operations (deposits, claims, distributions, and member updates).

- **Signature**: `set_paused(env: Env, admin: Address, paused: bool)`
- **Access**: `admin.require_auth()`
- **Parameters**:
  - `paused` (`bool`): `true` to pause contract; `false` to resume.
- **Events Emitted**: `("pause", admin) -> paused`
- **Panics / Errors**:
  - `"Unauthorized"`: Caller is not the registered admin.
- **CLI Example**:
  ```bash
  # Activate emergency circuit breaker
  soroban contract invoke \
    --id <CONTRACT_ID> \
    --source admin-identity \
    -- set_paused \
    --admin <ADMIN_ADDRESS> \
    --paused true

  # Resume normal operations
  soroban contract invoke \
    --id <CONTRACT_ID> \
    --source admin-identity \
    -- set_paused \
    --admin <ADMIN_ADDRESS> \
    --paused false
  ```

#### `is_paused`
Returns whether the contract is currently under an emergency pause.

- **Signature**: `is_paused(env: &Env) -> bool`
- **Access**: Public read-only

---

## Contract Events

LaxaFlow publishes native Soroban events for all state changes. Frontend clients can subscribe to these topics via Horizon / RPC:

| Event Topic (`Symbol`) | First Topic Data | Value Payload | Description |
|---|---|---|---|
| `add_str` | `member: Address` | `(rate_per_second: i128, cliff: u64)` | New salary stream created |
| `rem_str` | `member: Address` | `accrued: i128` | Stream removed and settled |
| `claim` | `member: Address` | `accrued: i128` | Salary claimed by member |
| `pause_st` | `member: Address` | `accrued: i128` | Individual stream paused |
| `resum_st` | `member: Address` | `timestamp: u64` | Individual stream resumed |
| `upd_rate` | `member: Address` | `new_rate: i128` | Stream rate per second changed |
| `distrib` | `admin: Address` | `total_amount: i128` | Revenue split distribution executed |
| `pause` | `admin: Address` | `paused: bool` | Global emergency pause state toggled |

---

## Error Handling & Panic States

| Panic / Error String | Cause | Resolution |
|---|---|---|
| `"Already initialized"` | Contract was already initialised with admin and token. | Do not re-call `initialize`. |
| `"Not initialized"` | Operation attempted before `initialize` was called. | Run `initialize` first. |
| `"Unauthorized"` | Caller did not match the registered contract `admin`. | Sign transaction with the correct admin key. |
| `"Contract is paused"` | Global circuit breaker is active (`paused == true`). | Wait for admin to resolve issue and call `set_paused(admin, false)`. |
| `"Amount must be positive"` | Deposit or distribution amount was `<= 0`. | Pass an amount `> 0`. |
| `"Rate must be positive"` | Salary rate per second was `<= 0`. | Pass a rate `> 0` (e.g. stroops/sec). |
| `"Not a registered member"` | Requested address does not have an active stream. | Register member via `add_member`. |
| `"Cliff period not met"` | Claim attempted before member's cliff timestamp. | Wait until `ledger.timestamp >= cliff`. |
| `"Basis points must sum to 10000"` | Revenue split pool BPS does not equal 10,000. | Ensure all pool `bps` values sum to exactly 10,000. |

---

## Frontend & SDK Integration Guide

### TypeScript / Stellar SDK Example

Below is a complete example of interacting with LaxaFlow using `@stellar/stellar-sdk`:

```typescript
import {
  Contract,
  Keypair,
  SorobanRpc,
  TransactionBuilder,
  Address,
  nativeToScVal,
  scValToNative,
} from "@stellar/stellar-sdk";

const SERVER_URL = "https://soroban-testnet.stellar.org";
const server = new SorobanRpc.Server(SERVER_URL);
const CONTRACT_ID = "CXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX";
const contract = new Contract(CONTRACT_ID);

/**
 * Check if the contract is paused
 */
export async function checkIsPaused(): Promise<boolean> {
  const tx = new TransactionBuilder(await server.getAccount("G..."), { fee: "100" })
    .addOperation(contract.call("is_paused"))
    .setTimeout(30)
    .build();

  const response = await server.simulateTransaction(tx);
  return scValToNative(response.result!.retval);
}

/**
 * Claim accrued streaming salary for a connected wallet
 */
export async function claimSalary(memberKeypair: Keypair): Promise<bigint> {
  const account = await server.getAccount(memberKeypair.publicKey());

  const tx = new TransactionBuilder(account, { fee: "10000", networkPassphrase: "Test SDF Network ; September 2015" })
    .addOperation(
      contract.call("claim", new Address(memberKeypair.publicKey()).toScVal())
    )
    .setTimeout(30)
    .build();

  const preparedTx = await server.prepareTransaction(tx);
  preparedTx.sign(memberKeypair);
  const sendResponse = await server.sendTransaction(preparedTx);
  return sendResponse.status === "PENDING" ? 0n : 0n;
}

/**
 * View accrued unclaimed earnings
 */
export async function getAccrued(memberAddress: string): Promise<bigint> {
  const account = await server.getAccount(memberAddress);
  const tx = new TransactionBuilder(account, { fee: "100" })
    .addOperation(
      contract.call("get_accrued", new Address(memberAddress).toScVal())
    )
    .setTimeout(30)
    .build();

  const sim = await server.simulateTransaction(tx);
  return BigInt(scValToNative(sim.result!.retval));
}
```
