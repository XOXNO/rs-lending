use common_constants::{RAY_PRECISION, SECONDS_PER_YEAR};
use common_structs::MarketParams;

use crate::{cache::Cache, storage};

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
    /// - If `utilization >= optimal_utilization`: `base_borrow_rate + slope1 + slope2 + ((utilization - optimal_utilization) * slope3 / (RAY - optimal_utilization))`.
    /// - The annual rate is then capped at `max_borrow_rate`.
    /// - The capped annual rate is converted to a per-second rate by dividing by `SECONDS_PER_YEAR`.
    ///
    /// # Arguments
    /// - `utilization`: Current pool utilization ratio (`ManagedDecimal<Self::Api, NumDecimals>`), RAY-based.
    /// - `params`: Market parameters (`MarketParams<Self::Api>`) containing rate model configuration.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: Per-second borrow rate (RAY-based).
    ///
    /// **Security Tip**: Relies on the caller to provide valid `utilization` and `params` inputs.
    fn calc_borrow_rate(
        &self,
        utilization: ManagedDecimal<Self::Api, NumDecimals>,
        params: MarketParams<Self::Api>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let annual_rate = if utilization < params.mid_utilization {
            // Region 1: utilization < mid_utilization
            let utilization_ratio = utilization.mul(params.slope1).div(params.mid_utilization);
            params.base_borrow_rate.add(utilization_ratio)
        } else if utilization < params.optimal_utilization {
            // Region 2: mid_utilization <= utilization < optimal_utilization
            let excess_utilization = utilization.sub(params.mid_utilization.clone());
            let slope_contribution = excess_utilization
                .mul(params.slope2)
                .div(params.optimal_utilization.sub(params.mid_utilization));
            params
                .base_borrow_rate
                .add(params.slope1)
                .add(slope_contribution)
        } else {
            // Region 3: utilization >= optimal_utilization, linear growth
            let base_rate = params
                .base_borrow_rate
                .add(params.slope1)
                .add(params.slope2);
            let excess_utilization = utilization.sub(params.optimal_utilization.clone());
            let slope_contribution = excess_utilization
                .mul(params.slope3)
                .div(self.ray().sub(params.optimal_utilization));
            base_rate.add(slope_contribution)
        };

        // Cap the rate at max_borrow_rate
        let capped_rate = if annual_rate > params.max_borrow_rate {
            params.max_borrow_rate
        } else {
            annual_rate
        };

        // Convert annual rate to per-second rate
        self.div_half_up(
            &capped_rate,
            &self.to_decimal(BigUint::from(SECONDS_PER_YEAR), 0),
            RAY_PRECISION,
        )
    }

    /// Calculates the deposit rate based on utilization, borrow rate, and reserve factor.
    ///
    /// **Scope**: Computes the rate suppliers earn from borrowers' interest payments.
    ///
    /// **Goal**: Ensure suppliers receive a fair share of interest after protocol fees.
    ///
    /// **Formula**:
    /// - `deposit_rate = utilization * borrow_rate * (1 - reserve_factor)`.
    /// - If `utilization` is zero, `deposit_rate` is zero.
    /// - `(1 - reserve_factor)` is calculated as `self.bps().sub(reserve_factor)`, assuming `bps()` represents 100% (scaled to RAY) and `reserve_factor` is also RAY-scaled.
    ///
    /// # Arguments
    /// - `utilization`: Current utilization ratio (`ManagedDecimal<Self::Api, NumDecimals>`), RAY-based.
    /// - `borrow_rate`: Current per-second borrow rate (`ManagedDecimal<Self::Api, NumDecimals>`), RAY-based.
    /// - `reserve_factor`: Protocol fee fraction (`ManagedDecimal<Self::Api, NumDecimals>`), RAY-based (representing a BPS value).
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: Per-second deposit rate (RAY-based).
    ///
    /// **Security Tip**: Assumes inputs are valid; no overflow or underflow checks within this specific function beyond standard `ManagedDecimal` operations.
    fn calc_deposit_rate(
        &self,
        utilization: ManagedDecimal<Self::Api, NumDecimals>,
        borrow_rate: ManagedDecimal<Self::Api, NumDecimals>,
        reserve_factor: ManagedDecimal<Self::Api, NumDecimals>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        if utilization == self.ray_zero() {
            return self.ray_zero();
        }

        self.mul_half_up(
            &self.mul_half_up(&utilization, &borrow_rate, RAY_PRECISION),
            &self.bps().sub(reserve_factor),
            RAY_PRECISION,
        )
    }

    /// Calculates the interest accumulation factor using a linear interest rate formula.
    ///
    /// **Formula**:
    /// - `Interest Factor = 1 + (rate * time_passed)`
    ///
    /// # Arguments
    /// - `rate`: The per-second interest rate (`ManagedDecimal<Self::Api, NumDecimals>`), in RAY.
    /// - `time_passed`: The duration in seconds (`u64`) for which interest is calculated.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The interest accumulation factor `(1 + r*t)`, in RAY.
    fn calculate_linear_interest(
        &self,
        rate: ManagedDecimal<Self::Api, NumDecimals>,
        time_passed: u64,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let factor = self.mul_half_up(
            &rate,
            &self.to_decimal(BigUint::from(time_passed), 0),
            RAY_PRECISION,
        );

        self.ray() + factor
    }

    /// Computes the interest growth factor using a Taylor series approximation for `e^(rate * exp)`.
    ///
    /// **Scope**: Approximates compounded interest over time for index updates.
    ///
    /// **Goal**: Provide a precise interest factor for small time intervals.
    ///
    /// **Formula**:
    /// - Approximates `e^x` where `x = rate * exp` using the Taylor expansion:
    ///   `factor = 1 + x + x^2/2! + x^3/3! + x^4/4! + x^5/5!`.
    /// - If `exp == 0`, returns `1` (RAY-scaled).
    /// - Suitable for small `x`; precision decreases for large intervals.
    ///
    /// # Arguments
    /// - `rate`: Current per-second borrow rate (`ManagedDecimal<Self::Api, NumDecimals>`), RAY-based.
    /// - `exp`: Time elapsed in seconds (`u64`).
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: Interest growth factor (RAY-based).
    ///
    /// **Security Tip**: Handles `exp == 0` to avoid unnecessary computation and potential division by zero if terms were structured differently.
    fn calculate_compounded_interest(
        &self,
        rate: ManagedDecimal<Self::Api, NumDecimals>,
        exp: u64,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        // Use Taylor expansion e^x = 1 + x + x^2/2! + x^3/3! + x^4/4! + x^5/5! + ...
        // where x = borrow_rate * exp

        let ray = self.ray();

        if exp == 0 {
            return ray;
        }

        let exp_dec = self.to_decimal(BigUint::from(exp), 0);

        // x = rate * time_delta
        let x = self.mul_half_up(&rate, &exp_dec, RAY_PRECISION);

        // Higher powers of x
        let x_sq = self.mul_half_up(&x, &x, RAY_PRECISION);
        let x_cub = self.mul_half_up(&x_sq, &x, RAY_PRECISION);
        let x_pow4 = self.mul_half_up(&x_cub, &x, RAY_PRECISION);
        let x_pow5 = self.mul_half_up(&x_pow4, &x, RAY_PRECISION);

        // Denominators for factorials
        let factor_2 = self.to_decimal(BigUint::from(2u64), 0);
        let factor_6 = self.to_decimal(BigUint::from(6u64), 0);
        let factor_24 = self.to_decimal(BigUint::from(24u64), 0);
        let factor_120 = self.to_decimal(BigUint::from(120u64), 0);

        // Calculate terms: x^n / n!
        let term2 = self.div_half_up(&x_sq, &factor_2, RAY_PRECISION);
        let term3 = self.div_half_up(&x_cub, &factor_6, RAY_PRECISION);
        let term4 = self.div_half_up(&x_pow4, &factor_24, RAY_PRECISION);
        let term5 = self.div_half_up(&x_pow5, &factor_120, RAY_PRECISION);

        // Sum terms: 1 + x + x^2/2 + x^3/6 + x^4/24 + x^5/120
        ray + x + term2 + term3 + term4 + term5
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
    /// - `interest_factor`: Computed interest growth factor (`ManagedDecimal<Self::Api, NumDecimals>`), RAY-based.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The old borrow index before update (RAY-based).
    ///
    /// **Security Tip**: Assumes `interest_factor` is valid; relies on `ManagedDecimal` operations for overflow checks.
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
    /// **Goal**: Ensure suppliers' yields reflect their share of interest earned.
    ///
    /// **Formula**:
    /// - `current_total_supplied_value_ray = cache.supplied * old_supply_index`
    /// - `rewards_ratio = rewards_increase_ray / current_total_supplied_value_ray` (if `current_total_supplied_value_ray > 0`).
    /// - `rewards_factor = 1 + rewards_ratio`.
    /// - `new_supply_index = old_supply_index * rewards_factor`.
    ///
    /// # Arguments
    /// - `cache`: Mutable reference to pool state (`Cache<Self>`), holding supplied amount and supply index.
    /// - `rewards_increase`: Net rewards for suppliers (`ManagedDecimal<Self::Api, NumDecimals>`), RAY-based.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The old supply index before update (RAY-based).
    ///
    /// **Security Tip**: Skips update if `cache.supplied == 0` (which implies `current_total_supplied_value_ray` would be zero if `old_supply_index` is not zero, or if `old_supply_index` is zero) to avoid division-by-zero.
    fn update_supply_index(
        &self,
        cache: &mut Cache<Self>,
        rewards_increase: ManagedDecimal<Self::Api, NumDecimals>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let old_supply_index = cache.supply_index.clone();
        if cache.supplied != cache.zero {
            let total_supplied_with_interest =
                self.mul_half_up(&cache.supplied, &cache.supply_index, RAY_PRECISION);
            let rewards_ratio = self.div_half_up(
                &rewards_increase,
                &total_supplied_with_interest,
                RAY_PRECISION,
            );

            let rewards_factor = self.ray() + rewards_ratio;

            cache.supply_index =
                self.mul_half_up(&cache.supply_index, &rewards_factor, RAY_PRECISION);
        }
        old_supply_index
    }

    /// Calculates supplier rewards by deducting protocol fees from accrued interest, after accounting for bad debt.
    ///
    /// **Scope**: Computes the rewards suppliers earn from interest paid by borrowers,
    /// after the protocol takes its share (reserve factor) and bad debt is covered. Used during index updates.
    ///
    /// **Goal**: Ensure suppliers receive their fair share of interest while updating protocol revenue and bad debt.
    ///
    /// **Formula**:
    /// 1. `total_scaled_borrowed = cache.borrowed` (this is the sum of initial principals scaled by their initial borrow indexes).
    /// 2. `accrued_interest_ray = total_scaled_borrowed * new_borrow_index - total_scaled_borrowed * old_borrow_index`.
    /// 3. If `accrued_interest_asset_decimals <= bad_debt_asset_decimals`:
    ///    - `new_bad_debt_asset_decimals = bad_debt_asset_decimals - accrued_interest_asset_decimals`.
    ///    - `supplier_rewards_ray = 0`.
    /// 4. Else (`accrued_interest_asset_decimals > bad_debt_asset_decimals`):
    ///    - `left_interest_asset_decimals = accrued_interest_asset_decimals - bad_debt_asset_decimals`.
    ///    - `new_bad_debt_asset_decimals = 0`.
    ///    - `protocol_fee_ray = left_interest_ray * reserve_factor`. (where `left_interest_ray` is `left_interest_asset_decimals` rescaled to RAY)
    ///    - `supplier_rewards_ray = left_interest_ray - protocol_fee_ray`.
    /// 5. `cache.revenue` is incremented by `protocol_fee_ray`.
    ///
    /// # Arguments
    /// - `cache`: Mutable reference to the pool state (`Cache<Self>`), containing borrow amounts, indexes, params, bad debt, and revenue.
    /// - `old_borrow_index`: The borrow index before the current update (`ManagedDecimal<Self::Api, NumDecimals>`), RAY-based.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: Net rewards for suppliers after protocol fees and bad debt processing (RAY-based).
    ///
    /// **Security Tip**: Relies on upstream validation of `cache` state. Ensures bad debt is covered before distributing rewards or accruing protocol fees from new interest.
    fn calc_supplier_rewards(
        &self,
        cache: &mut Cache<Self>,
        old_borrow_index: ManagedDecimal<Self::Api, NumDecimals>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        // Calculate the actual accrued interest correctly using calc_interest
        let old_total_debt = self.mul_half_up(&cache.borrowed, &old_borrow_index, RAY_PRECISION);
        let new_total_debt = self.mul_half_up(&cache.borrowed, &cache.borrow_index, RAY_PRECISION);

        let accrued_interest_ray = new_total_debt.sub(old_total_debt);

        // If the accrued interest (asset dec) is less than or equal to bad debt (asset dec)
        if accrued_interest_ray.le(&cache.bad_debt.rescale(RAY_PRECISION)) {
            // Subtract the asset decimal interest from asset decimal bad debt
            cache.bad_debt -=
                self.rescale_half_up(&accrued_interest_ray, cache.params.asset_decimals);
            // No rewards or fees generated
            self.ray_zero()
        } else {
            // Accrued interest is greater than bad debt.
            // Calculate remaining interest after clearing bad debt (in asset decimals)
            let left_interest_ray = accrued_interest_ray.sub(cache.bad_debt.rescale(RAY_PRECISION));

            // Clear bad debt (asset decimals)
            cache.bad_debt = self.to_decimal(BigUint::zero(), cache.params.asset_decimals);

            let protocol_fee = self.mul_half_up(
                &left_interest_ray,
                &cache.params.reserve_factor,
                RAY_PRECISION,
            );
            // Calculate supplier rewards (in asset decimals)
            let supplier_rewards = left_interest_ray - protocol_fee.clone();

            cache.revenue += protocol_fee;
            // Return rewards and fees converted back to RAY precision for consistency elsewhere
            supplier_rewards
        }
    }

    /// Updates both borrow and supply indexes based on elapsed time since the last update.
    ///
    /// **Scope**: Synchronizes the global state of the pool by recalculating borrow and supply indexes,
    /// factoring in interest growth over time and distributing rewards.
    ///
    /// **Goal**: Keep the pool's financial state current, ensuring accurate interest accrual and reward distribution.
    ///
    /// **Process**:
    /// 1. Computes time delta (`delta`) since `cache.last_timestamp`.
    /// 2. If `delta > 0`:
    ///    a. Calculates the current `borrow_rate` using `cache.get_utilization()` and `cache.params`.
    ///    b. Computes the `borrow_factor` using `calculate_compounded_interest(borrow_rate, delta)`.
    ///    c. Updates `cache.borrow_index` via `update_borrow_index`, storing the `old_borrow_index`.
    ///    d. Calculates `rewards` for suppliers using `calc_supplier_rewards(cache, old_borrow_index)`.
    ///    e. Updates `cache.supply_index` via `update_supply_index(cache, rewards)`.
    ///    f. Sets `cache.last_timestamp` to the current `cache.timestamp`.
    ///
    /// # Arguments
    /// - `cache`: Mutable reference to the pool state (`Cache<Self>`), holding timestamps, indexes, and all relevant financial figures.
    ///
    /// **Security Tip**: Skips updates if `delta == 0`, preventing redundant computation. Protected by caller ensuring valid `cache`. The sequence of operations ensures that rewards are calculated based on interest accrued in the current period.
    fn global_sync(&self, cache: &mut Cache<Self>) {
        let delta = cache.timestamp - cache.last_timestamp;

        if delta > 0 {
            let borrow_rate = self.calc_borrow_rate(cache.get_utilization(), cache.params.clone());
            let borrow_factor = self.calculate_compounded_interest(borrow_rate.clone(), delta);
            let old_borrow_index = self.update_borrow_index(cache, borrow_factor.clone());
            let rewards = self.calc_supplier_rewards(cache, old_borrow_index.clone());
            self.update_supply_index(cache, rewards);
            cache.last_timestamp = cache.timestamp;
        }
    }
}
