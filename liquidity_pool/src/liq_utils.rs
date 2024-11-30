multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use crate::{contexts::base::StorageCache, liq_math, liq_storage, view};

use common_structs::*;

#[multiversx_sc::module]
pub trait UtilsModule:
    liq_math::MathModule + liq_storage::StorageModule + common_events::EventsModule + view::ViewModule
{
    fn calculate_interest_factor(&self, rate: &BigUint, delta_timestamp: u64) -> BigUint {
        let bp = BigUint::from(BP);

        // Calculate accumulated rate and add BP (1.0) to get the multiplier
        bp + (rate * &BigUint::from(delta_timestamp) / BigUint::from(SECONDS_PER_YEAR))
    }

    fn update_borrow_index(
        &self,
        borrow_rate: &BigUint,
        delta_timestamp: u64,
        storage_cache: &mut StorageCache<Self>,
    ) {
        let interest_factor = self.calculate_interest_factor(borrow_rate, delta_timestamp);

        storage_cache.borrow_index =
            &storage_cache.borrow_index * &interest_factor / BigUint::from(BP);
    }

    fn update_supply_index(
        &self,
        rewards_increase: BigUint,
        storage_cache: &mut StorageCache<Self>,
    ) {
        if storage_cache.supplied_amount != BigUint::zero() {
            // Convert rewards to an index increase factor
            let rewards_factor =
                rewards_increase * BigUint::from(BP) / &storage_cache.supplied_amount;

            storage_cache.supply_index = &storage_cache.supply_index
                * &(&BigUint::from(BP) + &rewards_factor)
                / BigUint::from(BP);
        }
    }

    fn update_rewards_reserves(
        &self,
        borrow_rate: &BigUint,
        delta_timestamp: u64,
        storage_cache: &mut StorageCache<Self>,
    ) -> BigUint {
        // 1. Calculate compound interest factor
        let interest_factor = self.calculate_interest_factor(borrow_rate, delta_timestamp);

        // 2. Calculate total interest earned (compound)
        let new_borrowed_amount =
            &storage_cache.borrowed_amount * &interest_factor / BigUint::from(BP);
        let rewards_increase = new_borrowed_amount - &storage_cache.borrowed_amount;

        // 3. Calculate protocol's share
        let revenue = rewards_increase.clone() * &storage_cache.pool_params.reserve_factor / BP;

        // 4. Update reserves
        storage_cache.rewards_reserve += &revenue;

        // 5. Return suppliers' share
        rewards_increase - revenue
    }

    fn get_timestamp_diff(&self, initial_timestamp: u64, current_timestamp: u64) -> u64 {
        current_timestamp - initial_timestamp
    }

    fn update_interest_indexes(&self, storage_cache: &mut StorageCache<Self>) {
        let borrow_index_last_update_timestamp = storage_cache.borrow_index_last_update_timestamp;
        let delta_timestamp =
            self.get_timestamp_diff(borrow_index_last_update_timestamp, storage_cache.timestamp);

        if delta_timestamp > 0 {
            let borrow_rate = self.get_borrow_rate();
            self.update_borrow_index(&borrow_rate, delta_timestamp, storage_cache);
            let rewards =
                self.update_rewards_reserves(&borrow_rate, delta_timestamp, storage_cache);
            self.update_supply_index(rewards, storage_cache);
            // Update the last used round
            storage_cache.borrow_index_last_update_timestamp = storage_cache.timestamp;
        }
    }

    fn internal_update_collateral_with_interest(
        &self,
        mut deposit_position: AccountPosition<Self::Api>,
        storage_cache: &mut StorageCache<Self>,
    ) -> AccountPosition<Self::Api> {
        self.update_interest_indexes(storage_cache);

        let accrued_interest = self.compute_interest(
            &deposit_position.get_total_amount(),
            &storage_cache.supply_index,
            &deposit_position.index,
        );

        deposit_position.accumulated_interest += &accrued_interest;
        deposit_position.timestamp = storage_cache.timestamp;
        deposit_position.index = storage_cache.supply_index.clone();

        self.update_position_event(
            &accrued_interest,
            &deposit_position,
            OptionalValue::None,
            OptionalValue::None,
        );

        deposit_position
    }

    fn internal_update_borrows_with_debt(
        &self,
        mut borrow_position: AccountPosition<Self::Api>,
        storage_cache: &mut StorageCache<Self>,
    ) -> AccountPosition<Self::Api> {
        self.update_interest_indexes(storage_cache);

        let accumulated_debt = self.compute_interest(
            &borrow_position.get_total_amount(),
            &storage_cache.borrow_index,
            &borrow_position.index,
        );

        borrow_position.accumulated_interest += &accumulated_debt;
        borrow_position.timestamp = storage_cache.timestamp;
        borrow_position.index = storage_cache.borrow_index.clone();

        self.update_position_event(
            &accumulated_debt,
            &borrow_position,
            OptionalValue::None,
            OptionalValue::None,
        );

        borrow_position
    }
}
