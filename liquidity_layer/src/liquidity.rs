multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use common_errors::{
    ERROR_FLASHLOAN_RESERVE_ASSET, ERROR_INSUFFICIENT_LIQUIDITY, ERROR_INVALID_ASSET,
};
use common_structs::*;

use super::{cache::Cache, rates, storage, utils, view};

#[multiversx_sc::module]
pub trait LiquidityModule:
    storage::Storage
    + utils::UtilsModule
    + common_events::EventsModule
    + common_math::SharedMathModule
    + rates::InterestRates
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

        let scaled_amount = cache.get_scaled_supply_amount(&amount);
        position.scaled_amount += &scaled_amount;
        position.market_index = cache.supply_index.clone();
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

        let scaled_amount = cache.get_scaled_borrow_amount(amount);
        position.scaled_amount += &scaled_amount;
        position.market_index = cache.borrow_index.clone();

        cache.borrowed += scaled_amount;

        self.send_asset(&cache, amount, initial_caller);

        self.emit_market_update(&cache, price);

        position
    }

    /// Withdraws assets from the pool, supporting both normal withdrawals and liquidations.
    ///
    /// **Purpose**: Allows suppliers to retrieve their assets or handles liquidation events, adjusting for interest and fees.
    ///
    /// **Process**:
    /// 1. Updates global indexes.
    /// 2. Caps the withdrawal amount to the position's total (principal + interest).
    /// 3. Calculates principal, interest, and total withdrawal amount, applying liquidation fees if applicable.
    /// 4. Verifies sufficient reserves and supplied amounts.
    /// 5. Updates the pool state and position, then transfers the withdrawal amount.
    /// 6. Emits a market state event.
    ///
    /// # Arguments
    /// - `initial_caller`: The address withdrawing funds (`ManagedAddress`).
    /// - `amount`: Requested withdrawal amount (`ManagedDecimal<Self::Api, NumDecimals>`).
    /// - `position`: The supplier's position (`AccountPosition<Self::Api>`).
    /// - `is_liquidation`: Indicates if this is a liquidation event (`bool`).
    /// - `protocol_fee_opt`: Optional liquidation fee (`Option<ManagedDecimal<Self::Api, NumDecimals>>`).
    /// - `price`: The asset price for market update (`ManagedDecimal<Self::Api, NumDecimals>`).
    ///
    /// # Returns
    /// - `AccountPosition<Self::Api>`: The updated position after withdrawal.
    ///
    /// **Security Considerations**: Caps withdrawal amounts and uses `require!` to prevent over-withdrawal from reserves or supplied totals.
    /// Can only be called by the owner (via controller contract).
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
        position.market_index = cache.supply_index.clone();

        // 6. Send the net amount
        self.send_asset(&cache, &amount_to_transfer_net, initial_caller);

        // 7. Emit event and return position
        self.emit_market_update(&cache, price);
        position
    }

    /// Processes a repayment for a borrow position, handling full or partial repayments.
    ///
    /// **Purpose**: Reduces a borrower's debt by allocating repayment to principal and interest, refunding any overpayment.
    ///
    /// **Process**:
    /// 1. Retrieves and validates the repayment amount.
    /// 2. Updates global indexes (which also updates the position's implicit accrued interest).
    /// 3. Calculates the actual current debt of the position. Based on this and the payment amount,
    ///    it determines the scaled amount to repay and any overpaid amount.
    /// 4. Updates the position's scaled debt and the pool's total scaled borrowed amount.
    /// 5. Refunds any overpaid amount to the `initial_caller`.
    /// 6. Emits a market state event.
    ///
    /// # Arguments
    /// - `initial_caller`: The address repaying the debt (`ManagedAddress`).
    /// - `position`: The borrower's current position (`AccountPosition<Self::Api>`).
    /// - `price`: The asset price for market update (`ManagedDecimal<Self::Api, NumDecimals>`).
    ///
    /// # Returns
    /// - `AccountPosition<Self::Api>`: The updated position after repayment.
    ///
    /// **Security Considerations**: Ensures asset validity via `get_payment_amount` and handles overpayments to prevent fund loss.
    /// Can only be called by the owner (via controller contract).
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
        position.market_index = cache.borrow_index.clone();

        // 6. Subtract the same scaled amount from the total pool borrowed
        cache.borrowed -= &amount_to_repay_scaled;
        // 7. Send back any overpaid amount
        self.send_asset(&cache, &over_paid_amount, &initial_caller);

        self.emit_market_update(&cache, price);

        position
    }

    /// Provides a flash loan from the pool, enabling temporary borrowing without collateral.
    ///
    /// **Purpose**: Facilitates flash loans for strategies like arbitrage, requiring repayment with fees in the same transaction.
    ///
    /// **Process**:
    /// 1. Validates the borrowed token (`cache.params.asset_id`) and reserve availability for the `amount`.
    /// 2. Sends the `amount` to the `contract_address` for the external call. Protocol revenue is not yet affected, and total borrowed is not yet increased.
    /// 3. Computes the `required_repayment` (loan `amount` + `fees`).
    /// 4. Drops the `cache` to prevent reentrancy, then executes the external call to `contract_address` and `endpoint` with `arguments`.
    /// 5. Validates that the `back_transfers` (repayment) meet or exceed `required_repayment`.
    /// 6. Calculates the `protocol_fee` from `repayment - amount`.
    /// 7. Adds the `protocol_fee` (rescaled to RAY) to `cache.revenue`.
    /// 8. Emits a market state event.
    ///
    /// # Arguments
    /// - `borrowed_token`: The token to borrow (`EgldOrEsdtTokenIdentifier`).
    /// - `amount`: The amount to borrow (`ManagedDecimal<Self::Api, NumDecimals>`).
    /// - `contract_address`: The target contract address (`ManagedAddress`).
    /// - `endpoint`: The endpoint to call (`ManagedBuffer<Self::Api>`).
    /// - `arguments`: Arguments for the endpoint (`ManagedArgBuffer<Self::Api>`).
    /// - `fees`: The flash loan fee rate (`ManagedDecimal<Self::Api, NumDecimals>`).
    /// - `price`: The asset price for market update (`ManagedDecimal<Self::Api, NumDecimals>`).
    ///
    /// **Security Considerations**: Drops the cache before external calls to prevent reentrancy and uses `require!` to enforce asset and reserve checks.
    /// Can only be called by the owner (via controller contract).
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

        last_cache.revenue += protocol_fee.rescale(RAY_PRECISION);

        self.emit_market_update(&last_cache, price);
    }

    /// Simulates a flash loan strategy by borrowing assets without immediate repayment.
    ///
    /// **Purpose**: Enables internal strategies requiring temporary asset access, adding fees to protocol revenue.
    ///
    /// **Process**:
    /// 1. Validates the asset in the `position` and reserve availability for `strategy_amount`.
    /// 2. Calculates `effective_initial_debt = strategy_amount + strategy_fee`.
    /// 3. Increases the `position`'s scaled debt and the pool's total scaled `borrowed` amount by the scaled `effective_initial_debt`.
    /// 4. Adds `strategy_fee` (rescaled to RAY) to `cache.revenue`.
    /// 5. Transfers the `strategy_amount` to the caller.
    /// 6. Emits a market state event.
    ///
    /// # Arguments
    /// - `position`: The account's position for the asset being borrowed for the strategy (`AccountPosition<Self::Api>`).
    /// - `strategy_amount`: The amount to borrow for the strategy (`ManagedDecimal<Self::Api, NumDecimals>`).
    /// - `strategy_fee`: The fee for the strategy (`ManagedDecimal<Self::Api, NumDecimals>`).
    /// - `price`: The asset price for market update (`ManagedDecimal<Self::Api, NumDecimals>`).
    ///
    /// # Returns
    /// - `AccountPosition<Self::Api>`: The updated account position reflecting the new strategy debt.
    ///
    /// **Security Considerations**: Ensures asset validity and sufficient reserves with `require!` checks.
    /// Can only be called by the owner (via controller contract)
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

        let scaled_amount_to_add = cache.get_scaled_borrow_amount(&effective_initial_debt);

        position.scaled_amount += &scaled_amount_to_add;
        position.market_index = cache.borrow_index.clone();

        cache.borrowed += scaled_amount_to_add;

        cache.revenue += strategy_fee.rescale(RAY_PRECISION);

        self.emit_market_update(&cache, price);

        self.send_asset(&cache, strategy_amount, &self.blockchain().get_caller());

        position
    }

    /// Adds external revenue to the pool, such as from vault liquidations or other sources.
    /// It will first pay the bad debt and then add the remaining amount to revenue and reserves.
    ///
    /// **Purpose**: Increases protocol revenue and reserves with funds from external sources.
    ///
    /// **Process**:
    /// 1. Retrieves and validates the payment `amount`.
    /// 2. If `amount` is less than or equal to `cache.bad_debt`, it reduces `cache.bad_debt` by `amount`.
    /// 3. Otherwise, `cache.bad_debt` is cleared, and the `remaining_amount` (after covering bad debt) is added to `cache.revenue` (rescaled to RAY).
    /// 4. Pool reserves are implicitly increased by the incoming payment.
    /// 5. Emits a market state event.
    ///
    /// # Arguments
    /// - `price`: The asset price for market update (`ManagedDecimal<Self::Api, NumDecimals>`).
    ///
    /// **Security Considerations**: Validates the asset via `get_payment_amount` to ensure compatibility with the pool.
    /// Can only be called by the owner (via controller contract).
    #[payable]
    #[only_owner]
    #[endpoint(addProtocolRevenue)]
    fn add_protocol_revenue(&self, price: &ManagedDecimal<Self::Api, NumDecimals>) {
        let mut cache = Cache::new(self);
        let amount = self.get_payment_amount(&cache);

        if amount <= cache.bad_debt {
            cache.bad_debt -= &amount;
        } else {
            let remaining_amount = amount - cache.bad_debt.clone();
            cache.bad_debt = cache.zero.clone();
            cache.revenue += remaining_amount.rescale(RAY_PRECISION);
        }

        self.emit_market_update(&cache, price);
    }

    /// Adds bad debt to the pool, such as from liquidations.
    ///
    /// **Purpose**: Increases protocol bad debt.
    ///
    /// **Reason**: After liquidations, when bad debt is left over, the position will infinitely accrue interest that will never be repaid.
    /// This function allows the protocol to collect this bad debt and add it the bad debt tracker which will paid over time from the protocol revenue and suppliers interest.
    ///
    /// **Process**:
    /// 1. Updates global indexes.
    /// 2. Adds the amount to bad debt.
    /// 3. Subtracts the amount from borrowed.
    /// 4. Emits a market state event.
    ///
    /// # Arguments
    /// - `position`: The position to add bad debt to (`AccountPosition<Self::Api>`).
    /// - `price`: The asset price for market update (`ManagedDecimal<Self::Api, NumDecimals>`).
    ///
    /// # Returns
    /// - `AccountPosition<Self::Api>`: The updated position after adding bad debt.
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

        let current_debt_actual = cache.get_original_borrow_amount(&position.scaled_amount);

        cache.bad_debt += &current_debt_actual;

        cache.borrowed -= &position.scaled_amount;

        position.scaled_amount = self.ray_zero();
        position.market_index = cache.borrow_index.clone();

        self.emit_market_update(&cache, price);

        position
    }

    /// Seizes dust collateral from the pool, adding it to protocol revenue.
    ///
    /// **Purpose**: Allows the protocol to collect dust collateral from the pool, increasing revenue.
    ///
    /// **Reason**: After liquidations, when bad debt is left over, the supplied position might still have a dust balance that is not liquidatable.
    /// This function allows the protocol to collect this dust and add it to revenue, while clearing the position and the infinite interest that would be accrued on it.
    ///
    /// **Process**:
    /// 1. Updates global indexes.
    /// 2. Calculates the `current_dust_actual` (original value of the position's supplied collateral).
    /// 3. If `current_dust_actual` is less than or equal to `cache.bad_debt`, it reduces `cache.bad_debt` by `current_dust_actual`.
    /// 4. Otherwise, `cache.bad_debt` is cleared, and the `remaining_amount_actual` (after covering bad debt) is added to `cache.revenue` (rescaled to RAY).
    /// 5. Subtracts the position's scaled amount from `cache.supplied`.
    /// 6. Clears the position's scaled amount and updates its market index.
    /// 7. Emits a market state event.
    ///
    /// # Arguments
    /// - `position`: The position to seize dust collateral from (`AccountPosition<Self::Api>`).
    /// - `price`: The asset price for market update (`ManagedDecimal<Self::Api, NumDecimals>`).
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

        let current_dust_actual = cache.get_original_supply_amount(&position.scaled_amount);

        if current_dust_actual <= cache.bad_debt {
            cache.bad_debt -= &current_dust_actual;
        } else {
            let remaining_amount_actual = current_dust_actual - cache.bad_debt.clone();
            cache.bad_debt = cache.zero.clone();
            cache.revenue += remaining_amount_actual.rescale(RAY_PRECISION);
        }

        cache.supplied -= &position.scaled_amount;

        position.scaled_amount = self.ray_zero();
        position.market_index = cache.supply_index.clone();

        self.emit_market_update(&cache, price);

        position
    }

    /// Claims accumulated protocol revenue and transfers it to the owner.
    ///
    /// **Purpose**: Allows the protocol owner to withdraw earned revenue, ensuring accurate state updates.
    ///
    /// **Process**:
    /// 1. Updates global indexes.
    /// 2. Calculates available revenue, using contract balance if the pool is empty (borrowed and supplied are zero).
    /// 3. Updates reserves and revenue, then transfers the amount to the owner.
    /// 4. Emits a market state event.
    ///
    /// # Arguments
    /// - `price`: The asset price for market update (`ManagedDecimal<Self::Api, NumDecimals>`).
    ///
    /// # Returns
    /// - `EgldOrEsdtTokenPayment<Self::Api>`: The payment object representing the claimed revenue.
    ///
    /// **Security Considerations**: Handles edge cases (empty pool) by fallback to contract balance, ensuring all revenue is claimable.
    #[only_owner]
    #[endpoint(claimRevenue)]
    fn claim_revenue(
        &self,
        price: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> EgldOrEsdtTokenPayment<Self::Api> {
        let mut cache = Cache::new(self);
        self.global_sync(&mut cache);

        // Calculate Target Claim Amount based on accounted revenue
        let mut target_claim_asset_decimals =
            self.rescale_half_up(&cache.revenue, cache.params.asset_decimals);

        let actual_reserves_asset_decimals = cache.get_reserves();

        // Check if the pool is effectively empty of user funds
        let pool_is_empty = cache.supplied == self.ray_zero() && cache.borrowed == self.ray_zero();

        if pool_is_empty {
            // If the pool is empty, all actual reserves can be considered revenue.
            target_claim_asset_decimals = actual_reserves_asset_decimals.clone();
        }

        // Determine Actual Transferable Amount (capped by actual reserves)
        let amount_to_transfer_asset_decimals = self.get_min(
            target_claim_asset_decimals.clone(),
            actual_reserves_asset_decimals,
        );

        let controller = self.blockchain().get_caller();
        let mut payment =
            EgldOrEsdtTokenPayment::new(cache.params.asset_id.clone(), 0, BigUint::zero());

        if amount_to_transfer_asset_decimals > cache.zero {
            payment = self.send_asset(&cache, &amount_to_transfer_asset_decimals, &controller);

            if pool_is_empty {
                cache.revenue = self.ray_zero();
            } else {
                let transferred_amount_ray =
                    self.rescale_half_up(&amount_to_transfer_asset_decimals, RAY_PRECISION);
                if cache.revenue >= transferred_amount_ray {
                    cache.revenue -= transferred_amount_ray;
                } else {
                    cache.revenue = self.ray_zero();
                }
            }
        }

        self.emit_market_update(&cache, price);
        payment
    }
}
