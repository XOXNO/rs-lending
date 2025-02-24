multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use common_errors::{
    ERROR_FLASHLOAN_RESERVE_ASSET, ERROR_INSUFFICIENT_LIQUIDITY, ERROR_INVALID_ASSET,
};
use common_structs::*;

use super::{contexts::base::Cache, rates, storage, utils, view};

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
    fn update_indexes(&self, price: &ManagedDecimal<Self::Api, NumDecimals>) {
        let mut cache = Cache::new(self);

        self.global_sync(&mut cache);

        self.emit_market_update(&cache, price);
    }

    /// Synchronizes a user's account position with accrued interest since the last update.
    ///
    /// **Purpose**: Ensures a user's deposit or borrow position reflects the latest interest accrued, typically called before operations like repayments or withdrawals.
    ///
    /// **Process**:
    /// 1. Creates a `Cache` and updates global indexes via `global_sync`.
    /// 2. Updates the position with accrued interest using `position_sync`.
    /// 3. If a price is provided, emits a market state event.
    ///
    /// # Arguments
    /// - `position`: The user's account position to update (`AccountPosition<Self::Api>`).
    /// - `price`: Optional asset price for emitting a market update (`OptionalValue<ManagedDecimal<Self::Api, NumDecimals>>`).
    ///
    /// # Returns
    /// - `AccountPosition<Self::Api>`: The updated position with accrued interest applied.
    ///
    /// **Security Considerations**: Restricted to the owner (via controller contract) to ensure controlled updates.
    #[only_owner]
    #[endpoint(updatePositionInterest)]
    fn sync_position_interest(
        &self,
        mut position: AccountPosition<Self::Api>,
        price: OptionalValue<ManagedDecimal<Self::Api, NumDecimals>>,
    ) -> AccountPosition<Self::Api> {
        let mut cache = Cache::new(self);

        self.global_sync(&mut cache);

        self.position_sync(&mut position, &mut cache);

        if price.is_some() {
            self.emit_market_update(&cache, &price.into_option().unwrap());
        }
        position
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
    #[only_owner]
    #[payable("*")]
    #[endpoint(supply)]
    fn supply(
        &self,
        mut position: AccountPosition<Self::Api>,
        price: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> AccountPosition<Self::Api> {
        let mut cache = Cache::new(self);

        let amount = self.get_payment_amount(&cache);

        self.global_sync(&mut cache);

        self.position_sync(&mut position, &mut cache);

        position.principal_amount += &amount;

        cache.reserves += &amount;
        cache.supplied += amount;

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
        self.position_sync(&mut position, &mut cache);

        position.principal_amount += amount;

        require!(cache.has_reserves(amount), ERROR_INSUFFICIENT_LIQUIDITY);

        cache.borrowed += amount;
        cache.reserves -= amount;

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
        mut amount: ManagedDecimal<Self::Api, NumDecimals>,
        mut position: AccountPosition<Self::Api>,
        is_liquidation: bool,
        protocol_fee_opt: Option<ManagedDecimal<Self::Api, NumDecimals>>,
        price: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> AccountPosition<Self::Api> {
        let mut cache = Cache::new(self);

        self.global_sync(&mut cache);

        self.cap_withdrawal_amount(&mut amount, &position);
        // Calculate the withdrawal amount as well the principal and interest to be subtracted from the position and the reserves
        let (principal, interest, to_withdraw) = self.calc_withdrawal_amounts(
            &amount,
            &position,
            &mut cache,
            is_liquidation,
            protocol_fee_opt,
        );

        require!(
            cache.has_reserves(&to_withdraw),
            ERROR_INSUFFICIENT_LIQUIDITY
        );
        require!(cache.has_supplied(&principal), ERROR_INSUFFICIENT_LIQUIDITY);

        cache.reserves -= &to_withdraw;
        cache.supplied -= &principal;
        position.principal_amount -= &principal;
        position.interest_accrued -= &interest;

        self.send_asset(&cache, &to_withdraw, initial_caller);

        self.emit_market_update(&cache, price);
        position
    }

    /// Processes a repayment for a borrow position, handling full or partial repayments.
    ///
    /// **Purpose**: Reduces a borrower's debt by allocating repayment to principal and interest, refunding any overpayment.
    ///
    /// **Process**:
    /// 1. Retrieves and validates the repayment amount.
    /// 2. Updates global indexes and the position with accrued interest.
    /// 3. Splits the repayment into principal, interest, and overpayment using `split_repay`.
    /// 4. Updates the position and pool state, refunding any overpayment.
    /// 5. Emits a market state event.
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
    #[only_owner]
    #[payable("*")]
    #[endpoint(repay)]
    fn repay(
        &self,
        initial_caller: ManagedAddress,
        mut position: AccountPosition<Self::Api>,
        price: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> AccountPosition<Self::Api> {
        let mut cache = Cache::new(self);
        let amount = self.get_payment_amount(&cache);

        self.global_sync(&mut cache);
        self.position_sync(&mut position, &mut cache);

        let (principal, interest, over) = self.split_repay(&amount, &position, &mut cache);

        // Update the position:
        // - Reduce principal by the repaid principal.
        // - Reduce accumulated interest by the repaid interest.
        position.principal_amount -= &principal;
        position.interest_accrued -= &interest;

        // Update protocol bookkeeping:
        // - The net borrowed amount decreases only by the principal repaid.
        // - The reserves increase by the entire amount repaid (principal + interest).
        cache.borrowed -= &principal;
        cache.reserves += &(principal + interest);

        self.send_asset(&cache, &over, &initial_caller);

        self.emit_market_update(&cache, price);

        position
    }

    /// Provides a flash loan from the pool, enabling temporary borrowing without collateral.
    ///
    /// **Purpose**: Facilitates flash loans for strategies like arbitrage, requiring repayment with fees in the same transaction.
    ///
    /// **Process**:
    /// 1. Validates the borrowed token and reserve availability.
    /// 2. Deducts the loan amount from reserves.
    /// 3. Computes the required repayment with fees.
    /// 4. Drops the cache to prevent reentrancy, executes an external call, and validates repayment.
    /// 5. Updates reserves and protocol revenue with the repayment and fee.
    /// 6. Emits a market state event.
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

        cache.reserves -= amount;

        // Calculate flash loan min repayment amount
        let required_repayment = self
            .mul_half_up(amount, &(self.bps() + fees.clone()), RAY_PRECISION)
            .rescale(cache.pool_params.asset_decimals);

        let asset = cache.pool_asset.clone();
        // Prevent re entry attacks with loop flash loans
        drop(cache);
        let back_transfers = self
            .tx()
            .to(contract_address)
            .raw_call(endpoint)
            .arguments_raw(arguments)
            .egld_or_single_esdt(&asset, 0, amount.into_raw_units())
            .returns(ReturnsBackTransfers)
            .sync_call();

        let mut storage_cache_second = Cache::new(self);

        let repayment = self.validate_flash_repayment(
            &storage_cache_second,
            &back_transfers,
            amount,
            &required_repayment,
        );

        storage_cache_second.reserves += amount;
        let protocol_fee = repayment - amount.clone();

        storage_cache_second.revenue += &protocol_fee;
        storage_cache_second.reserves += &protocol_fee;

        self.emit_market_update(&storage_cache_second, price);
    }

    /// Simulates a flash loan strategy by borrowing assets without immediate repayment.
    ///
    /// **Purpose**: Enables internal strategies requiring temporary asset access, adding fees to protocol revenue.
    ///
    /// **Process**:
    /// 1. Validates the token and reserve availability.
    /// 2. Deducts from reserves, increases borrowed amount, and adds fees to revenue.
    /// 3. Transfers the borrowed amount to the caller.
    /// 4. Emits a market state event and returns the borrow index and timestamp.
    ///
    /// # Arguments
    /// - `token`: The token to borrow (`EgldOrEsdtTokenIdentifier`).
    /// - `strategy_amount`: The amount to borrow (`ManagedDecimal<Self::Api, NumDecimals>`).
    /// - `strategy_fee`: The fee for the strategy (`ManagedDecimal<Self::Api, NumDecimals>`).
    /// - `price`: The asset price for market update (`ManagedDecimal<Self::Api, NumDecimals>`).
    ///
    /// # Returns
    /// - `(ManagedDecimal<Self::Api, NumDecimals>, u64)`: The current borrow index and timestamp for later position updates.
    ///
    /// **Security Considerations**: Ensures asset validity and sufficient reserves with `require!` checks.
    /// Can only be called by the owner (via controller contract)
    #[only_owner]
    #[endpoint(createStrategy)]
    fn create_flash_strategy(
        &self,
        token: &EgldOrEsdtTokenIdentifier,
        strategy_amount: &ManagedDecimal<Self::Api, NumDecimals>,
        strategy_fee: &ManagedDecimal<Self::Api, NumDecimals>,
        price: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> (ManagedDecimal<Self::Api, NumDecimals>, u64) {
        let mut cache = Cache::new(self);

        self.global_sync(&mut cache);

        require!(cache.is_same_asset(token), ERROR_INVALID_ASSET);

        require!(
            cache.has_reserves(strategy_amount),
            ERROR_INSUFFICIENT_LIQUIDITY
        );

        cache.reserves -= strategy_amount;

        cache.borrowed += strategy_amount;

        cache.revenue += strategy_fee;

        self.emit_market_update(&cache, price);

        self.send_asset(&cache, &strategy_amount, &self.blockchain().get_caller());

        // Return latest market index and timestamp to be updated in place in the new position, that at this point is not created due to the need of flash borrow the tokens
        (cache.borrow_index.clone(), cache.timestamp)
    }

    /// Adds external revenue to the pool, such as from vault liquidations.
    ///
    /// **Purpose**: Increases protocol revenue and reserves with funds from external sources.
    ///
    /// **Process**:
    /// 1. Retrieves and validates the payment amount.
    /// 2. Adds the amount to both revenue and reserves.
    /// 3. Emits a market state event.
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

        cache.revenue += &amount;
        cache.reserves += &amount;

        self.emit_market_update(&cache, price);
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

        let revenue_biguint = if cache.borrowed == cache.zero && cache.supplied == cache.zero {
            let amount = self.blockchain().get_sc_balance(&cache.pool_asset, 0);

            cache.revenue = cache.zero.clone();
            cache.reserves = cache.zero.clone();

            cache.get_decimal_value(&amount)
        } else {
            let revenue = cache.available_revenue();
            cache.revenue -= &revenue;
            cache.reserves -= &revenue;

            revenue
        };

        let payment = self.send_asset(
            &cache,
            &revenue_biguint,
            &self.blockchain().get_owner_address(),
        );

        self.emit_market_update(&cache, price);

        payment
    }
}
