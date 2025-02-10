multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use crate::errors::*;
use common_constants::DECIMAL_PRECISION;
use common_structs::*;

use super::{contexts::base::StorageCache, rates, storage, utils, view};

#[multiversx_sc::module]
pub trait LiquidityModule:
    storage::StorageModule
    + utils::UtilsModule
    + common_events::EventsModule
    + rates::InterestRateMath
    + view::ViewModule
{
    /// Updates the market's interest indexes based on elapsed time.
    ///
    /// This function updates both the borrow and supply indexes. It first creates a StorageCache to read the
    /// current state, then updates the indexes by computing an interest factor based on the elapsed time,
    /// and finally emits a market state event.
    ///
    /// # Parameters
    /// - `asset_price`: The current price of the asset.
    ///
    /// # Returns
    /// - Nothing.
    #[only_owner]
    #[endpoint(updateIndexes)]
    fn update_indexes(&self, asset_price: &BigUint) {
        let mut storage_cache = StorageCache::new(self);

        self.update_interest_indexes(&mut storage_cache);

        self.update_market_state_event(
            storage_cache.timestamp,
            &storage_cache.supply_index,
            &storage_cache.borrow_index,
            &storage_cache.reserves_amount,
            &storage_cache.supplied_amount,
            &storage_cache.borrowed_amount,
            &storage_cache.protocol_revenue,
            &storage_cache.pool_asset,
            asset_price,
        );
    }

    /// Updates an account position with accrued interest.
    ///
    /// This function takes an `AccountPosition` (passed from the Controller SC) and updates it by applying the
    /// accrued interest since the last update. Optionally, if an asset price is provided, it emits an event to update
    /// the market state.
    ///
    /// # Parameters
    /// - `position`: The account position to update.
    /// - `asset_price`: An optional asset price used for updating market state events.
    ///
    /// # Returns
    /// - `AccountPosition<Self::Api>`: The updated account position with accrued interest.

    #[only_owner]
    #[endpoint(updatePositionInterest)]
    fn update_position_with_interest(
        &self,
        mut position: AccountPosition<Self::Api>,
        asset_price: OptionalValue<BigUint>,
    ) -> AccountPosition<Self::Api> {
        let mut storage_cache = StorageCache::new(self);

        self.update_interest_indexes(&mut storage_cache);

        self.internal_update_position_with_interest(&mut position, &mut storage_cache);

        if asset_price.is_some() {
            self.update_market_state_event(
                storage_cache.timestamp,
                &storage_cache.supply_index,
                &storage_cache.borrow_index,
                &storage_cache.reserves_amount,
                &storage_cache.supplied_amount,
                &storage_cache.borrowed_amount,
                &storage_cache.protocol_revenue,
                &storage_cache.pool_asset,
                &asset_price.into_option().unwrap(),
            );
        }
        position
    }

    /// Supplies liquidity to the pool.
    ///
    /// This function is called by the Controller SC to deposit assets into the market. It validates the asset,
    /// updates the depositor's position with accrued interest, increases the pool's reserves and total supplied amount,
    /// and emits a market state event.
    ///
    /// # Parameters
    /// - `deposit_position`: The current account position of the supplier.
    /// - `asset_price`: The current price of the asset.
    ///
    /// # Returns
    /// - `AccountPosition<Self::Api>`: The updated deposit position.
    #[only_owner]
    #[payable("*")]
    #[endpoint(supply)]
    fn supply(
        &self,
        mut deposit_position: AccountPosition<Self::Api>,
        asset_price: &BigUint,
    ) -> AccountPosition<Self::Api> {
        let mut storage_cache = StorageCache::new(self);

        let (deposit_asset, deposit_amount) = self.call_value().egld_or_single_fungible_esdt();

        require!(
            deposit_asset.eq(&storage_cache.pool_asset),
            ERROR_INVALID_ASSET
        );

        self.update_interest_indexes(&mut storage_cache);

        self.internal_update_position_with_interest(&mut deposit_position, &mut storage_cache);

        deposit_position.amount += &deposit_amount;
        deposit_position.timestamp = storage_cache.timestamp;
        deposit_position.index = storage_cache.supply_index.into_raw_units().clone();

        let deposit_amount_dec = storage_cache.get_decimal_value(&deposit_amount);

        storage_cache.reserves_amount += &deposit_amount_dec;

        storage_cache.supplied_amount += deposit_amount_dec;

        self.update_market_state_event(
            storage_cache.timestamp,
            &storage_cache.supply_index,
            &storage_cache.borrow_index,
            &storage_cache.reserves_amount,
            &storage_cache.supplied_amount,
            &storage_cache.borrowed_amount,
            &storage_cache.protocol_revenue,
            &storage_cache.pool_asset,
            asset_price,
        );

        deposit_position
    }

    /// Borrows liquidity from the pool.
    ///
    /// This function is called by the Controller SC to borrow assets. It updates the borrower's position with accrued interest,
    /// ensures sufficient liquidity is available, increases the total borrowed amount, deducts reserves, and transfers tokens to the borrower.
    ///
    /// # Parameters
    /// - `initial_caller`: The address of the borrower.
    /// - `borrow_amount`: The amount to borrow.
    /// - `borrow_position`: The borrower's current account position.
    /// - `asset_price`: The current asset price.
    ///
    /// # Returns
    /// - `AccountPosition<Self::Api>`: The updated borrow position.
    #[only_owner]
    #[endpoint(borrow)]
    fn borrow(
        &self,
        initial_caller: &ManagedAddress,
        borrow_amount: &BigUint,
        mut borrow_position: AccountPosition<Self::Api>,
        asset_price: &BigUint,
    ) -> AccountPosition<Self::Api> {
        let mut storage_cache = StorageCache::new(self);

        self.update_interest_indexes(&mut storage_cache);
        self.internal_update_position_with_interest(&mut borrow_position, &mut storage_cache);

        borrow_position.amount += borrow_amount;
        borrow_position.timestamp = storage_cache.timestamp;
        borrow_position.index = storage_cache.borrow_index.into_raw_units().clone();

        let borrow_amount_dec = storage_cache.get_decimal_value(borrow_amount);

        require!(
            &storage_cache.get_reserves() >= &borrow_amount_dec,
            ERROR_INSUFFICIENT_LIQUIDITY
        );

        storage_cache.borrowed_amount += &borrow_amount_dec;
        storage_cache.reserves_amount -= &borrow_amount_dec;

        self.tx()
            .to(initial_caller)
            .egld_or_single_esdt(&storage_cache.pool_asset, 0, borrow_amount)
            .transfer_if_not_empty();

        self.update_market_state_event(
            storage_cache.timestamp,
            &storage_cache.supply_index,
            &storage_cache.borrow_index,
            &storage_cache.reserves_amount,
            &storage_cache.supplied_amount,
            &storage_cache.borrowed_amount,
            &storage_cache.protocol_revenue,
            &storage_cache.pool_asset,
            asset_price,
        );

        borrow_position
    }

    /// Withdraws liquidity from the pool via normal withdraw or liquidations
    ///
    /// # Parameters
    /// - `initial_caller`: The address of the caller.
    /// - `amount`: The amount of the asset to withdraw.
    /// - `mut deposit_position`: The position to update.
    /// - `is_liquidation`: Whether the withdrawal is part of a liquidation process.
    /// - `protocol_liquidation_fee`: The protocol liquidation fee (if applicable, if not will be 0).
    /// - `asset_price`: The current asset price used to update market state.
    ///
    /// # Returns
    /// - `AccountPosition<Self::Api>`: The updated deposit position.
    #[only_owner]
    #[endpoint(withdraw)]
    fn withdraw(
        &self,
        initial_caller: &ManagedAddress,
        amount: &BigUint,
        mut deposit_position: AccountPosition<Self::Api>,
        is_liquidation: bool,
        protocol_liquidation_fee: &BigUint,
        asset_price: &BigUint,
    ) -> AccountPosition<Self::Api> {
        let mut storage_cache = StorageCache::new(self);

        self.update_interest_indexes(&mut storage_cache);

        let requested_amount = storage_cache.get_decimal_value(amount);

        // Unaccrued interest for the wanted amount
        let extra_interest = self.compute_interest(
            requested_amount.clone(),
            &storage_cache.supply_index,
            &ManagedDecimal::from_raw_units(deposit_position.index.clone(), DECIMAL_PRECISION),
        );

        let total_withdraw = requested_amount.clone() + extra_interest.clone();
        // Withdrawal amount = initial wanted amount + Unaccrued interest for that amount (this has to be paid back to the user that requested the withdrawal)
        let mut principal_amount = requested_amount.clone();

        // Check if there is enough liquidity to cover the withdrawal
        require!(
            &storage_cache.get_reserves() >= &total_withdraw,
            ERROR_INSUFFICIENT_LIQUIDITY,
        );

        // Update the reserves amount
        storage_cache.reserves_amount -= &total_withdraw;

        let mut accumulated_interest =
            storage_cache.get_decimal_value(&deposit_position.accumulated_interest);

        // If the total withdrawal amount is greater than the accumulated interest, we need to subtract the accumulated interest from the withdrawal amount
        if principal_amount >= accumulated_interest {
            principal_amount -= accumulated_interest;
            accumulated_interest = storage_cache.zero.clone();
        } else {
            accumulated_interest -= principal_amount;
            principal_amount = storage_cache.zero.clone();
        }

        deposit_position.accumulated_interest = accumulated_interest.into_raw_units().clone();

        // Check if there is enough liquidity to cover the withdrawal after the interest was subtracted
        if principal_amount.gt(&storage_cache.zero) {
            require!(
                storage_cache.supplied_amount >= principal_amount,
                ERROR_INSUFFICIENT_LIQUIDITY
            );
            deposit_position.amount -= principal_amount.into_raw_units().clone();
            storage_cache.supplied_amount -= &principal_amount;
        }

        if is_liquidation {
            let protocol_fee = storage_cache.get_decimal_value(protocol_liquidation_fee);

            storage_cache.protocol_revenue += &protocol_fee;
            storage_cache.reserves_amount += &protocol_fee;
            let amount_after_fee = total_withdraw - protocol_fee;

            self.tx()
                .to(initial_caller)
                .egld_or_single_esdt(
                    &storage_cache.pool_asset,
                    0,
                    amount_after_fee.into_raw_units(),
                )
                .transfer_if_not_empty();
        } else {
            self.tx()
                .to(initial_caller)
                .egld_or_single_esdt(
                    &storage_cache.pool_asset,
                    0,
                    total_withdraw.into_raw_units(),
                )
                .transfer_if_not_empty();
        }

        self.update_market_state_event(
            storage_cache.timestamp,
            &storage_cache.supply_index,
            &storage_cache.borrow_index,
            &storage_cache.reserves_amount,
            &storage_cache.supplied_amount,
            &storage_cache.borrowed_amount,
            &storage_cache.protocol_revenue,
            &storage_cache.pool_asset,
            asset_price,
        );
        deposit_position
    }

    /// Processes a repayment for a borrow position.
    ///
    /// This function handles both full and partial repayments. It updates the borrower's position with accrued interest,
    /// splits the repayment into principal and interest, issues refunds if the repayment exceeds the total debt, and
    /// updates the pool state accordingly.
    ///
    /// # Parameters
    /// - `initial_caller`: The address of the caller.
    /// - `mut position`: The borrower's current account position.
    /// - `asset_price`: The current asset price used for updating market state.
    ///
    /// # Returns
    /// - `AccountPosition<Self::Api>`: The updated borrow position.
    #[only_owner]
    #[payable("*")]
    #[endpoint(repay)]
    fn repay(
        &self,
        initial_caller: ManagedAddress,
        mut position: AccountPosition<Self::Api>,
        asset_price: &BigUint,
    ) -> AccountPosition<Self::Api> {
        let mut storage_cache = StorageCache::new(self);
        let (received_asset, received_amount) = self.call_value().egld_or_single_fungible_esdt();

        require!(
            received_asset == storage_cache.pool_asset,
            ERROR_INVALID_ASSET
        );

        self.update_interest_indexes(&mut storage_cache);
        self.internal_update_position_with_interest(&mut position, &mut storage_cache);

        // Extract outstanding principal and interest from the position.
        let outstanding_principal = storage_cache.get_decimal_value(&position.amount);
        let outstanding_interest = storage_cache.get_decimal_value(&position.accumulated_interest);
        let total_debt = storage_cache.get_decimal_value(&position.get_total_amount());
        let repayment = storage_cache.get_decimal_value(&received_amount);

        // Calculate repayment breakdown.
        let (principal_repaid, interest_repaid, over_repaid) = self
            .calculate_interest_and_principal(
                &repayment,
                outstanding_interest,
                outstanding_principal,
                total_debt,
            );

        // Update the position:
        // - Reduce principal by the repaid principal.
        // - Reduce accumulated interest by the repaid interest.
        position.amount -= principal_repaid.into_raw_units();
        position.accumulated_interest -= interest_repaid.into_raw_units();

        // Update protocol bookkeeping:
        // - The net borrowed amount decreases only by the principal repaid.
        // - The reserves increase by the entire amount repaid (principal + interest).
        storage_cache.borrowed_amount -= &principal_repaid;
        storage_cache.reserves_amount += &(principal_repaid.clone() + interest_repaid.clone());

        self.tx()
            .to(&initial_caller)
            .egld_or_single_esdt(&received_asset, 0, &over_repaid)
            .transfer_if_not_empty();

        self.update_market_state_event(
            storage_cache.timestamp,
            &storage_cache.supply_index,
            &storage_cache.borrow_index,
            &storage_cache.reserves_amount,
            &storage_cache.supplied_amount,
            &storage_cache.borrowed_amount,
            &storage_cache.protocol_revenue,
            &storage_cache.pool_asset,
            asset_price,
        );

        position
    }

    /// Provides a flash loan from the pool.
    ///
    /// This function allows a flash loan operation. It deducts the loan amount from reserves, computes the fee,
    /// makes an external call to the borrower's contract, verifies that the repayment (including fee) meets the minimum requirement,
    /// and then updates the pool state accordingly.
    ///
    /// # Parameters
    /// - `borrowed_token`: The token to be flash loaned (must match the pool asset).
    /// - `amount`: The amount to flash loan.
    /// - `contract_address`: The address of the contract to be called.
    /// - `endpoint`: The endpoint of the target contract.
    /// - `arguments`: The arguments to pass to the target contract.
    /// - `fees`: The fee rate for the flash loan.
    /// - `asset_price`: The current asset price.
    ///
    /// # Returns
    /// - Nothing.
    #[only_owner]
    #[endpoint(flashLoan)]
    fn flash_loan(
        &self,
        borrowed_token: &EgldOrEsdtTokenIdentifier,
        amount: &BigUint,
        contract_address: &ManagedAddress,
        endpoint: ManagedBuffer<Self::Api>,
        arguments: ManagedArgBuffer<Self::Api>,
        fees: &BigUint,
        asset_price: &BigUint,
    ) {
        let mut storage_cache = StorageCache::new(self);
        self.update_interest_indexes(&mut storage_cache);

        let asset = storage_cache.pool_asset.clone();
        require!(borrowed_token == &asset, ERROR_INVALID_ASSET);

        let loaned_amount = storage_cache.get_decimal_value(amount);

        require!(
            &storage_cache.get_reserves() >= &loaned_amount,
            ERROR_FLASHLOAN_RESERVE_ASSET
        );

        storage_cache.reserves_amount -= &loaned_amount;

        // Calculate flash loan fee
        let flash_loan_fee = loaned_amount.clone().mul_with_precision(
            ManagedDecimal::from_raw_units(fees.clone(), DECIMAL_PRECISION),
            DECIMAL_PRECISION,
        );

        // Calculate minimum required amount to be paid back
        let min_repayment_amount = loaned_amount.clone() + flash_loan_fee;

        drop(storage_cache);
        let back_transfers = self
            .tx()
            .to(contract_address)
            .raw_call(endpoint)
            .arguments_raw(arguments)
            .payment(EgldOrEsdtTokenPayment::new(asset, 0, amount.clone()))
            .returns(ReturnsBackTransfers)
            .sync_call();

        let mut storage_cache_second = StorageCache::new(self);
        let is_egld = borrowed_token == &EgldOrEsdtTokenIdentifier::egld();
        let repayment_amount = if is_egld {
            let _repayment_amount =
                storage_cache_second.get_decimal_value(&back_transfers.total_egld_amount);

            require!(
                &storage_cache_second.pool_asset.is_egld(),
                ERROR_INVALID_FLASHLOAN_REPAYMENT
            );

            _repayment_amount
        } else {
            require!(
                back_transfers.esdt_payments.len() == 1,
                ERROR_INVALID_FLASHLOAN_REPAYMENT
            );
            let payment = back_transfers.esdt_payments.get(0);
            require!(
                &payment.token_identifier == &storage_cache_second.pool_asset,
                ERROR_INVALID_FLASHLOAN_REPAYMENT
            );

            let _repayment_amount = storage_cache_second.get_decimal_value(&payment.amount);
            _repayment_amount
        };

        require!(
            repayment_amount >= min_repayment_amount && loaned_amount <= repayment_amount,
            ERROR_INVALID_FLASHLOAN_REPAYMENT
        );

        storage_cache_second.reserves_amount += &loaned_amount;
        let revenue = repayment_amount - loaned_amount;

        storage_cache_second.protocol_revenue += &revenue;
        storage_cache_second.reserves_amount += &revenue;

        self.update_market_state_event(
            storage_cache_second.timestamp,
            &storage_cache_second.supply_index,
            &storage_cache_second.borrow_index,
            &storage_cache_second.reserves_amount,
            &storage_cache_second.supplied_amount,
            &storage_cache_second.borrowed_amount,
            &storage_cache_second.protocol_revenue,
            &storage_cache_second.pool_asset,
            asset_price,
        );
    }

    /// Simulates a flash loan strategy.
    ///
    /// This function is used internally to simulate a strategy where a flash loan is taken (without immediate repayment),
    /// the accrued fee is added as interest to the position, and the tokens are transferred to the caller.
    /// It returns the current borrow index and timestamp for later updates to the position.
    ///
    /// # Parameters
    /// - `token`: The token identifier (must match the pool asset).
    /// - `amount`: The amount to flash borrow for the strategy.
    /// - `fee`: The fee for the flash loan.
    /// - `asset_price`: The current asset price.
    ///
    /// # Returns
    /// - `(BigUint, u64)`: A tuple containing the latest borrow index and the current timestamp.
    #[only_owner]
    #[endpoint(createStrategy)]
    fn internal_create_strategy(
        &self,
        token: &EgldOrEsdtTokenIdentifier,
        amount: &BigUint,
        fee: &BigUint,
        asset_price: &BigUint,
    ) -> (BigUint, u64) {
        let mut storage_cache = StorageCache::new(self);
        self.update_interest_indexes(&mut storage_cache);

        let asset = storage_cache.pool_asset.clone();

        require!(token == &asset, ERROR_INVALID_ASSET);

        let strategy_amount = storage_cache.get_decimal_value(amount);

        let strategy_fee = storage_cache.get_decimal_value(fee);

        require!(
            &storage_cache.get_reserves() >= &strategy_amount,
            ERROR_INSUFFICIENT_LIQUIDITY
        );

        storage_cache.reserves_amount -= &strategy_amount;

        storage_cache.borrowed_amount += &strategy_amount;

        storage_cache.protocol_revenue += &strategy_fee;

        self.update_market_state_event(
            storage_cache.timestamp,
            &storage_cache.supply_index,
            &storage_cache.borrow_index,
            &storage_cache.reserves_amount,
            &storage_cache.supplied_amount,
            &storage_cache.borrowed_amount,
            &storage_cache.protocol_revenue,
            &storage_cache.pool_asset,
            asset_price,
        );

        self.tx()
            .to(self.blockchain().get_caller())
            .payment(EgldOrEsdtTokenPayment::new(
                asset,
                0,
                strategy_amount.into_raw_units().clone(),
            ))
            .transfer();

        // Return latest market index and timestamp to be updated in place in the new position, that at this point is not created due to the need of flash borrow the tokens
        (
            storage_cache.borrow_index.into_raw_units().clone(),
            storage_cache.timestamp,
        )
    }

    /// Adds external protocol revenue to the pool.
    ///
    /// This function accepts an external payment (e.g., from vault liquidations) in the pool asset,
    /// converts it to a ManagedDecimal using the pool's decimals, and adds it to both the protocol revenue and reserves.
    /// It then updates the market state event.
    ///
    /// # Parameters
    /// - `asset_price`: The current asset price.
    ///
    /// # Returns
    /// - Nothing.
    #[payable]
    #[only_owner]
    #[endpoint(addExternalProtocolRevenue)]
    fn add_external_protocol_revenue(&self, asset_price: &BigUint) {
        let (received_asset, received_amount) = self.call_value().egld_or_single_fungible_esdt();
        let mut storage_cache = StorageCache::new(self);

        require!(
            &received_asset == &storage_cache.pool_asset,
            ERROR_INVALID_ASSET
        );

        let decimal_received = storage_cache.get_decimal_value(&received_amount);

        storage_cache.protocol_revenue += &decimal_received;
        storage_cache.reserves_amount += &decimal_received;

        self.update_market_state_event(
            storage_cache.timestamp,
            &storage_cache.supply_index,
            &storage_cache.borrow_index,
            &storage_cache.reserves_amount,
            &storage_cache.supplied_amount,
            &storage_cache.borrowed_amount,
            &storage_cache.protocol_revenue,
            &storage_cache.pool_asset,
            asset_price,
        );
    }

    /// Claims the protocol revenue.
    ///
    /// This function updates the market's interest indexes, calculates the available protocol revenue (by taking the minimum
    /// of the protocol revenue and reserves), and transfers that amount to the protocol owner.
    /// It then emits an event with the updated market state.
    ///
    /// # Parameters
    /// - `asset_price`: The current asset price.
    ///
    /// # Returns
    /// - `EgldOrEsdtTokenPayment<Self::Api>`: The payment representing the claimed protocol revenue.
    #[only_owner]
    #[endpoint(claimRevenue)]
    fn claim_revenue(&self, asset_price: &BigUint) -> EgldOrEsdtTokenPayment<Self::Api> {
        let mut storage_cache = StorageCache::new(self);
        self.update_interest_indexes(&mut storage_cache);

        let revenue_biguint = if storage_cache.borrowed_amount == storage_cache.zero
            && storage_cache.supplied_amount == storage_cache.zero
        {
            let amount = self
                .blockchain()
                .get_sc_balance(&storage_cache.pool_asset, 0);

            storage_cache.protocol_revenue = storage_cache.zero.clone();
            storage_cache.reserves_amount = storage_cache.zero.clone();

            amount
        } else {
            let revenue = storage_cache.available_revenue();
            storage_cache.protocol_revenue -= &revenue;
            storage_cache.reserves_amount -= &revenue;

            revenue.into_raw_units().clone()
        };

        let payment =
            EgldOrEsdtTokenPayment::new(storage_cache.pool_asset.clone(), 0, revenue_biguint);

        self.tx()
            .to(self.blockchain().get_owner_address())
            .payment(&payment)
            .transfer_if_not_empty();

        self.update_market_state_event(
            storage_cache.timestamp,
            &storage_cache.supply_index,
            &storage_cache.borrow_index,
            &storage_cache.reserves_amount,
            &storage_cache.supplied_amount,
            &storage_cache.borrowed_amount,
            &storage_cache.protocol_revenue,
            &storage_cache.pool_asset,
            asset_price,
        );

        payment
    }
}
