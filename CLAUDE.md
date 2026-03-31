# CLAUDE.md - MultiversX Lending Protocol

## Build Commands

```bash
make build                # Reproducible Docker build (preferred)
cargo build               # Standard Rust build
cargo test                # All tests
cargo clippy              # Lint
cargo test --package controller       # Controller only
cargo test --package liquidity_layer  # Pool only
make devnet <command>     # Devnet operations
make mainnet <command>    # Mainnet operations
```

---

## Architecture

```
CONTROLLER (main entry point, owns all protocol logic)
  positions/supply.rs    borrow.rs    withdraw.rs    repay.rs    liquidation.rs
  oracle/mod.rs          validation.rs               utils.rs (health factor)
  strategy.rs (leverage) lib.rs (flash loans)        config.rs (governance)
      │
      ▼  (cross-contract via proxies, execute_on_dest_context)
LIQUIDITY LAYER (one per market, owns pool state)
  liquidity.rs (supply/borrow/withdraw/repay/flash_loan/seize)
  cache/ (Drop-based write-back: load once, save on drop)
      │
      ▼
PRICE AGGREGATOR (oracle hub)
  Multi-source → TWAP validation → tolerance check

COMMON (shared crates)
  constants/  math/  rates/  structs/  errors/  events/  proxies/
```

### Key Patterns

- **NFT Positions**: Each user account is an NFT. Positions are stored as `nonce -> type -> asset -> AccountPosition`. NFT is burned when all positions are empty.
- **Scaled Amounts**: All positions store `scaled_amount = actual / index`. Actual value recovered as `scaled * index`. This auto-distributes interest.
- **Cache (Drop-based)**: `Cache::new()` loads all pool state once. `Drop` writes it back. Never manually save — let the destructor handle it.
- **Cross-contract**: Controller calls pools via typed proxies. All pool endpoints are `#[only_owner]` (controller is owner).
- **Position Limits**: Max 10 supply + 10 borrow positions per account (gas safety for liquidation iteration).

---

## Precision System

| Name | Value | Decimals | Usage |
|------|-------|----------|-------|
| RAY  | 10^27 | 27 | Interest rates, indexes, internal math |
| WAD  | 10^18 | 18 | Token amounts, health factors |
| BPS  | 10^4  | 4  | Percentages (100% = 10000) |

### Rounding Rules (CRITICAL)

**All arithmetic uses half-up rounding** (rounds 0.5 away from zero). This prevents systematic bias.

```
mul_half_up(a, b, precision)  = (a * b + 10^precision/2) / 10^precision
div_half_up(a, b, precision)  = (a * 10^precision + b/2) / b
rescale_half_up(val, new_prec) = half-up when downscaling, lossless when upscaling
```

Signed variants (`mul_half_up_signed`, `div_half_up_signed`) round **away from zero** for negative results (-1.5 -> -2).

### Common Conversions

```
RAY -> WAD:  rescale_half_up(ray_val, 18)
WAD -> asset decimals:  rescale_half_up(wad_val, asset_decimals)
BPS -> ratio:  div_half_up(bps_val, bps(), WAD_PRECISION)  // 8000 BPS -> 0.8
```

---

## Interest Rate Model

**3-region piecewise linear** (file: `common/rates/src/rates.rs`)

```
Region 1 (U < mid):     rate = base + U * slope1 / mid
Region 2 (mid <= U < opt): rate = base + slope1 + (U - mid) * slope2 / (opt - mid)
Region 3 (U >= opt):    rate = base + slope1 + slope2 + (U - opt) * slope3 / (1 - opt)
Final:                   rate = min(rate, max_borrow_rate) / MILLISECONDS_PER_YEAR
```

**Compound interest**: 5-term Taylor expansion of e^(rate * time_ms). ~0.0001% accuracy.

**Index updates**:
- `new_borrow_index = old_borrow_index * compound_interest_factor`
- `new_supply_index = old_supply_index * (1 + supplier_rewards / total_supplied_value)`
- `supplier_rewards = accrued_interest * (1 - reserve_factor/10000)`

---

## Liquidation

**Dutch Auction** with proportional collateral seizure.

### Health Factor

```
HF = Sum(supply_value * liquidation_threshold) / Sum(borrow_value)
Liquidatable when HF < 1.0 WAD
```

### Dynamic Bonus

```
gap = (1.02 - HF) / 1.02
bonus = base_bonus + (max_bonus - base_bonus) * min(2.0 * gap, 1)
final_bonus = min(bonus, 15%)   // MAX_LIQUIDATION_BONUS = 1500 BPS
```

### Debt Repayment Formula

```
ideal_repayment = (target_HF * total_debt - weighted_collateral) / (target_HF - (1 + bonus))
Target: HF = 1.02 (primary), fallback to 1.01
```

### Seizure

- Proportional across all collateral: `seizure_per_asset = total_seizure * (asset_value / total_collateral) / price`
- Protocol fee: `seizure * liquidation_fees_bps / 10000` (added to pool revenue)
- Bad debt threshold: <$5 USD remaining debt -> seize all, socialize loss to suppliers

---

## Oracle System

### Validation Pipeline

```
Off-chain submitters -> Consensus/Median -> TWAP cross-check -> Tolerance gate
```

### Tolerance Tiers

| Deviation | Supply/Repay | Borrow/Withdraw/Liquidate |
|-----------|-------------|--------------------------|
| <= 2% (first tier) | Safe price | Safe price |
| 2-5% (second tier) | Avg price | Avg price |
| > 5% | Avg price | **BLOCKED** |

Risk-increasing operations are blocked during high price deviation.

### Token Pricing Types

- **Normal**: Direct aggregator price
- **Derived** (xEGLD, LEGLD): `base_price * exchange_rate`
- **LP Token** (Arda formula): `(sqrt(K * pB/pA) * pA + sqrt(K * pA/pB) * pB) / total_supply`

### Price Cache

Transaction-level cache per token. Prevents intra-tx price manipulation and redundant oracle calls.

---

## E-Mode & Isolation

### E-Mode (Efficiency Mode)

Correlated asset groups get enhanced parameters (e.g., stablecoins: 97% LTV, 98% threshold, 2% bonus vs standard 75/80/5%).

**Rules**: Category chosen at account creation. Only category-registered assets allowed. Deprecated categories block new positions. **E-Mode XOR Isolation** (never both).

### Isolation Mode

High-risk assets with restricted exposure.

**Rules**: Single collateral asset only. Global USD debt ceiling per isolated asset tracked in `isolated_asset_debt_usd`. Separate NFT account required. Cannot combine with E-Mode. Standard (not E-Mode) parameters apply.

---

## Security Invariants

### Index Safety
- `supply_index >= 1e-27` (RAY) -- violation = total supplier loss
- `borrow_index >= 1e27` -- violation = interest calculation errors
- Indexes must be monotonically increasing (except bad debt socialization for supply_index)

### Solvency
- `reserves >= available_liquidity` -- withdrawal failures if violated
- `Sum(user_scaled) <= total_scaled` -- phantom liquidity if violated

### Health Factor
- HF uses **current** (synced) indexes, never stale values
- `HF >= 1.0` required after every borrow/withdraw operation
- `HF < 1.0` required to trigger liquidation (no wrongful liquidations)

### Risk Parameters
- `LTV < liquidation_threshold` always (otherwise impossible states)
- `liquidation_bonus <= 15%` (MAX_LIQUIDATION_BONUS)
- `reserve_factor < 100%`
- `optimal_utilization > mid_utilization` and `optimal_utilization < 1.0`

### Position Safety
- Max 10 positions per type (gas limits for liquidation iteration)
- Position type must remain consistent with storage key

### Isolation Safety
- Single isolated collateral per account (no mixing)
- `isolated_debt <= debt_ceiling` enforced on every borrow
- Isolation and E-Mode are mutually exclusive

### Oracle Safety
- Price staleness <= `max_price_stale_seconds` (typically 15 min)
- Price within configured tolerance bands
- Risk-increasing ops blocked beyond second tolerance tier

### Flash Loan Safety
- Reentrancy guard: no nested flash loans (`FLASH_LOAN_ALREADY_ONGOING`)
- Same-shard callback validation
- Endpoint cannot be a built-in function
- `repayment >= borrowed + fees` enforced after callback
- Cache dropped before callback, recreated after (reentrancy protection)

---

## Key Constants

```rust
RAY_PRECISION = 27;  WAD_PRECISION = 18;  BPS_PRECISION = 4;
MILLISECONDS_PER_YEAR = 31_556_926_000;
MAX_LIQUIDATION_BONUS = 1_500;  // 15%
K_SCALING_FACTOR = 20_000;      // 200% (for dynamic bonus)
MIN_FIRST_TOLERANCE = 50;       // 0.5%
MAX_FIRST_TOLERANCE = 5_000;    // 50%
MIN_LAST_TOLERANCE = 150;       // 1.5%
MAX_LAST_TOLERANCE = 10_000;    // 100%
```

---

## Key Files

| What | Where |
|------|-------|
| Supply | `controller/src/positions/supply.rs` |
| Borrow | `controller/src/positions/borrow.rs` |
| Withdraw | `controller/src/positions/withdraw.rs` |
| Repay | `controller/src/positions/repay.rs` |
| Liquidation | `controller/src/positions/liquidation.rs` |
| Flash loans | `controller/src/lib.rs` |
| Health factor | `controller/src/utils.rs` |
| Oracle | `controller/src/oracle/mod.rs` |
| Validation | `controller/src/validation.rs` |
| Pool ops | `liquidity_layer/src/liquidity.rs` |
| Pool cache | `liquidity_layer/src/cache/` |
| Interest rates | `common/rates/src/rates.rs` |
| Math ops | `common/math/src/math.rs` |
| Constants | `common/constants/src/constants.rs` |
| Errors | `common/errors/src/errors.rs` |
| Data structs | `common/structs/src/model.rs` |

---

## Agents

| Agent | Use For |
|-------|---------|
| `multiversx-defi-auditor` | Security audits, pre-deployment review |
| `rustsec-phd-mode` | High-rigor analysis, debating design decisions |
| `math-precision-validator` | Interest math, precision conversions |
| `gas-optimizer` | Storage optimization, loop complexity |
| `feature-architect` | New feature design following existing patterns |
| `invariant-checker` | Verify state consistency after changes |
| `oracle-debugger` | Price feed issues, tolerance violations |
| `debate-challenger` | Challenge assumptions, explore alternatives |
| `code-quality-guardian` | Naming, docs, architecture review |
