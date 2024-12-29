multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use crate::{contexts::base::StorageCache, liq_math, liq_storage, view};

use common_constants::{BP, DECIMAL_PRECISION, SECONDS_PER_YEAR};
use common_structs::*;

#[multiversx_sc::module]
pub trait UtilsModule:
    liq_math::MathModule + liq_storage::StorageModule + common_events::EventsModule + view::ViewModule
{
    /// Computes the interest factor using a Taylor series expansion up to the fourth term.
    ///
    /// # Parameters
    /// - `rate`: The interest rate as a `BigUint`.
    /// - `delta_timestamp`: The time difference in seconds.
    ///
    /// # Returns
    /// - `BigUint`: The computed interest factor.
    fn calculate_interest_factor(
        &self,
        rate: &ManagedDecimal<Self::Api, NumDecimals>,
        delta_timestamp: u64,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        // Constants
        let bp = ManagedDecimal::from_raw_units(BigUint::from(BP), DECIMAL_PRECISION);
        let seconds_per_year_dec =
            ManagedDecimal::from_raw_units(BigUint::from(SECONDS_PER_YEAR), 0);
        // Convert `rate` to ManagedDecimal
        let rate_dec = rate.clone();
        // Convert `delta_timestamp` to ManagedDecimal
        let delta_dec = ManagedDecimal::from_raw_units(BigUint::from(delta_timestamp), 0);
        // Calculate x = (rate * delta_timestamp) / SECONDS_PER_YEAR
        let x = rate_dec.mul(delta_dec).div(seconds_per_year_dec).add(bp);

        x
    }

    /// Updates the borrow index for the given storage cache.
    ///
    /// # Parameters
    /// - `borrow_rate`: The borrow rate.
    /// - `delta_timestamp`: The time difference in seconds, between the last update and the current timestamp.
    /// - `storage_cache`: The storage cache to update.
    fn update_borrow_index(
        &self,
        borrow_rate: &ManagedDecimal<Self::Api, NumDecimals>,
        delta_timestamp: u64,
        storage_cache: &mut StorageCache<Self>,
    ) {
        let interest_factor = self.calculate_interest_factor(borrow_rate, delta_timestamp);

        storage_cache.borrow_index = storage_cache
            .borrow_index
            .clone()
            .mul_with_precision(interest_factor, DECIMAL_PRECISION);
    }

    /// Updates the supply index for the given storage cache.
    ///
    /// # Parameters
    /// - `rewards_increase`: The total interest earned (compound) for the suppliers.
    /// - `storage_cache`: The storage cache to update.
    fn update_supply_index(
        &self,
        rewards_increase: ManagedDecimal<Self::Api, NumDecimals>,
        storage_cache: &mut StorageCache<Self>,
    ) {
        if storage_cache.supplied_amount
            != ManagedDecimal::from_raw_units(
                BigUint::zero(),
                storage_cache.pool_params.decimals,
            )
        {
            // Convert rewards to an index increase factor
            let rewards_factor = rewards_increase.clone() / storage_cache.supplied_amount.clone()
                + ManagedDecimal::from_raw_units(BigUint::from(BP), DECIMAL_PRECISION);

            storage_cache.supply_index = storage_cache
                .supply_index
                .clone()
                .mul_with_precision(rewards_factor, DECIMAL_PRECISION);
        }
    }

    /// Updates the rewards reserves for the given storage cache.
    ///
    /// # Parameters
    /// - `borrow_rate`: The borrow rate.
    /// - `delta_timestamp`: The time difference in seconds, between the last update and the current timestamp.
    /// - `storage_cache`: The storage cache to update.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The total interest earned (compound) for the suppliers.
    fn update_rewards_reserves(
        &self,
        borrow_rate: &ManagedDecimal<Self::Api, NumDecimals>,
        delta_timestamp: u64,
        storage_cache: &mut StorageCache<Self>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        // 1. Calculate compound interest factor
        let interest_factor = self.calculate_interest_factor(borrow_rate, delta_timestamp);

        let borrowed_amount_dec = storage_cache.borrowed_amount.clone();
        // 2. Calculate total interest earned (compound)
        let new_borrowed_amount = borrowed_amount_dec.clone().mul(interest_factor);
        let rewards_increase = new_borrowed_amount - borrowed_amount_dec;

        // 3. Calculate protocol's share
        let revenue = rewards_increase
            .clone()
            .mul(storage_cache.pool_params.reserve_factor.clone());

        // 4. Update reserves
        storage_cache.protocol_revenue += &revenue;

        // 5. Return suppliers' share
        rewards_increase - revenue
    }

    /// Updates the interest indexes for the given storage cache.
    ///
    /// # Parameters
    /// - `storage_cache`: The storage cache to update.
    fn update_interest_indexes(&self, storage_cache: &mut StorageCache<Self>) {
        let delta_timestamp = storage_cache.timestamp - storage_cache.last_update_timestamp;

        if delta_timestamp > 0 {
            let borrow_rate = self.get_borrow_rate_internal(storage_cache);
            self.update_borrow_index(&borrow_rate, delta_timestamp, storage_cache);
            let rewards =
                self.update_rewards_reserves(&borrow_rate, delta_timestamp, storage_cache);
            self.update_supply_index(rewards, storage_cache);
            // Update the last used round
            storage_cache.last_update_timestamp = storage_cache.timestamp;
        }
    }

    /// Updates the position with interest for the given storage cache.
    ///
    /// # Parameters
    /// - `position`: The position to update.
    /// - `storage_cache`: The storage cache to update.
    fn internal_update_position_with_interest(
        &self,
        position: &mut AccountPosition<Self::Api>,
        storage_cache: &mut StorageCache<Self>,
    ) {
        self.update_interest_indexes(storage_cache);
        let is_supply = position.deposit_type == AccountPositionType::Deposit;

        let index = if is_supply {
            storage_cache.supply_index.clone()
        } else {
            storage_cache.borrow_index.clone()
        };

        let accumulated_interest_dec = self.compute_interest(
            ManagedDecimal::from_raw_units(
                position.get_total_amount().clone(),
                storage_cache.pool_params.decimals,
            ),
            &index,
            &ManagedDecimal::from_raw_units(position.index.clone(), DECIMAL_PRECISION),
        );

        let accumulated_interest = accumulated_interest_dec.into_raw_units();

        if accumulated_interest.gt(&BigUint::zero()) {
            position.accumulated_interest += accumulated_interest;
            position.timestamp = storage_cache.timestamp;
            position.index = index.into_raw_units().clone();
        }
    }
}
