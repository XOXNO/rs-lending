# Liquidity Layer

Single-asset lending pool contract for the MultiversX lending protocol. Handles supply, borrow, flash loans, and liquidations with scaled token accounting and compound interest.

## Core Functions

### Primary Operations
- **Supply**: Deposit assets, receive scaled supply tokens that appreciate with interest
- **Borrow**: Take loans against collateral, tracked as scaled debt tokens  
- **Withdraw**: Redeem supply tokens for assets plus accrued interest
- **Repay**: Pay back debt with automatic overpayment refunds
- **Flash Loans**: Atomic uncollateralized loans with fee collection
- **Strategy Creation**: Leveraged borrowing with upfront fee deduction

### Administrative Functions  
- **Update Indexes**: Synchronize interest calculations
- **Seize Position**: Handle bad debt socialization or dust collection
- **Claim Revenue**: Extract accumulated protocol fees
- **Update Params**: Modify interest rate parameters

## Architecture

### Modules
- **Storage**: Persistent state (supply/borrow amounts, indexes, parameters)
- **Cache**: In-memory state snapshot with atomic commits via `Drop`
- **Liquidity**: Core financial operations
- **Utils**: Helper functions for calculations and validations
- **View**: Read-only endpoints for external queries

### Access Control
All functions restricted to `only_owner` (controller contract). No direct user access.

## Mathematical Model

### Scaled Token System
```rust
scaled_amount = actual_amount / current_index
actual_amount = scaled_amount * current_index
```
- Positions tracked as scaled amounts (RAY precision)
- Interest accrues through index appreciation
- Fair distribution regardless of timing

### Interest Rate Model
Three-tier piecewise linear model:
```rust
if utilization < mid_utilization:
    rate = base_rate + (utilization * slope1 / mid_utilization)
else if utilization < optimal_utilization:  
    rate = base_rate + slope1 + ((utilization - mid_utilization) * slope2 / ...)
else:
    rate = base_rate + slope1 + slope2 + ((utilization - optimal_utilization) * slope3 / ...)
```

### Compound Interest
Uses Taylor series approximation for continuous compounding:
```rust
compound_factor = 1 + x + x²/2! + x³/3! + x⁴/4! + x⁵/5!
where x = interest_rate * time_delta_ms
```

### Bad Debt Socialization
Immediate loss distribution by reducing supply index:
```rust
reduction_factor = (total_supplied - bad_debt) / total_supplied
new_supply_index = old_supply_index * reduction_factor
```

## Revenue Sources
- Interest rate spread (reserve factor %)
- Flash loan fees  
- Strategy creation fees
- Liquidation fees
- Dust position seizure

Revenue stored as scaled supply tokens that appreciate over time.

## Security Features
- **Reentrancy Protection**: Cache dropping before external calls
- **Asset Validation**: Strict token type checking
- **Precision Control**: RAY precision with half-up rounding
- **Reserve Constraints**: Liquidity availability validation

## View Functions
- `capitalUtilisation()`: Current utilization ratio
- `borrowRate()`: Current borrower APR
- `depositRate()`: Current supplier APY  
- `reserves()`: Available liquidity
- `suppliedAmount()`: Total deposits + interest
- `borrowedAmount()`: Total debt + interest
- `protocolRevenue()`: Claimable fees