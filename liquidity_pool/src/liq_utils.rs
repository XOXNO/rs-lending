multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use crate::{contexts::base::StorageCache, liq_math, liq_storage, view};

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
        let x = rate_dec.mul(delta_dec).div(seconds_per_year_dec);
        // Calculate terms using Taylor series expansion
        let term1 = x.clone(); // x
        let term2 = x
            .clone()
            .mul_with_precision(x.clone(), DECIMAL_PRECISION)
            .div(ManagedDecimal::from_raw_units(BigUint::from(2u32), 0)); // x²/2
        let term3 = x
            .clone()
            .mul_with_precision(x.clone(), DECIMAL_PRECISION)
            .mul_with_precision(x.clone(), DECIMAL_PRECISION)
            .div(ManagedDecimal::from_raw_units(BigUint::from(6u32), 0)); // x³/6
        let term4 = x
            .clone()
            .mul_with_precision(x.clone(), DECIMAL_PRECISION)
            .mul_with_precision(x.clone(), DECIMAL_PRECISION)
            .mul_with_precision(x.clone(), DECIMAL_PRECISION)
            .div(ManagedDecimal::from_raw_units(BigUint::from(24u32), 0)); // x⁴/24
                                                                           // Final formula: 1 + x + x²/2! + x³/3! + x⁴/4!
        let interest_factor_dec = bp.clone().add(term1).add(term2).add(term3).add(term4);
        // Convert the ManagedDecimal back to BigUint
        interest_factor_dec
    }

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

    fn update_supply_index(
        &self,
        rewards_increase: ManagedDecimal<Self::Api, NumDecimals>,
        storage_cache: &mut StorageCache<Self>,
    ) {
        if storage_cache.supplied_amount
            != ManagedDecimal::from_raw_units(
                BigUint::from(0u64),
                storage_cache.pool_params.decimals,
            )
        {
            // Convert rewards to an index increase factor
            let rewards_factor: ManagedDecimal<<Self as ContractBase>::Api, _> =
                rewards_increase.clone() / storage_cache.supplied_amount.clone()
                    + ManagedDecimal::from_raw_units(BigUint::from(BP), DECIMAL_PRECISION);

            storage_cache.supply_index = storage_cache
                .supply_index
                .clone()
                .mul_with_precision(rewards_factor, DECIMAL_PRECISION);
        }
    }

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
        storage_cache.rewards_reserve += &revenue;

        // 5. Return suppliers' share
        rewards_increase - revenue
    }

    fn get_timestamp_diff(&self, initial_timestamp: u64, current_timestamp: u64) -> u64 {
        current_timestamp - initial_timestamp
    }

    fn update_interest_indexes(&self, storage_cache: &mut StorageCache<Self>) {
        let delta_timestamp =
            self.get_timestamp_diff(storage_cache.last_update_timestamp, storage_cache.timestamp);

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

    fn internal_update_position_with_interest(
        &self,
        mut position: AccountPosition<Self::Api>,
        storage_cache: &mut StorageCache<Self>,
    ) -> AccountPosition<Self::Api> {
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

            self.update_position_event(
                accumulated_interest,
                &position,
                OptionalValue::None,
                OptionalValue::None,
                OptionalValue::None,
            );
        }

        position
    }
}
