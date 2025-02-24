use common_constants::{
    BPS, MAX_FIRST_TOLERANCE, MAX_LAST_TOLERANCE, MIN_FIRST_TOLERANCE, MIN_LAST_TOLERANCE, WAD,
    WAD_PRECISION,
};
use common_errors::{
    ERROR_DEBT_CAN_NOT_BE_NEGATIVE, ERROR_UNEXPECTED_ANCHOR_TOLERANCES,
    ERROR_UNEXPECTED_FIRST_TOLERANCE, ERROR_UNEXPECTED_LAST_TOLERANCE,
};
use common_events::{OraclePriceFluctuation, PriceFeedShort, RAY_PRECISION};

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
    fn convert_egld_to_tokens(
        &self,
        amount_in_egld: &ManagedDecimal<Self::Api, NumDecimals>,
        token_data: &PriceFeedShort<Self::Api>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        self.div_half_up(amount_in_egld, &token_data.price, RAY_PRECISION)
            .rescale(token_data.asset_decimals)
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
    fn get_token_usd_value(
        &self,
        amount: &ManagedDecimal<Self::Api, NumDecimals>,
        token_price: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        self.mul_half_up(amount, token_price, RAY_PRECISION)
            .rescale(WAD_PRECISION)
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
    fn get_token_egld_value(
        &self,
        amount: &ManagedDecimal<Self::Api, NumDecimals>,
        token_price: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        self.mul_half_up(amount, token_price, RAY_PRECISION)
            .rescale(WAD_PRECISION)
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
        if borrowed_value_in_egld == &self.to_decimal_wad(BigUint::zero()) {
            return self.to_decimal_wad(BigUint::from(u128::MAX));
        }
        let health_factor = self.div_half_up(
            weighted_collateral_in_egld,
            borrowed_value_in_egld,
            RAY_PRECISION,
        );
        health_factor.rescale(WAD_PRECISION)
    }

    /// Calculates upper and lower bounds for a tolerance in basis points.
    /// Determines acceptable price ranges for price fluctuation checks.
    ///
    /// # Arguments
    /// - `tolerance`: Tolerance value in basis points.
    ///
    /// # Returns
    /// - Tuple of (upper_bound, lower_bound) in BPS precision.
    fn calculate_tolerance_range(
        &self,
        tolerance: &BigUint,
    ) -> (
        ManagedDecimal<Self::Api, NumDecimals>,
        ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        let bps = BigUint::from(BPS);
        let tolerance_in_wad = (tolerance * &bps) / &bps;
        let upper = &bps + &tolerance_in_wad;
        let lower = &bps * &bps / &upper;
        (self.to_decimal_bps(upper), self.to_decimal_bps(lower))
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
            self.calculate_tolerance_range(first_tolerance);
        let (last_upper_ratio, last_lower_ratio) = self.calculate_tolerance_range(last_tolerance);

        OraclePriceFluctuation {
            first_upper_ratio,
            first_lower_ratio,
            last_upper_ratio,
            last_lower_ratio,
        }
    }

    /// Calculates a dynamic liquidation bonus based on health factors.
    /// Adjusts the bonus to incentivize liquidation while maintaining protocol stability.
    ///
    /// # Arguments
    /// - `current_hf`: Current health factor.
    /// - `target_hf`: Desired health factor post-liquidation.
    /// - `min_bonus`: Minimum bonus value.
    /// - `max_bonus`: Maximum bonus value.
    ///
    /// # Returns
    /// - Liquidation bonus clamped between `min_bonus` and `max_bonus`.
    fn calculate_dynamic_liquidation_bonus(
        &self,
        current_hf: &BigUint,
        target_hf: &BigUint,
        min_bonus: &BigUint,
        max_bonus: &BigUint,
    ) -> BigUint {
        let bonus_range = max_bonus - min_bonus;
        let bonus_increase = (bonus_range * (target_hf - current_hf)) / target_hf;
        let liquidation_bonus = min_bonus + &bonus_increase;
        BigUint::min(liquidation_bonus, max_bonus.clone())
    }

    /// Determines the maximum feasible liquidation bonus to achieve a target health factor.
    /// Ensures the bonus keeps liquidation profitable and safe.
    ///
    /// # Arguments
    /// - `total_collateral_value`: Total collateral value in WAD.
    /// - `total_debt_value`: Total debt value in WAD.
    /// - `proportion_seized`: Collateral proportion seized in BPS.
    /// - `target_hf`: Target health factor in WAD.
    /// - `min_bonus`: Minimum bonus in BPS.
    ///
    /// # Returns
    /// - Maximum feasible bonus in BPS.
    fn calculate_max_feasible_bonus(
        &self,
        total_collateral_value: &BigUint,
        total_debt_value: &BigUint,
        proportion_seized: &BigUint,
        target_hf: &BigUint,
        min_bonus: &BigUint,
    ) -> BigUint {
        let wad = BigUint::from(WAD);
        let bps = BigUint::from(BPS);

        let p = (proportion_seized * &wad) / &bps;
        let n = target_hf * total_debt_value - &p * total_collateral_value;
        let bound1_numerator = target_hf * &wad / &p;
        let bound1 = &bound1_numerator - &wad;

        let numerator =
            (total_collateral_value * target_hf * &wad) - (total_collateral_value * &p * &wad) - &n;
        let denominator = n + total_collateral_value * &p;
        let bound2 = &numerator / &denominator;

        let bonus_min_wad = (min_bonus * &wad) / &bps;
        let bonus_wad = BigUint::max(bonus_min_wad, BigUint::min(bound1, bound2));
        (bonus_wad * &bps) / &wad
    }

    /// Computes debt repayment, bonus, and new health factor for a liquidation.
    /// Simulates liquidation effects to meet the target health factor.
    ///
    /// # Arguments
    /// - `total_collateral`: Total collateral value.
    /// - `weighted_collateral`: Collateral value weighted by thresholds.
    /// - `proportion_seized`: Proportion of collateral seized.
    /// - `liquidation_bonus`: Applied bonus.
    /// - `total_debt`: Total debt value.
    /// - `target_hf`: Target health factor.
    ///
    /// # Returns
    /// - Tuple of (debt_to_repay, liquidation_bonus, new_health_factor).
    fn compute_liquidation_details(
        &self,
        total_collateral: &BigUint,
        weighted_collateral: &BigUint,
        proportion_seized: &BigUint,
        liquidation_bonus: &BigUint,
        total_debt: &BigUint,
        target_hf: BigUint,
    ) -> (BigUint, BigUint, BigUint) {
        let wad = BigUint::from(WAD);
        let bps = BigUint::from(BPS);

        let target_hf_dec = self.to_decimal_signed_wad(target_hf.clone());
        let wad_dec = self.to_decimal_signed_wad(wad.clone());
        let weighted_egld_dec = self.to_decimal_signed_wad(weighted_collateral.clone());

        let required_collateral = self.to_decimal_signed_wad(total_debt * &target_hf / &wad);
        let health_gap = weighted_egld_dec - required_collateral;

        let p_wad = (proportion_seized * &wad) / &bps;
        let collateral_loss_factor =
            self.to_decimal_signed_wad((p_wad * (&bps + liquidation_bonus) + &bps / 2u32) / &bps);
        let denom = collateral_loss_factor - target_hf_dec;

        let ideal_debt_to_repay = health_gap.mul(wad_dec).div(denom);
        require!(
            ideal_debt_to_repay.clone().sign().eq(&Sign::Plus),
            ERROR_DEBT_CAN_NOT_BE_NEGATIVE
        );

        let max_debt_to_liquidate = (total_collateral * &bps) / (&bps + liquidation_bonus);
        let debt_to_repay = BigUint::min(
            max_debt_to_liquidate,
            ideal_debt_to_repay
                .into_unsigned_or_fail()
                .into_raw_units()
                .clone(),
        );

        let seized_weighted = BigUint::min(
            proportion_seized * &debt_to_repay * (&bps + liquidation_bonus) / &bps / &bps,
            weighted_collateral.clone(),
        );
        let new_weighted = weighted_collateral - &seized_weighted;
        let new_health_factor = &new_weighted * &wad / (total_debt - &debt_to_repay);

        (debt_to_repay, liquidation_bonus.clone(), new_health_factor)
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
        weighted_collateral_in_egld: &BigUint,
        proportion_seized: &BigUint,
        total_collateral: &BigUint,
        total_debt: &BigUint,
        min_bonus: &BigUint,
        current_hf: &BigUint,
    ) -> (BigUint, BigUint) {
        let wad = BigUint::from(WAD);
        let bps = BigUint::from(BPS);
        let target_best = &wad * 2u32 / 100u32 + &wad; // 1.02 WAD

        let (safest_debt, safest_bonus, safe_new_hf) = self.simulate_liquidation(
            weighted_collateral_in_egld,
            proportion_seized,
            total_collateral,
            total_debt,
            min_bonus,
            current_hf,
            target_best,
        );

        if safe_new_hf >= wad {
            return (safest_debt, safest_bonus);
        }

        let (limit_debt, limit_bonus, max_safe_hf) = self.simulate_liquidation(
            weighted_collateral_in_egld,
            proportion_seized,
            total_collateral,
            total_debt,
            min_bonus,
            current_hf,
            wad.clone(),
        );

        if max_safe_hf >= wad {
            return (limit_debt, limit_bonus);
        }

        let max_debt_to_liquidate = self
            .div_half_up(
                &self.to_decimal_wad(total_collateral.clone()),
                &self.to_decimal_bps(bps + min_bonus.clone()),
                RAY_PRECISION,
            )
            .rescale(WAD_PRECISION)
            .into_raw_units()
            .clone();

        (max_debt_to_liquidate, min_bonus.clone())
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
        weighted_collateral_in_egld: &BigUint,
        proportion_seized: &BigUint,
        total_collateral: &BigUint,
        total_debt: &BigUint,
        min_bonus: &BigUint,
        current_hf: &BigUint,
        target_hf: BigUint,
    ) -> (BigUint, BigUint, BigUint) {
        let max_bonus = self.calculate_max_feasible_bonus(
            total_collateral,
            total_debt,
            proportion_seized,
            &target_hf,
            min_bonus,
        );
        let bonus =
            self.calculate_dynamic_liquidation_bonus(current_hf, &target_hf, min_bonus, &max_bonus);
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
