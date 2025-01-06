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
    /// Updates the indexes of the pool.
    ///
    /// # Parameters
    /// - `asset_price`: The price of the asset.
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

    /// Updates the position with interest.
    ///
    /// # Parameters
    /// - `position`: The position to update.
    /// - `asset_price`: The price of the asset, used to update the market state event.
    ///
    /// # Returns
    /// - `AccountPosition<Self::Api>`: The updated position.
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
    /// # Parameters
    /// - `deposit_position`: The position to update.
    /// - `asset_price`: The price of the asset, used to update the market state event.
    ///
    /// # Payment
    /// - `*`: The asset to deposit, has to be the same as the pool asset.
    ///
    /// # Returns
    /// - `AccountPosition<Self::Api>`: The updated position.
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

        let deposit_amount_dec =
            ManagedDecimal::from_raw_units(deposit_amount, storage_cache.pool_params.decimals);

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
    /// # Parameters
    /// - `initial_caller`: The address of the caller.
    /// - `borrow_amount`: The amount of the asset to borrow.
    /// - `existing_borrow_position`: The position to update.
    /// - `asset_price`: The price of the asset, used to update the market state event.
    ///
    /// # Payment
    /// - `*`: The asset to borrow, has to be the same as the pool asset.
    ///
    /// # Returns
    /// - `AccountPosition<Self::Api>`: The updated position.
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

        let borrow_amount_dec = ManagedDecimal::from_raw_units(
            borrow_amount.clone(),
            storage_cache.pool_params.decimals,
        );

        require!(
            &storage_cache.get_reserves() >= &borrow_amount_dec,
            ERROR_INSUFFICIENT_LIQUIDITY
        );

        storage_cache.borrowed_amount += &borrow_amount_dec;
        storage_cache.reserves_amount -= &borrow_amount_dec;

        self.tx()
            .to(initial_caller)
            .payment(EgldOrEsdtTokenPayment::new(
                storage_cache.pool_asset.clone(),
                0,
                borrow_amount.clone(),
            ))
            .transfer();

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

    /// Withdraws liquidity from the pool.
    ///
    /// # Parameters
    /// - `initial_caller`: The address of the caller.
    /// - `amount`: The amount of the asset to withdraw.
    /// - `mut deposit_position`: The position to update.
    /// - `is_liquidation`: Whether the withdrawal is a liquidation.
    /// - `protocol_liquidation_fee`: The protocol liquidation fee.
    /// - `asset_price`: The price of the asset, used to update the market state event.
    ///
    /// # Returns
    /// - `AccountPosition<Self::Api>`: The updated position.
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

        let requested_amount =
            ManagedDecimal::from_raw_units(amount.clone(), storage_cache.pool_params.decimals);

        // Unaccrued interest for the wanted amount
        let extra_interest = self.compute_interest(
            requested_amount.clone(),
            &storage_cache.supply_index,
            &ManagedDecimal::from_raw_units(deposit_position.index.clone(), DECIMAL_PRECISION),
        );

        let total_withdraw = requested_amount.clone() + extra_interest.clone();
        // Withdrawal amount = initial wanted amount + Unaccrued interest for that amount (this has to be paid back to the user that requested the withdrawal)
        let mut principal_amount = requested_amount;

        // Check if there is enough liquidity to cover the withdrawal
        require!(
            &storage_cache.get_reserves() >= &total_withdraw,
            ERROR_INSUFFICIENT_LIQUIDITY
        );

        // Update the reserves amount
        storage_cache.reserves_amount -= &total_withdraw;

        let mut accumulated_interest = ManagedDecimal::from_raw_units(
            deposit_position.accumulated_interest.clone(),
            storage_cache.pool_params.decimals,
        );

        let zero =
            ManagedDecimal::from_raw_units(BigUint::zero(), storage_cache.pool_params.decimals);

        // If the total withdrawal amount is greater than the accumulated interest, we need to subtract the accumulated interest from the withdrawal amount

        if principal_amount >= accumulated_interest {
            principal_amount -= accumulated_interest;
            accumulated_interest = zero.clone();
        } else {
            accumulated_interest -= principal_amount;
            principal_amount = zero.clone();
        }

        deposit_position.accumulated_interest = accumulated_interest.into_raw_units().clone();

        // Check if there is enough liquidity to cover the withdrawal after the interest was subtracted
        if principal_amount.gt(&zero) {
            require!(
                storage_cache.supplied_amount >= principal_amount,
                ERROR_INSUFFICIENT_LIQUIDITY
            );
            deposit_position.amount -= principal_amount.into_raw_units().clone();
            storage_cache.supplied_amount -= &principal_amount;
        }

        if is_liquidation {
            let protocol_fee = ManagedDecimal::from_raw_units(
                protocol_liquidation_fee.clone(),
                storage_cache.pool_params.decimals,
            );

            storage_cache.protocol_revenue += &protocol_fee;
            storage_cache.reserves_amount += &protocol_fee;
            let amount_after_fee = total_withdraw - protocol_fee;

            self.tx()
                .to(initial_caller)
                .payment(EgldOrEsdtTokenPayment::new(
                    storage_cache.pool_asset.clone(),
                    0,
                    amount_after_fee.into_raw_units().clone(),
                ))
                .transfer_if_not_empty();
        } else {
            self.tx()
                .to(initial_caller)
                .payment(EgldOrEsdtTokenPayment::new(
                    storage_cache.pool_asset.clone(),
                    0,
                    total_withdraw.into_raw_units().clone(),
                ))
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

    /// Repays a borrow position.
    ///
    /// # Parameters
    /// - `initial_caller`: The address of the caller.
    /// - `mut borrow_position`: The position to update.
    /// - `asset_price`: The price of the asset, used to update the market state event.
    ///
    /// # Payment
    /// - `*`: The asset to repay, has to be the same as the pool asset.
    ///
    /// # Returns
    /// - `AccountPosition<Self::Api>`: The updated position.
    /// -  Extra amount is sent back to the caller if the repayment is greater than the total owed.
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

        let total_borrorwed = ManagedDecimal::from_raw_units(
            position.get_total_amount(),
            storage_cache.pool_params.decimals,
        );

        let (principal, interest) = if received_amount.ge(&position.get_total_amount()) {
            // Full repayment
            let extra_amount = &received_amount - &position.get_total_amount();

            self.tx()
                .to(&initial_caller)
                .egld_or_single_esdt(&received_asset, 0, &extra_amount)
                .transfer_if_not_empty();

            let principal =
                ManagedDecimal::from_raw_units(position.amount, storage_cache.pool_params.decimals);

            position.amount = BigUint::zero();
            position.accumulated_interest = BigUint::zero();

            (principal, total_borrorwed)
        } else {
            let repayment =
                ManagedDecimal::from_raw_units(received_amount, storage_cache.pool_params.decimals);
            // Partial repayment
            let (principal, interest) = self.calculate_principal_and_interest(
                &repayment,
                &position,
                total_borrorwed,
                storage_cache.pool_params.decimals,
            );

            position.amount -= principal.into_raw_units();
            position.accumulated_interest -= interest.into_raw_units();

            (principal, repayment)
        };

        storage_cache.borrowed_amount -= &principal;
        storage_cache.reserves_amount += &interest;

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

    /// Handles a flash loan.
    ///
    /// # Parameters
    /// - `borrowed_token`: The token to borrow.
    /// - `amount`: The amount of the token to borrow.
    /// - `contract_address`: The address of the contract to call.
    /// - `endpoint`: The endpoint to call.
    /// - `arguments`: The arguments to pass to the endpoint.
    /// - `fees`: The fees to pay for the flash loan.
    /// - `asset_price`: The price of the asset, used to update the market state event.
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

        let loaned_amount =
            ManagedDecimal::from_raw_units(amount.clone(), storage_cache.pool_params.decimals);

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
            let _repayment_amount = ManagedDecimal::from_raw_units(
                back_transfers.total_egld_amount,
                storage_cache_second.pool_params.decimals,
            );

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

            let _repayment_amount = ManagedDecimal::from_raw_units(
                payment.amount.clone(),
                storage_cache_second.pool_params.decimals,
            );

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

        let strategy_amount =
            ManagedDecimal::from_raw_units(amount.clone(), storage_cache.pool_params.decimals);

        let strategy_fee =
            ManagedDecimal::from_raw_units(fee.clone(), storage_cache.pool_params.decimals);

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

        (
            storage_cache.borrow_index.into_raw_units().clone(),
            storage_cache.timestamp,
        )
    }

    /// Adds vault liquidation rewards to the pool.
    ///
    /// # Parameters
    /// - `asset_price`: The price of the asset, used to update the market state event.
    ///
    /// # Payment
    /// - `*`: The asset to add, has to be the same as the pool asset.
    #[payable("*")]
    #[only_owner]
    #[endpoint(addExternalProtocolRevenue)]
    fn add_external_protocol_revenue(&self, asset_price: &BigUint) {
        let (received_asset, received_amount) = self.call_value().egld_or_single_fungible_esdt();
        let mut storage_cache = StorageCache::new(self);

        require!(
            &received_asset == &storage_cache.pool_asset,
            ERROR_INVALID_ASSET
        );

        storage_cache.protocol_revenue += &ManagedDecimal::from_raw_units(
            received_amount.clone(),
            storage_cache.pool_params.decimals,
        );

        storage_cache.reserves_amount += &ManagedDecimal::from_raw_units(
            received_amount.clone(),
            storage_cache.pool_params.decimals,
        );

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

    /// Claims the revenue of the pool.
    ///
    /// # Parameters
    /// - `asset_price`: The price of the asset, used to update the market state event.
    ///
    /// # Returns
    /// - `EgldOrEsdtTokenPayment<Self::Api>`: The payment of the revenue.
    #[only_owner]
    #[endpoint(claimRevenue)]
    fn claim_revenue(&self, asset_price: &BigUint) -> EgldOrEsdtTokenPayment<Self::Api> {
        let mut storage_cache = StorageCache::new(self);
        self.update_interest_indexes(&mut storage_cache);

        let revenue_biguint = if storage_cache.borrowed_amount.into_raw_units() == &BigUint::zero()
            && storage_cache.supplied_amount.into_raw_units() == &BigUint::zero()
        {
            storage_cache.protocol_revenue -= storage_cache.protocol_revenue.clone();
            storage_cache.reserves_amount -= storage_cache.reserves_amount.clone();

            storage_cache.reserves_amount.into_raw_units().clone()
        } else {
            let rvn = storage_cache.available_revenue();
            storage_cache.protocol_revenue -= &rvn;
            storage_cache.reserves_amount -= &rvn;
            rvn.into_raw_units().clone()
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
