use common_constants::{
    BP, MAX_BONUS, MAX_FIRST_TOLERANCE, MAX_LAST_TOLERANCE, MIN_FIRST_TOLERANCE, MIN_LAST_TOLERANCE,
};
use common_events::{AssetConfig, OraclePriceFluctuation};

use crate::{
    ERROR_UNEXPECTED_ANCHOR_TOLERANCES, ERROR_UNEXPECTED_FIRST_TOLERANCE,
    ERROR_UNEXPECTED_LAST_TOLERANCE,
};

multiversx_sc::imports!();

pub struct MathHelpers;

#[multiversx_sc::module]
pub trait MathsModule {
    /// Computes the health factor for a position based on weighted collateral and borrowed value
    ///
    /// # Arguments
    /// * `weighted_collateral_in_egld` - Total EGLD value of collateral weighted by liquidation thresholds
    /// * `borrowed_value_in_egld` - Total EGLD value of borrowed assets
    ///
    /// # Returns
    /// * `BigUint` - Health factor in basis points (10000 = 100%)
    /// ```
    fn compute_health_factor(
        &self,
        weighted_collateral_in_egld: &BigUint,
        borrowed_value_in_egld: &BigUint,
    ) -> BigUint {
        // If there's no borrowed value, health factor is "infinite" (represented by max value)
        if borrowed_value_in_egld == &BigUint::zero() {
            return BigUint::from(u128::MAX);
        }

        let health_factor = weighted_collateral_in_egld
            .mul(&BigUint::from(BP))
            .div(borrowed_value_in_egld);

        health_factor
    }

    /// Calculates upper and lower bounds for a given tolerance
    ///
    /// # Arguments
    /// * `tolerance` - Tolerance value in basis points
    ///
    /// # Returns
    /// * `(BigUint, BigUint)` - Tuple containing:
    ///   - Upper bound (BP + tolerance)
    ///   - Lower bound (BP * BP / upper)
    ///
    /// ```
    fn get_range(&self, tolerance: &BigUint) -> (BigUint, BigUint) {
        let bp = BigUint::from(BP);
        let upper = &bp + tolerance;
        let lower = &bp * &bp / &upper;

        (upper, lower)
    }

    /// Validates and calculates oracle price fluctuation tolerances
    ///
    /// # Arguments
    /// * `first_tolerance` - Initial tolerance for price deviation
    /// * `last_tolerance` - Maximum allowed tolerance
    ///
    /// # Returns
    /// * `OraclePriceFluctuation` - Struct containing upper/lower bounds for both tolerances
    ///
    /// # Errors
    /// * `ERROR_UNEXPECTED_FIRST_TOLERANCE` - If first tolerance is out of range
    /// * `ERROR_UNEXPECTED_LAST_TOLERANCE` - If last tolerance is out of range
    /// * `ERROR_UNEXPECTED_ANCHOR_TOLERANCES` - If last tolerance is less than first
    ///
    /// # Example
    /// ```
    /// // For 5% first tolerance and 10% last tolerance
    /// first_tolerance = 500 (5%)
    /// last_tolerance = 1000 (10%)
    ///
    /// Returns:
    /// OraclePriceFluctuation {
    ///   first_upper_ratio: 10500,  // 105%
    ///   first_lower_ratio: 9524,   // ~95.24%
    ///   last_upper_ratio: 11000,   // 110%
    ///   last_lower_ratio: 9091     // ~90.91%
    /// }
    /// ```
    fn get_anchor_tolerances(
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

        let (first_upper_ratio, first_lower_ratio) = self.get_range(first_tolerance);
        let (last_upper_ratio, last_lower_ratio) = self.get_range(last_tolerance);

        let tolerances = OraclePriceFluctuation {
            first_upper_ratio,
            first_lower_ratio,
            last_upper_ratio,
            last_lower_ratio,
        };

        tolerances
    }

    /// Calculates the maximum feasible liquidation bonus
    ///
    /// # Arguments
    /// * `total_collateral_value` - Total value of collateral
    /// * `total_debt_value` - Total value of debt
    /// * `liquidation_th` - Liquidation threshold
    /// * `target_hf` - Target health factor
    /// * `liquidation_bonus_min` - Minimum liquidation bonus
    ///
    /// # Returns
    /// * `BigUint` - Maximum feasible liquidation bonus
    fn get_max_feasible_bonus(
        &self,
        total_collateral_value: &BigUint,
        total_debt_value: &BigUint,
        liquidation_th: &BigUint,
        target_hf: &BigUint,
        liquidation_bonus_min: &BigUint,
    ) -> BigUint {
        let bp = &BigUint::from(BP);

        let n = target_hf * total_debt_value - liquidation_th * total_collateral_value;
        let bound1_numerator = target_hf * bp / liquidation_th;

        let bound1 = bound1_numerator - bp; // keep denominator positive

        let numerator = (total_collateral_value * target_hf * bp)
            - (total_collateral_value * liquidation_th * bp)
            - &n;

        let denominator = n + total_collateral_value * liquidation_th;
        let bound2 = numerator / denominator;

        // The maximum feasible bonus is the smaller of these two then the max between min and the result
        return BigUint::max(liquidation_bonus_min.clone(), BigUint::min(bound1, bound2));
    }

    /// Calculates a dynamic protocol fee based on position health
    ///
    /// # Arguments
    /// * `health_factor` - Current health factor in basis points (10^21 = 100%)
    /// * `base_protocol_fee` - Base protocol fee in basis points (10^21 = 100%)
    ///
    /// # Returns
    /// * `BigUint` - Final protocol fee in basis points
    fn calculate_dynamic_protocol_fee(
        &self,
        health_factor: &BigUint,
        base_protocol_fee: &BigUint,
    ) -> BigUint {
        let bp = BigUint::from(BP);
        let max_bonus = BigUint::from(MAX_BONUS); // 30%
        let threshold = &bp * &BigUint::from(9u64) / &BigUint::from(10u64); // 90% threshold

        // Only start reducing fee when health factor < 90% of BP
        if health_factor > &threshold {
            return base_protocol_fee.clone();
        }

        // Calculate how far below 90% the HF is
        let distance_from_threshold = &threshold - health_factor;

        // Similar to bonus calculation, multiply by 2 for steeper reduction
        let health_factor_impact = distance_from_threshold.mul(2u64);

        // Calculate fee reduction based on how unhealthy the position is
        // More unhealthy = bigger fee reduction to incentivize quick liquidation
        let fee_reduction = BigUint::min(&health_factor_impact * base_protocol_fee / bp, max_bonus);

        // Ensure we never reduce more than 50% of the base fee
        let max_allowed_reduction = base_protocol_fee / &BigUint::from(2u64);
        let final_reduction = BigUint::min(fee_reduction, max_allowed_reduction);

        base_protocol_fee - &final_reduction
    }

    /// Calculates a dynamic liquidation bonus based on the current health factor
    ///
    /// # Arguments
    /// * `old_hf` - Current health factor
    /// * `liquidation_bonus_min` - Minimum liquidation bonus
    /// * `liquidation_bonus_max` - Maximum liquidation bonus
    /// * `min_hf` - Minimum health factor
    ///
    /// # Returns
    /// * `BigUint` - Calculated liquidation bonus
    fn calculate_dynamic_liquidation_bonus(
        &self,
        old_hf: &BigUint,
        liquidation_bonus_min: &BigUint,
        liquidation_bonus_max: &BigUint,
        min_hf: &BigUint,
    ) -> BigUint {
        let bp = BigUint::from(BP);

        // Scale the bonus linearly between minHF and BP
        let scaling_factor = (&bp - old_hf) * &bp / (&bp - min_hf); // Normalized between 0 and 1
        let liquidation_bonus = liquidation_bonus_min
            + &((scaling_factor * (liquidation_bonus_max - liquidation_bonus_min)) / &bp);

        return BigUint::min(liquidation_bonus, liquidation_bonus_max.clone()); // Ensure it does not exceed the maximum
    }

    /// Calculates the amount of debt to repay and collateral to seize during liquidation
    ///
    /// # Arguments
    /// * `total_collateral_all_assets` - Total value of all collateral
    /// * `total_collateral_of_asset` - Total value of the specific asset being liquidated
    /// * `liquidation_th` - Liquidation threshold of the asset being liquidated
    /// * `liquidation_bonus` - Liquidation bonus of the asset being liquidated
    /// * `total_debt` - Total value of all debt
    /// * `target_hf` - Target health factor after liquidation
    ///
    /// # Returns
    /// * `(BigUint, BigUint)` - Amount of debt to repay and collateral to seize
    fn calculate_liquidation(
        &self,
        total_collateral_all_assets: &BigUint, // Total EGLD value of ALL collateral
        total_collateral_of_asset: &BigUint, // Total EGLD value of the specific asset being liquidated
        liquidation_th: &BigUint,            // Liquidation threshold of the asset being liquidated
        liquidation_bonus: &BigUint,         // Liquidation bonus of the asset being liquidated
        total_debt: &BigUint,                // Total EGLD value of all debt
        target_hf: &BigUint,                 // Target health factor after liquidation
    ) -> (BigUint, BigUint) {
        let bp = BigUint::from(BP);

        // 1. Calculate the ideal debt to repay to reach the target health factor.
        // Note that we use total_collateral_all_assets here, as the health factor is based on the overall collateral.
        let ideal_debt_to_repay = (target_hf * total_debt
            - liquidation_th * total_collateral_all_assets)
            / (target_hf.clone() - (liquidation_th * &(&bp + liquidation_bonus)) / &bp);

        // 2. Calculate the maximum debt that can be liquidated based on the available collateral of the specific asset.
        // max_debt_to_liquidate = (total_collateral_of_asset * bp) / (bp + liquidation_bonus)
        let max_debt_to_liquidate = (total_collateral_of_asset * &bp) / (&bp + liquidation_bonus);

        // 3. Determine the actual debt to be repaid, which is the minimum of what's allowed and what's ideal.
        let debt_to_repay = BigUint::min(max_debt_to_liquidate, ideal_debt_to_repay);

        (debt_to_repay, liquidation_bonus.clone())
    }

    /// Calculates the protocol fee for a liquidation based on the bonus amount
    ///
    /// # Arguments
    /// * `collateral_amount_after_bonus` - Total collateral amount including the liquidation bonus
    /// * `collateral_amount_before_bonus` - Original collateral amount without the bonus
    /// * `asset_config` - Configuration of the collateral asset being liquidated
    /// * `health_factor` - Current health factor of the position
    ///
    /// # Returns
    /// * `BigUint` - Amount that goes to the protocol as fee
    fn calculate_liquidation_fees(
        &self,
        liq_bonus_amount: &BigUint,
        asset_config: &AssetConfig<Self::Api>,
        health_factor: &BigUint,
    ) -> BigUint {
        // Calculate dynamic protocol fee based on health factor
        let dynamic_fee =
            self.calculate_dynamic_protocol_fee(health_factor, &asset_config.liquidation_max_fee);

        // Calculate protocol's share of the bonus based on dynamic fee
        liq_bonus_amount * &dynamic_fee / &BigUint::from(BP)
    }

    /// Estimates the amount of debt to repay and collateral to seize during liquidation
    ///
    /// # Arguments
    /// * `total_token_amount` - Total amount of the specific token
    /// * `total_collateral` - Total value of all collateral
    /// * `total_debt` - Total value of all debt
    /// * `liquidation_th` - Liquidation threshold
    /// * `min_bonus` - Minimum liquidation bonus
    /// * `old_hf` - Current health factor
    ///
    /// # Returns
    /// * `(BigUint, BigUint)` - Estimated amount of debt to repay and collateral to seize
    fn estimate_liquidation_amount(
        &self,
        total_token_amount: &BigUint,
        total_collateral: &BigUint,
        total_debt: &BigUint,
        liquidation_th: &BigUint,
        min_bonus: &BigUint,
        old_hf: &BigUint,
    ) -> (BigUint, BigUint) {
        // (debt, seize, bonus)
        let bp = BigUint::from(BP);
        let min_hf = bp.clone() / 2u32; // 0.5
        let target = &bp * 5u32 / 100u32 + &bp;

        let max_bonus = self.get_max_feasible_bonus(
            total_collateral,
            total_debt,
            liquidation_th,
            &target,
            min_bonus,
        );
        let bonus =
            self.calculate_dynamic_liquidation_bonus(&old_hf, min_bonus, &max_bonus, &min_hf);

        return self.calculate_liquidation(
            total_collateral,
            total_token_amount,
            liquidation_th,
            &bonus,
            total_debt,
            &target,
        );
    }
}
