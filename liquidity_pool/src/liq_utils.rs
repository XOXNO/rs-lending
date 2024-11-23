multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use crate::{contexts::base::StorageCache, liq_math, liq_storage, view};

use common_structs::*;

#[multiversx_sc::module]
pub trait UtilsModule:
    liq_math::MathModule + liq_storage::StorageModule + common_events::EventsModule + view::ViewModule
{
    fn update_borrow_index(
        &self,
        borrow_rate: &BigUint,
        delta_rounds: u64,
        storage_cache: &mut StorageCache<Self>,
    ) {
        storage_cache.borrow_index += borrow_rate * delta_rounds;
    }

    fn update_supply_index(
        &self,
        rewards_increase: BigUint,
        storage_cache: &mut StorageCache<Self>,
    ) {
        if storage_cache.supplied_amount != BigUint::zero() {
            storage_cache.supply_index += rewards_increase * BP / &storage_cache.supplied_amount;
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

        storage_cache.rewards_reserve += &revenue;

        rewards_increase - revenue
    }

    fn get_round_diff(&self, initial_round: u64, current_round: u64) -> u64 {
        require!(current_round >= initial_round, "Invalid round");

        current_round - initial_round
    }

    fn update_interest_indexes(&self, storage_cache: &mut StorageCache<Self>) {
        let borrow_index_last_update_round = storage_cache.borrow_index_last_update_round;
        let delta_rounds = self.get_round_diff(borrow_index_last_update_round, storage_cache.round);

        if delta_rounds > 0 {
            let borrow_rate = self.get_borrow_rate();

            self.update_borrow_index(&borrow_rate, delta_rounds, storage_cache);
            let rewards = self.update_rewards_reserves(&borrow_rate, delta_rounds, storage_cache);
            self.update_supply_index(rewards, storage_cache);
            // Update the last used round
            storage_cache.borrow_index_last_update_round = storage_cache.round;
        }
    }

    fn internal_update_collateral_with_interest(
        &self,
        mut deposit_position: AccountPosition<Self::Api>,
        storage_cache: &mut StorageCache<Self>,
    ) -> AccountPosition<Self::Api> {
        self.update_interest_indexes(storage_cache);

        let accrued_interest = self.compute_interest(
            &deposit_position.amount,
            &storage_cache.supply_index,
            &deposit_position.index,
        );

        deposit_position.amount += &accrued_interest;
        deposit_position.round = storage_cache.round;
        deposit_position.index = storage_cache.supply_index.clone();

        self.update_position_event(&accrued_interest, &deposit_position, None, None);

        deposit_position
    }

    fn internal_update_borrows_with_debt(
        &self,
        mut borrow_position: AccountPosition<Self::Api>,
        storage_cache: &mut StorageCache<Self>,
    ) -> AccountPosition<Self::Api> {
        self.update_interest_indexes(storage_cache);

        let accumulated_debt =
            self.get_debt_interest(&borrow_position.amount, &borrow_position.index);

        borrow_position.amount += &accumulated_debt;
        borrow_position.round = storage_cache.round;
        borrow_position.index = storage_cache.borrow_index.clone();

        self.update_position_event(&accumulated_debt, &borrow_position, None, None);

        borrow_position
    }

    #[inline]
    fn is_full_repay(
        &self,
        borrow_position: &AccountPosition<Self::Api>,
        borrow_token_repaid: &BigUint,
    ) -> bool {
        &borrow_position.amount == borrow_token_repaid
    }
}
