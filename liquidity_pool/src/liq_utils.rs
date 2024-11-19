multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use crate::{contexts::base::StorageCache, liq_math, liq_storage, view};

use common_structs::*;

#[multiversx_sc::module]
pub trait UtilsModule:
    liq_math::MathModule
    + liq_storage::StorageModule
    + price_aggregator_proxy::PriceAggregatorModule
    + crate::events::EventsModule
    + view::ViewModule
{
    fn update_borrow_index(&self, borrow_rate: &BigUint, delta_rounds: u64) {
        self.borrow_index()
            .update(|new_index| *new_index += borrow_rate * delta_rounds);
    }

    fn update_supply_index(
        &self,
        rewards_increase: BigUint,
        storage_cache: &mut StorageCache<Self>,
    ) {
        if storage_cache.supplied_amount != BigUint::zero() {
            self.supply_index().update(|new_index| {
                *new_index += rewards_increase * BP / &storage_cache.supplied_amount
            });
        }
    }

    fn update_rewards_reserves(
        &self,
        borrow_rate: &BigUint,
        delta_rounds: u64,
        storage_cache: &mut StorageCache<Self>,
    ) -> BigUint {
        let rewards_increase = borrow_rate * &storage_cache.borrowed_amount * delta_rounds / BP;

        let revenue = rewards_increase.clone() * &storage_cache.pool_params.reserve_factor / BP;

        self.rewards_reserves().update(|rewards_reserves| {
            *rewards_reserves += &revenue;
        });

        rewards_increase - revenue
    }

    fn update_index_last_used(&self, current_block_round: u64) {
        self.borrow_index_last_update_round()
            .set(current_block_round);
    }

    fn get_round_diff(&self, initial_round: u64) -> u64 {
        let current_round = self.blockchain().get_block_round();
        require!(current_round >= initial_round, "Invalid round");

        current_round - initial_round
    }

    fn update_interest_indexes(&self, storage_cache: &mut StorageCache<Self>) {
        let borrow_index_last_update_round = self.borrow_index_last_update_round().get();
        let delta_rounds = self.get_round_diff(borrow_index_last_update_round);

        if delta_rounds > 0 {
            let borrow_rate = self.get_borrow_rate();

            self.update_borrow_index(&borrow_rate, delta_rounds);
            let rewards = self.update_rewards_reserves(&borrow_rate, delta_rounds, storage_cache);
            self.update_supply_index(rewards, storage_cache);
            self.update_index_last_used(storage_cache.round);
        }
    }

    fn internal_update_collateral_with_interest(
        &self,
        mut deposit_position: DepositPosition<Self::Api>,
        storage_cache: &mut StorageCache<Self>,
    ) -> DepositPosition<Self::Api> {
        let supply_index = self.supply_index().get();

        self.update_interest_indexes(storage_cache);

        let accrued_interest = self.compute_interest(
            &deposit_position.amount,
            &supply_index,
            &deposit_position.initial_supply_index,
        );

        deposit_position.amount += &accrued_interest;
        deposit_position.round = storage_cache.round;
        deposit_position.initial_supply_index = supply_index;

        self.add_accrued_interest_event(
            deposit_position.owner_nonce,
            &accrued_interest,
            &deposit_position,
        );

        deposit_position
    }

    fn internal_update_borrows_with_debt(
        &self,
        mut borrow_position: BorrowPosition<Self::Api>,
        storage_cache: &mut StorageCache<Self>,
    ) -> BorrowPosition<Self::Api> {
        let borrow_index = self.borrow_index().get();

        self.update_interest_indexes(storage_cache);

        let accumulated_debt = self.get_debt_interest(
            &borrow_position.amount,
            &borrow_position.initial_borrow_index,
        );

        borrow_position.amount += &accumulated_debt;
        borrow_position.round = storage_cache.round;
        borrow_position.initial_borrow_index = borrow_index;

        self.add_debt_interest_event(
            borrow_position.owner_nonce,
            &accumulated_debt,
            &borrow_position,
        );

        borrow_position
    }

    #[inline]
    fn is_full_repay(
        &self,
        borrow_position: &BorrowPosition<Self::Api>,
        borrow_token_repaid: &BigUint,
    ) -> bool {
        &borrow_position.amount == borrow_token_repaid
    }
}
