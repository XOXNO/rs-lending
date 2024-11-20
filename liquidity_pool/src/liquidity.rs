multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use common_structs::*;

use crate::errors::ERROR_INSUFFICIENT_LIQUIDITY;
use crate::errors::ERROR_INVALID_ASSET;

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
    #[endpoint(updatePositionInterest)]
    fn update_collateral_with_interest(
        &self,
        deposit_position: AccountPosition<Self::Api>,
    ) -> AccountPosition<Self::Api> {
        let mut storage_cache = StorageCache::new(self);
        self.internal_update_collateral_with_interest(deposit_position, &mut storage_cache)
    }

    #[only_owner]
    #[endpoint(updatePositionDebt)]
    fn update_borrows_with_debt(
        &self,
        borrow_position: AccountPosition<Self::Api>,
    ) -> AccountPosition<Self::Api> {
        let mut storage_cache = StorageCache::new(self);
        self.internal_update_borrows_with_debt(borrow_position, &mut storage_cache)
    }

    #[only_owner]
    #[payable("*")]
    #[endpoint(supply)]
    fn supply(&self, deposit_position: AccountPosition<Self::Api>) -> AccountPosition<Self::Api> {
        let mut storage_cache = StorageCache::new(self);

        let (deposit_asset, deposit_amount) = self.call_value().single_fungible_esdt();
        let mut ret_deposit_position = deposit_position.clone();

        require!(
            deposit_asset == storage_cache.pool_asset,
            ERROR_INVALID_ASSET
        );

        self.update_interest_indexes(&mut storage_cache);

        if deposit_position.amount != 0 {
            ret_deposit_position =
                self.internal_update_collateral_with_interest(deposit_position, &mut storage_cache);
        }

        ret_deposit_position.amount += &deposit_amount;
        ret_deposit_position.round = storage_cache.round;
        ret_deposit_position.initial_index = storage_cache.supply_index.clone();

        storage_cache.reserves_amount += &deposit_amount;
        storage_cache.supplied_amount += deposit_amount;

        self.update_market_state_event(
            storage_cache.round,
            &storage_cache.supply_index,
            &storage_cache.borrow_index,
            &storage_cache.reserves_amount,
            &storage_cache.supplied_amount,
            &storage_cache.borrowed_amount,
            &storage_cache.rewards_reserve,
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
    ) -> AccountPosition<Self::Api> {
        let mut storage_cache = StorageCache::new(self);

        let mut ret_borrow_position = existing_borrow_position.clone();
        self.require_non_zero_address(initial_caller);
        require!(
            &storage_cache.reserves_amount >= borrow_amount,
            ERROR_INSUFFICIENT_LIQUIDITY
        );

        self.update_interest_indexes(&mut storage_cache);
        if ret_borrow_position.amount != 0 {
            ret_borrow_position = self
                .internal_update_borrows_with_debt(existing_borrow_position, &mut storage_cache);
        }

        ret_borrow_position.amount += borrow_amount;
        ret_borrow_position.round = storage_cache.round;
        ret_borrow_position.initial_index = storage_cache.borrow_index.clone();

        storage_cache.borrowed_amount += borrow_amount;

        storage_cache.reserves_amount -= borrow_amount;

        self.send()
            .direct_esdt(initial_caller, &storage_cache.pool_asset, 0, borrow_amount);

        self.update_market_state_event(
            storage_cache.round,
            &storage_cache.supply_index,
            &storage_cache.borrow_index,
            &storage_cache.reserves_amount,
            &storage_cache.supplied_amount,
            &storage_cache.borrowed_amount,
            &storage_cache.rewards_reserve,
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
    ) -> AccountPosition<Self::Api> {
        let mut storage_cache = StorageCache::new(self);

        self.require_non_zero_address(initial_caller);
        self.require_amount_greater_than_zero(amount);

        self.update_interest_indexes(&mut storage_cache);

        // Withdrawal amount = initial_deposit + Interest
        let withdrawal_amount = self.compute_withdrawal_amount(
            amount,
            &storage_cache.supply_index,
            &deposit_position.initial_index,
        );

        require!(
            &storage_cache.reserves_amount >= &withdrawal_amount,
            ERROR_INSUFFICIENT_LIQUIDITY
        );

        storage_cache.reserves_amount -= &withdrawal_amount;

        require!(
            storage_cache.supplied_amount >= *amount,
            ERROR_INSUFFICIENT_LIQUIDITY
        );

        storage_cache.supplied_amount -= amount;

        deposit_position.amount -= amount;

        if !is_liquidation {
            self.send().direct_esdt(
                initial_caller,
                &storage_cache.pool_asset,
                0,
                &withdrawal_amount,
            );
        } else {
            let params = self.pool_params().get();
            let protocol_liquidation_fee = params.protocol_liquidation_fee;
            let liquidation_fee = &withdrawal_amount * &protocol_liquidation_fee / BP;
            let amount_after_fee = &withdrawal_amount - &liquidation_fee;

            storage_cache.rewards_reserve += &liquidation_fee;

            self.send().direct_esdt(
                initial_caller,
                &storage_cache.pool_asset,
                0,
                &amount_after_fee,
            );
        }

        self.update_market_state_event(
            storage_cache.round,
            &storage_cache.supply_index,
            &storage_cache.borrow_index,
            &storage_cache.reserves_amount,
            &storage_cache.supplied_amount,
            &storage_cache.borrowed_amount,
            &storage_cache.rewards_reserve,
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
    ) -> AccountPosition<Self::Api> {
        let mut storage_cache = StorageCache::new(self);
        let (received_asset, mut received_amount) = self.call_value().single_fungible_esdt();

        self.require_non_zero_address(&initial_caller);
        self.require_amount_greater_than_zero(&received_amount);
        require!(
            received_asset == storage_cache.pool_asset,
            ERROR_INVALID_ASSET
        );

        self.update_interest_indexes(&mut storage_cache);

        let accumulated_debt =
            self.get_debt_interest(&borrow_position.amount, &borrow_position.initial_index);

        let mut ret_borrow_position = self.update_borrows_with_debt(borrow_position);

        let total_owed_with_interest = ret_borrow_position.amount.clone();

        if received_amount >= total_owed_with_interest {
            // He pays in full the entire debt
            let extra_amount = &received_amount - &total_owed_with_interest;
            self.send()
                .direct_esdt(&initial_caller, &received_asset, 0, &extra_amount);
            received_amount -= &extra_amount;
            ret_borrow_position.amount = BigUint::zero();
        } else {
            // Always make sure he pays the interest first then remove from his position
            let principal_amount = &received_amount - &accumulated_debt;
            ret_borrow_position.amount -= &principal_amount;
        }

        let amount_without_interest = &received_amount - &accumulated_debt;

        storage_cache.borrowed_amount -= amount_without_interest;

        storage_cache.reserves_amount += &received_amount;

        ret_borrow_position
    }
}
