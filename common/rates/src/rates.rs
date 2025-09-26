#![no_std]
use common_constants::{MILLISECONDS_PER_YEAR, RAY_PRECISION};
use common_structs::{MarketIndex, MarketParams};

multiversx_sc::imports!();

/// The InterestRates module provides functions for calculating market rates,
/// interest accrual, and capital utilization based on pool parameters and current state.
///
/// **Scope**: Manages dynamic interest rates and index updates for the lending pool.
///
/// **Goal**: Ensure accurate, fair, and auditable interest mechanics for borrowers and suppliers.
#[multiversx_sc::module]
pub trait InterestRates: common_math::SharedMathModule {
    /// Calculates per-millisecond borrow rate using piecewise linear model.
    /// Rate increases with utilization: gradual before kink, steep after kink.
    /// Caps at max_borrow_rate and converts from annual to millisecond rate.
    fn calculate_borrow_rate(
        &self,
        utilization: ManagedDecimal<Self::Api, NumDecimals>,
        parameters: MarketParams<Self::Api>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let annual_rate = if utilization < parameters.mid_utilization_ray {
            // Region 1: utilization < mid_utilization
            let utilization_ratio = utilization
                .mul(parameters.slope1_ray)
                .div(parameters.mid_utilization_ray);
            parameters.base_borrow_rate_ray.add(utilization_ratio)
        } else if utilization < parameters.optimal_utilization_ray {
            // Region 2: mid_utilization <= utilization < optimal_utilization
            let excess_utilization = utilization.sub(parameters.mid_utilization_ray.clone());
            let slope_contribution = excess_utilization.mul(parameters.slope2_ray).div(
                parameters
                    .optimal_utilization_ray
                    .sub(parameters.mid_utilization_ray),
            );
            parameters
                .base_borrow_rate_ray
                .add(parameters.slope1_ray)
                .add(slope_contribution)
        } else {
            // Region 3: utilization >= optimal_utilization, linear growth
            let base_rate = parameters
                .base_borrow_rate_ray
                .add(parameters.slope1_ray)
                .add(parameters.slope2_ray);
            let excess_utilization = utilization.sub(parameters.optimal_utilization_ray.clone());
            let slope_contribution = excess_utilization
                .mul(parameters.slope3_ray)
                .div(self.ray().sub(parameters.optimal_utilization_ray));
            base_rate.add(slope_contribution)
        };

        // Cap the rate at max_borrow_rate
        let capped_rate = if annual_rate > parameters.max_borrow_rate_ray {
            parameters.max_borrow_rate_ray
        } else {
            annual_rate
        };

        // Convert annual rate to per-millisecond rate
        self.div_half_up(
            &capped_rate,
            &self.to_decimal(BigUint::from(MILLISECONDS_PER_YEAR), 0),
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
    /// - `(1 - reserve_factor)` is calculated as `self.bps().sub(reserve_factor)`, assuming `bps()` represents 100% and `reserve_factor` is also BPS-scaled.
    ///
    /// # Arguments
    /// - `utilization`: Current utilization ratio (`ManagedDecimal<Self::Api, NumDecimals>`), RAY-based.
    /// - `borrow_rate`: Current per-millisecond borrow rate (`ManagedDecimal<Self::Api, NumDecimals>`), RAY-based.
    /// - `reserve_factor`: Protocol fee fraction (`ManagedDecimal<Self::Api, NumDecimals>`), BPS-based.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: Per-millisecond deposit rate (RAY-based).
    ///
    /// **Security Tip**: Assumes inputs are valid; no overflow or underflow checks within this specific function beyond standard `ManagedDecimal` operations.
    fn calculate_deposit_rate(
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

    /// Approximates compound interest factor using Taylor series expansion.
    /// Calculates e^(rate * time) for small time intervals with 5-term precision.
    /// Returns growth factor for index updates.
    fn calculate_compounded_interest(
        &self,
        rate: ManagedDecimal<Self::Api, NumDecimals>,
        expiration: u64,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        // Use Taylor expansion e^x = 1 + x + x^2/2! + x^3/3! + x^4/4! + x^5/5! + ...
        // where x = borrow_rate * expiration

        let ray = self.ray();

        if expiration == 0 {
            return ray;
        }

        let expiration_decimal = self.to_decimal(BigUint::from(expiration), 0);

        // x = rate * time_delta
        let x = self.mul_half_up(&rate, &expiration_decimal, RAY_PRECISION);

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
        old_borrow_index: ManagedDecimal<Self::Api, NumDecimals>,
        interest_factor: ManagedDecimal<Self::Api, NumDecimals>,
    ) -> (
        ManagedDecimal<Self::Api, NumDecimals>,
        ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        let new_borrow_index = self.mul_half_up(&old_borrow_index, &interest_factor, RAY_PRECISION);

        (new_borrow_index, old_borrow_index)
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
    /// **Security Tip**: Skips update if `cache.supplied == 0` (which implies `current_total_supplied_value_ray` would be zero if `old_supply_index` is not zero, or if `old_supply_index` is zero) to avoid division-by-zero.
    fn update_supply_index(
        &self,
        supplied: ManagedDecimal<Self::Api, NumDecimals>,
        old_supply_index: ManagedDecimal<Self::Api, NumDecimals>,
        rewards_increase: ManagedDecimal<Self::Api, NumDecimals>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        if supplied != self.ray_zero() && rewards_increase != self.ray_zero() {
            let total_supplied_with_interest =
                self.mul_half_up(&supplied, &old_supply_index, RAY_PRECISION);
            let rewards_ratio = self.div_half_up(
                &rewards_increase,
                &total_supplied_with_interest,
                RAY_PRECISION,
            );

            let rewards_factor = self.ray() + rewards_ratio;

            return self.mul_half_up(&old_supply_index, &rewards_factor, RAY_PRECISION);
        }
        return old_supply_index;
    }

    /// Calculates supplier rewards and protocol fees
    /// This simplified version directly distributes accrued interest between suppliers and protocol.
    ///
    /// # Arguments
    /// - `parameters`: The market parameters including reserve factor
    /// - `borrowed`: The total scaled borrowed amount
    /// - `new_borrow_index`: The updated borrow index after interest accrual
    /// - `old_borrow_index`: The previous borrow index
    ///
    /// # Returns
    /// - `(supplier_rewards_ray, protocol_fee_ray)`: Interest distribution in RAY precision
    fn calculate_supplier_rewards(
        &self,
        parameters: MarketParams<Self::Api>,
        borrowed: &ManagedDecimal<Self::Api, NumDecimals>,
        new_borrow_index: &ManagedDecimal<Self::Api, NumDecimals>,
        old_borrow_index: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> (
        ManagedDecimal<Self::Api, NumDecimals>, // supplier_rewards_ray
        ManagedDecimal<Self::Api, NumDecimals>, // protocol_fee_ray
    ) {
        // Calculate total accrued interest
        let old_total_debt = self.scaled_to_original_ray(borrowed, old_borrow_index);
        let new_total_debt = self.scaled_to_original_ray(borrowed, new_borrow_index);

        let accrued_interest_ray = new_total_debt.sub(old_total_debt);

        // Direct distribution: protocol fee first, then supplier rewards
        let protocol_fee = self.mul_half_up(
            &accrued_interest_ray,
            &parameters.reserve_factor_bps,
            RAY_PRECISION,
        );
        let supplier_rewards_ray = accrued_interest_ray - protocol_fee.clone();

        (supplier_rewards_ray, protocol_fee)
    }

    /// Computes market utilization as borrowed / supplied in RAY precision.
    /// Returns zero when total supplied is zero to avoid division-by-zero.
    ///
    /// Arguments
    /// - `borrowed_ray`: Total borrowed amount in original units (RAY)
    /// - `supplied_ray`: Total supplied amount in original units (RAY)
    ///
    /// Returns
    /// - Utilization ratio in RAY precision
    fn utilization(
        &self,
        borrowed_ray: &ManagedDecimal<Self::Api, NumDecimals>,
        supplied_ray: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        if supplied_ray == &self.ray_zero() {
            return self.ray_zero();
        }
        self.div_half_up(borrowed_ray, supplied_ray, RAY_PRECISION)
    }

    /// Applies an interest index to a scaled amount, returning original units (RAY).
    ///
    /// Math
    /// - original = scaled_amount * index / RAY
    ///
    /// Arguments
    /// - `scaled_amount`: RAY-scaled principal amount
    /// - `index`: Interest index in RAY precision
    ///
    /// Returns
    /// - Original amount in RAY precision including accrued interest
    fn scaled_to_original_ray(
        &self,
        scaled_amount: &ManagedDecimal<Self::Api, NumDecimals>,
        index: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        self.mul_half_up(scaled_amount, index, RAY_PRECISION)
    }

    /// Applies an interest index to a scaled amount and rescales to asset decimals.
    ///
    /// Math
    /// - original_ray = scaled_amount * index / RAY
    /// - original = rescale(original_ray, asset_decimals)
    ///
    /// Arguments
    /// - `scaled_amount`: RAY-scaled principal amount
    /// - `index`: Interest index in RAY precision
    /// - `asset_decimals`: Target decimals for result
    ///
    /// Returns
    /// - Original amount in asset decimal precision
    fn scaled_to_original(
        &self,
        scaled_amount: &ManagedDecimal<Self::Api, NumDecimals>,
        index: &ManagedDecimal<Self::Api, NumDecimals>,
        asset_decimals: usize,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let original_amount = self.mul_half_up(scaled_amount, index, RAY_PRECISION);
        self.rescale_half_up(&original_amount, asset_decimals)
    }

    /// Simulates index update without state mutation, returning updated indices.
    ///
    /// Purpose
    /// - Compute new borrow/supply indices for a given timestamp delta using
    ///   current parameters and totals, for read-only contexts and views.
    ///
    /// Methodology
    /// 1. If `delta == 0`, return current indexes
    /// 2. Compute utilization from original totals
    /// 3. Calculate borrow rate and compounded factor for delta
    /// 4. Update borrow index, split accrued interest, update supply index
    ///
    /// Arguments
    /// - `current_timestamp`: Current timestamp (ms)
    /// - `last_timestamp`: Last index update timestamp (ms)
    /// - `borrowed`: Total scaled borrowed amount (RAY-scaled)
    /// - `current_borrowed_index`: Current borrow index (RAY)
    /// - `supplied`: Total scaled supplied amount (RAY-scaled)
    /// - `current_supply_index`: Current supply index (RAY)
    /// - `parameters`: Market parameters including reserve factor and slopes
    ///
    /// Returns
    /// - `MarketIndex` with updated supply and borrow indexes
    fn simulate_update_indexes(
        &self,
        current_timestamp: u64,
        last_timestamp: u64,
        borrowed: ManagedDecimal<Self::Api, NumDecimals>,
        current_borrowed_index: ManagedDecimal<Self::Api, NumDecimals>,
        supplied: ManagedDecimal<Self::Api, NumDecimals>,
        current_supply_index: ManagedDecimal<Self::Api, NumDecimals>,
        parameters: MarketParams<Self::Api>,
    ) -> MarketIndex<Self::Api> {
        let delta = current_timestamp - last_timestamp;

        if delta > 0 {
            let borrowed_original = self.scaled_to_original_ray(&borrowed, &current_borrowed_index);
            let supplied_original = self.scaled_to_original_ray(&supplied, &current_supply_index);
            let utilization = self.utilization(&borrowed_original, &supplied_original);
            let borrow_rate = self.calculate_borrow_rate(utilization, parameters.clone());
            let borrow_factor = self.calculate_compounded_interest(borrow_rate.clone(), delta);
            let (new_borrow_index, old_borrow_index) =
                self.update_borrow_index(current_borrowed_index.clone(), borrow_factor.clone());

            // 3 raw split
            let (supplier_rewards_ray, _) = self.calculate_supplier_rewards(
                parameters.clone(),
                &borrowed,
                &new_borrow_index,
                &old_borrow_index,
            );

            let new_supply_index =
                self.update_supply_index(supplied, current_supply_index, supplier_rewards_ray);

            MarketIndex {
                supply_index_ray: new_supply_index,
                borrow_index_ray: new_borrow_index,
            }
        } else {
            MarketIndex {
                supply_index_ray: current_supply_index,
                borrow_index_ray: current_borrowed_index,
            }
        }
    }
}
