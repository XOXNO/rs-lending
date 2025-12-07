# CLAUDE.md - MultiversX Lending Protocol Technical Reference

This document provides comprehensive guidance for Claude Code when working with this codebase. It covers architecture, mathematical foundations, all endpoints, invariants, and security considerations.

---

## Table of Contents

1. [Project Overview](#1-project-overview)
2. [Build & Development Commands](#2-build--development-commands)
3. [Architecture Deep Dive](#3-architecture-deep-dive)
4. [Mathematical Precision System](#4-mathematical-precision-system)
5. [Interest Rate Model](#5-interest-rate-model)
6. [Controller Endpoints](#6-controller-endpoints)
7. [Liquidity Layer Endpoints](#7-liquidity-layer-endpoints)
8. [Liquidation Algorithm](#8-liquidation-algorithm)
9. [Oracle System](#9-oracle-system)
10. [E-Mode & Isolation](#10-e-mode--isolation)
11. [NFT Position System](#11-nft-position-system)
12. [Common Libraries](#12-common-libraries)
13. [Security Invariants](#13-security-invariants)
14. [Storage Patterns](#14-storage-patterns)
15. [Development Workflows](#15-development-workflows)
16. [Agent Reference](#16-agent-reference)

---

## 1. Project Overview

### Purpose
MultiversX Lending Protocol is a sophisticated DeFi application implementing comprehensive lending and borrowing with:
- NFT-based position management
- Advanced risk controls (E-Mode, Isolation)
- Dutch auction liquidations
- Multi-source oracle validation
- Flash loan support

### Technology Stack
- **Language**: Rust (no_std, MultiversX framework)
- **Blockchain**: MultiversX
- **Contracts**: Controller, Liquidity Layer, Price Aggregator
- **Precision**: Fixed-point arithmetic (RAY/WAD/BPS)

### Repository Structure
```
/controller/           # Main protocol logic
  /src/
    lib.rs            # Entry points & flash loans
    router.rs         # Pool deployment
    config.rs         # Governance & configuration
    storage/          # Storage mappers
    cache/            # Transaction-level cache
    oracle/           # Price feed integration
    validation.rs     # Security checks
    utils.rs          # Health factor calculations
    helpers/          # Math operations
    views.rs          # Read-only queries
    strategy.rs       # Leverage strategies
    positions/        # Position management
      account.rs      # NFT account handling
      supply.rs       # Deposit logic
      borrow.rs       # Borrowing logic
      repay.rs        # Repayment handling
      withdraw.rs     # Withdrawal logic
      liquidation.rs  # Liquidation engine
      update.rs       # Position updates
      emode.rs        # E-Mode logic

/liquidity_layer/      # Per-market pool contracts
  /src/
    lib.rs            # Init & upgrade
    liquidity.rs      # Core operations
    storage/          # Market state
    cache/            # Index caching
    view.rs           # Market queries
    utils.rs          # Internal helpers

/price_aggregator/     # Oracle hub
  /src/
    lib.rs            # Price submission & queries

/common/               # Shared libraries
  constants/          # RAY, WAD, BPS definitions
  errors/             # Protocol error types
  events/             # Event emissions
  math/               # Arithmetic operations
  rates/              # Interest calculations
  structs/            # Data structures
  proxies/            # Cross-contract interfaces
```

---

## 2. Build & Development Commands

### Core Build Commands
```bash
make build                # Reproducible Docker build
cargo build              # Standard Rust build
cargo test               # Run all unit tests
cargo clippy             # Lint checks
```

### Network Operations
```bash
make devnet <command>    # Execute on devnet
make mainnet <command>   # Execute on mainnet
```

### Common Development Tasks
```bash
# Deployment
deployController         # Deploy main controller
deployPriceAggregator   # Deploy oracle hub
deployTemplateMarket    # Deploy pool template

# Market Operations
createMarket            # Create new lending market
upgradeMarket           # Upgrade market contract
listMarkets             # List all markets

# Oracle Management
createOracle            # Configure new oracle
addOracles              # Add oracle submitters
editOracleTolerance     # Adjust price tolerance

# E-Mode Setup
addEModeCategory        # Create efficiency mode
addAssetToEMode         # Register e-mode asset
listEModeCategories     # List all e-modes

# Verification
verifyController        # Verify controller state
verifyMarket            # Verify market state
verifyPriceAggregator   # Verify oracle state
```

### Testing Commands
```bash
cargo test                           # All tests
cargo test --package controller      # Controller tests
cargo test --package liquidity_layer # Pool tests
cargo test test_liquidation          # Specific test
```

---

## 3. Architecture Deep Dive

### Component Hierarchy
```
┌─────────────────────────────────────────────────────────────────┐
│                         CONTROLLER                               │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌───────────┐  │
│  │  Position   │ │ Liquidation │ │   E-Mode    │ │   Flash   │  │
│  │ Management  │ │   Engine    │ │  Manager    │ │   Loans   │  │
│  └─────────────┘ └─────────────┘ └─────────────┘ └───────────┘  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      LIQUIDITY LAYER                             │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌───────────┐  │
│  │  Interest   │ │   Scaled    │ │   Revenue   │ │  Bad Debt │  │
│  │   Accrual   │ │   Tokens    │ │  Management │ │  Handler  │  │
│  └─────────────┘ └─────────────┘ └─────────────┘ └───────────┘  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                     PRICE AGGREGATOR                             │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐                │
│  │ Multi-Source│ │    TWAP     │ │   Derived   │                │
│  │   Oracle    │ │ Validation  │ │   Tokens    │                │
│  └─────────────┘ └─────────────┘ └─────────────┘                │
└─────────────────────────────────────────────────────────────────┘
```

### Data Flow: Supply Operation
```
User calls supply()
    │
    ▼
Controller validates payment
    │
    ▼
Controller gets/creates account NFT
    │
    ▼
Controller calls pool.supply() via proxy
    │
    ▼
Liquidity Layer syncs indexes
    │
    ▼
Liquidity Layer calculates scaled amount
    │
    ▼
Liquidity Layer updates totals
    │
    ▼
Controller updates position on NFT
    │
    ▼
Controller emits event
    │
    ▼
NFT returned to user
```

### Data Flow: Borrow Operation
```
User calls borrow() with NFT
    │
    ▼
Controller validates account
    │
    ▼
Controller syncs all positions
    │
    ▼
Controller calculates LTV collateral value
    │
    ▼
For each borrow:
    │
    ├─► Validate borrowability
    ├─► Check borrow cap
    ├─► Check isolation ceiling (if applicable)
    ├─► Call pool.borrow() via proxy
    └─► Update position
    │
    ▼
Controller validates health factor >= 1.0
    │
    ▼
NFT returned to user
```

### Cross-Contract Communication
```rust
// Controller → Liquidity Layer (via proxy)
self.pool_proxy(pool_address)
    .supply(position, &price)
    .execute_on_dest_context()

// Controller → Price Aggregator (via proxy)
self.price_aggregator_proxy()
    .latest_price_feed(base, quote)
    .execute_on_dest_context()
```

---

## 4. Mathematical Precision System

### Precision Constants

| Name | Value | Decimals | Usage |
|------|-------|----------|-------|
| RAY | 10^27 | 27 | Interest rates, indexes, internal calculations |
| WAD | 10^18 | 18 | Token amounts, health factors, final values |
| BPS | 10^4 | 4 | Percentages (100% = 10000) |

### Conversion Functions

**Location**: `/common/math/src/math.rs`

```rust
// Create from raw BigUint
fn to_decimal_wad(value: BigUint) -> ManagedDecimal  // value at 18 decimals
fn to_decimal_ray(value: BigUint) -> ManagedDecimal  // value at 27 decimals
fn to_decimal_bps(value: BigUint) -> ManagedDecimal  // value at 4 decimals
fn to_decimal(value: BigUint, precision: usize) -> ManagedDecimal

// Convenience values
fn wad() -> ManagedDecimal         // 1.0 at WAD precision (10^18)
fn ray() -> ManagedDecimal         // 1.0 at RAY precision (10^27)
fn double_ray() -> ManagedDecimal  // 2.0 at RAY precision (2×10^27)
fn bps() -> ManagedDecimal         // 100% at BPS precision (10000)

fn wad_zero() -> ManagedDecimal    // 0 at WAD precision
fn ray_zero() -> ManagedDecimal    // 0 at RAY precision
fn bps_zero() -> ManagedDecimal    // 0 at BPS precision
```

### Arithmetic Operations

**Half-Up Rounding** (rounds 0.5 and above away from zero):

```rust
// Multiplication with half-up rounding
fn mul_half_up(
    a: &ManagedDecimal,
    b: &ManagedDecimal,
    precision: usize
) -> ManagedDecimal
// Formula: (a_scaled × b_scaled + 10^precision/2) / 10^precision

// Division with half-up rounding
fn div_half_up(
    a: &ManagedDecimal,
    b: &ManagedDecimal,
    precision: usize
) -> ManagedDecimal
// Formula: (a_scaled × 10^precision + b_scaled/2) / b_scaled

// Signed multiplication (away from zero)
fn mul_half_up_signed(
    a: &ManagedDecimalSigned,
    b: &ManagedDecimalSigned,
    precision: usize
) -> ManagedDecimalSigned
// For negative results: rounds away from zero (-1.5 → -2)

// Signed division (away from zero)
fn div_half_up_signed(
    a: &ManagedDecimalSigned,
    b: &ManagedDecimalSigned,
    precision: usize
) -> ManagedDecimalSigned

// Rescale to different precision
fn rescale_half_up(
    value: &ManagedDecimal,
    new_precision: usize
) -> ManagedDecimal
// Upscale: multiply by 10^(new - old) [lossless]
// Downscale: divide with half-up rounding
```

### Precision Conversion Examples

```rust
// RAY to WAD (27 → 18 decimals)
let ray_value: ManagedDecimal = ...; // 1.5 RAY = 1.5×10^27
let wad_value = rescale_half_up(&ray_value, WAD_PRECISION);
// Result: 1.5×10^18

// WAD to asset decimals (e.g., USDC with 6 decimals)
let wad_amount: ManagedDecimal = ...; // 100.0 WAD = 100×10^18
let usdc_amount = rescale_half_up(&wad_amount, 6);
// Result: 100×10^6

// BPS to WAD ratio
let ltv_bps: ManagedDecimal = to_decimal_bps(8000); // 80%
let ltv_ratio = div_half_up(&ltv_bps, &bps(), WAD_PRECISION);
// Result: 0.8×10^18
```

### Rounding Behavior

| Operation | Direction | Example |
|-----------|-----------|---------|
| mul_half_up | Away from zero | 1.5 → 2, -1.5 → -2 |
| div_half_up | Away from zero | 1.5 → 2, -1.5 → -2 |
| rescale (down) | Half-up | 1.555 (3dp → 2dp) → 1.56 |

**Important**: All protocol calculations use half-up rounding for consistency and to prevent systematic bias.

---

## 5. Interest Rate Model

### Piecewise Linear Model (3 Regions)

**Location**: `/common/rates/src/rates.rs:calculate_borrow_rate`

```
                     Rate
                      │
     max_rate ────────┼─────────────────────────────────
                      │                              ╱
                      │                            ╱
     base+s1+s2+s3 ───┼──────────────────────── ╱
                      │                      ╱│
                      │                   ╱   │
     base+s1+s2 ──────┼────────────────╱     │
                      │              ╱│       │
                      │           ╱   │       │
     base+s1 ─────────┼────────╱     │       │
                      │      ╱│       │       │
                      │   ╱   │       │       │
     base ────────────┼─╱─────┼───────┼───────┼──────────
                      │       │       │       │
                      └───────┴───────┴───────┴──► Utilization
                      0      mid     opt     100%
```

### Formula

```
Region 1 (utilization < mid_utilization):
    rate = base_rate + (utilization × slope1 / mid_utilization)

Region 2 (mid_utilization ≤ utilization < optimal_utilization):
    rate = base_rate + slope1 +
           ((utilization - mid_utilization) × slope2 /
            (optimal_utilization - mid_utilization))

Region 3 (utilization ≥ optimal_utilization):
    rate = base_rate + slope1 + slope2 +
           ((utilization - optimal_utilization) × slope3 /
            (1 - optimal_utilization))

Final:
    rate = min(rate, max_borrow_rate)
    per_ms_rate = rate / MILLISECONDS_PER_YEAR
```

### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| base_borrow_rate_ray | RAY | Minimum rate when utilization = 0 |
| slope1_ray | RAY | Rate increase in region 1 |
| slope2_ray | RAY | Rate increase in region 2 |
| slope3_ray | RAY | Rate increase in region 3 (steep penalty) |
| mid_utilization_ray | RAY | First kink point (e.g., 40%) |
| optimal_utilization_ray | RAY | Second kink point (e.g., 80%) |
| max_borrow_rate_ray | RAY | Absolute rate cap |

### Deposit Rate Calculation

```rust
fn calculate_deposit_rate(
    utilization: ManagedDecimal,      // RAY
    borrow_rate: ManagedDecimal,      // RAY (per-ms)
    reserve_factor: ManagedDecimal,   // BPS
) -> ManagedDecimal                   // RAY (per-ms)

// Formula:
if utilization == 0:
    deposit_rate = 0
else:
    deposit_rate = utilization × borrow_rate × (1 - reserve_factor/10000)
```

### Compound Interest (Taylor Series)

```rust
fn calculate_compounded_interest(
    rate: ManagedDecimal,     // RAY (per-ms rate)
    expiration: u64,          // Milliseconds elapsed
) -> ManagedDecimal           // RAY (growth factor)

// Taylor expansion of e^(rate × time):
x = rate × time_ms
e^x ≈ 1 + x + x²/2! + x³/3! + x⁴/4! + x⁵/5!
    = 1 + x + x²/2 + x³/6 + x⁴/24 + x⁵/120
```

**Precision**: 5-term approximation provides ~0.0001% accuracy for typical rate ranges.

### Index Update Mechanics

```rust
// Borrow index update
fn update_borrow_index(
    old_index: ManagedDecimal,       // RAY
    interest_factor: ManagedDecimal, // RAY (from compound interest)
) -> (ManagedDecimal, ManagedDecimal)

// new_borrow_index = old_borrow_index × interest_factor
// Returns: (new_index, old_index)

// Supply index update
fn update_supply_index(
    supplied: ManagedDecimal,         // RAY (scaled total)
    old_index: ManagedDecimal,        // RAY
    rewards_increase: ManagedDecimal, // RAY
) -> ManagedDecimal

// if supplied == 0 || rewards_increase == 0:
//     return old_index (no change)
// total_supplied_value = supplied × old_index
// rewards_ratio = rewards_increase / total_supplied_value
// new_supply_index = old_index × (1 + rewards_ratio)
```

### Interest Distribution

```rust
fn calculate_supplier_rewards(
    parameters: &MarketParams,
    borrowed: &ManagedDecimal,        // Scaled borrowed
    new_borrow_index: &ManagedDecimal,
    old_borrow_index: &ManagedDecimal,
) -> (ManagedDecimal, ManagedDecimal) // (supplier_rewards, protocol_fee)

// old_debt = borrowed × old_borrow_index
// new_debt = borrowed × new_borrow_index
// accrued_interest = new_debt - old_debt
// protocol_fee = accrued_interest × reserve_factor / 10000
// supplier_rewards = accrued_interest - protocol_fee
```

---

## 6. Controller Endpoints

### 6.1 Supply

**Location**: `/controller/src/positions/supply.rs`

```rust
#[payable("*")]
#[endpoint(supply)]
fn supply(
    &self,
    optional_account_nonce: OptionalValue<u64>,
    e_mode_category: OptionalValue<u8>,
) -> AccountNft
```

**Parameters**:
- `optional_account_nonce`: Existing account NFT nonce (if adding to position)
- `e_mode_category`: E-mode category ID (0 = disabled)

**Payment**:
- Optional: Account NFT (to add to existing position)
- Required: One or more collateral tokens

**Flow**:
1. Validate payment structure (extract NFT if present)
2. Check for isolated asset constraints
3. Get or create account with mode settings
4. Validate e-mode category not deprecated
5. For each collateral token:
   - Validate asset is supported and collateralizable
   - Check supply cap not exceeded
   - Get or create position
   - Call pool.supply() via proxy
   - Update position with new scaled amount
   - Emit position update event
6. Return/transfer NFT to caller

**Validation Checks**:
- Asset must be supported (`ASSET_NOT_SUPPORTED`)
- Asset must be collateralizable (`ASSET_NOT_COLLATERALIZABLE`)
- Supply cap not exceeded (`SUPPLY_CAP`)
- E-mode category valid and not deprecated
- Isolated assets cannot be bulk supplied

### 6.2 Withdraw

**Location**: `/controller/src/positions/withdraw.rs`

```rust
#[payable("*")]
#[endpoint(withdraw)]
fn withdraw(
    &self,
    collaterals: ManagedVec<TokenAmount>,
) -> AccountNft
```

**Parameters**:
- `collaterals`: List of (token, amount) pairs to withdraw

**Payment**:
- Required: Account NFT only

**Flow**:
1. Validate account NFT payment
2. Sync all deposit positions (update indexes)
3. For each collateral:
   - Validate withdrawal amount (0 = full withdrawal)
   - Get price and sync indexes
   - Call pool.withdraw() via proxy
   - Update or remove position
   - Emit position update event
4. Validate health factor >= 1.0 (if debt exists)
5. Return NFT or burn if empty

**Validation Checks**:
- Account must exist (`ACCOUNT_NOT_FOUND`)
- Position must exist (`POSITION_NOT_FOUND`)
- Health factor must remain >= 1.0 (`HEALTH_FACTOR_WITHDRAW`)
- Safe price required (`UN_SAFE_PRICE_NOT_ALLOWED`)

### 6.3 Borrow

**Location**: `/controller/src/positions/borrow.rs`

```rust
#[payable("*")]
#[endpoint(borrow)]
fn borrow(
    &self,
    borrowed_tokens: ManagedVec<TokenAmount>,
) -> AccountNft
```

**Parameters**:
- `borrowed_tokens`: List of (token, amount) pairs to borrow

**Payment**:
- Required: Account NFT only

**Flow**:
1. Validate account NFT payment
2. Sync all deposit positions
3. Calculate LTV-weighted collateral value
4. Validate bulk position limits (max 10 borrow)
5. For each token:
   - Validate borrowability
   - Check e-mode compatibility
   - Check borrow cap
   - Check isolation debt ceiling (if applicable)
   - Validate amount <= remaining borrow capacity
   - Get or create position
   - Call pool.borrow() via proxy
   - Update isolated debt tracking
   - Emit position update event
6. Validate health factor >= 1.0
7. Return NFT to caller

**Validation Checks**:
- Asset must be borrowable (`ASSET_NOT_BORROWABLE`)
- Borrow cap not exceeded (`BORROW_CAP`)
- Isolation debt ceiling respected (`DEBT_CEILING_REACHED`)
- Position limit not exceeded (`POSITION_LIMIT_EXCEEDED`)
- Health factor >= 1.0 (`HEALTH_FACTOR`)
- Safe price required (`UN_SAFE_PRICE_NOT_ALLOWED`)

### 6.4 Repay

**Location**: `/controller/src/positions/repay.rs`

```rust
#[payable("*")]
#[endpoint(repay)]
fn repay(
    &self,
    account_nonce: u64,
) -> EgldOrEsdtTokenPayment
```

**Parameters**:
- `account_nonce`: NFT nonce of account to repay

**Payment**:
- Required: One or more debt tokens (to repay)

**Flow**:
1. Validate account exists
2. For each payment:
   - Convert to appropriate decimals
   - Get position for token
   - Call pool.repay() via proxy
   - Update isolated debt tracking (if applicable)
   - Update or remove position
   - Emit position update event
3. Return any overpayment to caller

**Validation Checks**:
- Account must exist (`ACCOUNT_NOT_FOUND`)
- Position must exist for repaid asset (`POSITION_NOT_FOUND`)
- Amount must be > 0 (`INVALID_AMOUNT`)

### 6.5 Liquidate

**Location**: `/controller/src/positions/liquidation.rs`

```rust
#[payable("*")]
#[endpoint(liquidate)]
fn liquidate(
    &self,
    account_nonce: u64,
) -> ManagedVec<EgldOrEsdtTokenPayment>
```

**Parameters**:
- `account_nonce`: NFT nonce of account to liquidate

**Payment**:
- Required: One or more debt tokens for repayment

**Returns**: Seized collateral tokens

**Flow**:
1. Validate payments and liquidator
2. Execute liquidation core (see Section 8)
3. Process debt repayments through pools
4. Process collateral seizure with fees
5. Refund excess payments to liquidator
6. Return seized collateral

**Validation Checks**:
- Health factor < 1.0 (`HEALTH_FACTOR`)
- Safe prices required (`UN_SAFE_PRICE_NOT_ALLOWED`)
- Valid payment tokens

### 6.6 Flash Loan

**Location**: `/controller/src/lib.rs`

```rust
#[endpoint(flashLoan)]
fn flash_loan(
    &self,
    borrowed_asset: EgldOrEsdtTokenIdentifier,
    amount_raw: BigUint,
    contract_address: ManagedAddress,
    endpoint: ManagedBuffer,
    arguments: ManagedArgBuffer,
)
```

**Parameters**:
- `borrowed_asset`: Token to borrow
- `amount_raw`: Amount in token decimals
- `contract_address`: Callback contract
- `endpoint`: Callback endpoint name
- `arguments`: Additional callback arguments

**Flow**:
1. Validate flash loans not already ongoing
2. Validate asset supports flash loans
3. Validate callback in same shard
4. Validate endpoint not a built-in function
5. Set reentrancy guard
6. Call pool.flash_loan() via proxy
7. Clear reentrancy guard

**Security**:
- Reentrancy guard (`FLASHLOAN_ALREADY_ONGOING`)
- Same-shard validation (`INVALID_SHARD`)
- Endpoint validation (`INVALID_ENDPOINT`)

### 6.7 Admin Endpoints

```rust
// Pool Management
#[only_owner] fn create_liquidity_pool(...)
#[only_owner] fn upgrade_liquidity_pool(...)
#[only_owner] fn claim_revenue(asset)

// Configuration
#[only_owner] fn register_account_token(...)
#[only_owner] fn set_token_oracle(token, oracle_config)
#[only_owner] fn edit_token_oracle_tolerance(token, tolerance)
#[only_owner] fn edit_asset_config(token, config)

// E-Mode
#[only_owner] fn add_e_mode_category(category)
#[only_owner] fn add_asset_to_e_mode_category(token, category_id)
#[only_owner] fn deprecate_e_mode_category(category_id)

// Risk Management
#[only_owner] fn update_account_threshold(asset, has_risks, nonces)
#[only_owner] fn update_indexes(assets)
#[only_owner] fn clean_bad_debt(account_nonce)
#[only_owner] fn set_position_limits(max_supply, max_borrow)
```

---

## 7. Liquidity Layer Endpoints

### 7.1 Initialization

**Location**: `/liquidity_layer/src/lib.rs`

```rust
#[init]
fn init(
    asset: EgldOrEsdtTokenIdentifier,
    max_borrow_rate: BigUint,
    base_borrow_rate: BigUint,
    slope1: BigUint,
    slope2: BigUint,
    slope3: BigUint,
    mid_utilization: BigUint,
    optimal_utilization: BigUint,
    reserve_factor: BigUint,
    asset_decimals: usize,
)
```

**Validations**:
- `max_borrow_rate > base_borrow_rate`
- `optimal_utilization > mid_utilization`
- `optimal_utilization < 1 RAY`
- `reserve_factor < 1 RAY`

**Initializes**:
- `borrow_index = 1 RAY`
- `supply_index = 1 RAY`
- `supplied = 0`
- `borrowed = 0`
- `revenue = 0`
- `last_timestamp = current_block_timestamp`

### 7.2 Supply

```rust
#[payable("*")]
#[only_owner]
#[endpoint(supply)]
fn supply(
    mut position: AccountPosition,
    price: &ManagedDecimal,
) -> AccountPosition
```

**Flow**:
1. Get payment amount
2. Sync global indexes
3. Validate asset matches pool
4. Calculate scaled supply: `scaled = amount / supply_index`
5. Add to position and total supplied
6. Emit market update event
7. Return updated position

### 7.3 Borrow

```rust
#[only_owner]
#[endpoint(borrow)]
fn borrow(
    initial_caller: &ManagedAddress,
    amount: &ManagedDecimal,
    mut position: AccountPosition,
    price: &ManagedDecimal,
) -> AccountPosition
```

**Flow**:
1. Sync global indexes
2. Validate asset and reserves
3. Calculate scaled borrow: `scaled = amount / borrow_index`
4. Transfer tokens to caller
5. Update position and total borrowed
6. Emit market update event
7. Return updated position

### 7.4 Withdraw

```rust
#[only_owner]
#[endpoint(withdraw)]
fn withdraw(
    initial_caller: &ManagedAddress,
    amount: ManagedDecimal,
    mut position: AccountPosition,
    is_liquidation: bool,
    protocol_fee_opt: Option<ManagedDecimal>,
    price: &ManagedDecimal,
) -> AccountPosition
```

**Flow**:
1. Sync global indexes
2. Calculate gross withdrawal (full or partial)
3. Deduct liquidation fee if applicable
4. Validate sufficient reserves
5. Burn scaled tokens
6. Transfer to caller
7. Emit market update event
8. Return updated position

### 7.5 Repay

```rust
#[payable("*")]
#[only_owner]
#[endpoint(repay)]
fn repay(
    initial_caller: ManagedAddress,
    mut position: AccountPosition,
    price: &ManagedDecimal,
) -> AccountPosition
```

**Flow**:
1. Get payment amount
2. Sync global indexes
3. Calculate current debt: `debt = scaled × borrow_index`
4. Determine repayment: min(payment, debt)
5. Calculate scaled repaid: `scaled = repay / borrow_index`
6. Update position and total borrowed
7. Refund overpayment (if any)
8. Emit market update event
9. Return updated position

### 7.6 Flash Loan

```rust
#[only_owner]
#[endpoint(flashLoan)]
fn flash_loan(
    borrowed_token: &EgldOrEsdtTokenIdentifier,
    amount: &ManagedDecimal,
    contract_address: &ManagedAddress,
    endpoint: ManagedBuffer,
    arguments: ManagedArgBuffer,
    fees: &ManagedDecimal,
    price: &ManagedDecimal,
)
```

**Flow**:
1. Sync indexes
2. Validate asset and reserves
3. Calculate required repayment: `amount × (1 + fees/10000)`
4. Drop cache (reentrancy protection)
5. Call callback with borrowed tokens
6. Collect back-transfers
7. Create fresh cache
8. Validate repayment >= required
9. Add fees to revenue
10. Emit market update event

### 7.7 Seize Position (Bad Debt)

```rust
#[only_owner]
#[endpoint(seizePosition)]
fn seize_position(
    mut position: AccountPosition,
    price: &ManagedDecimal,
) -> AccountPosition
```

**For Borrow Position**:
1. Calculate bad debt: `debt = scaled × borrow_index`
2. Reduce supply index proportionally (loss to suppliers)
3. Clear borrowed amount
4. Clear position

**For Supply Position (Dust)**:
1. Add position value to revenue
2. Clear position

### 7.8 Add Rewards

```rust
#[payable("*")]
#[only_owner]
#[endpoint(addRewards)]
fn add_reward(price: &ManagedDecimal)
```

**Flow**:
1. Get payment amount
2. Sync indexes
3. Add to supplied (scaled by supply_index)
4. Update supply_index to distribute to all suppliers
5. Emit market update event

### 7.9 Claim Revenue

```rust
#[only_owner]
#[endpoint(claimRevenue)]
fn claim_revenue(price: &ManagedDecimal) -> EgldOrEsdtTokenPayment
```

**Flow**:
1. Sync indexes
2. If revenue == 0, return empty payment
3. Calculate actual revenue: `actual = scaled × supply_index`
4. Transfer min(reserves, actual) to caller
5. Burn proportional scaled revenue
6. Emit market update event

### 7.10 Update Indexes

```rust
#[only_owner]
#[endpoint(updateIndexes)]
fn update_indexes(price: &ManagedDecimal) -> MarketIndex
```

**Flow**:
1. Sync global state
2. Emit market update event
3. Return current indexes

### 7.11 Update Parameters

```rust
#[only_owner]
#[endpoint(updateParams)]
fn update_params(
    max_borrow_rate: BigUint,
    base_borrow_rate: BigUint,
    slope1: BigUint,
    slope2: BigUint,
    slope3: BigUint,
    mid_utilization: BigUint,
    optimal_utilization: BigUint,
    reserve_factor: BigUint,
    asset_price: &ManagedDecimal,
)
```

**Flow**:
1. Sync indexes with current parameters
2. Validate new parameters
3. Update stored parameters
4. Emit market update event

---

## 8. Liquidation Algorithm

### Overview

The protocol uses a **Dutch Auction** liquidation mechanism with **proportional collateral seizure** and **dynamic bonus calculation**.

### Health Factor Calculation

```rust
fn calculate_health_factor(
    supplies: &ManagedVec<AccountPosition>,
    borrows: &ManagedVec<AccountPosition>,
    prices: &HashMap<TokenId, ManagedDecimal>,
) -> ManagedDecimal  // WAD precision

// Formula:
weighted_collateral = Σ(supply_value × liquidation_threshold)
total_debt = Σ(borrow_value)
health_factor = weighted_collateral / total_debt
```

**Liquidatable**: `health_factor < 1.0 WAD`

### Dynamic Liquidation Bonus

```rust
fn calculate_dynamic_bonus(
    current_hf: ManagedDecimal,
    base_bonus: ManagedDecimal,
    max_bonus: ManagedDecimal,
) -> ManagedDecimal

// Formula:
target_hf = 1.02 WAD  // 102%
gap = (target_hf - current_hf) / target_hf
k = 2.0  // K_SCALING_FACTOR / 10000
bonus = base_bonus + (max_bonus - base_bonus) × min(k × gap, 1)
final_bonus = min(bonus, MAX_LIQUIDATION_BONUS)  // Cap at 15%
```

**Example**:
```
current_hf = 0.95 (95%)
gap = (1.02 - 0.95) / 1.02 = 0.0686
scaled_gap = 2.0 × 0.0686 = 0.1373
bonus = 0.05 + (0.15 - 0.05) × 0.1373 = 0.0637 (6.37%)
```

### Dutch Auction Algorithm

**Target**: Restore health factor to 1.02 (primary) or 1.01 (secondary)

```rust
fn calculate_ideal_debt_repayment(
    target_hf: ManagedDecimal,
    total_debt: ManagedDecimal,
    weighted_collateral: ManagedDecimal,
    bonus_rate: ManagedDecimal,
) -> ManagedDecimal

// Formula:
// Solving for d (debt to repay):
// (weighted_collateral - d × (1 + bonus)) / (total_debt - d) = target_hf
//
// d = (target_hf × total_debt - weighted_collateral) /
//     (target_hf - (1 + bonus))
```

### Proportional Collateral Seizure

```rust
fn calculate_proportional_seizure(
    positions: &ManagedVec<AccountPosition>,
    total_seizure_value: ManagedDecimal,
    total_collateral_value: ManagedDecimal,
) -> ManagedVec<(TokenId, ManagedDecimal)>

// For each collateral:
proportion = collateral_value / total_collateral_value
seizure_amount = total_seizure_value × proportion / price
```

### Bad Debt Handling

**Threshold**: $5 USD remaining debt

```rust
fn check_bad_debt(
    remaining_debt_usd: ManagedDecimal,
    remaining_collateral_value: ManagedDecimal,
) -> bool

// Bad debt when:
// 1. Remaining debt > 0
// 2. Remaining debt < $5 USD
// 3. OR remaining collateral insufficient to cover debt
```

**Action**: Seize all remaining collateral, socialize remaining debt to suppliers.

### Complete Liquidation Flow

```
1. Validate health factor < 1.0
    │
    ▼
2. Calculate dynamic bonus based on HF gap
    │
    ▼
3. Calculate ideal debt repayment (target HF = 1.02)
    │
    ├─► If cannot reach 1.02: try 1.01
    │
    ▼
4. Cap repayment at liquidator's payment
    │
    ▼
5. Calculate total seizure value (debt × (1 + bonus))
    │
    ▼
6. Calculate proportional seizure per collateral
    │
    ▼
7. Check for bad debt threshold
    │
    ├─► If bad debt: seize all, socialize loss
    │
    ▼
8. Process debt repayments through pools
    │
    ▼
9. Process collateral seizure with fees
    │
    ▼
10. Refund excess payment to liquidator
```

### Liquidation Fees

```rust
// Liquidation fee = portion of seized collateral to protocol
liquidation_fee = seizure_amount × liquidation_fees_bps / 10000

// Net to liquidator
net_seizure = seizure_amount - liquidation_fee

// Fee added to pool revenue
pool.revenue += liquidation_fee (scaled by supply_index)
```

---

## 9. Oracle System

### Three-Tier Validation

```
┌────────────────────────────────────────────────────────────┐
│                    PRICE AGGREGATOR                         │
│  ┌──────────────────┐  ┌──────────────────┐                │
│  │ Oracle Submitter │  │ Oracle Submitter │  ...           │
│  │   (Off-chain)    │  │   (Off-chain)    │                │
│  └────────┬─────────┘  └────────┬─────────┘                │
│           │                      │                          │
│           ▼                      ▼                          │
│  ┌──────────────────────────────────────────┐              │
│  │         Consensus/Median Price           │              │
│  └─────────────────────┬────────────────────┘              │
└────────────────────────┼────────────────────────────────────┘
                         │
                         ▼
┌────────────────────────────────────────────────────────────┐
│                    TWAP VALIDATION                          │
│  ┌──────────────────────────────────────────┐              │
│  │     On-chain DEX Time-Weighted Average   │              │
│  │         (15-minute freshness)            │              │
│  └─────────────────────┬────────────────────┘              │
└────────────────────────┼────────────────────────────────────┘
                         │
                         ▼
┌────────────────────────────────────────────────────────────┐
│                   TOLERANCE CHECK                           │
│                                                             │
│  First Tier:  |aggregator - twap| / twap <= 2%             │
│  Second Tier: |aggregator - twap| / twap <= 5%             │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Oracle Configuration

```rust
pub struct OracleProvider {
    pub base_token_id: EgldOrEsdtTokenIdentifier,    // Source token
    pub quote_token_id: EgldOrEsdtTokenIdentifier,   // Target (usually USD)
    pub tolerance: OraclePriceFluctuation,           // Tolerance bounds
    pub oracle_contract_address: ManagedAddress,     // Oracle endpoint
    pub pricing_method: PricingMethod,               // Safe | Instant | Aggregator
    pub oracle_type: OracleType,                     // Normal | Derived | Lp
    pub exchange_source: ExchangeSource,             // XExchange | LXOXNO | etc
    pub asset_decimals: usize,                       // Token decimals
    pub onedex_pair_id: usize,                       // OneDex pair (if applicable)
    pub max_price_stale_seconds: u64,                // Freshness requirement
}

pub struct OraclePriceFluctuation {
    pub first_upper_ratio_bps: ManagedDecimal,       // +2% (first tolerance)
    pub first_lower_ratio_bps: ManagedDecimal,       // -2% (first tolerance)
    pub last_upper_ratio_bps: ManagedDecimal,        // +5% (fallback)
    pub last_lower_ratio_bps: ManagedDecimal,        // -5% (fallback)
}
```

### Token Types

**Normal**: Direct price feed
```rust
price_usd = aggregator_price
```

**Derived**: Exchange rate based (xEGLD, LEGLD)
```rust
exchange_rate = get_exchange_rate(derived_token)
base_price = get_price(base_token)  // e.g., EGLD
price_usd = base_price × exchange_rate
```

**LP Token**: Arda formula
```rust
// Get pair reserves
K = reserve_A × reserve_B

// Calculate virtual reserves at equilibrium
X' = sqrt(K × price_B / price_A)
Y' = sqrt(K × price_A / price_B)

// LP token value
LP_value = X' × price_A + Y' × price_B

// LP token price
LP_price = LP_value / total_supply
```

### Operation Safety Matrix

| Operation | Within First | Within Second | Beyond Second |
|-----------|-------------|---------------|---------------|
| Supply | Safe price | Avg price | Avg price |
| Repay | Safe price | Avg price | Avg price |
| Borrow | Safe price | Avg price | **BLOCKED** |
| Withdraw | Safe price | Avg price | **BLOCKED** |
| Liquidate | Safe price | Avg price | **BLOCKED** |

**Rationale**: Operations that increase risk (borrow, withdraw, liquidate) are blocked during high price deviation to prevent manipulation.

### Price Caching

```rust
// Transaction-level cache prevents:
// 1. Intra-transaction price manipulation
// 2. Multiple oracle calls for same asset
// 3. Inconsistent prices within operation

fn token_price(&self, token: &TokenId) -> ManagedDecimal {
    if let Some(cached) = self.price_cache.get(token) {
        return cached;
    }
    let price = self.fetch_and_validate_price(token);
    self.price_cache.insert(token, price);
    price
}
```

---

## 10. E-Mode & Isolation

### E-Mode (Efficiency Mode)

**Purpose**: Optimize capital efficiency for correlated asset groups

```rust
pub struct EModeCategory {
    pub category_id: u8,
    pub loan_to_value_bps: ManagedDecimal,           // Enhanced LTV
    pub liquidation_threshold_bps: ManagedDecimal,   // Optimized threshold
    pub liquidation_bonus_bps: ManagedDecimal,       // Adjusted bonus
    pub is_deprecated: bool,
}

pub struct EModeAssetConfig {
    pub is_collateralizable: bool,   // Can supply in this e-mode
    pub is_borrowable: bool,         // Can borrow in this e-mode
}
```

### E-Mode Benefits

| Parameter | Standard | E-Mode (Stablecoins) |
|-----------|----------|---------------------|
| LTV | 75% | 97% |
| Liquidation Threshold | 80% | 98% |
| Liquidation Bonus | 5% | 2% |

### E-Mode Rules

1. **Category Assignment**: Account chooses e-mode at creation
2. **Asset Eligibility**: Only assets registered in category usable
3. **Parameter Override**: E-mode params replace standard params
4. **Deprecation**: Deprecated categories prevent new positions
5. **Mutual Exclusivity**: E-mode XOR Isolation (never both)

### E-Mode Flow

```
User creates position with e_mode_category = 1
    │
    ▼
Validate category exists and not deprecated
    │
    ▼
For each supplied asset:
    │
    ├─► Check asset registered in category 1
    ├─► Check e-mode collateralizable flag
    └─► Apply e-mode LTV/threshold
    │
    ▼
For each borrowed asset:
    │
    ├─► Check asset registered in category 1
    └─► Check e-mode borrowable flag
```

### Isolation Mode

**Purpose**: Restrict high-risk assets to limit systemic risk

```rust
pub struct IsolatedAssetConfig {
    pub is_isolated_asset: bool,
    pub isolation_debt_ceiling_usd_wad: ManagedDecimal,
    pub isolation_borrow_enabled: bool,
}
```

### Isolation Rules

1. **Single Collateral**: Only one collateral asset allowed
2. **Debt Ceiling**: Global USD limit per isolated asset
3. **No E-Mode**: Cannot combine with efficiency mode
4. **Separate NFT**: Requires dedicated account NFT

### Isolation Debt Tracking

```rust
// Storage
#[storage_mapper("isolated_debt")]
fn isolated_asset_debt_usd(&self, token: &TokenId)
    -> SingleValueMapper<ManagedDecimal>;

// On borrow
fn on_isolated_borrow(token: &TokenId, amount_usd: ManagedDecimal) {
    let current = self.isolated_asset_debt_usd(token).get();
    let new_debt = current + amount_usd;
    require!(
        new_debt <= debt_ceiling,
        Errors::DEBT_CEILING_REACHED
    );
    self.isolated_asset_debt_usd(token).set(new_debt);
}

// On repay
fn on_isolated_repay(token: &TokenId, amount_usd: ManagedDecimal) {
    let current = self.isolated_asset_debt_usd(token).get();
    let new_debt = current - amount_usd;
    self.isolated_asset_debt_usd(token).set(new_debt);
}
```

### Isolation Flow

```
User supplies isolated asset
    │
    ▼
Create new account with is_isolated = true
    │
    ▼
Store isolated_token in attributes
    │
    ▼
On borrow:
    │
    ├─► Validate debt ceiling not exceeded
    ├─► Add to global isolated debt tracker
    └─► Apply standard (not e-mode) params
    │
    ▼
On repay:
    └─► Subtract from global isolated debt tracker
```

---

## 11. NFT Position System

### Account NFT Structure

```rust
pub struct AccountAttributes {
    pub is_isolated_position: bool,              // In isolation mode
    pub e_mode_category_id: u8,                  // E-mode category (0 = disabled)
    pub mode: PositionMode,                      // Position type
    pub isolated_token: ManagedOption<Token>,    // Isolated collateral (if applicable)
}

pub enum PositionMode {
    None,
    Normal,     // Standard lending position
    Multiply,   // Leveraged position
    Long,       // Long derivative
    Short,      // Short derivative
}
```

### Position Storage

```rust
// Two-level mapping: nonce → type → asset → position
#[storage_mapper("positions")]
fn positions(
    &self,
    account_nonce: u64,
    position_type: &AccountPositionType,
) -> MapMapper<EgldOrEsdtTokenIdentifier, AccountPosition>;

pub struct AccountPosition {
    pub position_type: AccountPositionType,      // Deposit | Borrow
    pub asset_id: EgldOrEsdtTokenIdentifier,
    pub scaled_amount_ray: ManagedDecimal,       // Compressed amount
    pub account_nonce: u64,
    pub liquidation_threshold_bps: ManagedDecimal,
    pub liquidation_bonus_bps: ManagedDecimal,
    pub liquidation_fees_bps: ManagedDecimal,
    pub loan_to_value_bps: ManagedDecimal,
}
```

### Scaled Amount System

```
At deposit:
  scaled = actual_amount / supply_index
  // User's proportional share of pool

Over time:
  supply_index grows from interest accrual

At withdrawal:
  actual = scaled × supply_index
  // Scaled share now worth more (includes interest)
```

**Example**:
```
Day 0: Deposit 100 EGLD, supply_index = 1.0
       scaled = 100 / 1.0 = 100

Day 30: supply_index = 1.001 (0.1% interest)
       current_value = 100 × 1.001 = 100.1 EGLD
```

### Position Limits

```rust
pub struct PositionLimits {
    pub max_borrow_positions: u8,   // Default: 10
    pub max_supply_positions: u8,   // Default: 10
}
```

**Rationale**:
- Liquidation must iterate all positions
- O(n) health factor + O(n) pool calls
- Gas limits require bounded complexity

### NFT Lifecycle

```
1. CREATION
   User calls supply() without NFT
   → Mint new NFT with fresh nonce
   → Initialize attributes
   → Create first position

2. MODIFICATION
   User calls operation with NFT
   → Update positions
   → Update attributes if needed
   → Return NFT to user

3. DESTRUCTION
   User withdraws all collateral, repays all debt
   → All positions empty
   → Burn NFT
```

---

## 12. Common Libraries

### 12.1 Constants

**Location**: `/common/constants/src/constants.rs`

```rust
// Precision constants
pub const RAY_PRECISION: usize = 27;
pub const WAD_PRECISION: usize = 18;
pub const BPS_PRECISION: usize = 4;

pub const RAY: u128 = 1_000_000_000_000_000_000_000_000_000;
pub const DOUBLE_RAY: u128 = 2_000_000_000_000_000_000_000_000_000;
pub const WAD: u128 = 1_000_000_000_000_000_000;
pub const BPS: u64 = 10_000;

// Time
pub const MILLISECONDS_PER_YEAR: u64 = 31_556_926_000;
pub const SECONDS_PER_MINUTE: u64 = 60;

// Risk parameters
pub const MAX_LIQUIDATION_BONUS: u64 = 1_500;  // 15%
pub const K_SCALING_FACTOR: u64 = 20_000;       // 200%

// Oracle tolerances
pub const MIN_FIRST_TOLERANCE: u64 = 50;        // 0.5%
pub const MAX_FIRST_TOLERANCE: u64 = 5_000;     // 50%
pub const MIN_LAST_TOLERANCE: u64 = 150;        // 1.5%
pub const MAX_LAST_TOLERANCE: u64 = 10_000;     // 100%

// Tickers
pub const EGLD_TICKER: &[u8] = b"EGLD";
pub const WEGLD_TICKER: &[u8] = b"WEGLD";
pub const USD_TICKER: &[u8] = b"USD";
```

### 12.2 Errors

**Location**: `/common/errors/src/errors.rs`

#### Market & Asset Errors
```rust
ASSET_NOT_SUPPORTED
ASSET_ALREADY_SUPPORTED
INVALID_TICKER
NO_POOL_FOUND
TEMPLATE_EMPTY
```

#### Health & Collateral Errors
```rust
INSUFFICIENT_COLLATERAL
HEALTH_FACTOR               // HF not low enough for liquidation
HEALTH_FACTOR_WITHDRAW      // Withdrawal would breach HF
POSITION_LIMIT_EXCEEDED     // Max 10 positions per type
```

#### Price & Oracle Errors
```rust
PRICE_AGGREGATOR_NOT_SET
PRICE_FEED_STALE
UN_SAFE_PRICE_NOT_ALLOWED
INVALID_ORACLE_TOKEN_TYPE
ORACLE_TOKEN_NOT_FOUND
```

#### E-Mode Errors
```rust
EMODE_CATEGORY_NOT_FOUND
EMODE_CATEGORY_DEPRECATED
ASSET_ALREADY_SUPPORTED_IN_EMODE
CANNOT_USE_EMODE_WITH_ISOLATED_ASSETS
```

#### Isolation Errors
```rust
ASSET_NOT_BORROWABLE_IN_ISOLATION
ASSET_NOT_BORROWABLE_IN_SILOED
MIX_ISOLATED_COLLATERAL
SWAP_COLLATERAL_NOT_SUPPORTED
```

#### Cap Errors
```rust
SUPPLY_CAP
BORROW_CAP
DEBT_CEILING_REACHED
```

#### Flash Loan Errors
```rust
FLASHLOAN_NOT_ENABLED
INVALID_SHARD
INVALID_ENDPOINT
FLASHLOAN_RESERVE_ASSET
FLASH_LOAN_ALREADY_ONGOING
```

#### Rate Configuration Errors
```rust
INVALID_BORROW_RATE_PARAMS      // max < base
INVALID_UTILIZATION_RANGE       // optimal < mid
OPTIMAL_UTILIZATION_TOO_HIGH    // optimal >= 1.0
INVALID_RESERVE_FACTOR          // >= 100%
```

#### Position Errors
```rust
ACCOUNT_NOT_IN_THE_MARKET
POSITION_NOT_FOUND
NO_DEBT_PAYMENTS_TO_PROCESS
CANNOT_CLEAN_BAD_DEBT
WITHDRAW_AMOUNT_LESS_THAN_FEE
```

### 12.3 Events

**Location**: `/common/events/src/events.rs`

```rust
// Market events
fn create_market_params_event(...)
fn update_market_params_event(...)
fn update_market_state_event(
    timestamp, supply_index, borrow_index,
    reserves, supplied, borrowed, revenue,
    asset, price
)

// Position events
fn update_position_event(
    index,              // Position index
    amount,             // Amount changed
    position,           // Updated position
    asset_price,        // Current price (optional)
    caller,             // Initiator (optional)
    account_attributes, // Account state (optional)
)

// Configuration events
fn update_debt_ceiling_event(asset, ceiling)
fn update_asset_config_event(asset, config)
fn update_e_mode_category_event(category)
fn update_e_mode_asset_event(asset, category_id)
fn update_asset_oracle_event(asset, oracle_config)

// Special events
fn emit_trigger_clean_bad_debt(account_nonce, debt_amount)
fn initial_multiply_payment_event(...)
```

### 12.4 Data Structures

**Location**: `/common/structs/src/model.rs`

```rust
pub struct MarketParams {
    pub max_borrow_rate_ray: ManagedDecimal,
    pub base_borrow_rate_ray: ManagedDecimal,
    pub slope1_ray: ManagedDecimal,
    pub slope2_ray: ManagedDecimal,
    pub slope3_ray: ManagedDecimal,
    pub mid_utilization_ray: ManagedDecimal,
    pub optimal_utilization_ray: ManagedDecimal,
    pub reserve_factor_bps: ManagedDecimal,
    pub asset_id: EgldOrEsdtTokenIdentifier,
    pub asset_decimals: usize,
}

pub struct AssetConfig {
    pub loan_to_value_bps: ManagedDecimal,
    pub liquidation_threshold_bps: ManagedDecimal,
    pub liquidation_bonus_bps: ManagedDecimal,
    pub liquidation_fees_bps: ManagedDecimal,
    pub is_collateralizable: bool,
    pub is_borrowable: bool,
    pub e_mode_enabled: bool,
    pub is_isolated_asset: bool,
    pub isolation_debt_ceiling_usd_wad: ManagedDecimal,
    pub is_siloed_borrowing: bool,
    pub is_flashloanable: bool,
    pub flashloan_fee_bps: ManagedDecimal,
    pub isolation_borrow_enabled: bool,
    pub borrow_cap_wad: Option<BigUint>,
    pub supply_cap_wad: Option<BigUint>,
}

pub struct MarketIndex {
    pub borrow_index_ray: ManagedDecimal,
    pub supply_index_ray: ManagedDecimal,
}
```

---

## 13. Security Invariants

### Index Invariants

| ID | Invariant | Location | Violation Impact |
|----|-----------|----------|-----------------|
| INV-1 | `supply_index >= 1e-27` | `utils.rs:apply_bad_debt` | Total supplier loss |
| INV-2 | `borrow_index >= 1e27` | `lib.rs:init` | Interest errors |
| INV-3 | Indexes only increase | `rates.rs:update_*_index` | Interest manipulation |

### Solvency Invariants

| ID | Invariant | Location | Violation Impact |
|----|-----------|----------|-----------------|
| INV-4 | `reserves >= available_liquidity` | `cache.rs:has_reserves` | Failed withdrawals |
| INV-5 | `Σ(user_scaled) <= total_scaled` | Position updates | Phantom liquidity |

### Health Factor Invariants

| ID | Invariant | Location | Violation Impact |
|----|-----------|----------|-----------------|
| INV-6 | `HF < 1.0` triggers liquidation | `liquidation.rs` | Wrongful liquidations |
| INV-7 | HF uses current indexes | `utils.rs` | Incorrect risk |
| INV-8 | `HF >= 1.0` after operations | `validation.rs` | Unhealthy positions |

### Risk Parameter Invariants

| ID | Invariant | Location | Violation Impact |
|----|-----------|----------|-----------------|
| INV-9 | `LTV < liquidation_threshold` | Config validation | Impossible states |
| INV-10 | `liquidation_bonus <= 15%` | `constants.rs` | Excessive profits |
| INV-11 | `reserve_factor < 100%` | Market params | Interest errors |

### Position Invariants

| ID | Invariant | Location | Violation Impact |
|----|-----------|----------|-----------------|
| INV-12 | `positions <= 10` per type | `validation.rs` | Gas failures |
| INV-13 | Position type consistent | Storage | State corruption |

### Isolation Invariants

| ID | Invariant | Location | Violation Impact |
|----|-----------|----------|-----------------|
| INV-14 | Single isolated collateral | `supply.rs` | Broken isolation |
| INV-15 | `isolated_debt <= ceiling` | `borrow.rs` | Risk limits |
| INV-16 | `isolation XOR e-mode` | `emode.rs` | Conflicting params |

### Oracle Invariants

| ID | Invariant | Location | Violation Impact |
|----|-----------|----------|-----------------|
| INV-17 | Price age <= 15 minutes | Oracle validation | Stale prices |
| INV-18 | Price within tolerance | `oracle/mod.rs` | Manipulation |

### Flash Loan Invariants

| ID | Invariant | Location | Violation Impact |
|----|-----------|----------|-----------------|
| INV-19 | Repayment >= borrowed + fees | `liquidity.rs` | Unpaid loans |
| INV-20 | No nested flash loans | `lib.rs` | Reentrancy |

---

## 14. Storage Patterns

### Cache Pattern

**Purpose**: Minimize storage reads/writes within a transaction

```rust
pub struct Cache<'a, C> {
    pub supplied_ray: ManagedDecimal,
    pub borrowed_ray: ManagedDecimal,
    pub revenue_ray: ManagedDecimal,
    pub timestamp: u64,
    pub parameters: MarketParams,
    pub borrow_index_ray: ManagedDecimal,
    pub supply_index_ray: ManagedDecimal,
    // Internal: sc_ref for final write
}

impl<'a, C> Cache<'a, C> {
    pub fn new(sc_ref: &'a C) -> Self {
        // Load all state from storage once
    }
}

impl<'a, C> Drop for Cache<'a, C> {
    fn drop(&mut self) {
        // Write all modified state back to storage once
    }
}
```

**Usage**:
```rust
fn operation(&self) {
    let mut cache = Cache::new(self);

    // All operations use cache
    cache.supplied_ray += amount;
    cache.borrow_index_ray = new_index;

    // Drop automatically saves all changes
}
```

### Storage Mapper Types

```rust
// Single value
#[storage_mapper("value")]
fn value(&self) -> SingleValueMapper<Type>;

// Parameterized single value
#[storage_mapper("config")]
fn config(&self, key: &Key) -> SingleValueMapper<Type>;

// Map (key → value)
#[storage_mapper("map")]
fn map(&self) -> MapMapper<Key, Value>;

// Set
#[storage_mapper("set")]
fn set(&self) -> SetMapper<Item>;

// Linked list
#[storage_mapper("list")]
fn list(&self) -> LinkedListMapper<Item>;
```

### Gas Cost Considerations

| Operation | Approximate Cost |
|-----------|-----------------|
| Storage read | ~5,000 gas/byte |
| Storage write | ~50,000 gas/byte |
| Storage delete | ~-25,000 gas refund |
| BigUint operation | ~10-100 gas |
| Cross-contract call | ~15,000,000 base |

---

## 15. Development Workflows

### Adding a New Market

1. **Configure market parameters**
   ```json
   // configs/devnet_market_configs.json
   {
     "asset": "TOKEN-123456",
     "decimals": 18,
     "base_borrow_rate": "10000000000000000000000000",
     "slope1": "40000000000000000000000000",
     ...
   }
   ```

2. **Create oracle configuration**
   ```bash
   make devnet createOracle
   ```

3. **Deploy market**
   ```bash
   make devnet createMarket
   ```

4. **Configure asset risk parameters**
   - Set LTV, liquidation threshold, caps
   - Configure E-mode eligibility (if applicable)

5. **Add to monitoring**
   - Verify with `verifyMarket`
   - Monitor indexes with `updateIndexes`

### Implementing New Features

1. **Follow module patterns**
   - Storage in `storage/mod.rs`
   - Logic in feature-specific module
   - Events in `common/events`

2. **Use proxy patterns**
   ```rust
   // Define in common/proxies
   fn new_operation(&self, params) -> ReturnType;

   // Implement in target contract
   #[only_owner]
   #[endpoint(newOperation)]
   fn new_operation(&self, params) -> ReturnType { ... }

   // Call via proxy
   self.pool_proxy(address)
       .new_operation(params)
       .execute_on_dest_context()
   ```

3. **Emit events for all state changes**

4. **Add comprehensive validation**

5. **Include unit tests**

### Debugging Oracle Issues

1. **Check aggregator price**
   ```bash
   make devnet verifyPriceAggregator
   ```

2. **Verify TWAP freshness**
   - Must be < 15 minutes old
   - Check DEX activity for updates

3. **Review tolerance configuration**
   - First tier: ±2% default
   - Second tier: ±5% default

4. **Check derived token pricing**
   - Verify exchange rate sources
   - Check base token price

### Testing Patterns

```rust
#[test]
fn test_supply_basic() {
    // 1. Setup
    let mut state = setup_contract();

    // 2. Action
    state.supply(user, token, amount);

    // 3. Verify
    assert_eq!(state.get_position(user, token).amount, amount);
    assert!(state.get_health_factor(user) >= WAD);
}

#[test]
fn test_liquidation_boundary() {
    // Test at exact HF = 1.0 boundary
    let mut state = setup_liquidatable_position();

    // Should fail at HF >= 1.0
    state.set_health_factor(user, WAD);
    assert_err!(state.liquidate(user), Errors::HEALTH_FACTOR);

    // Should succeed at HF < 1.0
    state.set_health_factor(user, WAD - 1);
    assert_ok!(state.liquidate(user));
}
```

---

## 16. Agent Reference

### Available Agents

| Agent | Purpose | When to Use |
|-------|---------|-------------|
| `rustsec-phd-mode` | High-rigor technical analysis | Debating, validating claims, security analysis |
| `multiversx-defi-auditor` | Security audits | Pre-deployment, new features, vulnerability assessment |
| `code-quality-guardian` | Code quality review | Post-implementation, refactoring |
| `math-precision-validator` | Mathematical correctness | Interest calculations, precision conversions |
| `gas-optimizer` | Gas optimization | Performance issues, batch operations |
| `feature-architect` | Feature design | New features, architectural changes |
| `invariant-checker` | Invariant verification | Code changes, state consistency |
| `oracle-debugger` | Oracle debugging | Price errors, tolerance issues |
| `debate-challenger` | Design challenges | Architecture decisions, trade-offs |

### Agent Selection Guide

**Security Questions**:
- "Is this liquidation logic correct?" → `multiversx-defi-auditor`
- "Could this be exploited?" → `rustsec-phd-mode`

**Implementation Questions**:
- "How should I add this feature?" → `feature-architect`
- "Why is this slow?" → `gas-optimizer`
- "Is this math correct?" → `math-precision-validator`

**Validation Questions**:
- "Are all invariants preserved?" → `invariant-checker`
- "Why is the oracle failing?" → `oracle-debugger`
- "Is this the best approach?" → `debate-challenger`

**Quality Questions**:
- "Is this code clean?" → `code-quality-guardian`

---

## Quick Reference

### Key Formulas

```
Health Factor:
  HF = Σ(deposit × threshold) / Σ(borrow)

Borrow Rate (simplified):
  rate = base + slope × utilization

Scaled Amount:
  scaled = actual / index
  actual = scaled × index

Liquidation Bonus:
  bonus = base + (max - base) × min(k × gap, 1)
  gap = (1.02 - HF) / 1.02
```

### Key Thresholds

| Threshold | Value | Purpose |
|-----------|-------|---------|
| Liquidation HF | 1.0 WAD | Triggers liquidation |
| Target HF | 1.02 WAD | Post-liquidation target |
| Max positions | 10 each | Gas safety |
| Max bonus | 15% | Caps liquidator profit |
| TWAP freshness | 15 min | Oracle validity |
| First tolerance | ±2% | Normal operations |
| Last tolerance | ±5% | Fallback operations |

### Key Files

| Component | Primary File |
|-----------|-------------|
| Supply | `/controller/src/positions/supply.rs` |
| Borrow | `/controller/src/positions/borrow.rs` |
| Liquidation | `/controller/src/positions/liquidation.rs` |
| Interest rates | `/common/rates/src/rates.rs` |
| Math operations | `/common/math/src/math.rs` |
| Pool operations | `/liquidity_layer/src/liquidity.rs` |
| Oracle integration | `/controller/src/oracle/mod.rs` |

---

*Document Version: 1.0*
*Last Updated: December 2024*
*Lines: ~1400*
