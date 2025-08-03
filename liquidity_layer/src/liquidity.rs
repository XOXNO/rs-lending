/*!
# MultiversX Lending Protocol - Liquidity Layer

This module implements the core liquidity pool mechanics for a single-asset lending protocol
built on the MultiversX blockchain. The protocol enables users to supply assets to earn interest
and borrow assets against collateral while maintaining system stability through advanced
mathematical models and risk management mechanisms.

## Core Protocol Features

### 🏦 **Liquidity Pool Operations**
- **Supply**: Deposit assets to earn interest through scaled supply tokens
- **Borrow**: Obtain loans against collateral with compound interest
- **Withdraw**: Redeem supply tokens for underlying assets plus interest
- **Repay**: Pay back borrowed amounts with automatic overpayment handling

### ⚡ **Flash Loan System**
- **Atomic Borrowing**: Instant loans repaid within the same transaction
- **Arbitrage Support**: Enable complex DeFi strategies and liquidations
- **Fee Collection**: Generate protocol revenue from flash loan fees
- **Reentrancy Protection**: Secure external call handling

### 📈 **Leveraged Strategies**
- **Strategy Creation**: Build leveraged positions with upfront fee collection
- **Position Management**: Track leveraged exposures with debt accumulation
- **Risk Control**: Ensure proper collateralization of strategy positions

### 🛡️ **Risk Management**
- **Bad Debt Socialization**: Immediate loss distribution to prevent supplier flight
- **Liquidation Support**: Collateral seizure with protocol fee collection
- **Dust Management**: Clean up economically unviable small positions

## Mathematical Foundation

### 📊 **Scaled Amount System**
The protocol uses a scaled token system to track positions and interest accrual:

```rust
// Core scaling formulas:
scaled_amount = actual_amount / current_index
actual_amount = scaled_amount * current_index

// Interest accrual through index growth:
new_index = old_index * compound_factor
compound_factor = (1 + interest_rate)^time_delta
```

### 💰 **Interest Rate Model**
Dynamic interest rates based on capital utilization:

```rust
// Utilization calculation:
utilization = total_borrowed_value / total_supplied_value

// Kinked interest rate model:
if utilization <= kink_point:
    borrow_rate = base_rate + (utilization * slope1)
else:
    borrow_rate = base_rate + (kink * slope1) + ((utilization - kink) * slope2)

// Supplier rate calculation:
deposit_rate = borrow_rate * utilization * (1 - reserve_factor)
```

### 🔄 **Revenue Distribution**
Protocol revenue sharing between suppliers and treasury:

```rust
// Interest distribution:
total_interest = borrowed_scaled * (new_borrow_index - old_borrow_index)
supplier_share = total_interest * (1 - reserve_factor)
protocol_share = total_interest * reserve_factor

// Supply index update:
new_supply_index = old_supply_index + (supplier_share / total_scaled_supplied)
```

### ⚠️ **Bad Debt Handling**
Immediate loss socialization mechanism:

```rust
// Supply index reduction for bad debt:
loss_ratio = bad_debt_amount / total_supplied_value
new_supply_index = old_supply_index * (1 - loss_ratio)

// Each supplier's proportional loss:
supplier_loss = supplier_scaled_tokens * old_supply_index * loss_ratio
```

## Security Architecture

### 🔒 **Access Control**
- All functions restricted to `only_owner` (controller contract)
- Prevents direct user interaction with liquidity layer
- Ensures proper validation and authorization flows

### 🛡️ **Reentrancy Protection**
- Cache dropping before external calls in flash loans
- State synchronization before and after operations
- Atomic transaction requirements for flash loans

### ⚖️ **Precision Management**
- RAY precision (27 decimals) for internal calculations
- Scaled amounts prevent rounding manipulation
- Half-up rounding for consistent behavior

### 🎯 **Invariant Preservation**
- Global synchronization ensures accurate interest calculation
- Asset validation prevents wrong token operations
- Reserve validation maintains liquidity constraints

## Economic Model

### 💎 **Revenue Sources**
1. **Interest Spread**: Reserve factor percentage of borrower interest
2. **Flash Loan Fees**: Fees on temporary liquidity provision
3. **Strategy Fees**: Upfront fees for leveraged position creation
4. **Liquidation Fees**: Fees collected during collateral liquidation
5. **Dust Seizure**: Small uneconomical position cleanup

### 📈 **Growth Mechanisms**
- Supply index appreciation from borrower interest payments
- Compound interest accrual over time
- Automatic fee reinvestment through scaled token minting
- Revenue appreciation alongside supplier deposits

### ⚡ **Stability Features**
- Immediate bad debt socialization prevents bank runs
- Dynamic interest rates maintain healthy utilization
- Reserve requirements ensure withdrawal capacity
- Minimum index floors prevent total value collapse

## Usage Patterns

### 👤 **For Suppliers**
```rust
// Earn interest on deposits
supply(position, price) -> Updated position with scaled tokens
withdraw(caller, amount, position, ...) -> Assets + accrued interest
```

### 💳 **For Borrowers**
```rust
// Borrow against collateral
borrow(caller, amount, position, price) -> Debt position with interest
repay(caller, position, price) -> Reduced debt + overpayment refund
```

### 🔄 **For Arbitrageurs**
```rust
// Flash loan for atomic strategies
flash_loan(token, amount, target, endpoint, args, fees, price)
// Must repay loan + fees in same transaction
```

### 📊 **For Integrators**
```rust
// View current pool state
get_capital_utilisation() -> Current utilization ratio
get_borrow_rate() -> Current APR for borrowers
get_deposit_rate() -> Current APY for suppliers
```

This lending protocol provides a robust, mathematically sound foundation for decentralized
lending with advanced features like flash loans, leveraged strategies, and sophisticated
risk management mechanisms.
*/

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

pub use common_constants::{BPS_PRECISION, RAY_PRECISION, WAD_PRECISION};
use common_errors::{
    ERROR_FLASHLOAN_RESERVE_ASSET, ERROR_INSUFFICIENT_LIQUIDITY, ERROR_INVALID_ASSET,
    ERROR_STRATEGY_FEE_EXCEEDS_AMOUNT,
};
use common_structs::*;

use super::{cache::Cache, storage, utils, view};

#[multiversx_sc::module]
pub trait LiquidityModule:
    storage::Storage
    + utils::UtilsModule
    + common_events::EventsModule
    + common_math::SharedMathModule
    + common_rates::InterestRates
    + view::ViewModule
{
    /// Updates borrow and supply indexes based on elapsed time since last update.
    /// Synchronizes global pool state and emits market update event.
    /// Returns current market indexes.
    #[only_owner]
    #[endpoint(updateIndexes)]
    fn update_indexes(
        &self,
        price: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> MarketIndex<Self::Api> {
        let mut cache = Cache::new(self);

        self.global_sync(&mut cache);

        self.emit_market_update(&cache, price);

        MarketIndex {
            borrow_index_ray: cache.borrow_index_ray.clone(),
            supply_index_ray: cache.supply_index_ray.clone(),
        }
    }

    /// Processes asset deposit, adding to reserves and updating supplier position.
    /// Validates payment asset and converts amount to scaled tokens.
    /// Returns updated position with accrued interest.
    #[payable]
    #[only_owner]
    #[endpoint(supply)]
    fn supply(
        &self,
        mut position: AccountPosition<Self::Api>,
        price: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> AccountPosition<Self::Api> {
        let mut cache = Cache::new(self);

        let amount = self.payment_amount(&cache);
        require!(cache.is_same_asset(&position.asset_id), ERROR_INVALID_ASSET);

        self.global_sync(&mut cache);

        let scaled_amount = cache.calculate_scaled_supply(&amount);
        position.scaled_amount_ray += &scaled_amount;
        cache.supplied_ray += scaled_amount;

        self.emit_market_update(&cache, price);

        position
    }

    /// Borrows assets against collateral, transferring funds to caller.
    /// Validates sufficient liquidity and updates debt position.
    /// Returns updated borrow position.
    #[only_owner]
    #[endpoint(borrow)]
    fn borrow(
        &self,
        initial_caller: &ManagedAddress,
        amount: &ManagedDecimal<Self::Api, NumDecimals>,
        mut position: AccountPosition<Self::Api>,
        price: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> AccountPosition<Self::Api> {
        let mut cache = Cache::new(self);

        self.global_sync(&mut cache);

        require!(cache.is_same_asset(&position.asset_id), ERROR_INVALID_ASSET);
        require!(cache.has_reserves(amount), ERROR_INSUFFICIENT_LIQUIDITY);

        let scaled_amount = cache.calculate_scaled_borrow(amount);
        position.scaled_amount_ray += &scaled_amount;

        cache.borrowed_ray += scaled_amount;

        self.send_asset(&cache, amount, initial_caller);

        self.emit_market_update(&cache, price);

        position
    }

    /// Withdraws assets from supply position, handling liquidation fees if applicable.
    /// Supports full/partial withdrawals and burns corresponding scaled tokens.
    /// Returns updated position with reduced supply.
    #[only_owner]
    #[endpoint(withdraw)]
    fn withdraw(
        &self,
        initial_caller: &ManagedAddress,
        amount: ManagedDecimal<Self::Api, NumDecimals>,
        mut position: AccountPosition<Self::Api>,
        is_liquidation: bool,
        protocol_fee_opt: Option<ManagedDecimal<Self::Api, NumDecimals>>,
        price: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> AccountPosition<Self::Api> {
        let mut cache = Cache::new(self);

        self.global_sync(&mut cache);

        require!(cache.is_same_asset(&position.asset_id), ERROR_INVALID_ASSET);

        // 1. Determine gross withdrawal amounts (scaled and actual)
        let (scaled_withdrawal_amount_gross, mut amount_to_transfer_net) = self
            .calculate_gross_withdrawal_amounts(
                &cache,
                &position.scaled_amount_ray,
                &amount, // `amount` is the requested_amount_actual
            );

        self.process_liquidation_fee_details(
            &mut cache, // Pass cache as mutable
            is_liquidation,
            &protocol_fee_opt,
            &mut amount_to_transfer_net,
        );

        // 4. Check for sufficient reserves
        require!(
            cache.has_reserves(&amount_to_transfer_net),
            ERROR_INSUFFICIENT_LIQUIDITY
        );

        // 5. Update pool and position state by subtracting the determined scaled amount
        cache.supplied_ray -= &scaled_withdrawal_amount_gross;
        position.scaled_amount_ray -= &scaled_withdrawal_amount_gross;

        // 6. Send the net amount
        self.send_asset(&cache, &amount_to_transfer_net, initial_caller);

        // 7. Emit event and return position
        self.emit_market_update(&cache, price);
        position
    }

    /// Repays borrowed amount, reducing debt and refunding overpayments.
    /// Handles both full and partial repayments with interest included.
    /// Returns updated position with reduced debt.
    #[payable]
    #[only_owner]
    #[endpoint(repay)]
    fn repay(
        &self,
        initial_caller: ManagedAddress,
        mut position: AccountPosition<Self::Api>,
        price: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> AccountPosition<Self::Api> {
        let mut cache = Cache::new(self);
        let payment_amount = self.payment_amount(&cache);
        self.global_sync(&mut cache); // 2. Update indexes

        require!(cache.is_same_asset(&position.asset_id), ERROR_INVALID_ASSET);

        // 3. Determine scaled repayment amount and any overpayment
        let (amount_to_repay_scaled, over_paid_amount) =
            self.calculate_repayment_details(&cache, &position.scaled_amount_ray, &payment_amount);

        // 5. Subtract the determined scaled repayment amount from the position's scaled amount

        position.scaled_amount_ray -= &amount_to_repay_scaled;

        // 6. Subtract the same scaled amount from the total pool borrowed
        cache.borrowed_ray -= &amount_to_repay_scaled;
        // 7. Send back any overpaid amount
        self.send_asset(&cache, &over_paid_amount, &initial_caller);

        self.emit_market_update(&cache, price);

        position
    }

    /// Provides atomic flash loan with fee collection.
    /// Transfers amount to target contract, validates repayment, adds protocol revenue.
    /// Must be repaid with fees in same transaction.
    #[only_owner]
    #[endpoint(flashLoan)]
    fn flash_loan(
        &self,
        borrowed_token: &EgldOrEsdtTokenIdentifier,
        amount: &ManagedDecimal<Self::Api, NumDecimals>,
        contract_address: &ManagedAddress,
        endpoint: ManagedBuffer<Self::Api>,
        arguments: ManagedArgBuffer<Self::Api>,
        fees: &ManagedDecimal<Self::Api, NumDecimals>,
        price: &ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        let mut cache = Cache::new(self);
        self.global_sync(&mut cache);

        require!(cache.is_same_asset(borrowed_token), ERROR_INVALID_ASSET);
        require!(cache.has_reserves(amount), ERROR_FLASHLOAN_RESERVE_ASSET);

        // Calculate flash loan min repayment amount
        let required_repayment = self.rescale_half_up(
            &self.mul_half_up(amount, &(self.bps() + fees.clone()), RAY_PRECISION),
            cache.parameters.asset_decimals,
        );

        let asset = cache.parameters.asset_id.clone();
        // Prevent re entry attacks with loop flash loans
        drop(cache);
        let back_transfers = self
            .tx()
            .to(contract_address)
            .raw_call(endpoint)
            .arguments_raw(arguments)
            .egld_or_single_esdt(&asset, 0, amount.into_raw_units())
            .returns(ReturnsBackTransfersReset)
            .sync_call();

        let mut last_cache = Cache::new(self);

        let repayment =
            self.validate_flash_repayment(&last_cache, &back_transfers, &required_repayment);

        let protocol_fee = repayment - amount.clone();

        self.internal_add_protocol_revenue(&mut last_cache, protocol_fee);

        self.emit_market_update(&last_cache, price);
    }

    /// Creates leveraged position by borrowing with upfront fee deduction.
    /// User receives (amount - fee) but owes full amount plus interest.
    /// Returns updated position with increased debt.
    #[only_owner]
    #[endpoint(createStrategy)]
    fn create_strategy(
        &self,
        mut position: AccountPosition<Self::Api>,
        strategy_amount: &ManagedDecimal<Self::Api, NumDecimals>,
        strategy_fee: &ManagedDecimal<Self::Api, NumDecimals>,
        price: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> AccountPosition<Self::Api> {
        let mut cache = Cache::new(self);

        self.global_sync(&mut cache);

        require!(cache.is_same_asset(&position.asset_id), ERROR_INVALID_ASSET);

        require!(
            cache.has_reserves(strategy_amount),
            ERROR_INSUFFICIENT_LIQUIDITY
        );

        // Ensure fee doesn't exceed the strategy amount
        require!(
            strategy_fee <= strategy_amount,
            ERROR_STRATEGY_FEE_EXCEEDS_AMOUNT
        );

        // Calculate the amount to send to user (strategy amount minus fee)
        let amount_to_send = strategy_amount.clone() - strategy_fee.clone();

        // Only add the borrowed amount to debt (not the fee)
        let scaled_amount_to_add = cache.calculate_scaled_borrow(strategy_amount);

        position.scaled_amount_ray += &scaled_amount_to_add;

        cache.borrowed_ray += scaled_amount_to_add;

        self.internal_add_protocol_revenue(&mut cache, strategy_fee.clone());

        self.emit_market_update(&cache, price);

        // Send the net amount (after fee deduction) to the user
        self.send_asset(&cache, &amount_to_send, &self.blockchain().get_caller());

        position
    }

    /// Socializes bad debt by reducing supply index or seizes dust collateral.
    /// For borrow positions: applies loss to all suppliers immediately.
    /// For supply positions: adds dust to protocol revenue.
    #[only_owner]
    #[endpoint(seizePosition)]
    fn seize_position(
        &self,
        mut position: AccountPosition<Self::Api>,
        price: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> AccountPosition<Self::Api> {
        let mut cache = Cache::new(self);

        self.global_sync(&mut cache);

        require!(cache.is_same_asset(&position.asset_id), ERROR_INVALID_ASSET);

        match position.position_type {
            AccountPositionType::Borrow => {
                let current_debt_actual = cache.calculate_original_borrow_ray(&position.scaled_amount_ray);

                // Apply immediate supply index reduction for bad debt socialization
                self.apply_bad_debt_to_supply_index(&mut cache, current_debt_actual);

                // Remove debt from borrowed amounts
                cache.borrowed_ray -= &position.scaled_amount_ray;

                // Clear the position
                position.scaled_amount_ray = self.ray_zero();
            },
            AccountPositionType::Deposit => {
                // Add the dust collateral directly to protocol revenue
                cache.revenue_ray += &position.scaled_amount_ray;

                // Clear the user's position.
                position.scaled_amount_ray = self.ray_zero();
            },
            _ => {},
        }

        self.emit_market_update(&cache, price);

        position
    }

    /// Claims accumulated protocol revenue and transfers to owner.
    /// Revenue includes interest spreads, fees, and liquidation penalties.
    /// Limited by available reserves to preserve user withdrawals.
    #[only_owner]
    #[endpoint(claimRevenue)]
    fn claim_revenue(
        &self,
        price: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> EgldOrEsdtTokenPayment<Self::Api> {
        let mut cache = Cache::new(self);
        self.global_sync(&mut cache);

        let revenue_scaled = cache.revenue_ray.clone();

        if revenue_scaled == self.ray_zero() {
            self.emit_market_update(&cache, price);
            return EgldOrEsdtTokenPayment::new(cache.parameters.asset_id.clone(), 0, BigUint::zero());
        }

        let treasury_actual = cache.calculate_original_supply(&revenue_scaled);
        let current_reserves = cache.calculate_reserves();

        let amount_to_transfer = self.min(current_reserves.clone(), treasury_actual.clone());

        if amount_to_transfer > cache.zero {
            let controller = self.blockchain().get_caller();
            let payment = self.send_asset(&cache, &amount_to_transfer, &controller);

            // Determine the scaled amount to burn
            let scaled_to_burn = if amount_to_transfer >= treasury_actual {
                // Full claim: burn the exact revenue_scaled to avoid precision loss
                revenue_scaled.clone()
            } else {
                // Partial claim: calculate proportional scaled amount
                // scaled_to_burn = revenue_scaled * (amount_to_transfer / treasury_actual)
                let ratio_ray = self.div_half_up(&amount_to_transfer, &treasury_actual, RAY_PRECISION);
                self.mul_half_up(&revenue_scaled, &ratio_ray, RAY_PRECISION)
            };

            // Ensure we don't burn more than what exists (safety check)
            let actual_revenue_burn_ray = self.min(scaled_to_burn.clone(), cache.revenue_ray.clone());
            let actual_supplied_burn_ray = self.min(scaled_to_burn, cache.supplied_ray.clone());

            // Burn the calculated amount
            cache.revenue_ray -= &actual_revenue_burn_ray;
            cache.supplied_ray -= &actual_supplied_burn_ray;

            self.emit_market_update(&cache, price);
            return payment;
        }

        self.emit_market_update(&cache, price);
        EgldOrEsdtTokenPayment::new(cache.parameters.asset_id.clone(), 0, BigUint::zero())
    }
}
