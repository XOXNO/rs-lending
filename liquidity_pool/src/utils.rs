multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use crate::{contexts::base::StorageCache, rates, storage, view};

use common_constants::{BP, DECIMAL_PRECISION, SECONDS_PER_YEAR};
use common_structs::*;

/// The UtilsModule trait contains helper functions for updating interest indexes,
/// computing interest factors, and adjusting account positions with accrued interest.
#[multiversx_sc::module]
pub trait UtilsModule:
    rates::InterestRateMath + storage::StorageModule + common_events::EventsModule + view::ViewModule
{
    /// Computes the interest factor for a given time delta using a linear approximation.
    ///
    /// The interest factor represents the growth multiplier for interest accrual over the time interval,
    /// based on the current borrow rate.
    ///
    /// # Parameters
    /// - `storage_cache`: A mutable reference to the StorageCache containing the current market state.
    /// - `delta_timestamp`: The time elapsed (in seconds) since the last update.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The computed interest factor.
    fn calculate_interest_factor(
        &self,
        storage_cache: &mut StorageCache<Self>,
        delta_timestamp: u64,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        // Constants
        let rate = self.get_borrow_rate_internal(storage_cache);

        let bp = ManagedDecimal::from_raw_units(BigUint::from(BP), DECIMAL_PRECISION);
        let seconds_per_year_dec =
            ManagedDecimal::from_raw_units(BigUint::from(SECONDS_PER_YEAR), 0);
        // Convert `delta_timestamp` to ManagedDecimal
        let delta_dec = ManagedDecimal::from_raw_units(BigUint::from(delta_timestamp), 0);
        // Calculate x = (rate * delta_timestamp) / SECONDS_PER_YEAR
        let x = rate
            .clone()
            .mul_with_precision(delta_dec, DECIMAL_PRECISION)
            .div(seconds_per_year_dec)
            .add(bp);

        x
    }

    /// Updates the borrow index using the provided interest factor.
    ///
    /// This function multiplies the current borrow index by the interest factor to reflect accrued interest.
    ///
    /// # Parameters
    /// - `storage_cache`: The StorageCache containing current market state.
    /// - `interest_factor`: The computed interest factor.
    fn update_borrow_index(
        &self,
        storage_cache: &mut StorageCache<Self>,
        interest_factor: &ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        storage_cache.borrow_index = storage_cache
            .borrow_index
            .clone()
            .mul_with_precision(interest_factor.clone(), DECIMAL_PRECISION);
    }

    /// Updates the supply index based on net rewards for suppliers.
    ///
    /// Net rewards are calculated after subtracting the protocol fee from the total accrued interest.
    /// The supply index is updated by applying a rewards factor that increases depositors' yield.
    ///
    /// # Parameters
    /// - `rewards_increase`: The net accrued interest for suppliers.
    /// - `storage_cache`: The StorageCache containing current state.
    fn update_supply_index(
        &self,
        rewards_increase: ManagedDecimal<Self::Api, NumDecimals>,
        storage_cache: &mut StorageCache<Self>,
    ) {
        if storage_cache.supplied_amount != storage_cache.zero {
            let bp_dec = ManagedDecimal::from_raw_units(BigUint::from(BP), DECIMAL_PRECISION);

            // Convert rewards to an index increase factor
            let rewards_factor =
                rewards_increase * bp_dec.clone() / storage_cache.supplied_amount.clone() + bp_dec;

            storage_cache.supply_index = storage_cache
                .supply_index
                .clone()
                .mul_with_precision(rewards_factor, DECIMAL_PRECISION);
        }
    }

    /// Updates the rewards reserves by computing accrued interest on borrowings.
    ///
    /// The function calculates the new borrowed amount by applying the interest factor, determines the total accrued interest,
    /// computes the protocol fee using the reserve factor, updates protocol revenue, and returns the net rewards for suppliers.
    ///
    /// # Parameters
    /// - `storage_cache`: The StorageCache with current market data.
    /// - `interest_factor`: The computed interest factor.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The net accrued interest for suppliers.
    fn update_rewards_reserves(
        &self,
        storage_cache: &mut StorageCache<Self>,
        interest_factor: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let borrowed_amount_dec = storage_cache.borrowed_amount.clone();
        // 2. Calculate total interest earned (compound)
        let new_borrowed_amount = borrowed_amount_dec
            .clone()
            .mul_with_precision(interest_factor.clone(), storage_cache.pool_params.decimals);

        let rewards_increase = new_borrowed_amount - borrowed_amount_dec;

        // 3. Calculate protocol's share
        let revenue = rewards_increase.clone().mul_with_precision(
            storage_cache.pool_params.reserve_factor.clone(),
            storage_cache.pool_params.decimals,
        );

        // 4. Update reserves
        storage_cache.protocol_revenue += &revenue;

        // 5. Return suppliers' share
        rewards_increase - revenue
    }

    /// Updates both borrow and supply indexes based on the elapsed time.
    ///
    /// This function computes the interest factor from the time delta, updates the borrow index,
    /// calculates net rewards (by updating the rewards reserves), and applies these rewards to update the supply index.
    /// Finally, it refreshes the last update timestamp.
    ///
    /// # Parameters
    /// - `storage_cache`: The StorageCache containing the current state.
    fn update_interest_indexes(&self, storage_cache: &mut StorageCache<Self>) {
        let delta_timestamp = storage_cache.timestamp - storage_cache.last_update_timestamp;

        if delta_timestamp > 0 {
            let interest_factor = self.calculate_interest_factor(storage_cache, delta_timestamp);

            self.update_borrow_index(storage_cache, &interest_factor);
            let rewards = self.update_rewards_reserves(storage_cache, &interest_factor);
            self.update_supply_index(rewards, storage_cache);
            // Update the last used round
            storage_cache.last_update_timestamp = storage_cache.timestamp;
        }
    }

    /// Updates an account position with the accrued interest.
    ///
    /// For a given account position (either a deposit or borrow), this function calculates the additional interest accrued
    /// since the position's last update and adjusts the accumulated interest, timestamp, and index accordingly.
    ///
    /// # Parameters
    /// - `position`: The account position to update.
    /// - `storage_cache`: The StorageCache containing current market state.
    fn internal_update_position_with_interest(
        &self,
        position: &mut AccountPosition<Self::Api>,
        storage_cache: &mut StorageCache<Self>,
    ) {
        if position.get_total_amount().eq(&BigUint::zero()) {
            return;
        }

        let is_supply = position.deposit_type == AccountPositionType::Deposit;

        let index = if is_supply {
            storage_cache.supply_index.clone()
        } else {
            storage_cache.borrow_index.clone()
        };

        let accumulated_interest_dec = self.compute_interest(
            storage_cache.get_decimal_value(&position.get_total_amount()),
            &index,
            &ManagedDecimal::from_raw_units(position.index.clone(), DECIMAL_PRECISION),
        );

        if accumulated_interest_dec.gt(&storage_cache.zero) {
            position.accumulated_interest += accumulated_interest_dec.into_raw_units();
            position.timestamp = storage_cache.timestamp;
            position.index = index.into_raw_units().clone();
        }
    }

    /// Calculates how much of the repayment goes toward interest and how much toward principal.
    ///
    /// The function applies the received repayment first to cover as much of the outstanding
    /// interest as possible, and then any remaining amount is used to reduce the principal.
    /// In the case of an overpayment, the function will cap the amounts to the outstanding balances.
    ///
    /// # Parameters
    /// - `repayment`: The total repayment amount received.
    /// - `outstanding_interest`: The total interest that is currently owed.
    /// - `outstanding_principal`: The total principal that is currently owed.
    /// - `total_debt`: The total debt that is currently owed.
    ///
    /// # Returns
    /// A tuple containing:
    /// - `(principal_repaid, interest_repaid)`
    ///   - `principal_repaid`: The portion of the repayment that will reduce the principal.
    ///   - `interest_repaid`: The portion of the repayment that will reduce the interest.
    ///   - `over_repaid`: The portion of the repayment that will be refunded to the caller.
    fn calculate_interest_and_principal(
        &self,
        repayment: &ManagedDecimal<Self::Api, NumDecimals>,
        outstanding_interest: ManagedDecimal<Self::Api, NumDecimals>,
        outstanding_principal: ManagedDecimal<Self::Api, NumDecimals>,
        total_debt: ManagedDecimal<Self::Api, NumDecimals>,
    ) -> (
        ManagedDecimal<Self::Api, NumDecimals>, // principal_repaid
        ManagedDecimal<Self::Api, NumDecimals>, // interest_repaid
        BigUint,                                // over_repaid
    ) {
        if repayment >= &total_debt {
            // Full repayment with possible overpayment.
            let over_repaid = repayment.clone() - total_debt;
            // The entire outstanding debt is cleared.
            (
                outstanding_principal,
                outstanding_interest,
                over_repaid.into_raw_units().clone(),
            )
        } else {
            // Partial repayment: first cover interest, then principal.
            let interest_repaid = if repayment > &outstanding_interest {
                outstanding_interest.clone()
            } else {
                repayment.clone()
            };
            let remaining = repayment.clone() - interest_repaid.clone();
            let principal_repaid = if remaining > outstanding_principal {
                outstanding_principal.clone()
            } else {
                remaining
            };
            (principal_repaid, interest_repaid, BigUint::zero())
        }
    }
}
