use common_constants::{
    BPS, MAX_FIRST_TOLERANCE, MAX_LAST_TOLERANCE, MIN_FIRST_TOLERANCE, MIN_LAST_TOLERANCE, WAD,
    WAD_PRECISION,
};
use common_events::{OraclePriceFluctuation, PriceFeedShort, BPS_PRECISION, RAY_PRECISION};

use crate::{
    ERROR_DEBT_CAN_NOT_BE_NEGATIVE, ERROR_UNEXPECTED_ANCHOR_TOLERANCES,
    ERROR_UNEXPECTED_FIRST_TOLERANCE, ERROR_UNEXPECTED_LAST_TOLERANCE,
};

multiversx_sc::imports!();

#[multiversx_sc::module]
pub trait MathsModule: common_math::SharedMathModule {
    /// Compute amount in tokens
    ///
    /// This function is used to compute the amount of a token in tokens from the amount in egld
    /// It uses the price of the token to convert the amount to tokens
    fn compute_egld_in_tokens(
        &self,
        amount_in_egld: &ManagedDecimal<Self::Api, NumDecimals>,
        token_data: &PriceFeedShort<Self::Api>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        self.div_half_up(amount_in_egld, &token_data.price, RAY_PRECISION)
            .rescale(token_data.decimals)
    }

    /// Get token amount in dollars raw
    ///
    /// This function is used to get the amount of a token in dollars from the raw price
    /// It uses the price of the token to convert the amount to dollars
    fn get_token_amount_in_dollars_raw(
        &self,
        amount: &ManagedDecimal<Self::Api, NumDecimals>,
        token_price: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        self.mul_half_up(amount, token_price, RAY_PRECISION)
            .rescale(WAD_PRECISION)
    }

    /// Get token amount in egld raw
    ///
    /// This function is used to get the amount of a token in egld from the raw price
    /// It converts the amount of the token to egld using the price of the token and the decimals
    fn get_token_amount_in_egld_raw(
        &self,
        amount: &ManagedDecimal<Self::Api, NumDecimals>,
        token_price: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        self.mul_half_up(amount, token_price, RAY_PRECISION)
            .rescale(WAD_PRECISION)
    }

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
        weighted_collateral_in_egld: &ManagedDecimal<Self::Api, NumDecimals>,
        borrowed_value_in_egld: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        // If there's no borrowed value, health factor is "infinite" (represented by max value)
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

    /// Calculates upper and lower bounds for a given tolerance
    ///
    /// # Arguments
    /// * `tolerance` - Tolerance value in basis points
    ///
    /// # Returns
    /// * `(BigUint, BigUint)` - Tuple containing:
    ///   - Upper bound (wad + tolerance)
    ///   - Lower bound (wad * wad / upper)
    ///
    /// ```
    fn get_range(
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

        (
            ManagedDecimal::from_raw_units(upper, BPS_PRECISION),
            ManagedDecimal::from_raw_units(lower, BPS_PRECISION),
        )
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
        let bonus_range = liquidation_bonus_max - liquidation_bonus_min;
        let bonus_increase = (bonus_range * (target_hf - old_hf)) / target_hf;
        let liquidation_bonus = liquidation_bonus_min + &bonus_increase;

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
        total_collateral_value: &BigUint, // in WAD (e.g. collateral value in EGLD, 18 decimals)
        total_debt_value: &BigUint,       // in WAD
        proportion_of_weighted_seized: &BigUint, // in BPS (e.g. 8000 means 80%)
        target_hf: &BigUint,              // in WAD (e.g. 1.02e18 for 1.02)
        proportion_of_weighted_bonus: &BigUint, // in BPS (minimum allowed bonus)
    ) -> BigUint {
        let wad = BigUint::from(WAD); // e.g., 1e18
        let bps = BigUint::from(BPS); // e.g., 10000

        // Convert the raw BPS inputs to WAD-scale fractions.
        // p: the proportion seized, in WAD (so 8000 BPS becomes 0.8*wad)
        let p = (proportion_of_weighted_seized * &wad) / &bps;

        // 1. Calculate 'n'
        // n = target_hf * total_debt - p * total_collateral
        let n = target_hf * total_debt_value - &p * total_collateral_value;

        // 2. BOUND 1: bonus < (T / p) - 1, with T and p in WAD.
        // Compute: bound1 = (target_hf / p - 1) scaled to WAD = target_hf * wad / p - wad
        let bound1_numerator = target_hf * &wad / &p;
        let bound1 = &bound1_numerator - &wad;

        // 3. BOUND 2: bonus ≤ (C * T - C * p - n) / (n + C * p)
        // Multiply numerator by wad to maintain consistent WAD scaling:
        let numerator =
            (total_collateral_value * target_hf * &wad) - (total_collateral_value * &p * &wad) - &n;
        let denominator = n + total_collateral_value * &p;
        let bound2 = &numerator / &denominator;

        // Convert the minimum allowed bonus (given in BPS) into WAD-scale:
        let bonus_min = (proportion_of_weighted_bonus * &wad) / &bps;

        // Choose the smaller of the two bounds, but at least bonus_min:
        let bonus_wad = BigUint::max(bonus_min, BigUint::min(bound1, bound2));

        // Now, convert the bonus from WAD-scale back to BPS:
        // bonus_bps = (bonus_wad * bps) / wad
        let bonus_bps = (bonus_wad * &bps) / &wad;

        bonus_bps
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
        total_collateral_all_assets: &BigUint,
        weighted_collateral_in_egld: &BigUint,
        proportion_of_weighted_seized: &BigUint,
        liquidation_bonus: &BigUint,
        total_debt: &BigUint,
        target_hf: BigUint,
    ) -> (BigUint, BigUint, BigUint) {
        // Constants in BigUint
        let wad = BigUint::from(WAD);
        let bps = BigUint::from(BPS);

        // Convert target_hf and wad to decimal type once.
        let target_hf_dec = self.to_decimal_signed_wad(target_hf.clone());
        let wad_dec = self.to_decimal_signed_wad(wad.clone());
        let weighted_egld_dec = self.to_decimal_signed_wad(weighted_collateral_in_egld.clone());

        // 1. Compute the health gap (n): required collateral vs. actual weighted collateral.
        // health_gap = weighted_egld - (total_debt * target_hf / wad)
        let required_collateral = self.to_decimal_signed_wad(total_debt * &target_hf / &wad);
        let health_gap = weighted_egld_dec - required_collateral;

        // Convert the raw seized proportion to a WAD value.
        let p_wad = (proportion_of_weighted_seized * &wad) / &bps;

        // Compute the collateral loss factor t.
        // t = p_wad * (bps + liquidation_bonus) / bps (with rounding adjustment)
        let collateral_loss_factor =
            self.to_decimal_signed_wad((p_wad * (&bps + liquidation_bonus) + &bps / 2u32) / &bps);

        // Denom: difference between loss factor and target health factor.
        let denom = collateral_loss_factor - target_hf_dec;

        // Ideal debt to repay:
        let ideal_debt_to_repay = health_gap.mul(wad_dec).div(denom);
        require!(
            ideal_debt_to_repay.clone().sign().eq(&Sign::Plus),
            ERROR_DEBT_CAN_NOT_BE_NEGATIVE
        );

        // 2. Maximum debt that can be liquidated based on available collateral:
        let max_debt_to_liquidate =
            (total_collateral_all_assets * &bps) / (&bps + liquidation_bonus);

        // 3. Actual debt to repay is the minimum of ideal and maximum allowed.
        let debt_to_repay = BigUint::min(
            max_debt_to_liquidate,
            ideal_debt_to_repay
                .into_unsigned_or_fail()
                .into_raw_units()
                .clone(),
        );

        // 4. Compute the weighted collateral seized based on the proportion.
        let seized_weighted = BigUint::min(
            proportion_of_weighted_seized * &debt_to_repay * (&bps + liquidation_bonus)
                / &bps
                / &bps,
            weighted_collateral_in_egld.clone(),
        );
        let new_weighted = weighted_collateral_in_egld - &seized_weighted;
        let new_health_factor = &new_weighted * &wad / (total_debt - &debt_to_repay);

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
        weighted_collateral_in_egld: &BigUint,
        proportion_of_weighted_seized: &BigUint,
        total_collateral: &BigUint,
        total_debt: &BigUint,
        min_bonus: &BigUint,
        old_hf: &BigUint,
    ) -> (BigUint, BigUint) {
        let wad = BigUint::from(WAD);
        let bps = BigUint::from(BPS);
        let target_best = &wad * 2u32 / 100u32 + &wad;
        // Try to bring it to at least 1.02 HF to be in a safer position
        let (safest_debt, safest_bonus, safe_new_hf) = self.simulation_target_liquidation(
            weighted_collateral_in_egld,
            proportion_of_weighted_seized,
            total_collateral,
            total_debt,
            min_bonus,
            old_hf,
            target_best,
        );
        if &safe_new_hf >= &wad {
            return (safest_debt, safest_bonus);
        }

        // When 1.02 is not possible try to bring it to minimum 1.0 if possible
        let (limit_debt, limit_bonus, max_safe_hf) = self.simulation_target_liquidation(
            weighted_collateral_in_egld,
            proportion_of_weighted_seized,
            total_collateral,
            total_debt,
            min_bonus,
            old_hf,
            wad.clone(),
        );

        if &max_safe_hf >= &wad {
            return (limit_debt, limit_bonus);
        }

        // let max_debt_to_liquidate = (total_collateral * &bps) / (&bps + &((min_bonus + &bps) / &bps));
        let numerator = total_collateral * &bps;
        let denominator = &bps + min_bonus;
        let max_debt_to_liquidate =
            (numerator + denominator.clone() / BigUint::from(2u64)) / denominator;
        sc_print!("max_debt_to_liquidate {}", max_debt_to_liquidate);
        sc_print!("total_collateral      {}", total_collateral);
        sc_print!("min_bonus             {}", min_bonus);
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

    // #[view(getMaxLeverage)]
    // fn calculate_max_leverage(
    //     &self,
    //     initial_deposit: &BigUint,
    //     health_factor: &BigUint,
    //     e_mode: &Option<EModeCategory<Self::Api>>,
    //     asset_config: &AssetConfig<Self::Api>,
    //     total_reserves: &BigUint,
    //     reserve_buffer: &BigUint,
    // ) -> BigUint {
    //     let wad = BigUint::from(WAD);
    //     let wad_dec = ManagedDecimal::from_raw_units(BigUint::from(WAD), WAD_PRECISION);
    //     let liquidation_threshold = if let Some(mode) = e_mode {
    //         &mode.liquidation_threshold
    //     } else {
    //         &asset_config.liquidation_threshold
    //     };
    //     let flash_loan_fee = &asset_config.flash_loan_fee;
    //     // If both `health_factor` and `flash_loan_fee` are already scaled by BP, then
    //     //    (1 + fee) = (bp + flash_loan_fee), also in BP scale.
    //     // We multiply them and THEN divide by BP to keep the result in the same BP scale.
    //     let hf_plus_fee = (health_factor * (wad_dec + flash_loan_fee.clone())) / &wad;

    //     // `liquidation_threshold` is already in the same BP scale as HF, so we subtract it directly.
    //     // The denominator is HF*(1+F) - LT, all in the same BP scale.
    //     // let max_l_hf_numerator = &hf_plus_fee;
    //     let max_l_hf_denominator = &hf_plus_fee - liquidation_threshold;

    //     // The final result for max leverage by HF formula:
    //     //   MaxL_HF = [ HF*(1 + fee) ] / [ HF*(1 + fee) - liquidation_threshold ]
    //     // Since everything is in BP scale, we multiply by BP one more time to keep output in BP scale.
    //     let max_l_hf = &hf_plus_fee * &wad / &max_l_hf_denominator;

    //     // --- Reserve-based constraint:
    //     // AR = total_reserves * (1 - reserve_buffer).
    //     // Because `reserve_buffer` is in BP scale, do the appropriate scaling by `bp` to get normal units.
    //     let available_reserves = (total_reserves * &(&wad - reserve_buffer)) / &wad;

    //     // Suppose we want to say:
    //     //   "If we have AR available, how many times bigger is that than `initial_deposit`?"
    //     //   ratio = AR / D  (but in normal arithmetic, ratio is dimensionless).
    //     //   Then leverage-limited = ratio + 1  => in BP scale => ratio * BP + BP
    //     // So we do:
    //     //   ratio_in_bp = (AR * BP) / D
    //     //   max_l_reserves = ratio_in_bp + BP
    //     let ratio_in_bp = (&available_reserves * &wad) / initial_deposit;
    //     let max_l_reserves = ratio_in_bp + &wad;

    //     // Final max leverage is the minimum of the HF-based limit and the Reserve-based limit
    //     let max_l = BigUint::min(max_l_hf, max_l_reserves);

    //     max_l
    // }
}
