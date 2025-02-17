multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use crate::{contexts::base::StorageCache, rates, storage, view};

use common_constants::{RAY, RAY_PRECISION, SECONDS_PER_YEAR};
use common_structs::*;

/// The UtilsModule trait contains helper functions for updating interest indexes,
/// computing interest factors, and adjusting account positions with accrued interest.
#[multiversx_sc::module]
pub trait UtilsModule:
    rates::InterestRateMath
    + storage::StorageModule
    + common_events::EventsModule
    + view::ViewModule
    + common_math::SharedMathModule
{
    fn calculate_interest_factor(
        &self,
        storage_cache: &mut StorageCache<Self>,
        exp: u64,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let ray = self.ray(); // ManagedDecimal::from_raw_units(BigUint::from(RAY), RAY_PRECISION)
        if exp == 0 {
            return ray;
        }

        let exp_dec = ManagedDecimal::from_raw_units(BigUint::from(exp), 0);
        let per_second_rate = self.get_borrow_rate_internal(storage_cache);
        sc_print!("Per-second rate: {}", per_second_rate);

        let exp_minus_one = exp - 1;
        let exp_minus_two = if exp > 2 { exp - 2 } else { 0 };
        let exp_minus_one_dec = ManagedDecimal::from_raw_units(BigUint::from(exp_minus_one), 0);
        let exp_minus_two_dec = ManagedDecimal::from_raw_units(BigUint::from(exp_minus_two), 0);

        // Base powers using per-second rate
        let base_power_two = self.mul_half_up(&per_second_rate, &per_second_rate, RAY_PRECISION);
        let base_power_three = self.mul_half_up(&base_power_two, &per_second_rate, RAY_PRECISION);

        // Second term: (exp * (exp - 1) * base_power_two) / 2
        let second_term = self.div_half_up(
            &self.mul_half_up(
                &self.mul_half_up(&exp_dec, &exp_minus_one_dec, RAY_PRECISION),
                &base_power_two,
                RAY_PRECISION,
            ),
            &ManagedDecimal::from_raw_units(BigUint::from(2u64), 0),
            RAY_PRECISION,
        );

        // Third term: (exp * (exp - 1) * (exp - 2) * base_power_three) / 6
        let third_term = self.div_half_up(
            &self.mul_half_up(
                &self.mul_half_up(
                    &self.mul_half_up(&exp_dec, &exp_minus_one_dec, RAY_PRECISION),
                    &exp_minus_two_dec,
                    RAY_PRECISION,
                ),
                &base_power_three,
                RAY_PRECISION,
            ),
            &ManagedDecimal::from_raw_units(BigUint::from(6u64), 0),
            RAY_PRECISION,
        );

        // Main term: per_second_rate * exp
        let main_term = self.mul_half_up(&per_second_rate, &exp_dec, RAY_PRECISION);

        // Interest factor = 1 + main_term + second_term + third_term
        let interest_factor = ray + main_term + second_term + third_term;
        sc_print!("Interest factor: {}", interest_factor);
        interest_factor
    }
    // fn calculate_interest_factor(
    //     &self,
    //     storage_cache: &mut StorageCache<Self>,
    //     exp: u64,
    // ) -> ManagedDecimal<Self::Api, NumDecimals> {
    //     // if exp == 0 {
    //     //     return self.ray();
    //     // }

    //     // let exp_dec = ManagedDecimal::from_raw_units(BigUint::from(exp), 0);
    //     // let per_second_rate = self.get_borrow_rate_internal(storage_cache);

    //     // // Linear approximation: interest_factor = 1 + (per_second_rate * exp)
    //     // let interest_factor =
    //     //     self.ray() + self.mul_half_up(&per_second_rate, &exp_dec, RAY_PRECISION);
    //     // interest_factor
    //     // let ray = ManagedDecimal::from_raw_units(BigUint::from(RAY), RAY_PRECISION);

    //     // if exp == 0 {
    //     //     return self.ray();
    //     // };

    //     // let exp_dec = ManagedDecimal::from_raw_units(BigUint::from(exp), 0);

    //     // let rate = self.get_borrow_rate_internal(storage_cache);

    //     // let seconds_per_year_dec =
    //     //     ManagedDecimal::from_raw_units(BigUint::from(SECONDS_PER_YEAR), 0);

    //     // let exp_minus_one = exp - 1;
    //     // let exp_minus_two = if exp > 2 { exp - 2 } else { 0 };
    //     // let exp_minus_one_dec = ManagedDecimal::from_raw_units(BigUint::from(exp_minus_one), 0);
    //     // let exp_minus_two_dec = ManagedDecimal::from_raw_units(BigUint::from(exp_minus_two), 0);

    //     // let base_power_two = rate.clone().mul_with_precision(rate.clone(), RAY_PRECISION)
    //     //     / (seconds_per_year_dec.clone() * seconds_per_year_dec.clone());

    //     // let base_power_three = base_power_two
    //     //     .clone()
    //     //     .mul_with_precision(rate.clone(), RAY_PRECISION)
    //     //     / seconds_per_year_dec.clone();

    //     // let second_term = (exp_dec
    //     //     .clone()
    //     //     .mul_with_precision(exp_minus_one_dec.clone(), RAY_PRECISION)
    //     //     .mul_with_precision(base_power_two.clone(), RAY_PRECISION))
    //     //     / 2;

    //     // let third_term =
    //     //     (exp_dec.clone() * exp_minus_one_dec * exp_minus_two_dec * base_power_three) / 6;

    //     // self.ray() + (rate * exp_dec) / seconds_per_year_dec + second_term + third_term
    // }

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
        storage_cache.borrow_index =
            self.mul_half_up(&storage_cache.borrow_index, &interest_factor, RAY_PRECISION);
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
            let ray = storage_cache.ray.clone();
            let total_supplied_amount = self.mul_half_up(
                &storage_cache.supplied_amount,
                &storage_cache.supply_index,
                RAY_PRECISION,
            );

            let rewards_ratio =
                self.div_half_up(&rewards_increase, &total_supplied_amount, RAY_PRECISION);

            let rewards_factor = ray + rewards_ratio;
            storage_cache.supply_index =
                self.mul_half_up(&storage_cache.supply_index, &rewards_factor, RAY_PRECISION);
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
        old_borrow_index: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        // // Get total borrow + interest with old borrow index
        let total_debt_current_debt_with_interest = self
            .mul_half_up(
                &storage_cache.borrowed_amount,
                old_borrow_index,
                RAY_PRECISION,
            )
            .rescale(storage_cache.pool_params.decimals);

        let new_interest = self.compute_interest(
            &total_debt_current_debt_with_interest,
            &storage_cache.borrow_index,
            old_borrow_index,
        );

        // 3. Calculate protocol's share
        let revenue = self
            .mul_half_up(
                &new_interest,
                &storage_cache.pool_params.reserve_factor,
                RAY_PRECISION,
            )
            .rescale(storage_cache.pool_params.decimals);
        // 4. Update reserves
        storage_cache.protocol_revenue += &revenue;

        // 5. Return suppliers' share
        new_interest - revenue
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
        let delta_timestamp = storage_cache.timestamp - storage_cache.last_timestamp;

        if delta_timestamp > 0 {
            let factor = self.calculate_interest_factor(storage_cache, delta_timestamp);

            let old_borrow_index = storage_cache.borrow_index.clone();

            self.update_borrow_index(storage_cache, &factor);

            let rewards = self.update_rewards_reserves(storage_cache, &old_borrow_index);
            self.update_supply_index(rewards.clone(), storage_cache);

            // Update the last used timestamp
            storage_cache.last_timestamp = storage_cache.timestamp;
        }
    }

    /// Computes the interest accrued on a given position.
    ///
    /// The accrued interest is calculated using the formula:
    /// `interest = amount * (current_index / account_position_index) - amount`.
    ///
    /// # Parameters
    /// - `amount`: The principal amount of the asset.
    /// - `current_index`: The current market index (reflecting compounded interest).
    /// - `account_position_index`: The index at the time the position was last updated.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The interest accrued since the last update.

    fn compute_interest(
        &self,
        amount: &ManagedDecimal<Self::Api, NumDecimals>, // Amount of the asset
        current_index: &ManagedDecimal<Self::Api, NumDecimals>, // Market index
        account_position_index: &ManagedDecimal<Self::Api, NumDecimals>, // Account position index
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        // // Auto keeps the decimals of the amount
        let numerator = self.mul_half_up(amount, current_index, RAY_PRECISION);
        let new_amount = self
            .div_half_up(&numerator, account_position_index, RAY_PRECISION)
            .rescale(amount.scale());

        new_amount.sub(amount.clone())
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
        let is_supply = position.deposit_type == AccountPositionType::Deposit;
        let index = if is_supply {
            storage_cache.supply_index.clone()
        } else {
            storage_cache.borrow_index.clone()
        };

        if position.get_total_amount().eq(&storage_cache.zero) {
            position.timestamp = storage_cache.timestamp;
            position.index = index.clone();
            return;
        }
        let accumulated_interest =
            self.compute_interest(&position.get_total_amount(), &index, &position.index);

        if accumulated_interest.gt(&storage_cache.zero) {
            position.accumulated_interest += &accumulated_interest;
            position.timestamp = storage_cache.timestamp;
            position.index = index.clone();
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
        repayment: ManagedDecimal<Self::Api, NumDecimals>,
        outstanding_interest: ManagedDecimal<Self::Api, NumDecimals>,
        outstanding_principal: ManagedDecimal<Self::Api, NumDecimals>,
        total_debt: ManagedDecimal<Self::Api, NumDecimals>,
    ) -> (
        ManagedDecimal<Self::Api, NumDecimals>, // principal_repaid
        ManagedDecimal<Self::Api, NumDecimals>, // interest_repaid
        BigUint,                                // over_repaid
    ) {
        if repayment >= total_debt {
            // Full repayment with possible overpayment.
            let over_repaid = repayment - total_debt;
            // The entire outstanding debt is cleared.
            (
                outstanding_principal,
                outstanding_interest,
                over_repaid.into_raw_units().clone(),
            )
        } else {
            // Partial repayment: first cover interest, then principal.
            let interest_repaid = if repayment > outstanding_interest {
                outstanding_interest.clone()
            } else {
                repayment.clone()
            };
            let remaining = repayment - interest_repaid.clone();
            let principal_repaid = if remaining > outstanding_principal {
                outstanding_principal
            } else {
                remaining
            };
            (principal_repaid, interest_repaid, BigUint::zero())
        }
    }
}
