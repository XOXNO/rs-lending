use common_constants::{RAY_PRECISION, SECONDS_PER_YEAR};
use common_events::MarketParams;

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

    /// Calculates the deposit rate based on utilization and borrow rate.
    ///
    /// **Scope**: Computes the rate suppliers earn from borrowers' interest payments.
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
        if utilization == self.ray_zero() {
            return self.ray_zero();
        }

        self.mul_half_up(
            &self.mul_half_up(&utilization, &borrow_rate, RAY_PRECISION),
            &self.bps().sub(reserve_factor),
            RAY_PRECISION,
        )
    }

    /// @dev Function to calculate the interest accumulated using a linear interest rate formula
    /// @param rate The interest rate, in ray
    /// @param last_update_timestamp The timestamp of the last update of the interest
    /// @return The interest rate linearly accumulated during the timeDelta, in ray
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
    /// - `borrow_rate`: Current borrow rate (`ManagedDecimal<Self::Api, NumDecimals>`).
    /// - `exp`: Time elapsed in seconds (`u64`). Always higher than 0.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: Interest factor (RAY-based).
    ///
    /// **Security Tip**: Returns 1 (RAY) if `exp == 0` to avoid unnecessary computation.
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
    /// **Goal**: Ensure suppliers' yields reflect their share of interest earned.
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

    /// Calculates supplier rewards by deducting protocol fees from accrued interest.
    ///
    /// **Scope**: This function computes the rewards suppliers earn from interest paid by borrowers,
    /// after the protocol takes its share (reserve factor). It's used during index updates to distribute profits.
    ///
    /// **Goal**: Ensure suppliers receive their fair share of interest while updating protocol revenue.
    ///
    /// **Formula**:
    /// - Accrued interest = `borrowed * (borrow_index / old_borrow_index - 1)`
    /// - Rewards = `accrued_interest * (1 - reserve_factor)`
    ///
    /// # Arguments
    /// - `cache`: Mutable reference to the pool state (`Cache<Self>`), containing borrow amounts, indexes, and params.
    /// - `old_borrow_index`: The borrow index before the current update (`ManagedDecimal<Self::Api, NumDecimals>`).
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: Net rewards for suppliers after protocol fees.
    ///
    /// **Security Tip**: No direct `require!` checks here; relies on upstream validation of `cache` state (e.g., in `global_sync`).
    fn calc_supplier_rewards(
        &self,
        cache: &mut Cache<Self>,
        old_borrow_index: ManagedDecimal<Self::Api, NumDecimals>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        // Calculate the actual accrued interest correctly using calc_interest (returns asset decimals)
        let old_total_debt = self.mul_half_up(&cache.borrowed, &old_borrow_index, RAY_PRECISION);
        let new_total_debt = self.mul_half_up(&cache.borrowed, &cache.borrow_index, RAY_PRECISION);

        let accrued_interest_ray = new_total_debt.sub(old_total_debt);
        let accrued_interest_asset_decimals =
            accrued_interest_ray.rescale(cache.params.asset_decimals);
        // If the accrued interest is less than the bad debt, we can collect the entire accrued interest
        if accrued_interest_asset_decimals.le(&cache.bad_debt) {
            // Use asset decimal version for subtracting from bad_debt storage
            cache.bad_debt -= &accrued_interest_asset_decimals;
            self.ray_zero()
        } else {
            // If the accrued interest is greater than the bad debt, we clear the bad debt and calculate the protocol's share and the suppliers' share
            let left_interest_after_bad_debt =
                accrued_interest_asset_decimals - cache.bad_debt.clone();
            cache.bad_debt = self.to_decimal(BigUint::zero(), cache.params.asset_decimals);

            let protocol_fee = self.mul_half_up(
                &left_interest_after_bad_debt,
                &cache.params.reserve_factor, // Ensure reserve_factor is RAY scaled
                RAY_PRECISION,
            );
            // Update revenue and reserves storage with fee scaled to asset decimals
            let protocol_fee_asset_decimals = protocol_fee.rescale(cache.params.asset_decimals);

            cache.revenue += &protocol_fee_asset_decimals;

            // Return net rewards (interest after bad debt and fee) in RAY precision
            left_interest_after_bad_debt.rescale(RAY_PRECISION) - protocol_fee
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
    /// 1. Computes time delta since last update.
    /// 2. Updates borrow index using growth factor.
    /// 3. Calculates supplier rewards.
    /// 4. Updates supply index with rewards.
    /// 5. Refreshes last update timestamp.
    ///
    /// # Arguments
    /// - `cache`: Mutable reference to the pool state (`Cache<Self>`), holding timestamps and indexes.
    ///
    /// **Security Tip**: Skips updates if `delta == 0`, preventing redundant computation. Protected by caller ensuring valid `cache`.
    fn global_sync(&self, cache: &mut Cache<Self>) {
        let delta = cache.timestamp - cache.last_timestamp;

        if delta > 0 {
            let borrow_rate = self.calc_borrow_rate(cache.get_utilization(), cache.params.clone());
            let borrow_factor = self.calculate_compounded_interest(borrow_rate.clone(), delta);
            let old_borrow_index = self.update_borrow_index(cache, borrow_factor.clone());
            let supplier_rewards = self.calc_supplier_rewards(cache, old_borrow_index.clone());

            self.update_supply_index(cache, supplier_rewards);

            cache.last_timestamp = cache.timestamp;
        }
    }
}
