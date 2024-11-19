multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use common_structs::*;

use crate::contexts::base::StorageCache;
use crate::errors::ERROR_INSUFFICIENT_LIQUIDITY;
use crate::errors::ERROR_INVALID_ASSET;

use super::liq_math;
use super::liq_storage;
use super::liq_utils;
use super::view;
#[multiversx_sc::module]
pub trait LiquidityModule:
    liq_storage::StorageModule
    + common_tokens::AccountTokenModule
    + liq_utils::UtilsModule
    + crate::events::EventsModule
    + liq_math::MathModule
    + view::ViewModule
    + price_aggregator_proxy::PriceAggregatorModule
    + common_checks::ChecksModule
{
    #[only_owner]
    #[payable("*")]
    #[endpoint(updateCollateralWithInterest)]
    fn update_collateral_with_interest(
        &self,
        deposit_position: DepositPosition<Self::Api>,
    ) -> DepositPosition<Self::Api> {
        let mut storage_cache = StorageCache::new(self);
        self.internal_update_collateral_with_interest(deposit_position, &mut storage_cache)
    }

    #[only_owner]
    #[payable("*")]
    #[endpoint(updateBorrowsWithDebt)]
    fn update_borrows_with_debt(
        &self,
        borrow_position: BorrowPosition<Self::Api>,
    ) -> BorrowPosition<Self::Api> {
        let mut storage_cache = StorageCache::new(self);
        self.internal_update_borrows_with_debt(borrow_position, &mut storage_cache)
    }

    #[only_owner]
    #[payable("*")]
    #[endpoint(addCollateral)]
    fn add_collateral(
        &self,
        deposit_position: DepositPosition<Self::Api>,
    ) -> DepositPosition<Self::Api> {
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

        let supply_index = self.supply_index().get();
        ret_deposit_position.amount += &deposit_amount;
        ret_deposit_position.round = storage_cache.round;
        ret_deposit_position.initial_supply_index = supply_index.clone();

        storage_cache.reserves_amount += &deposit_amount;
        storage_cache.supplied_amount += deposit_amount;

        self.update_market_state_event(
            storage_cache.round,
            &supply_index,
            &self.borrow_index().get(),
            &storage_cache.reserves_amount,
            &storage_cache.supplied_amount,
            &storage_cache.borrowed_amount,
        );

        ret_deposit_position
    }

    #[only_owner]
    #[payable("*")]
    #[endpoint]
    fn borrow(
        &self,
        initial_caller: &ManagedAddress,
        borrow_amount: &BigUint,
        existing_borrow_position: BorrowPosition<Self::Api>,
    ) -> BorrowPosition<Self::Api> {
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

        let borrow_index = self.borrow_index().get();
        ret_borrow_position.amount += borrow_amount;
        ret_borrow_position.round = storage_cache.round;
        ret_borrow_position.initial_borrow_index = borrow_index.clone();

        storage_cache.borrowed_amount += borrow_amount;

        storage_cache.reserves_amount -= borrow_amount;

        self.send()
            .direct_esdt(initial_caller, &storage_cache.pool_asset, 0, borrow_amount);

        self.update_market_state_event(
            storage_cache.round,
            &self.supply_index().get(),
            &borrow_index,
            &storage_cache.reserves_amount,
            &storage_cache.supplied_amount,
            &storage_cache.borrowed_amount,
        );

        ret_borrow_position
    }

    #[only_owner]
    #[payable("*")]
    #[endpoint]
    fn remove_collateral(
        &self,
        initial_caller: &ManagedAddress,
        amount: &BigUint,
        mut deposit_position: DepositPosition<Self::Api>,
    ) -> DepositPosition<Self::Api> {
        let mut storage_cache = StorageCache::new(self);

        self.require_non_zero_address(initial_caller);
        self.require_amount_greater_than_zero(amount);

        self.update_interest_indexes(&mut storage_cache);

        // Withdrawal amount = initial_deposit + Interest
        let supply_index = self.supply_index().get();
        let withdrawal_amount = self.compute_withdrawal_amount(
            amount,
            &supply_index,
            &deposit_position.initial_supply_index,
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

        self.send().direct_esdt(
            initial_caller,
            &storage_cache.pool_asset,
            0,
            &withdrawal_amount,
        );

        self.update_market_state_event(
            storage_cache.round,
            &supply_index,
            &self.borrow_index().get(),
            &storage_cache.reserves_amount,
            &storage_cache.supplied_amount,
            &storage_cache.borrowed_amount,
        );
        deposit_position
    }

    #[only_owner]
    #[payable("*")]
    #[endpoint]
    fn repay(
        &self,
        initial_caller: ManagedAddress,
        borrow_position: BorrowPosition<Self::Api>,
    ) -> BorrowPosition<Self::Api> {
        let mut storage_cache = StorageCache::new(self);
        let (received_asset, mut received_amount) = self.call_value().single_fungible_esdt();

        self.require_non_zero_address(&initial_caller);
        self.require_amount_greater_than_zero(&received_amount);
        require!(
            received_asset == storage_cache.pool_asset,
            ERROR_INVALID_ASSET
        );

        self.update_interest_indexes(&mut storage_cache);

        let accumulated_debt = self.get_debt_interest(
            &borrow_position.amount,
            &borrow_position.initial_borrow_index,
        );

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

    #[only_owner]
    #[endpoint(sendTokens)]
    fn send_tokens(&self, initial_caller: ManagedAddress, payment_amount: BigUint) {
        let storage_cache = StorageCache::new(self);

        self.send().direct_esdt(
            &initial_caller,
            &storage_cache.pool_asset,
            0,
            &payment_amount,
        );
    }
}
