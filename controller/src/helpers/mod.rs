use common_constants::{
    BPS_PRECISION, K_SCALLING_FACTOR, MAX_FIRST_TOLERANCE, MAX_LAST_TOLERANCE,
    MAX_LIQUIDATION_BONUS, MIN_FIRST_TOLERANCE, MIN_LAST_TOLERANCE, RAY_PRECISION, WAD_PRECISION,
};
use common_errors::{
    ERROR_UNEXPECTED_ANCHOR_TOLERANCES, ERROR_UNEXPECTED_FIRST_TOLERANCE,
    ERROR_UNEXPECTED_LAST_TOLERANCE,
};
use common_structs::{OraclePriceFluctuation, PriceFeedShort};

multiversx_sc::imports!();

#[multiversx_sc::module]
pub trait MathsModule: common_math::SharedMathModule {
    /// Converts an EGLD amount to token units using the token's price feed data.
    /// Normalizes EGLD values to token-specific decimals for cross-asset calculations.
    ///
    /// # Arguments
    /// - `amount_in_egld`: EGLD amount to convert.
    /// - `token_data`: Price feed data with token price and decimals.
    ///
    /// # Returns
    /// - Token amount adjusted to the token's decimal precision.
    #[inline]
    fn convert_egld_to_tokens(
        &self,
        amount_in_egld: &ManagedDecimal<Self::Api, NumDecimals>,
        token_data: &PriceFeedShort<Self::Api>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        self.rescale_half_up(
            &self.convert_egld_to_tokens_ray(amount_in_egld, token_data),
            token_data.asset_decimals,
        )
    }

    /// Converts an EGLD amount to token units using the token's price feed data.
    /// Normalizes EGLD values to token-specific decimals for cross-asset calculations.
    ///
    /// # Arguments
    /// - `amount_in_egld`: EGLD amount to convert.
    /// - `token_data`: Price feed data with token price and decimals.
    ///
    /// # Returns
    /// - Token amount adjusted to the RAY precision.
    #[inline]
    fn convert_egld_to_tokens_ray(
        &self,
        amount_in_egld: &ManagedDecimal<Self::Api, NumDecimals>,
        token_data: &PriceFeedShort<Self::Api>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        self.div_half_up(amount_in_egld, &token_data.price, RAY_PRECISION)
    }

    /// Computes the USD value of a token amount using its price.
    /// Used for standardizing asset values in USD for collateral and borrow calculations.
    ///
    /// # Arguments
    /// - `amount`: Token amount to evaluate.
    /// - `token_price`: USD price of the token.
    ///
    /// # Returns
    /// - USD value in WAD precision.
    #[inline]
    fn get_egld_usd_value(
        &self,
        amount: &ManagedDecimal<Self::Api, NumDecimals>,
        token_price: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        self.rescale_half_up(
            &self.mul_half_up(amount, token_price, RAY_PRECISION),
            WAD_PRECISION,
        )
    }

    /// Computes the EGLD value of a token amount using its price.
    /// Facilitates internal calculations with EGLD as the base unit.
    ///
    /// # Arguments
    /// - `amount`: Token amount to convert.
    /// - `token_price`: EGLD price of the token.
    ///
    /// # Returns
    /// - EGLD value in WAD precision.
    #[inline]
    fn get_token_egld_value(
        &self,
        amount: &ManagedDecimal<Self::Api, NumDecimals>,
        token_price: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        self.rescale_half_up(
            &self.mul_half_up(amount, token_price, RAY_PRECISION),
            WAD_PRECISION,
        )
    }

    /// Computes the EGLD value of a token amount using its price.
    /// Facilitates internal calculations with EGLD as the base unit.
    ///
    /// # Arguments
    /// - `amount`: Token amount to convert.
    /// - `token_price`: EGLD price of the token.
    ///
    /// # Returns
    /// - EGLD value in WAD precision.
    #[inline]
    fn get_token_egld_value_ray(
        &self,
        amount: &ManagedDecimal<Self::Api, NumDecimals>,
        token_price: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        self.mul_half_up(amount, token_price, RAY_PRECISION)
    }

    /// Calculates the health factor from weighted collateral and borrowed value.
    /// Assesses the risk level of a user's position; higher values indicate safer positions.
    ///
    /// # Arguments
    /// - `weighted_collateral_in_egld`: Collateral value weighted by liquidation thresholds.
    /// - `borrowed_value_in_egld`: Total borrowed value in EGLD.
    ///
    /// # Returns
    /// - Health factor in WAD precision; `u128::MAX` if no borrows exist.
    fn compute_health_factor(
        &self,
        weighted_collateral_in_egld: &ManagedDecimal<Self::Api, NumDecimals>,
        borrowed_value_in_egld: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        if borrowed_value_in_egld == &self.wad_zero() {
            return self.to_decimal_wad(BigUint::from(u128::MAX));
        }

        let health_factor = self.div_half_up(
            weighted_collateral_in_egld,
            borrowed_value_in_egld,
            RAY_PRECISION,
        );

        self.rescale_half_up(&health_factor, WAD_PRECISION)
    }

    /// Calculates upper and lower bounds for a tolerance in basis points.
    /// Determines acceptable price ranges for price fluctuation checks.
    ///
    /// # Arguments
    /// - `tolerance`: Tolerance value in basis points.
    ///
    /// # Returns
    /// - Tuple of (upper_bound, lower_bound) in BPS precision.
    #[inline]
    fn calculate_tolerance_range(
        &self,
        tolerance: ManagedDecimal<Self::Api, NumDecimals>,
    ) -> (
        ManagedDecimal<Self::Api, NumDecimals>,
        ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        let upper = self.bps() + tolerance;
        let lower = self.div_half_up(&self.bps(), &upper, BPS_PRECISION);

        (upper, lower)
    }

    /// Validates and computes oracle price fluctuation tolerances.
    /// Ensures price deviations stay within safe limits for oracle reliability.
    ///
    /// # Arguments
    /// - `first_tolerance`: Initial tolerance for price deviation.
    /// - `last_tolerance`: Maximum allowed tolerance.
    ///
    /// # Returns
    /// - `OraclePriceFluctuation` struct with bounds for both tolerances.
    fn validate_and_calculate_tolerances(
        &self,
        first_tolerance: &BigUint,
        last_tolerance: &BigUint,
    ) -> OraclePriceFluctuation<Self::Api> {
        require!(
            first_tolerance >= &BigUint::from(MIN_FIRST_TOLERANCE)
                && first_tolerance <= &BigUint::from(MAX_FIRST_TOLERANCE),
            ERROR_UNEXPECTED_FIRST_TOLERANCE
        );
        require!(
            last_tolerance >= &BigUint::from(MIN_LAST_TOLERANCE)
                && last_tolerance <= &BigUint::from(MAX_LAST_TOLERANCE),
            ERROR_UNEXPECTED_LAST_TOLERANCE
        );
        require!(
            last_tolerance >= first_tolerance,
            ERROR_UNEXPECTED_ANCHOR_TOLERANCES
        );

        let (first_upper_ratio, first_lower_ratio) =
            self.calculate_tolerance_range(self.to_decimal_bps(first_tolerance.clone()));
        let (last_upper_ratio, last_lower_ratio) =
            self.calculate_tolerance_range(self.to_decimal_bps(last_tolerance.clone()));

        OraclePriceFluctuation {
            first_upper_ratio,
            first_lower_ratio,
            last_upper_ratio,
            last_lower_ratio,
        }
    }

    /// Calculates a linearly scaled liquidation bonus based on the health factor gap.
    /// Scales from min_bonus to max_bonus using a constant k.
    ///
    /// # Arguments
    /// - `current_hf`: Current health factor (WAD precision, 10^18).
    /// - `target_hf`: Target health factor (WAD precision, 10^18).
    /// - `min_bonus`: Minimum bonus (BPS precision, 10^4, e.g., 250 for 2.5%).
    /// - `max_bonus`: Maximum bonus (BPS precision, 10^4, e.g., 1500 for 15%).
    /// - `k`: Scaling constant (BPS precision, 10^4, e.g., 2.0).
    ///
    /// # Returns
    /// - Bonus in BPS (10^4 precision).
    fn calculate_linear_bonus(
        &self,
        current_hf: &ManagedDecimal<Self::Api, NumDecimals>,
        target_hf: &ManagedDecimal<Self::Api, NumDecimals>,
        min_bonus: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        // Capped at 15%
        let max_bonus = self.to_decimal_bps(BigUint::from(MAX_LIQUIDATION_BONUS));

        // Scaling factor of 200%
        let k = self.to_decimal_bps(BigUint::from(K_SCALLING_FACTOR));

        // Calculate the health factor gap: (target_hf - current_hf) / target_hf
        let gap = self.div_half_up(
            &(target_hf.clone() - current_hf.clone()),
            target_hf,
            RAY_PRECISION,
        );
        // Calculate the scaled term: k * gap
        let scaled_term = self.mul_half_up(&k, &gap, RAY_PRECISION);
        // Clamp the scaled term between 0 and 1
        let clamped_term = if scaled_term > self.ray() {
            self.ray()
        } else {
            scaled_term
        };
        // Calculate the bonus range: max_bonus - min_bonus
        let bonus_range = max_bonus.clone() - min_bonus.clone();
        // Calculate the bonus increment: bonus_range * clamped_term
        let bonus_increment = self.rescale_half_up(
            &self.mul_half_up(&bonus_range, &clamped_term, RAY_PRECISION),
            BPS_PRECISION,
        );

        // Final bonus: min_bonus + bonus_increment
        min_bonus.clone() + bonus_increment
    }

    /// Computes debt repayment, bonus, and new health factor for a liquidation.
    /// Simulates liquidation effects to meet the target health factor.
    ///
    /// # Arguments
    /// - `total_collateral`: Total collateral value.
    /// - `weighted_collateral`: Collateral value weighted by thresholds.
    /// - `proportion_seized`: Proportion of collateral seized (in BPS).
    /// - `liquidation_bonus`: Applied bonus (in BPS).
    /// - `total_debt`: Total debt value.
    /// - `target_hf`: Target health factor (in WAD).
    ///
    /// # Returns
    /// - Tuple of (debt_to_repay, liquidation_bonus, new_health_factor).
    fn compute_liquidation_details(
        &self,
        total_collateral: &ManagedDecimal<Self::Api, NumDecimals>,
        weighted_collateral: &ManagedDecimal<Self::Api, NumDecimals>,
        proportion_seized: &ManagedDecimal<Self::Api, NumDecimals>,
        liquidation_bonus: &ManagedDecimal<Self::Api, NumDecimals>,
        total_debt: &ManagedDecimal<Self::Api, NumDecimals>,
        target_hf: ManagedDecimal<Self::Api, NumDecimals>,
    ) -> (
        ManagedDecimal<Self::Api, NumDecimals>,
        ManagedDecimal<Self::Api, NumDecimals>,
        ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        // Constants
        let bps = self.bps();

        // Convert to signed for intermediate calculations
        let total_debt_ray_signed = total_debt.clone().into_signed().rescale(RAY_PRECISION);
        let target_health = target_hf.clone().into_signed().rescale(RAY_PRECISION);
        let weighted_collateral_ray = weighted_collateral
            .clone()
            .into_signed()
            .rescale(RAY_PRECISION);

        // Compute 1 + b
        let one_plus_bonus = self.bps() + liquidation_bonus.clone();
        let d_max = self.div_half_up(
            &self.mul_half_up(total_collateral, &bps, RAY_PRECISION),
            &one_plus_bonus,
            RAY_PRECISION,
        );

        let denominator_term = self
            .mul_half_up(proportion_seized, &one_plus_bonus, RAY_PRECISION)
            .into_signed();

        // Avoid edge case where target_health == denominator_term which would result in division by zero
        let debt_to_repay_ray = if target_health == denominator_term {
            d_max
        } else {
            // Compute d_ideal
            let numerator_term =
                self.mul_half_up_signed(&target_health, &total_debt_ray_signed, RAY_PRECISION);
            let numerator = numerator_term - weighted_collateral_ray;
            let denominator = target_health - denominator_term;
            let d_ideal = self.div_half_up_signed(&numerator, &denominator, RAY_PRECISION);

            // Determine debt_to_repay, will fall back to d_max if d_ideal is negative since it's not possible to be heatlhy anymore
            if d_ideal.sign() == Sign::Minus {
                d_max
            } else {
                self.get_min(d_ideal.into_unsigned_or_fail(), d_max)
            }
        };

        // Calculate new health factor
        let new_health_factor = self.calculate_post_liquidation_health_factor(
            weighted_collateral,
            total_debt,
            &debt_to_repay_ray,
            proportion_seized,
            liquidation_bonus,
        );

        (
            debt_to_repay_ray,
            liquidation_bonus.clone(),
            new_health_factor,
        )
    }

    /// Estimates optimal debt repayment and bonus for liquidation.
    /// Simulates scenarios to achieve a safe health factor.
    ///
    /// # Arguments
    /// - `weighted_collateral_in_egld`: Weighted collateral in EGLD.
    /// - `proportion_seized`: Proportion of collateral seized.
    /// - `total_collateral`: Total collateral value.
    /// - `total_debt`: Total debt value.
    /// - `min_bonus`: Minimum bonus.
    /// - `current_hf`: Current health factor.
    ///
    /// # Returns
    /// - Tuple of (debt_to_repay, bonus).
    fn estimate_liquidation_amount(
        &self,
        weighted_collateral_in_egld: &ManagedDecimal<Self::Api, NumDecimals>,
        proportion_seized: &ManagedDecimal<Self::Api, NumDecimals>,
        total_collateral: &ManagedDecimal<Self::Api, NumDecimals>,
        total_debt: &ManagedDecimal<Self::Api, NumDecimals>,
        min_bonus: &ManagedDecimal<Self::Api, NumDecimals>,
        current_hf: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> (
        ManagedDecimal<Self::Api, NumDecimals>,
        ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        let wad = self.wad();

        let target_best = wad.clone().into_raw_units() / 50u32 + wad.into_raw_units(); // 1.02 WAD

        let (safest_debt, safest_bonus, safe_new_hf) = self.simulate_liquidation(
            weighted_collateral_in_egld,
            proportion_seized,
            total_collateral,
            total_debt,
            min_bonus,
            current_hf,
            self.to_decimal_wad(target_best),
        );

        if safe_new_hf >= self.wad() {
            return (safest_debt, safest_bonus);
        }

        let target_best_second = wad.clone().into_raw_units() / 100u32 + wad.into_raw_units(); // 1.01 WAD
        let (limit_debt, limit_bonus, _) = self.simulate_liquidation(
            weighted_collateral_in_egld,
            proportion_seized,
            total_collateral,
            total_debt,
            min_bonus,
            current_hf,
            self.to_decimal_wad(target_best_second),
        );

        (limit_debt, limit_bonus)
    }

    /// Calculates the new health factor after a liquidation operation.
    ///
    /// **Purpose**: Simulates the health factor that would result from a liquidation
    /// with the given parameters, useful for validation and estimation.
    ///
    /// **Note on Precision**: This calculation works in EGLD terms with high precision.
    /// The actual health factor after liquidation may be slightly lower (typically ~0.1-0.3%)
    /// due to rounding when converting between EGLD and individual token amounts with
    /// different decimal precisions. This is expected behavior and ensures the position
    /// remains safely above the minimum health factor of 1.0.
    ///
    /// # Arguments
    /// - `weighted_collateral`: Current weighted collateral value (WAD precision)
    /// - `total_debt`: Current total debt value (RAY precision)
    /// - `debt_to_repay`: Amount of debt being repaid (RAY precision)
    /// - `proportion_seized`: Proportion of collateral seized (BPS precision)
    /// - `liquidation_bonus`: Liquidation bonus rate (BPS precision)
    ///
    /// # Returns
    /// - New health factor after liquidation (WAD precision)
    fn calculate_post_liquidation_health_factor(
        &self,
        weighted_collateral: &ManagedDecimal<Self::Api, NumDecimals>,
        total_debt: &ManagedDecimal<Self::Api, NumDecimals>,
        debt_to_repay_ray: &ManagedDecimal<Self::Api, NumDecimals>,
        proportion_seized: &ManagedDecimal<Self::Api, NumDecimals>,
        liquidation_bonus: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let one_plus_bonus = self.bps() + liquidation_bonus.clone();

        // Compute seized_weighted
        let seized = self.mul_half_up(proportion_seized, debt_to_repay_ray, RAY_PRECISION);
        let seized_weighted_raw = self.mul_half_up(&seized, &one_plus_bonus, RAY_PRECISION);
        let seized_weighted = self.get_min(
            self.rescale_half_up(&seized_weighted_raw, WAD_PRECISION),
            weighted_collateral.clone(),
        );
        // Compute new weighted collateral and total debt
        let new_weighted_wad = weighted_collateral.clone() - seized_weighted;
        let total_debt_ray = total_debt.clone().rescale(RAY_PRECISION);
        let new_total_debt_ray = if *debt_to_repay_ray >= total_debt_ray {
            self.ray_zero()
        } else {
            total_debt_ray - debt_to_repay_ray.clone()
        };
        // Compute new_health_factor
        self.compute_health_factor(
            &new_weighted_wad,
            &self.rescale_half_up(&new_total_debt_ray, WAD_PRECISION),
        )
    }

    /// Simulates a liquidation to estimate debt repayment, bonus, and new health factor.
    /// Tests different scenarios for `estimate_liquidation_amount`.
    ///
    /// # Arguments
    /// - `weighted_collateral_in_egld`: Weighted collateral in EGLD.
    /// - `proportion_seized`: Proportion of collateral seized.
    /// - `total_collateral`: Total collateral value.
    /// - `total_debt`: Total debt value.
    /// - `min_bonus`: Minimum bonus.
    /// - `current_hf`: Current health factor.
    /// - `target_hf`: Target health factor.
    ///
    /// # Returns
    /// - Tuple of (debt_to_repay, bonus, new_health_factor).
    fn simulate_liquidation(
        &self,
        weighted_collateral_in_egld: &ManagedDecimal<Self::Api, NumDecimals>,
        proportion_seized: &ManagedDecimal<Self::Api, NumDecimals>,
        total_collateral: &ManagedDecimal<Self::Api, NumDecimals>,
        total_debt: &ManagedDecimal<Self::Api, NumDecimals>,
        min_bonus: &ManagedDecimal<Self::Api, NumDecimals>,
        current_hf: &ManagedDecimal<Self::Api, NumDecimals>,
        target_hf: ManagedDecimal<Self::Api, NumDecimals>,
    ) -> (
        ManagedDecimal<Self::Api, NumDecimals>,
        ManagedDecimal<Self::Api, NumDecimals>,
        ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        let bonus = self.calculate_linear_bonus(current_hf, &target_hf, min_bonus);

        self.compute_liquidation_details(
            total_collateral,
            weighted_collateral_in_egld,
            proportion_seized,
            &bonus,
            total_debt,
            target_hf,
        )
    }
}
