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

use common_errors::{
    ERROR_FLASHLOAN_RESERVE_ASSET, ERROR_INSUFFICIENT_LIQUIDITY, ERROR_INVALID_ASSET,
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
    /// Updates the market's borrow and supply indexes based on elapsed time since the last update.
    ///
    /// **Purpose**: Ensures the pool's interest calculations reflect the latest state by computing an interest factor based on time elapsed and applying it to the borrow and supply indexes.
    ///
    /// **Process**:
    /// 1. Creates a `Cache` to snapshot the current pool state.
    /// 2. Calls `global_sync` to update the borrow and supply indexes.
    /// 3. Emits a market state event to log the updated state.
    ///
    /// # Arguments
    /// - `price`: The current price of the pool asset (`ManagedDecimal<Self::Api, NumDecimals>`).
    ///
    /// **Security Considerations**: Restricted to the owner (via controller contract) to ensure controlled updates.
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
            borrow_index: cache.borrow_index.clone(),
            supply_index: cache.supply_index.clone(),
        }
    }

    /// Supplies assets to the lending pool, increasing reserves and the supplier's position.
    ///
    /// **Purpose**: Allows users to deposit assets into the pool to earn interest, increasing available liquidity.
    ///
    /// **Process**:
    /// 1. Retrieves and validates the payment amount using `get_payment_amount`.
    /// 2. Updates global indexes and the supplier's position with accrued interest.
    /// 3. Adds the supplied amount to the position, reserves, and total supplied.
    /// 4. Emits a market state event.
    ///
    /// # Arguments
    /// - `position`: The supplier's current position (`AccountPosition<Self::Api>`).
    /// - `price`: The asset price for market update (`ManagedDecimal<Self::Api, NumDecimals>`).
    ///
    /// # Returns
    /// - `AccountPosition<Self::Api>`: The updated position with the new supply amount.
    ///
    /// **Security Considerations**: Validates the asset type via `get_payment_amount` to ensure only the pool's asset is supplied.
    /// Can only be called by the owner (via controller contract).
    #[payable]
    #[only_owner]
    #[endpoint(supply)]
    fn supply(
        &self,
        mut position: AccountPosition<Self::Api>,
        price: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> AccountPosition<Self::Api> {
        let mut cache = Cache::new(self);

        let amount = self.get_payment_amount(&cache);
        require!(cache.is_same_asset(&position.asset_id), ERROR_INVALID_ASSET);

        self.global_sync(&mut cache);

        let scaled_amount = cache.scaled_supply(&amount);
        position.scaled_amount += &scaled_amount;
        cache.supplied += scaled_amount;

        self.emit_market_update(&cache, price);

        position
    }

    /// Borrows assets from the pool against a user's collateral.
    ///
    /// **Purpose**: Enables users to borrow assets, deducting from reserves and increasing their debt.
    ///
    /// **Process**:
    /// 1. Updates global indexes and the borrower's position with accrued interest.
    /// 2. Verifies sufficient reserves are available.
    /// 3. Increases the borrower's debt and total borrowed, then deducts from reserves.
    /// 4. Transfers the borrowed amount to the caller.
    /// 5. Emits a market state event.
    ///
    /// # Arguments
    /// - `initial_caller`: The borrower's address (`ManagedAddress`).
    /// - `amount`: The amount to borrow (`ManagedDecimal<Self::Api, NumDecimals>`).
    /// - `position`: The borrower's current position (`AccountPosition<Self::Api>`).
    /// - `price`: The asset price for market update (`ManagedDecimal<Self::Api, NumDecimals>`).
    ///
    /// # Returns
    /// - `AccountPosition<Self::Api>`: The updated borrow position.
    ///
    /// **Security Considerations**: Uses `require!` to ensure sufficient liquidity, preventing over-borrowing.
    /// Can only be called by the owner (via controller contract).
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

        let scaled_amount = cache.scaled_borrow(amount);
        position.scaled_amount += &scaled_amount;

        cache.borrowed += scaled_amount;

        self.send_asset(&cache, amount, initial_caller);

        self.emit_market_update(&cache, price);

        position
    }

    /// Withdraws assets from the pool, supporting both normal withdrawals and liquidations.
    ///
    /// **Purpose**: Enables suppliers to redeem their scaled tokens for underlying assets,
    /// realizing accumulated interest, or facilitates liquidation of collateral.
    ///
    /// **Mathematical Process**:
    /// 1. **Global Sync**: Update indexes to include latest interest
    /// 2. **Current Value Calculation**: `current_value = scaled_position * supply_index`
    /// 3. **Withdrawal Amount Determination**:
    ///    - Full withdrawal: `amount = min(requested, current_value)`
    ///    - Partial withdrawal: `amount = requested`
    /// 4. **Scaling Conversion**: `scaled_to_burn = amount / supply_index`
    /// 5. **Fee Processing** (if liquidation): `net_amount = gross_amount - liquidation_fee`
    /// 6. **State Updates**:
    ///    - `position.scaled_amount -= scaled_to_burn`
    ///    - `total_supplied -= scaled_to_burn`
    ///
    /// **Interest Realization Formula**:
    /// ```
    /// // Interest earned since supply:
    /// interest = scaled_tokens * (current_supply_index - supply_index_at_deposit)
    /// total_withdrawal = principal + interest - fees
    /// ```
    ///
    /// **Liquidation Fee Mechanism**:
    /// During liquidations, a protocol fee may be deducted:
    /// ```
    /// net_transfer = gross_withdrawal - liquidation_fee
    /// protocol_revenue += liquidation_fee (scaled)
    /// ```
    ///
    /// **Reserve Validation**:
    /// Ensures sufficient contract balance exists for the withdrawal.
    ///
    /// # Arguments
    /// - `initial_caller`: Recipient of withdrawn assets
    /// - `amount`: Requested withdrawal amount
    /// - `position`: User's current supply position
    /// - `is_liquidation`: Flag for liquidation-specific processing
    /// - `protocol_fee_opt`: Optional liquidation fee to deduct
    /// - `price`: Asset price for event logging
    ///
    /// # Returns
    /// - Updated position with reduced scaled supply
    ///
    /// **Security Considerations**:
    /// - Amount capping prevents over-withdrawal
    /// - Reserve validation ensures liquidity availability
    /// - Fee validation prevents insufficient withdrawal amounts
    /// - Scaled burning maintains pool integrity
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
            .determine_gross_withdrawal_amounts(
                &cache,
                &position.scaled_amount,
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
        cache.supplied -= &scaled_withdrawal_amount_gross;
        position.scaled_amount -= &scaled_withdrawal_amount_gross;

        // 6. Send the net amount
        self.send_asset(&cache, &amount_to_transfer_net, initial_caller);

        // 7. Emit event and return position
        self.emit_market_update(&cache, price);
        position
    }

    /// Processes a repayment for a borrow position, handling full or partial repayments with overpayment refunds.
    ///
    /// **Purpose**: Reduces borrower debt by burning scaled debt tokens proportional to the
    /// payment amount, automatically handling interest and refunding overpayments.
    ///
    /// **Mathematical Process**:
    /// 1. **Global Sync**: Update borrow index to accrue latest interest
    /// 2. **Current Debt Calculation**: `current_debt = scaled_debt * current_borrow_index`
    /// 3. **Repayment Allocation**:
    ///    - If `payment >= current_debt`: Full repayment + overpayment
    ///    - If `payment < current_debt`: Partial repayment
    /// 4. **Scaling Conversion**:
    ///    - Full: `scaled_to_burn = entire_scaled_position`
    ///    - Partial: `scaled_to_burn = payment / current_borrow_index`
    /// 5. **State Updates**:
    ///    - `position.scaled_amount -= scaled_to_burn`
    ///    - `total_borrowed -= scaled_to_burn`
    /// 6. **Overpayment Handling**: `refund = max(0, payment - current_debt)`
    ///
    /// **Debt Reduction Formula**:
    /// ```
    /// // Current total debt including interest:
    /// total_debt = scaled_debt * current_borrow_index
    ///
    /// // Proportion of debt being repaid:
    /// repayment_ratio = min(1, payment_amount / total_debt)
    /// scaled_to_burn = scaled_debt * repayment_ratio
    /// ```
    ///
    /// **Interest Payment Mechanism**:
    /// Interest is automatically included in the debt calculation through
    /// the borrow index, so borrowers pay accrued interest proportionally.
    ///
    /// **Overpayment Protection**:
    /// Excess payments are automatically refunded to prevent loss of funds:
    /// ```
    /// if payment > total_debt:
    ///     actual_payment = total_debt
    ///     refund = payment - total_debt
    /// ```
    ///
    /// # Arguments
    /// - `initial_caller`: Address to receive any overpayment refund
    /// - `position`: User's current borrow position
    /// - `price`: Asset price for event logging
    ///
    /// # Returns
    /// - Updated position with reduced scaled debt
    ///
    /// **Security Considerations**:
    /// - Asset validation prevents wrong token repayments
    /// - Overpayment refunds prevent accidental loss
    /// - Scaled burning maintains precise debt tracking
    /// - Global sync ensures fair interest calculation
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
        let payment_amount = self.get_payment_amount(&cache);
        self.global_sync(&mut cache); // 2. Update indexes

        require!(cache.is_same_asset(&position.asset_id), ERROR_INVALID_ASSET);

        // 3. Determine scaled repayment amount and any overpayment
        let (amount_to_repay_scaled, over_paid_amount) =
            self.determine_repayment_details(&cache, &position.scaled_amount, &payment_amount);

        // 5. Subtract the determined scaled repayment amount from the position's scaled amount

        position.scaled_amount -= &amount_to_repay_scaled;

        // 6. Subtract the same scaled amount from the total pool borrowed
        cache.borrowed -= &amount_to_repay_scaled;
        // 7. Send back any overpaid amount
        self.send_asset(&cache, &over_paid_amount, &initial_caller);

        self.emit_market_update(&cache, price);

        position
    }

    /// Provides a flash loan from the pool, enabling temporary borrowing without collateral within a single transaction.
    ///
    /// **Purpose**: Facilitates atomic operations like arbitrage, liquidations, or complex DeFi strategies
    /// by providing instant liquidity that must be repaid with fees in the same transaction.
    ///
    /// **Mathematical Process**:
    /// 1. **Liquidity Validation**: Verify `amount <= available_reserves`
    /// 2. **Fee Calculation**: `required_repayment = amount * (1 + fee_rate)`
    /// 3. **Asset Transfer**: Send `amount` to target contract
    /// 4. **External Execution**: Call target contract with provided parameters
    /// 5. **Repayment Validation**: Ensure `back_transfer >= required_repayment`
    /// 6. **Protocol Revenue**: `fee = repayment - amount`, add to treasury
    ///
    /// **Flash Loan Fee Formula**:
    /// ```
    /// fee_basis_points = fees_parameter  // e.g., 30 = 0.30%
    /// fee_rate = fee_basis_points / 10000
    /// required_repayment = loan_amount * (1 + fee_rate)
    /// protocol_fee = repayment_amount - loan_amount
    /// ```
    ///
    /// **Atomic Transaction Requirement**:
    /// The entire flash loan operation must complete in a single transaction:
    /// ```
    /// 1. Borrow assets from pool
    /// 2. Execute arbitrary logic (arbitrage, liquidation, etc.)
    /// 3. Repay loan + fees to pool
    /// 4. Transaction reverts if repayment insufficient
    /// ```
    ///
    /// **Reentrancy Protection**:
    /// Cache is dropped before external call to prevent state manipulation:
    /// ```
    /// // State snapshot before external call
    /// drop(cache);
    /// // External call execution
    /// let result = contract.call();
    /// // Fresh state validation after call
    /// validate_repayment();
    /// ```
    ///
    /// **Revenue Distribution**:
    /// Flash loan fees are added directly to protocol treasury as scaled supply tokens.
    ///
    /// # Arguments
    /// - `borrowed_token`: Asset to borrow (must match pool asset)
    /// - `amount`: Loan amount in asset decimals
    /// - `contract_address`: Target contract for strategy execution
    /// - `endpoint`: Function to call on target contract
    /// - `arguments`: Parameters for the external call
    /// - `fees`: Fee rate in basis points (e.g., 30 = 0.30%)
    /// - `price`: Asset price for event logging
    ///
    /// **Security Considerations**:
    /// - Reentrancy protection via cache dropping
    /// - Asset validation prevents wrong token loans
    /// - Reserve validation ensures liquidity availability
    /// - Repayment validation enforces fee collection
    /// - Atomic execution prevents partial failures
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
            cache.params.asset_decimals,
        );

        let asset = cache.params.asset_id.clone();
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

    /// Creates a leveraged strategy position by borrowing assets with upfront fee collection.
    ///
    /// **Purpose**: Enables creation of leveraged positions (e.g., leveraged staking, yield farming)
    /// where users borrow assets to amplify their exposure, with strategy fees collected upfront.
    ///
    /// **Mathematical Process**:
    /// 1. **Liquidity Validation**: Verify `strategy_amount <= available_reserves`
    /// 2. **Total Debt Calculation**: `total_debt = strategy_amount + strategy_fee`
    /// 3. **Scaling Conversion**: `scaled_debt = total_debt / current_borrow_index`
    /// 4. **Position Update**: `new_scaled_debt = old_scaled_debt + scaled_debt`
    /// 5. **Pool State Update**: `total_borrowed += scaled_debt`
    /// 6. **Revenue Collection**: Add `strategy_fee` to protocol treasury
    /// 7. **Asset Transfer**: Send `strategy_amount` to user for strategy execution
    ///
    /// **Strategy Debt Structure**:
    /// ```
    /// // User receives strategy_amount but owes total_debt:
    /// assets_received = strategy_amount
    /// debt_created = strategy_amount + strategy_fee
    /// protocol_fee = strategy_fee (collected immediately)
    /// ```
    ///
    /// **Interest Accrual on Total Debt**:
    /// The entire debt (including the upfront fee) accrues interest over time:
    /// ```
    /// initial_scaled_debt = (strategy_amount + strategy_fee) / borrow_index_at_creation
    /// future_debt = initial_scaled_debt * current_borrow_index
    /// total_repayment_needed = future_debt
    /// ```
    ///
    /// **Leveraged Position Example**:
    /// ```
    /// User wants 2x leverage on 100 USDC:
    /// 1. User supplies 100 USDC as collateral
    /// 2. Strategy borrows 100 USDC (+ 1 USDC fee)
    /// 3. User receives 100 USDC to buy more assets
    /// 4. User's debt: 101 USDC (accruing interest)
    /// 5. User's exposure: 200 USDC worth of assets
    /// ```
    ///
    /// **Fee Collection Model**:
    /// Strategy fees are collected upfront and added to protocol revenue,
    /// providing immediate income while the borrowed amount generates ongoing interest.
    ///
    /// # Arguments
    /// - `position`: User's existing borrow position for this asset
    /// - `strategy_amount`: Amount to borrow for strategy execution
    /// - `strategy_fee`: Upfront fee charged for strategy creation
    /// - `price`: Asset price for event logging
    ///
    /// # Returns
    /// - Updated position with increased scaled debt (amount + fee)
    ///
    /// **Security Considerations**:
    /// - Liquidity validation prevents over-borrowing
    /// - Asset validation ensures correct token
    /// - Upfront fee collection reduces protocol risk
    /// - Debt includes fee to prevent undercollateralization
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

        let effective_initial_debt = strategy_amount.clone() + strategy_fee.clone();

        let scaled_amount_to_add = cache.scaled_borrow(&effective_initial_debt);

        position.scaled_amount += &scaled_amount_to_add;

        cache.borrowed += scaled_amount_to_add;

        self.internal_add_protocol_revenue(&mut cache, strategy_fee.clone());

        self.emit_market_update(&cache, price);

        self.send_asset(&cache, strategy_amount, &self.blockchain().get_caller());

        position
    }

    /// Socializes bad debt by immediately reducing the supply index, distributing losses among all suppliers.
    ///
    /// **Purpose**: Prevents supplier flight during bad debt events by immediately socializing
    /// losses rather than allowing infinite interest accrual on uncollectable debt.
    ///
    /// **Problem Being Solved**:
    /// Traditional bad debt handling creates a race condition where rational suppliers
    /// would withdraw immediately upon learning of bad debt, leaving remaining suppliers
    /// to bear disproportionate losses. This mechanism prevents such dynamics.
    ///
    /// **Mathematical Process**:
    /// 1. **Current Debt Calculation**: `bad_debt = scaled_debt * current_borrow_index`
    /// 2. **Total Supply Value**: `total_value = total_scaled_supply * current_supply_index`
    /// 3. **Loss Ratio Calculation**: `loss_ratio = bad_debt / total_value`
    /// 4. **Supply Index Reduction**: `new_supply_index = old_supply_index * (1 - loss_ratio)`
    /// 5. **Debt Removal**: Remove scaled debt from total borrowed
    /// 6. **Position Clearing**: Set position scaled amount to zero
    ///
    /// **Socialization Formula**:
    /// ```
    /// // Immediate loss distribution:
    /// total_supplier_value = total_scaled_supplied * supply_index
    /// loss_per_unit = bad_debt_amount / total_supplier_value
    /// new_supply_index = old_supply_index * (1 - loss_per_unit)
    ///
    /// // Each supplier's loss:
    /// supplier_loss = supplier_scaled_tokens * old_supply_index * loss_per_unit
    /// supplier_new_value = supplier_scaled_tokens * new_supply_index
    /// ```
    ///
    /// **Prevention of Supplier Flight**:
    /// By applying losses immediately and proportionally, no supplier can avoid
    /// their share by withdrawing after bad debt is discovered.
    ///
    /// **Economic Rationale**:
    /// - Spreads losses fairly among all participants
    /// - Maintains pool stability during stress events
    /// - Prevents bank-run scenarios
    /// - Eliminates need for bad debt tracking/provisioning
    ///
    /// **Impact on Existing Positions**:
    /// All existing supply positions instantly lose value proportional to the bad debt,
    /// but their scaled token amounts remain unchanged.
    ///
    /// # Arguments
    /// - `position`: The insolvent borrow position to clear
    /// - `price`: Asset price for event logging
    ///
    /// # Returns
    /// - Cleared position with zero scaled debt
    ///
    /// **Security Considerations**:
    /// - Immediate application prevents gaming/arbitrage
    /// - Proportional distribution ensures fairness
    /// - Supply index has minimum floor to prevent total collapse
    #[only_owner]
    #[endpoint(addBadDebt)]
    fn add_bad_debt(
        &self,
        mut position: AccountPosition<Self::Api>,
        price: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> AccountPosition<Self::Api> {
        let mut cache = Cache::new(self);

        self.global_sync(&mut cache);

        require!(cache.is_same_asset(&position.asset_id), ERROR_INVALID_ASSET);

        let current_debt_actual = cache.original_borrow(&position.scaled_amount);

        // Apply immediate supply index reduction for bad debt socialization
        self.apply_bad_debt_to_supply_index(&mut cache, &current_debt_actual);

        // Remove debt from borrowed amounts
        cache.borrowed -= &position.scaled_amount;

        // Clear the position
        position.scaled_amount = self.ray_zero();

        self.emit_market_update(&cache, price);

        position
    }

    /// Seizes dust collateral from a position, transferring it directly to protocol revenue.
    ///
    /// **Purpose**: Enables collection of economically unviable small collateral amounts
    /// that remain after liquidations, converting them to protocol revenue.
    ///
    /// **Use Case Scenario**:
    /// After a liquidation and bad debt socialization, a user's supply position may
    /// have a small remaining balance that is:
    /// - Too small to be economically liquidated (gas costs > value)
    /// - Below minimum transaction thresholds
    /// - Creates accounting complexity if left unclaimed
    ///
    /// **Mathematical Process**:
    /// 1. **Global Sync**: Update indexes to current state
    /// 2. **Direct Transfer**: `protocol_revenue += position.scaled_amount`
    /// 3. **Position Clearing**: `position.scaled_amount = 0`
    /// 4. **Supply Maintenance**: Total supplied remains unchanged (dust becomes revenue)
    ///
    /// **Revenue Conversion**:
    /// ```
    /// // Dust collateral becomes protocol revenue:
    /// dust_value = scaled_dust * current_supply_index
    /// protocol_revenue_scaled += scaled_dust
    /// user_position_scaled = 0
    /// ```
    ///
    /// **Economic Justification**:
    /// Small balances create operational overhead and user confusion.
    /// Converting them to protocol revenue:
    /// - Simplifies account management
    /// - Reduces storage requirements
    /// - Provides clean closure of positions
    /// - Generates modest protocol income
    ///
    /// **Dust Threshold Considerations**:
    /// While this function doesn't enforce a threshold, it's typically used for
    /// amounts that are economically unviable for users to withdraw due to
    /// transaction costs exceeding the value.
    /// The treshold is set inside the controller contract that is the only one able to call this function.
    ///
    /// **Impact on Pool Accounting**:
    /// - User's scaled position: reduced to zero
    /// - Protocol revenue: increased by dust amount
    /// - Total supplied: unchanged (internal transfer)
    /// - Pool liquidity: unchanged
    ///
    /// # Arguments
    /// - `position`: Supply position containing dust collateral
    /// - `price`: Asset price for event logging
    ///
    /// # Returns
    /// - Cleared position with zero scaled supply
    ///
    /// **Security Considerations**:
    /// - Should only be used for genuinely uneconomical amounts
    /// - Requires careful governance to prevent abuse
    /// - Position is completely cleared (irreversible)
    #[only_owner]
    #[endpoint(seizeDustCollateral)]
    fn seize_dust_collateral(
        &self,
        mut position: AccountPosition<Self::Api>,
        price: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> AccountPosition<Self::Api> {
        let mut cache = Cache::new(self);

        self.global_sync(&mut cache);

        require!(cache.is_same_asset(&position.asset_id), ERROR_INVALID_ASSET);

        // Add the dust collateral directly to protocol revenue
        cache.revenue += &position.scaled_amount;

        // Clear the user's position.
        position.scaled_amount = self.ray_zero();

        self.emit_market_update(&cache, price);

        position
    }

    /// Claims accumulated protocol revenue and transfers it to the owner.
    ///
    /// **Purpose**: Enables protocol owner to withdraw earned revenue from various sources
    /// including interest spreads, flash loan fees, strategy fees, and liquidation fees.
    ///
    /// **Mathematical Process**:
    /// 1. **Global Sync**: Update indexes to include latest revenue
    /// 2. **Revenue Calculation**: `revenue_actual = revenue_scaled * current_supply_index`
    /// 3. **Available Balance Check**: Determine withdrawable amount based on reserves
    /// 4. **Empty Pool Handling**: If pool has no user funds, claim entire contract balance
    /// 5. **Proportional Withdrawal**: If partial withdrawal, burn proportional scaled revenue
    /// 6. **Transfer Execution**: Send claimed amount to controller
    ///
    /// **Revenue Sources**:
    /// ```
    /// Protocol Revenue += {
    ///     interest_spread: (borrow_rate - supply_rate) * borrowed_amount * time
    ///     flash_loan_fees: loan_amount * flash_fee_rate
    ///     strategy_fees: strategy_amount * strategy_fee_rate
    ///     liquidation_fees: liquidated_amount * liquidation_fee_rate
    ///     dust_collateral: seized_dust_amounts
    /// }
    /// ```
    ///
    /// **Empty Pool Logic**:
    /// When `user_supplied_scaled = 0` and `borrowed_scaled = 0`:
    /// - All contract balance belongs to protocol
    /// - Claim entire available balance
    /// - Useful for final revenue extraction
    ///
    /// **Partial Withdrawal Mechanics**:
    /// ```
    /// // When reserves < total_revenue:
    /// withdrawal_ratio = available_reserves / total_revenue_value
    /// scaled_to_burn = revenue_scaled * withdrawal_ratio
    /// remaining_revenue_scaled = revenue_scaled - scaled_to_burn
    /// ```
    ///
    /// **Reserve Constraints**:
    /// Revenue withdrawal is limited by available contract balance to ensure
    /// user withdrawals remain possible.
    ///
    /// **Accounting Precision**:
    /// Uses scaled amounts to maintain precision in revenue tracking,
    /// preventing rounding errors from accumulating over time.
    ///
    /// **Revenue Realization**:
    /// Revenue is stored as scaled supply tokens that appreciate with the supply index,
    /// ensuring protocol revenue grows alongside user deposits.
    ///
    /// # Arguments
    /// - `price`: Asset price for event logging
    ///
    /// # Returns
    /// - Payment object representing the claimed revenue amount
    ///
    /// **Security Considerations**:
    /// - Reserve validation ensures pool liquidity preservation
    /// - Empty pool detection prevents user fund seizure
    /// - Proportional burning maintains accurate accounting
    /// - Only callable by owner (controller contract)
    #[only_owner]
    #[endpoint(claimRevenue)]
    fn claim_revenue(
        &self,
        price: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> EgldOrEsdtTokenPayment<Self::Api> {
        let mut cache = Cache::new(self);
        self.global_sync(&mut cache);

        // 1. Treasury (protocol) funds are held in `cache.revenue` **scaled** units.
        let treasury_scaled = cache.revenue.clone();

        // Nothing to do if treasury has no balance.
        if treasury_scaled == self.ray_zero() {
            return EgldOrEsdtTokenPayment::new(cache.params.asset_id.clone(), 0, BigUint::zero());
        }

        // Convert the full treasury position to asset-decimals using the *current* supply index.
        let treasury_actual = cache.original_supply(&treasury_scaled);

        // Contract balance that is really available right now.
        let current_reserves = cache.get_reserves();

        // A pool is considered empty of user funds when all supplied tokens belong to the treasury
        // and there is no outstanding debt.
        let user_supplied_scaled = if cache.supplied >= treasury_scaled {
            cache.supplied.clone() - treasury_scaled.clone()
        } else {
            self.ray_zero()
        };
        let pool_is_empty =
            user_supplied_scaled == self.ray_zero() && cache.borrowed == self.ray_zero();

        // Decide whether we can withdraw the entire treasury balance.
        let full_withdrawable = current_reserves >= treasury_actual;

        // Amount that will actually be transferred to the controller.
        let amount_to_transfer = if pool_is_empty {
            // When pool is empty, everything in the contract is by definition revenue or dust – take it all.
            current_reserves.clone()
        } else if full_withdrawable {
            treasury_actual.clone()
        } else {
            current_reserves.clone()
        };

        let mut payment =
            EgldOrEsdtTokenPayment::new(cache.params.asset_id.clone(), 0, BigUint::zero());
        if amount_to_transfer > cache.zero {
            // 1. Transfer the funds to the controller
            let controller = self.blockchain().get_caller();
            payment = self.send_asset(&cache, &amount_to_transfer, &controller);

            // 2. Burn / reduce the scaled treasury balance
            if pool_is_empty || full_withdrawable {
                // We removed the entire treasury position – burn it without any rescaling loss.
                cache.supplied -= &treasury_scaled;
                cache.revenue = cache.zero.clone();
            } else {
                // Partial withdrawal – compute exact scaled portion of what we transferred.
                let scaled_burn = cache.scaled_supply(&amount_to_transfer);
                cache.revenue -= &scaled_burn;
                cache.supplied -= &scaled_burn;
            }
        }

        self.emit_market_update(&cache, price);
        payment
    }
}
