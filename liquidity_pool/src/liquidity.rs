multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use crate::errors::*;
use common_structs::*;

use super::{contexts::base::StorageCache, liq_math, liq_storage, liq_utils, view};

#[multiversx_sc::module]
pub trait LiquidityModule:
    liq_storage::StorageModule
    + common_tokens::AccountTokenModule
    + liq_utils::UtilsModule
    + common_events::EventsModule
    + liq_math::MathModule
    + view::ViewModule
    + common_checks::ChecksModule
{
    #[only_owner]
    #[endpoint(updateIndexes)]
    fn update_indexes(&self, asset_usd_price: &BigUint) {
        let mut storage_cache = StorageCache::new(self);

        self.update_interest_indexes(&mut storage_cache);

        self.update_market_state_event(
            storage_cache.timestamp,
            storage_cache.supply_index.into_raw_units(),
            storage_cache.borrow_index.into_raw_units(),
            storage_cache.reserves_amount.into_raw_units(),
            storage_cache.supplied_amount.into_raw_units(),
            storage_cache.borrowed_amount.into_raw_units(),
            storage_cache.rewards_reserve.into_raw_units(),
            &storage_cache.pool_asset,
            asset_usd_price,
        );
    }

    #[only_owner]
    #[endpoint(updatePositionInterest)]
    fn update_position_with_interest(
        &self,
        position: AccountPosition<Self::Api>,
        asset_usd_price: OptionalValue<BigUint>,
    ) -> AccountPosition<Self::Api> {
        let mut storage_cache = StorageCache::new(self);
        let account = self.internal_update_position_with_interest(position, &mut storage_cache);

        if asset_usd_price.is_some() {
            self.update_market_state_event(
                storage_cache.timestamp,
                storage_cache.supply_index.into_raw_units(),
                storage_cache.borrow_index.into_raw_units(),
                storage_cache.reserves_amount.into_raw_units(),
                storage_cache.supplied_amount.into_raw_units(),
                storage_cache.borrowed_amount.into_raw_units(),
                storage_cache.rewards_reserve.into_raw_units(),
                &storage_cache.pool_asset,
                &asset_usd_price.into_option().unwrap(),
            );
        }
        account
    }

    #[only_owner]
    #[payable("*")]
    #[endpoint(supply)]
    fn supply(
        &self,
        deposit_position: AccountPosition<Self::Api>,
        asset_usd_price: &BigUint,
    ) -> AccountPosition<Self::Api> {
        let mut storage_cache = StorageCache::new(self);

        let (deposit_asset, deposit_amount) = self.call_value().single_fungible_esdt();
        let deposit_amount_dec = ManagedDecimal::from_raw_units(
            deposit_amount.clone(),
            storage_cache.pool_params.decimals,
        );
        let mut ret_deposit_position = deposit_position.clone();

        require!(
            deposit_asset == storage_cache.pool_asset,
            ERROR_INVALID_ASSET
        );

        self.update_interest_indexes(&mut storage_cache);

        if deposit_position.amount.gt(&BigUint::zero()) {
            ret_deposit_position =
                self.internal_update_position_with_interest(deposit_position, &mut storage_cache);
        }

        ret_deposit_position.amount += &deposit_amount;
        ret_deposit_position.timestamp = storage_cache.timestamp;
        ret_deposit_position.index = storage_cache.supply_index.into_raw_units().clone();

        storage_cache.reserves_amount += &deposit_amount_dec;

        storage_cache.supplied_amount += deposit_amount_dec;

        self.update_market_state_event(
            storage_cache.timestamp,
            storage_cache.supply_index.into_raw_units(),
            storage_cache.borrow_index.into_raw_units(),
            storage_cache.reserves_amount.into_raw_units(),
            storage_cache.supplied_amount.into_raw_units(),
            storage_cache.borrowed_amount.into_raw_units(),
            storage_cache.rewards_reserve.into_raw_units(),
            &storage_cache.pool_asset,
            asset_usd_price,
        );

        ret_deposit_position
    }

    #[only_owner]
    #[payable("*")]
    #[endpoint(borrow)]
    fn borrow(
        &self,
        initial_caller: &ManagedAddress,
        borrow_amount: &BigUint,
        existing_borrow_position: AccountPosition<Self::Api>,
        asset_usd_price: &BigUint,
    ) -> AccountPosition<Self::Api> {
        let mut storage_cache = StorageCache::new(self);

        let mut ret_borrow_position = existing_borrow_position.clone();

        self.require_non_zero_address(initial_caller);
        let borrow_amount_dec = ManagedDecimal::from_raw_units(
            borrow_amount.clone(),
            storage_cache.pool_params.decimals,
        );
        require!(
            &storage_cache.reserves_amount >= &borrow_amount_dec,
            ERROR_INSUFFICIENT_LIQUIDITY
        );

        self.update_interest_indexes(&mut storage_cache);

        if ret_borrow_position.amount.gt(&BigUint::zero()) {
            ret_borrow_position = self.internal_update_position_with_interest(
                existing_borrow_position,
                &mut storage_cache,
            );
        }

        ret_borrow_position.amount += borrow_amount;
        ret_borrow_position.timestamp = storage_cache.timestamp;
        ret_borrow_position.index = storage_cache.borrow_index.into_raw_units().clone();

        storage_cache.borrowed_amount += borrow_amount_dec.clone();
        storage_cache.reserves_amount -= borrow_amount_dec.clone();

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
            storage_cache.supply_index.into_raw_units(),
            storage_cache.borrow_index.into_raw_units(),
            storage_cache.reserves_amount.into_raw_units(),
            storage_cache.supplied_amount.into_raw_units(),
            storage_cache.borrowed_amount.into_raw_units(),
            storage_cache.rewards_reserve.into_raw_units(),
            &storage_cache.pool_asset,
            asset_usd_price,
        );

        ret_borrow_position
    }

    #[only_owner]
    #[payable("*")]
    #[endpoint(withdraw)]
    fn withdraw(
        &self,
        initial_caller: &ManagedAddress,
        amount: &BigUint,
        mut deposit_position: AccountPosition<Self::Api>,
        is_liquidation: bool,
        protocol_liquidation_fee: &BigUint,
        asset_usd_price: &BigUint,
    ) -> AccountPosition<Self::Api> {
        let mut storage_cache = StorageCache::new(self);

        self.require_non_zero_address(initial_caller);
        self.require_amount_greater_than_zero(amount);

        self.update_interest_indexes(&mut storage_cache);
        let amount_dec =
            ManagedDecimal::from_raw_units(amount.clone(), storage_cache.pool_params.decimals);
        // Unaccrued interest for the wanted amount
        let extra_interest = self.compute_interest(
            amount_dec.clone(),
            &storage_cache.supply_index,
            &ManagedDecimal::from_raw_units(deposit_position.index.clone(), DECIMAL_PRECISION),
        );

        let total_withdraw = amount_dec.clone() + extra_interest;
        // Withdrawal amount = initial wanted amount + Unaccrued interest for that amount (this has to be paid back to the user that requested the withdrawal)
        let mut principal_amount = total_withdraw.clone();
        // Check if there is enough liquidity to cover the withdrawal

        require!(
            &storage_cache.reserves_amount >= &principal_amount,
            ERROR_INSUFFICIENT_LIQUIDITY
        );

        // Update the reserves amount
        storage_cache.reserves_amount -= &principal_amount;

        let mut accumulated_interest = ManagedDecimal::from_raw_units(
            deposit_position.accumulated_interest.clone(),
            storage_cache.pool_params.decimals,
        );

        let zero =
            ManagedDecimal::from_raw_units(BigUint::from(0u64), storage_cache.pool_params.decimals);
        // If the total withdrawal amount is greater than the accumulated interest, we need to subtract the accumulated interest from the withdrawal amount
        if principal_amount >= accumulated_interest {
            principal_amount -= &accumulated_interest;
            accumulated_interest = zero.clone();
        }

        // If the accumulated interest is greater than the withdrawal amount, we need to subtract the withdrawal amount from the accumulated interest
        if accumulated_interest >= principal_amount {
            accumulated_interest -= principal_amount.clone();
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

        if !is_liquidation {
            self.tx()
                .to(initial_caller)
                .payment(EgldOrEsdtTokenPayment::new(
                    storage_cache.pool_asset.clone(),
                    0,
                    total_withdraw.into_raw_units().clone(),
                ))
                .transfer();
        } else {
            let protocol_liquidation_fee_dec = ManagedDecimal::from_raw_units(
                protocol_liquidation_fee.clone(),
                storage_cache.pool_params.decimals,
            );

            storage_cache.rewards_reserve += &protocol_liquidation_fee_dec;
            storage_cache.reserves_amount += &protocol_liquidation_fee_dec;
            let amount_after_fee = total_withdraw - protocol_liquidation_fee_dec;

            self.tx()
                .to(initial_caller)
                .payment(EgldOrEsdtTokenPayment::new(
                    storage_cache.pool_asset.clone(),
                    0,
                    amount_after_fee.into_raw_units().clone(),
                ))
                .transfer();
        }
        
        self.update_market_state_event(
            storage_cache.timestamp,
            storage_cache.supply_index.into_raw_units(),
            storage_cache.borrow_index.into_raw_units(),
            storage_cache.reserves_amount.into_raw_units(),
            storage_cache.supplied_amount.into_raw_units(),
            storage_cache.borrowed_amount.into_raw_units(),
            storage_cache.rewards_reserve.into_raw_units(),
            &storage_cache.pool_asset,
            asset_usd_price,
        );
        deposit_position
    }

    #[only_owner]
    #[payable("*")]
    #[endpoint(repay)]
    fn repay(
        &self,
        initial_caller: ManagedAddress,
        borrow_position: AccountPosition<Self::Api>,
        asset_usd_price: &BigUint,
    ) -> AccountPosition<Self::Api> {
        let mut storage_cache = StorageCache::new(self);
        let (received_asset, received_amount) = self.call_value().egld_or_single_fungible_esdt();

        self.require_non_zero_address(&initial_caller);
        self.require_amount_greater_than_zero(&received_amount);
        require!(
            received_asset == storage_cache.pool_asset,
            ERROR_INVALID_ASSET
        );
        let received_amount_dec = ManagedDecimal::from_raw_units(
            received_amount.clone(),
            storage_cache.pool_params.decimals,
        );
        self.update_interest_indexes(&mut storage_cache);
        let mut ret_borrow_position =
            self.internal_update_position_with_interest(borrow_position, &mut storage_cache);

        let total_owed_with_interest = ManagedDecimal::from_raw_units(
            ret_borrow_position.get_total_amount().clone(),
            storage_cache.pool_params.decimals,
        );

        if received_amount_dec >= total_owed_with_interest {
            // Full repayment
            let extra_amount = received_amount - ret_borrow_position.get_total_amount();
            if extra_amount > BigUint::zero() {
                self.tx()
                    .to(&initial_caller)
                    .payment(EgldOrEsdtTokenPayment::new(
                        received_asset,
                        0,
                        extra_amount.clone(),
                    ))
                    .transfer();
            }
            // Reduce borrowed by principal
            storage_cache.borrowed_amount -= &ManagedDecimal::from_raw_units(
                ret_borrow_position.amount.clone(),
                storage_cache.pool_params.decimals,
            );
            // Add full payment (principal + interest) to reserves

            storage_cache.reserves_amount += &total_owed_with_interest;

            ret_borrow_position.amount = BigUint::zero();
            ret_borrow_position.accumulated_interest = BigUint::zero();
        } else {
            // Partial repayment
            let total_debt = total_owed_with_interest.clone();
            // Calculate principal portion of the payment
            let principal_portion = received_amount_dec
                .clone()
                .mul(ManagedDecimal::from_raw_units(
                    ret_borrow_position.amount.clone(),
                    storage_cache.pool_params.decimals,
                ))
                .div(total_debt);
            let interest_portion = received_amount_dec.clone() - principal_portion.clone();

            // Reduce position amounts
            ret_borrow_position.amount -= &principal_portion.into_raw_units().clone();
            ret_borrow_position.accumulated_interest -= &interest_portion.into_raw_units().clone();

            // Update storage
            storage_cache.borrowed_amount -= &principal_portion;

            storage_cache.reserves_amount += &received_amount_dec; // Full payment goes to reserves
        }

        self.update_market_state_event(
            storage_cache.timestamp,
            storage_cache.supply_index.into_raw_units(),
            storage_cache.borrow_index.into_raw_units(),
            storage_cache.reserves_amount.into_raw_units(),
            storage_cache.supplied_amount.into_raw_units(),
            storage_cache.borrowed_amount.into_raw_units(),
            storage_cache.rewards_reserve.into_raw_units(),
            &storage_cache.pool_asset,
            asset_usd_price,
        );

        ret_borrow_position
    }

    #[payable("*")]
    #[only_owner]
    #[endpoint(vaultRewards)]
    fn vault_rewards(&self, asset_usd_price: &BigUint) {
        let (received_asset, received_amount) = self.call_value().egld_or_single_fungible_esdt();
        let mut storage_cache = StorageCache::new(self);
        require!(
            received_asset == storage_cache.pool_asset,
            ERROR_INVALID_ASSET
        );
        storage_cache.rewards_reserve += &ManagedDecimal::from_raw_units(
            received_amount.clone(),
            storage_cache.pool_params.decimals,
        );
        self.update_market_state_event(
            storage_cache.timestamp,
            storage_cache.supply_index.into_raw_units(),
            storage_cache.borrow_index.into_raw_units(),
            storage_cache.reserves_amount.into_raw_units(),
            storage_cache.supplied_amount.into_raw_units(),
            storage_cache.borrowed_amount.into_raw_units(),
            storage_cache.rewards_reserve.into_raw_units(),
            &storage_cache.pool_asset,
            asset_usd_price,
        );
    }

    #[endpoint(flashLoan)]
    fn flash_loan(
        &self,
        borrowed_token: &EgldOrEsdtTokenIdentifier,
        amount: &BigUint,
        contract_address: &ManagedAddress,
        endpoint: ManagedBuffer<Self::Api>,
        arguments: ManagedArgBuffer<Self::Api>,
        fees: &BigUint,
        asset_usd_price: &BigUint,
    ) {
        let mut storage_cache = StorageCache::new(self);

        require!(
            borrowed_token == &storage_cache.pool_asset,
            ERROR_INVALID_ASSET
        );

        let amount_dec =
            ManagedDecimal::from_raw_units(amount.clone(), storage_cache.pool_params.decimals);
        require!(
            &storage_cache.reserves_amount >= &amount_dec,
            ERROR_FLASHLOAN_RESERVE_ASSET
        );

        // Calculate flash loan fee
        let flash_loan_fee =
            amount_dec.clone() * ManagedDecimal::from_raw_units(fees.clone(), DECIMAL_PRECISION);

        // Calculate minimum required amount to be paid back
        let min_required_amount = amount_dec + flash_loan_fee;

        // TODO: Maybe before the execution drop the cache to save the new available liquidity
        // Does it make a difference? I tend to say no.
        // My concern is what if the call does another flashloan in a loop?
        let back_transfers = self
            .tx()
            .to(contract_address)
            .raw_call(endpoint)
            .arguments_raw(arguments)
            .payment(EgldOrEsdtTokenPayment::new(
                storage_cache.pool_asset.clone(),
                0,
                amount.clone(),
            ))
            .returns(ReturnsBackTransfers)
            .sync_call();

        let is_egld = borrowed_token == &EgldOrEsdtTokenIdentifier::egld();
        if is_egld {
            let amount_dec = ManagedDecimal::from_raw_units(
                back_transfers.total_egld_amount,
                storage_cache.pool_params.decimals,
            );
            require!(
                amount_dec >= min_required_amount,
                ERROR_INVALID_FLASHLOAN_REPAYMENT
            );
            let extra_amount = amount_dec - min_required_amount;
            storage_cache.rewards_reserve += &extra_amount;
        } else {
            require!(
                back_transfers.esdt_payments.len() == 1,
                ERROR_INVALID_FLASHLOAN_REPAYMENT
            );
            let payment = back_transfers.esdt_payments.get(0);
            require!(
                &payment.token_identifier == &storage_cache.pool_asset,
                ERROR_INVALID_FLASHLOAN_REPAYMENT
            );

            let amount_dec = ManagedDecimal::from_raw_units(
                payment.amount.clone(),
                storage_cache.pool_params.decimals,
            );

            require!(
                amount_dec >= min_required_amount,
                ERROR_INVALID_FLASHLOAN_REPAYMENT
            );

            let extra_amount = amount_dec - min_required_amount;
            storage_cache.rewards_reserve += &extra_amount;
        }

        self.update_market_state_event(
            storage_cache.timestamp,
            &storage_cache.supply_index.into_raw_units().clone(),
            &storage_cache.borrow_index.into_raw_units().clone(),
            &storage_cache.reserves_amount.into_raw_units().clone(),
            &storage_cache.supplied_amount.into_raw_units().clone(),
            &storage_cache.borrowed_amount.into_raw_units().clone(),
            &storage_cache.rewards_reserve.into_raw_units().clone(),
            &storage_cache.pool_asset,
            asset_usd_price,
        );
    }
}
