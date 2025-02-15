use common_constants::{
    BP, BP_EGLD, DECIMAL_PRECISION, EGLD_DECIMAL_PRECISION, MAX_BONUS, MAX_FIRST_TOLERANCE,
    MAX_LAST_TOLERANCE, MIN_FIRST_TOLERANCE, MIN_LAST_TOLERANCE,
};
use common_events::{AccountPosition, AssetConfig, EModeCategory, OraclePriceFluctuation};

use crate::{
    contexts::base::StorageCache, oracle, storage, ERROR_UNEXPECTED_ANCHOR_TOLERANCES,
    ERROR_UNEXPECTED_FIRST_TOLERANCE, ERROR_UNEXPECTED_LAST_TOLERANCE,
};

multiversx_sc::imports!();

pub struct MathHelpers;

#[multiversx_sc::module]
pub trait MathsModule: oracle::OracleModule + storage::LendingStorageModule {
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
        target_hf: &BigUint,
        liquidation_bonus_min: &BigUint,
        liquidation_bonus_max: &BigUint,
    ) -> BigUint {
        let bp = BigUint::from(BP);

        // Scale the bonus linearly between minHF and BP
        let scaling_factor = (target_hf - old_hf) * &bp / target_hf; // Normalized between 0 and HF target
        let liquidation_bonus =
            liquidation_bonus_min + &((scaling_factor * liquidation_bonus_min) * 2u64 / &bp);

        return BigUint::min(liquidation_bonus, liquidation_bonus_max.clone()); // Ensure it does not exceed the maximum
    }

    /// Calculates the maximum feasible liquidation bonus that allows reaching target HF
    ///
    /// The core liquidation formula for debt repayment is:
    ///   weighted_collateral - (debt_to_repay * proportion_seized * (1 + bonus))
    /// divided by
    ///   (total_debt - debt_to_repay)
    /// must equal target_hf
    ///
    /// This function calculates the maximum bonus that makes this possible.
    ///
    /// # Arguments
    /// * `total_collateral_value` - Total value of collateral in base units
    /// * `total_debt_value` - Total value of debt in base units
    /// * `proportion_of_weighted_seized` - What fraction of weighted collateral is seized (e.g. 0.8 = 80%)
    /// * `target_hf` - Target health factor to achieve (e.g. 10000 = 1.0)
    /// * `proportion_of_weighted_bonus` - Minimum bonus value (won't go below this)
    ///
    /// # Returns
    /// Maximum feasible bonus in base points (e.g. 1000 = 10%)
    fn get_max_feasible_bonus(
        &self,
        total_collateral_value: &BigUint,
        total_debt_value: &BigUint,
        proportion_of_weighted_seized: &BigUint,
        target_hf: &BigUint,
        proportion_of_weighted_bonus: &BigUint,
    ) -> BigUint {
        let bp = &BigUint::from(BP);

        // Calculate 'n' term which appears in multiple places
        // n = target_hf * total_debt - proportion_seized * total_collateral
        // This term represents the "gap" between current and desired position
        // - If n > 0: Position needs more collateral to reach target HF
        // - If n < 0: Position has excess collateral
        let n =
            target_hf * total_debt_value - proportion_of_weighted_seized * total_collateral_value;

        // BOUND 1: Ensures denominator in repayment calculation stays negative
        // The repayment formula denominator is: (p*(1+b) - T) where:
        // - p is proportion_seized
        // - b is bonus
        // - T is target_HF
        //
        // For the denominator to stay negative:
        // p*(1+b) - T < 0
        // p*(1+b) < T
        // 1 + b < T/p
        // b < T/p - 1
        //
        // Therefore bound1 = T/p - 1
        let bound1_numerator = target_hf * bp / proportion_of_weighted_seized;
        let bound1 = bound1_numerator - bp;

        // BOUND 2: Ensures we don't try to seize more collateral than available
        // This comes from solving the repayment equation for the bonus:
        // (C - px(1+b))/(D-x) = T
        // where x is the repayment amount
        //
        // After algebraic manipulation:
        // b ≤ (C*T - C*p - n)/(n + C*p)
        let numerator = (total_collateral_value * target_hf * bp)
            - (total_collateral_value * proportion_of_weighted_seized * bp)
            - &n;

        let denominator = n + total_collateral_value * proportion_of_weighted_seized;
        let bound2 = numerator / denominator;

        // Take the minimum of both bounds (to satisfy both constraints)
        // Then take maximum with minimum required bonus to ensure we don't go below minimum
        //
        // bound1: Prevents denominator from becoming positive
        // bound2: Prevents overseizing of collateral
        // proportion_of_weighted_bonus: Minimum allowed bonus
        BigUint::max(
            proportion_of_weighted_bonus.clone(),
            BigUint::min(bound1, bound2),
        )
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
        total_collateral_all_assets: &BigUint, // Total EGLD value of ALL collaterals
        weighted_collateral_in_egld: &BigUint, // Total Weighted EGLD value of ALL collaterals
        proportion_of_weighted_seized: &BigUint, // How much I lose per 1$ liquidated, example for each 1$ paid the weighted amount lowers by 0.8$
        liquidation_bonus: &BigUint, // Weighted bonus of all assets part of the position
        total_debt: &BigUint,        // Total EGLD value of all debt
        target_hf: BigUint, // Where we would like to bring the user health factor after this potential liquidation
    ) -> (BigUint, BigUint, BigUint) {
        let bp = BigUint::from(BP);
        let target_hf_signed = ManagedDecimalSigned::from_raw_units(
            BigInt::from_biguint(Sign::Plus, target_hf.clone()),
            DECIMAL_PRECISION,
        );
        let bg_signed = ManagedDecimalSigned::from_raw_units(
            BigInt::from_biguint(Sign::Plus, bp.clone()),
            DECIMAL_PRECISION,
        );
        // 1. Calculate the ideal debt to repay to reach the target health factor.
        // Note that we use total_collateral_all_assets here, as the health factor is based on the overall collateral.
        let weighted_egld = ManagedDecimalSigned::from_raw_units(
            BigInt::from_biguint(Sign::Plus, weighted_collateral_in_egld.clone()),
            EGLD_DECIMAL_PRECISION,
        );

        let numerator = weighted_egld
            - ManagedDecimalSigned::from_raw_units(
                BigInt::from_biguint(Sign::Plus, total_debt * &target_hf / &bp),
                EGLD_DECIMAL_PRECISION,
            );
        
        let t: ManagedDecimalSigned<<Self as ContractBase>::Api, usize> =
            ManagedDecimalSigned::from_raw_units(
                BigInt::from_biguint(
                    Sign::Plus,
                    proportion_of_weighted_seized * &(&bp + liquidation_bonus) / &bp,
                ),
                DECIMAL_PRECISION,
            );

        let denominator = t - target_hf_signed;

        let ideal_debt_to_repay = numerator.mul(bg_signed).div(denominator);

        require!(
            ideal_debt_to_repay.clone().sign().eq(&Sign::Plus),
            "Debt repaid can not be negative!"
        );

        // 2. Calculate the maximum debt that can be liquidated based on the available collateral of the specific asset.
        // max_debt_to_liquidate = (total_collateral_of_asset * bp) / (bp + liquidation_bonus)
        let max_debt_to_liquidate = (total_collateral_all_assets * &bp) / (&bp + liquidation_bonus);

        // 3. Determine the actual debt to be repaid, which is the minimum of what's allowed and what's ideal.
        let debt_to_repay = BigUint::min(
            max_debt_to_liquidate,
            ideal_debt_to_repay
                .into_unsigned_or_fail()
                .into_raw_units()
                .clone(),
        );

        // 3) "t" factor = proportion_of_weighted_seized * (bp + liquidation_bonus) / bp
        //    same as in your code (we do a Signed decimal to keep the same style)
        let seized_weighted = BigUint::min(
            proportion_of_weighted_seized * &debt_to_repay * &(&bp + liquidation_bonus) / &bp / &bp,
            weighted_collateral_in_egld.clone(),
        );

        let new_weighted = weighted_collateral_in_egld - &seized_weighted;

        let new_health_factor = &new_weighted * &bp / (total_debt - &debt_to_repay);

        (debt_to_repay, liquidation_bonus.clone(), new_health_factor)
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
        weighted_collateral_in_egld: BigUint,
        proportion_of_weighted_seized: &BigUint,
        total_collateral: &BigUint,
        total_debt: &BigUint,
        min_bonus: &BigUint,
        old_hf: &BigUint,
    ) -> (BigUint, BigUint) {
        let bp = BigUint::from(BP);
        let target_best = &bp * 2u32 / 100u32 + &bp;
        // Try to bring it to at least 1.02 HF to be in a safer position
        let (safest_debt, safest_bonus, safe_new_hf) = self.simulation_target_liquidation(
            &weighted_collateral_in_egld,
            proportion_of_weighted_seized,
            total_collateral,
            total_debt,
            min_bonus,
            old_hf,
            target_best,
        );

        if &safe_new_hf >= &bp {
            return (safest_debt, safest_bonus);
        }

        // When 1.02 is not possible try to bring it to minimum 1.0 if possible
        let (limit_debt, limit_bonus, _) = self.simulation_target_liquidation(
            &weighted_collateral_in_egld,
            proportion_of_weighted_seized,
            total_collateral,
            total_debt,
            min_bonus,
            old_hf,
            bp.clone(),
        );

        if &limit_debt >= &bp {
            return (limit_debt, limit_bonus);
        }

        // When 1.02 or 1.00 targets can not be reached we consider the position as bad debt and we fallback to max_debt using min bonus.
        let max_debt_to_liquidate = (total_collateral * &bp) / (&bp + min_bonus);

        // return (max_debt_to_liquidate, min_bonus);
        return (max_debt_to_liquidate, min_bonus.clone());
    }

    fn simulation_target_liquidation(
        &self,
        weighted_collateral_in_egld: &BigUint,
        proportion_of_weighted_seized: &BigUint,
        total_collateral: &BigUint,
        total_debt: &BigUint,
        min_bonus: &BigUint,
        old_hf: &BigUint,
        target_hf: BigUint,
    ) -> (BigUint, BigUint, BigUint) {
        // Max bonus prevents the calculate_liquidation from returning negative numbers due to high bonus used
        let max_bonus = self.get_max_feasible_bonus(
            total_collateral,
            &total_debt,
            proportion_of_weighted_seized,
            &target_hf,
            min_bonus,
        );

        // Does a liniar scaling from the base bonus towards the max bonus based on the health factor difference
        let bonus =
            self.calculate_dynamic_liquidation_bonus(&old_hf, &target_hf, min_bonus, &max_bonus);

        return self.calculate_liquidation(
            total_collateral,
            weighted_collateral_in_egld,
            proportion_of_weighted_seized,
            &bonus,
            total_debt,
            target_hf,
        );
    }

    fn seize_collateral_proportionally(
        &self,
        collaterals: &ManagedVec<AccountPosition<Self::Api>>,
        total_collateral_value: &BigUint,
        debt_to_be_repaid: &BigUint,
        bonus: &BigUint,
        storage_cache: &mut StorageCache<Self>,
    ) -> ManagedVec<MultiValue2<EgldOrEsdtTokenPayment, BigUint>> {
        let mut seized_amounts_by_collateral = ManagedVec::new();
        let bp = BigUint::from(BP);
        let bp_egld = BigUint::from(BP_EGLD);
        for asset in collaterals {
            // proportion of total let collateral_in_egld =
            let total_amount = asset.get_total_amount();
            let asset_data = self.get_token_price(&asset.token_id, storage_cache);
            let asset_egld_value = self.get_token_amount_in_egld_raw(&total_amount, &asset_data);

            let proportion = &asset_egld_value * &bp_egld / total_collateral_value;

            let seized_egld = &proportion * debt_to_be_repaid / &bp_egld;
            let seized_units = self.compute_amount_in_tokens(&seized_egld, &asset_data);
            let seized_units_after_bonus = &seized_units * &(&bp + bonus) / &bp;
            let protocol_fee =
                (&seized_units_after_bonus - &seized_units) * &asset.entry_liquidation_fees / &bp;

            let final_amount = BigUint::min(seized_units_after_bonus, total_amount);

            seized_amounts_by_collateral.push(MultiValue2::from((
                EgldOrEsdtTokenPayment::new(asset.token_id.clone(), 0, final_amount),
                protocol_fee,
            )));
        }

        seized_amounts_by_collateral
    }

    #[view(getMaxLeverage)]
    fn calculate_max_leverage(
        &self,
        initial_deposit: &BigUint,
        health_factor: &BigUint,
        e_mode: &Option<EModeCategory<Self::Api>>,
        asset_config: &AssetConfig<Self::Api>,
        total_reserves: &BigUint,
        reserve_buffer: &BigUint,
    ) -> BigUint {
        let bp = BigUint::from(BP);
        let liquidation_threshold = if let Some(mode) = e_mode {
            &mode.liquidation_threshold
        } else {
            &asset_config.liquidation_threshold
        };
        let flash_loan_fee = &asset_config.flash_loan_fee;
        // If both `health_factor` and `flash_loan_fee` are already scaled by BP, then
        //    (1 + fee) = (bp + flash_loan_fee), also in BP scale.
        // We multiply them and THEN divide by BP to keep the result in the same BP scale.
        let hf_plus_fee = (health_factor * &(&bp + flash_loan_fee)) / &bp;

        // `liquidation_threshold` is already in the same BP scale as HF, so we subtract it directly.
        // The denominator is HF*(1+F) - LT, all in the same BP scale.
        // let max_l_hf_numerator = &hf_plus_fee;
        let max_l_hf_denominator = &hf_plus_fee - liquidation_threshold;

        // The final result for max leverage by HF formula:
        //   MaxL_HF = [ HF*(1 + fee) ] / [ HF*(1 + fee) - liquidation_threshold ]
        // Since everything is in BP scale, we multiply by BP one more time to keep output in BP scale.
        let max_l_hf = &hf_plus_fee * &bp / &max_l_hf_denominator;

        // --- Reserve-based constraint:
        // AR = total_reserves * (1 - reserve_buffer).
        // Because `reserve_buffer` is in BP scale, do the appropriate scaling by `bp` to get normal units.
        let available_reserves = (total_reserves * &(&bp - reserve_buffer)) / &bp;

        // Suppose we want to say:
        //   "If we have AR available, how many times bigger is that than `initial_deposit`?"
        //   ratio = AR / D  (but in normal arithmetic, ratio is dimensionless).
        //   Then leverage-limited = ratio + 1  => in BP scale => ratio * BP + BP
        // So we do:
        //   ratio_in_bp = (AR * BP) / D
        //   max_l_reserves = ratio_in_bp + BP
        let ratio_in_bp = (&available_reserves * &bp) / initial_deposit;
        let max_l_reserves = ratio_in_bp + &bp;

        // Final max leverage is the minimum of the HF-based limit and the Reserve-based limit
        let max_l = BigUint::min(max_l_hf, max_l_reserves);

        max_l
    }
}
