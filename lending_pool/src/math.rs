multiversx_sc::imports!();

use crate::{oracle, storage, ERROR_NO_COLLATERAL_TOKEN};
use common_constants::{BP, MAX_BONUS};

#[multiversx_sc::module]
pub trait LendingMathModule: storage::LendingStorageModule + oracle::OracleModule {
    /// Computes the health factor for a position based on weighted collateral and borrowed value
    ///
    /// # Arguments
    /// * `weighted_collateral_in_egld` - Total EGLD value of collateral weighted by liquidation thresholds
    /// * `borrowed_value_in_egld` - Total EGLD value of borrowed assets
    ///
    /// # Returns
    /// * `BigUint` - Health factor in basis points (10000 = 100%)
    ///
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

    fn calculate_liquidation(
        &self,
        total_collateral: &BigUint,
        liquidation_th: &BigUint,
        liquidation_bonus: &BigUint,
        total_debt: &BigUint,
        target_hf: &BigUint,
    ) -> (BigUint, BigUint, bool) {
        let bp = BigUint::from(BP);
        // Calculate the debt required to reach the target health factor
        let debt = &((target_hf * total_debt - liquidation_th * total_collateral)
            / (target_hf.clone() - (liquidation_th * &(&bp + liquidation_bonus)) / &bp));

        // Calculate total_seize based on the debt and liquidationBonus
        let total_seize = debt + &((debt * liquidation_bonus) / &bp);

        // Check if total_seize exceeds totalCollateral
        if &total_seize > total_collateral {
            return (debt.clone(), total_seize, true);
        }

        return (debt.clone(), total_seize, false);
    }

    fn estimate_liquidation_amount(
        &self,
        total_collateral: &BigUint,
        total_debt: &BigUint,
        liquidation_th: &BigUint,
        min_bonus: &BigUint,
        old_hf: &BigUint,
    ) -> (BigUint, BigUint, BigUint) {
        // (debt, seize, bonus)
        let bp = BigUint::from(BP);
        let min_hf = bp.clone() / 2u32; // 0.5
        let target = &bp * 8u32 / 100u32 + &bp;

        let max_bonus = self.get_max_feasible_bonus(
            total_collateral,
            total_debt,
            liquidation_th,
            &target,
            min_bonus,
        );
        let liquidation_bonus =
            self.calculate_dynamic_liquidation_bonus(&old_hf, min_bonus, &max_bonus, &min_hf);

        let (debt, total_seize, over_seize) = self.calculate_liquidation(
            total_collateral,
            liquidation_th,
            &liquidation_bonus,
            total_debt,
            &target,
        );

        if !over_seize {
            (debt, total_seize, liquidation_bonus)
        } else {
            let fallback_debt = total_collateral.mul(&bp).div(bp.add(min_bonus));
            let total_seize = total_collateral.clone();

            (fallback_debt, total_seize, min_bonus.clone())
        }
    }

    /// Calculates the maximum amount of a specific collateral asset that can be liquidated
    ///
    /// # Arguments
    /// * `total_debt_in_egld` - Total EGLD value of user's debt
    /// * `total_collateral_in_egld` - Total EGLD value of all collateral
    /// * `token_to_liquidate` - Token identifier of collateral to liquidate
    /// * `token_price_data` - Price feed data for the collateral token
    /// * `liquidatee_account_nonce` - NFT nonce of the account being liquidated
    /// * `debt_payment_in_egld` - Optional EGLD value of debt being repaid
    /// * `base_liquidation_bonus` - Base liquidation bonus in basis points (10^21 = 100%)
    /// * `health_factor` - Current health factor in basis points (10^21 = 100%)
    ///
    /// # Returns
    /// * `BigUint` - Maximum EGLD value of the specific collateral that can be liquidated
    ///
    /// ```
    fn calculate_single_asset_liquidation_amount(
        &self,
        total_debt_in_egld: &BigUint,
        total_collateral_in_egld: &BigUint,
        token_to_liquidate: &EgldOrEsdtTokenIdentifier,
        liquidatee_account_nonce: u64,
        debt_payment_in_egld: OptionalValue<BigUint>,
        base_liquidation_bonus: &BigUint,
        health_factor: &BigUint,
    ) -> (BigUint, BigUint) {
        // Get the available collateral value for this specific asset
        let deposit_position = self
            .deposit_positions(liquidatee_account_nonce)
            .get(token_to_liquidate)
            .unwrap_or_else(|| sc_panic!(ERROR_NO_COLLATERAL_TOKEN));

        let (max_repayable_debt, seized, bonus) = self.estimate_liquidation_amount(
            total_collateral_in_egld,
            total_debt_in_egld,
            &deposit_position.entry_liquidation_threshold,
            base_liquidation_bonus,
            health_factor,
        );

        sc_print!("max_liquidatable_amount: {}", max_repayable_debt);
        sc_print!("collateral_seized      : {}", seized);
        sc_print!("final_liquidation_bonus: {}", bonus);

        if debt_payment_in_egld.is_some() {
            // Take the minimum between what we need and what's available and what the liquidator is paying
            (
                BigUint::min(
                    debt_payment_in_egld.into_option().unwrap(),
                    max_repayable_debt,
                ),
                bonus,
            )
        } else {
            (max_repayable_debt, bonus)
        }
    }

    /// Calculates a dynamic protocol fee based on position health
    ///
    /// # Arguments
    /// * `health_factor` - Current health factor in basis points (10^21 = 100%)
    /// * `base_protocol_fee` - Base protocol fee in basis points (10^21 = 100%)
    ///
    /// # Returns
    /// * `BigUint` - Final protocol fee in basis points
    /// ```
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
}
