use common_constants::{RAY_PRECISION, SECONDS_PER_YEAR};

use crate::{contexts::base::Cache, storage};

multiversx_sc::imports!();

/// The InterestRates module provides functions for calculating market rates,
/// interest accrual, and capital utilization based on pool parameters and current state.
///
/// **Scope**: Manages dynamic interest rates and index updates for the lending pool.
///
/// **Goal**: Ensure accurate, fair, and auditable interest mechanics for borrowers and suppliers.
#[multiversx_sc::module]
pub trait InterestRates: common_math::SharedMathModule + storage::Storage {
    /// Calculates the borrow rate based on current utilization and pool parameters.
    ///
    /// **Scope**: Determines the interest rate borrowers pay based on pool utilization.
    ///
    /// **Goal**: Provide a dynamic rate that adjusts with demand, using a piecewise linear model.
    ///
    /// **Formula**:
    /// - If `utilization <= mid_utilization`: `base_borrow_rate + (utilization * slope1 / mid_utilization)`.
    /// - If `mid_utilization < utilization < optimal_utilization`: `base_borrow_rate + slope1 + ((utilization - mid_utilization) * slope2 / (optimal_utilization - mid_utilization))`.
    /// - If `utilization >= optimal_utilization`: `base_borrow_rate + slope1 + slope2 + ((utilization - optimal_utilization) * slope3 / (1 - optimal_utilization))`.
    /// - Capped at `max_borrow_rate` and converted to a per-second rate.
    ///
    /// # Arguments
    /// - `cache`: Reference to the pool state (`Cache<Self>`), providing utilization and parameters.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: Per-second borrow rate (RAY-based).
    ///
    /// **Security Tip**: Relies on `cache.get_utilization()`; no direct input validation.
    fn calc_borrow_rate(&self, cache: &Cache<Self>) -> ManagedDecimal<Self::Api, NumDecimals> {
        let utilization = cache.get_utilization();
        let pool_params = cache.pool_params.clone();
        let sec_per_year = ManagedDecimal::from_raw_units(BigUint::from(SECONDS_PER_YEAR), 0);

        let annual_rate = if utilization < pool_params.mid_utilization {
            // Region 1: utilization < mid_utilization
            let utilization_ratio = utilization.mul(pool_params.slope1).div(pool_params.mid_utilization);
            pool_params.base_borrow_rate.add(utilization_ratio)
        } else if utilization < pool_params.optimal_utilization {
            // Region 2: mid_utilization <= utilization < optimal_utilization
            let excess_utilization = utilization.sub(pool_params.mid_utilization.clone());
            let slope_contribution = excess_utilization
                .mul(pool_params.slope2)
                .div(pool_params.optimal_utilization.sub(pool_params.mid_utilization));
            pool_params
                .base_borrow_rate
                .add(pool_params.slope1)
                .add(slope_contribution)
        } else {
            // Region 3: utilization >= optimal_utilization, linear growth
            let base_rate = pool_params
                .base_borrow_rate
                .add(pool_params.slope1)
                .add(pool_params.slope2);
            let excess_utilization = utilization.sub(pool_params.optimal_utilization.clone());
            let slope_contribution = excess_utilization
                .mul(pool_params.slope3)
                .div(self.ray().sub(pool_params.optimal_utilization));
            base_rate.add(slope_contribution)
        };

        // Cap the rate at max_borrow_rate
        let capped_rate = if annual_rate > pool_params.max_borrow_rate {
            pool_params.max_borrow_rate
        } else {
            annual_rate
        };

        // Convert annual rate to per-second rate
        let per_second_rate = capped_rate / sec_per_year;

        per_second_rate
    }

    /// Calculates the deposit rate based on utilization and borrow rate.
    ///
    /// **Scope**: Computes the rate suppliers earn from borrowers’ interest payments.
    ///
    /// **Goal**: Ensure suppliers receive a fair share of interest after protocol fees.
    ///
    /// **Formula**:
    /// - `deposit_rate = utilization * borrow_rate * (1 - reserve_factor)`.
    ///
    /// # Arguments
    /// - `utilization`: Current utilization ratio (`ManagedDecimal<Self::Api, NumDecimals>`).
    /// - `borrow_rate`: Current borrow rate (`ManagedDecimal<Self::Api, NumDecimals>`).
    /// - `reserve_factor`: Protocol fee fraction (`ManagedDecimal<Self::Api, NumDecimals>`).
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: Per-second deposit rate (RAY-based).
    ///
    /// **Security Tip**: Assumes inputs are valid; no overflow or underflow checks.
    fn calc_deposit_rate(
        &self,
        utilization: ManagedDecimal<Self::Api, NumDecimals>,
        borrow_rate: ManagedDecimal<Self::Api, NumDecimals>,
        reserve_factor: ManagedDecimal<Self::Api, NumDecimals>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let rate = self.mul_half_up(
            &self.mul_half_up(&utilization, &borrow_rate, RAY_PRECISION),
            &self.bps().sub(reserve_factor),
            RAY_PRECISION,
        );

        rate
    }

    /// Computes the interest growth factor using a Taylor series approximation.
    ///
    /// **Scope**: Approximates compounded interest over time for index updates.
    ///
    /// **Goal**: Provide a precise interest factor for small time intervals.
    ///
    /// **Formula**:
    /// - `factor = 1 + (r * t) + (r * t)^2 / 2 + (r * t)^3 / 6`.
    /// - Where `r` is the per-second borrow rate, `t` is elapsed seconds (`exp`).
    /// - Suitable for small `t`; precision decreases for large intervals.
    ///
    /// # Arguments
    /// - `cache`: Mutable reference to pool state (`Cache<Self>`), used for borrow rate.
    /// - `exp`: Time elapsed in seconds (`u64`).
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: Interest factor (RAY-based).
    ///
    /// **Security Tip**: Returns 1 (RAY) if `exp == 0` to avoid unnecessary computation.
    fn growth_factor(
        &self,
        cache: &mut Cache<Self>,
        exp: u64,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let ray = self.ray(); // ManagedDecimal::from_raw_units(BigUint::from(RAY), RAY_PRECISION)
        if exp == 0 {
            return ray;
        }

        let exp_dec = ManagedDecimal::from_raw_units(BigUint::from(exp), 0);
        let borrow_rate = self.calc_borrow_rate(&cache);

        let exp_minus_one = exp - 1;
        let exp_minus_two = if exp > 2 { exp - 2 } else { 0 };
        let exp_minus_one_dec = ManagedDecimal::from_raw_units(BigUint::from(exp_minus_one), 0);
        let exp_minus_two_dec = ManagedDecimal::from_raw_units(BigUint::from(exp_minus_two), 0);

        // Base powers using per-second rate
        let base_power_two = self.mul_half_up(&borrow_rate, &borrow_rate, RAY_PRECISION);
        let base_power_three = self.mul_half_up(&base_power_two, &borrow_rate, RAY_PRECISION);

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
        let main_term = self.mul_half_up(&borrow_rate, &exp_dec, RAY_PRECISION);

        // Interest factor = 1 + main_term + second_term + third_term
        let factor = ray + main_term + second_term + third_term;
        factor
    }

    /// Updates the borrow index using the provided interest factor.
    ///
    /// **Scope**: Adjusts the borrow index to reflect compounded interest over time.
    ///
    /// **Goal**: Keep the borrow index current for accurate debt calculations.
    ///
    /// **Formula**:
    /// - `new_borrow_index = old_borrow_index * interest_factor`.
    ///
    /// # Arguments
    /// - `cache`: Mutable reference to pool state (`Cache<Self>`), holding the borrow index.
    /// - `interest_factor`: Computed interest factor (`ManagedDecimal<Self::Api, NumDecimals>`).
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The old borrow index before update.
    ///
    /// **Security Tip**: Assumes `interest_factor` is valid; no overflow checks.
    fn update_borrow_index(
        &self,
        cache: &mut Cache<Self>,
        interest_factor: ManagedDecimal<Self::Api, NumDecimals>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let old_index = cache.borrow_index.clone();
        cache.borrow_index = self.mul_half_up(&cache.borrow_index, &interest_factor, RAY_PRECISION);

        old_index
    }

    /// Updates the supply index based on net rewards for suppliers.
    ///
    /// **Scope**: Adjusts the supply index to distribute rewards to suppliers.
    ///
    /// **Goal**: Ensure suppliers’ yields reflect their share of interest earned.
    ///
    /// **Formula**:
    /// - `rewards_ratio = rewards_increase / supplied`.
    /// - `rewards_factor = 1 + rewards_ratio`.
    /// - `new_supply_index = old_supply_index * rewards_factor`.
    ///
    /// # Arguments
    /// - `rewards_increase`: Net rewards for suppliers (`ManagedDecimal<Self::Api, NumDecimals>`).
    /// - `cache`: Mutable reference to pool state (`Cache<Self>`), holding supplied amount and index.
    ///
    /// **Security Tip**: Skips update if `supplied == 0` to avoid division-by-zero.
    fn update_supply_index(
        &self,
        rewards_increase: ManagedDecimal<Self::Api, NumDecimals>,
        cache: &mut Cache<Self>,
    ) {
        if cache.supplied != cache.zero {
            let rewards_ratio = self.div_half_up(&rewards_increase, &cache.supplied, RAY_PRECISION);

            let rewards_factor = self.ray() + rewards_ratio;

            cache.supply_index =
                self.mul_half_up(&cache.supply_index, &rewards_factor, RAY_PRECISION);
        }
    }
}
