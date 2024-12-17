multiversx_sc::imports!();

use crate::{oracle, storage, ERROR_NO_COLLATERAL_TOKEN};
use common_constants::{BP, MAX_BONUS};
use common_structs::PriceFeedShort;

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
    /// # Examples
    /// ```
    /// // Example 1: No borrows
    /// weighted_collateral = 1000 EGLD
    /// borrowed_value = 0 EGLD
    /// health_factor = u128::MAX (effectively infinite)
    ///
    /// // Example 2: Healthy position
    /// weighted_collateral = 150 EGLD
    /// borrowed_value = 100 EGLD
    /// health_factor = 15000 (150%)
    ///
    /// // Example 3: Unhealthy position
    /// weighted_collateral = 90 EGLD
    /// borrowed_value = 100 EGLD
    /// health_factor = 9000 (90%)
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

    /// Calculates the maximum amount that can be liquidated in a single transaction
    ///
    /// # Arguments
    /// * `total_debt_in_egld` - Total EGLD value of user's debt
    /// * `weighted_collateral_in_egld` - Total EGLD value of collateral weighted by liquidation thresholds
    /// * `liquidation_bonus` - Bonus percentage for liquidators in basis points
    ///
    /// # Returns
    /// * `BigUint` - Maximum EGLD value that can be liquidated
    ///
    /// # Examples
    /// ```
    /// // Example 1: Position slightly unhealthy
    /// total_debt = 100 EGLD
    /// weighted_collateral = 95 EGLD
    /// liquidation_bonus = 500 (5%)
    /// target_hf = 1.05
    /// required_repayment ≈ 20 EGLD
    ///
    /// // Example 2: Position deeply unhealthy
    /// total_debt = 100 EGLD
    /// weighted_collateral = 50 EGLD
    /// liquidation_bonus = 500 (5%)
    /// target_hf = 1.05
    /// required_repayment = 100 EGLD (full liquidation)
    /// ```
    fn calculate_max_liquidatable_amount_in_egld(
        &self,
        total_debt_in_egld: &BigUint,
        weighted_collateral_in_egld: &BigUint,
        liquidation_bonus: &BigUint,
    ) -> BigUint {
        let bp = BigUint::from(BP);
        // Target HF = 1.05 for all liquidations
        let target_hf = bp.clone() + (bp.clone() / &BigUint::from(20u32)); // 1.05

        // Calculate required repayment to reach target HF
        let adjusted_debt = total_debt_in_egld * &target_hf / &bp;
        // Since HF < 1.0, total_debt > weighted_collateral, and we multiply by 1.05,
        // adjusted_debt is always > weighted_collateral
        let debt_surplus = adjusted_debt.clone() - weighted_collateral_in_egld;
        let required_repayment = &debt_surplus * &bp / &(liquidation_bonus + &(&target_hf - &bp));

        // If required repayment is more than total debt, it means we can't restore HF > 1.05
        // In this case, allow maximum liquidation (100% of debt)
        if &required_repayment > total_debt_in_egld {
            total_debt_in_egld.clone()
        } else {
            required_repayment
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
    /// * `liquidation_bonus` - Bonus percentage for liquidators in basis points
    ///
    /// # Returns
    /// * `BigUint` - Maximum EGLD value of the specific collateral that can be liquidated
    ///
    /// # Examples
    /// ```
    /// // Example 1: Single collateral liquidation
    /// total_debt = 100 EGLD
    /// total_collateral = 95 EGLD
    /// available_collateral = 95 EGLD (EGLD)
    /// debt_payment = None
    /// max_liquidatable = min(20 EGLD, 95 EGLD) = 20 EGLD
    ///
    /// // Example 2: Partial liquidation with payment limit
    /// total_debt = 100 EGLD
    /// total_collateral = 95 EGLD
    /// available_collateral = 95 EGLD (EGLD)
    /// debt_payment = Some(10 EGLD)
    /// max_liquidatable = min(20 EGLD, 95 EGLD, 10 EGLD) = 10 EGLD
    /// ```
    fn calculate_single_asset_liquidation_amount(
        &self,
        total_debt_in_egld: &BigUint,
        total_collateral_in_egld: &BigUint,
        token_to_liquidate: &EgldOrEsdtTokenIdentifier,
        token_price_data: &PriceFeedShort<Self::Api>,
        liquidatee_account_nonce: u64,
        debt_payment_in_egld: OptionalValue<BigUint>,
        liquidation_bonus: &BigUint,
    ) -> BigUint {
        // Get the available collateral value for this specific asset
        let deposit_position = self
            .deposit_positions(liquidatee_account_nonce)
            .get(token_to_liquidate)
            .unwrap_or_else(|| sc_panic!(ERROR_NO_COLLATERAL_TOKEN));

        let available_collateral_value_in_egld = self
            .get_token_amount_in_egld_raw(&deposit_position.get_total_amount(), token_price_data);

        let max_liquidatable_amount = self.calculate_max_liquidatable_amount_in_egld(
            total_debt_in_egld,
            total_collateral_in_egld,
            liquidation_bonus,
        );

        if debt_payment_in_egld.is_some() {
            // Take the minimum between what we need and what's available and what the liquidator is paying
            BigUint::min(max_liquidatable_amount, available_collateral_value_in_egld)
                .min(debt_payment_in_egld.into_option().unwrap())
        } else {
            BigUint::min(max_liquidatable_amount, available_collateral_value_in_egld)
        }
    }

    /// Calculates a dynamic liquidation bonus based on position health
    ///
    /// # Arguments
    /// * `health_factor` - Current health factor in basis points (10^21 = 100%)
    /// * `initial_bonus` - Base liquidation bonus in basis points (10^21 = 100%)
    ///
    /// # Returns
    /// * `BigUint` - Final liquidation bonus in basis points
    ///
    /// # Examples
    /// ```
    /// // Example 1: Slightly unhealthy position (HF = 95%)
    /// health_factor = 950_000_000_000_000_000_000 (95%)
    /// initial_bonus = 50_000_000_000_000_000_000 (5%)
    /// health_factor_impact = (1_000_000_000_000_000_000_000 - 950_000_000_000_000_000_000) * 2 = 100_000_000_000_000_000_000
    /// bonus_increase = min(100_000_000_000_000_000_000 * 50_000_000_000_000_000_000 / 1_000_000_000_000_000_000_000, 300_000_000_000_000_000_000) = 5_000_000_000_000_000_000
    /// final_bonus = 55_000_000_000_000_000_000 (5.5%)
    ///
    /// // Example 2: Very unhealthy position (HF = 50%)
    /// health_factor = 500_000_000_000_000_000_000 (50%)
    /// initial_bonus = 50_000_000_000_000_000_000 (5%)
    /// health_factor_impact = (1_000_000_000_000_000_000_000 - 500_000_000_000_000_000_000) * 2 = 1_000_000_000_000_000_000_000
    /// bonus_increase = min(1_000_000_000_000_000_000_000 * 50_000_000_000_000_000_000 / 1_000_000_000_000_000_000_000, 300_000_000_000_000_000_000) = 50_000_000_000_000_000_000
    /// final_bonus = 100_000_000_000_000_000_000 (10%)
    /// ```
    fn calculate_dynamic_liquidation_bonus(
        &self,
        health_factor: &BigUint,
        initial_bonus: BigUint,
    ) -> BigUint {
        let bp = BigUint::from(BP);

        // Multiply by 2 to increase the bonus for more unhealthy positions
        let health_factor_impact = (&bp - health_factor).mul(2u64);

        // Max bonus increase is 30% (300_000_000_000_000_000_000 basis points)
        let max_bonus_increase = BigUint::from(MAX_BONUS);

        // Calculate bonus increase based on how unhealthy the position is
        // More unhealthy = bigger bonus to incentivize quick liquidation
        let bonus_increase = BigUint::min(
            &health_factor_impact * &initial_bonus / bp,
            max_bonus_increase,
        );

        initial_bonus + bonus_increase
    }

    /// Calculates a dynamic protocol fee based on position health
    ///
    /// # Arguments
    /// * `health_factor` - Current health factor in basis points (10^21 = 100%)
    /// * `base_protocol_fee` - Base protocol fee in basis points (10^21 = 100%)
    ///
    /// # Returns
    /// * `BigUint` - Final protocol fee in basis points
    ///
    /// # Examples
    /// ```
    /// // Example 1: Healthy position (HF = 100%)
    /// health_factor = 1_000_000_000_000_000_000_000 (100%)
    /// base_protocol_fee = 100_000_000_000_000_000_000 (10%)
    /// final_fee = 100_000_000_000_000_000_000 (10%, no reduction)
    ///
    /// // Example 2: Unhealthy position (HF = 80%)
    /// health_factor = 800_000_000_000_000_000_000 (80%)
    /// base_protocol_fee = 100_000_000_000_000_000_000 (10%)
    /// distance = 200_000_000_000_000_000_000 (20%)
    /// health_factor_impact = 400_000_000_000_000_000_000 (40%)
    /// fee_reduction = min(40_000_000_000_000_000_000, 300_000_000_000_000_000_000) = 40_000_000_000_000_000_000
    /// max_allowed_reduction = 50_000_000_000_000_000_000 (5%)
    /// final_reduction = min(40_000_000_000_000_000_000, 50_000_000_000_000_000_000) = 40_000_000_000_000_000_000
    /// final_fee = 60_000_000_000_000_000_000 (6%)
    ///
    /// // Example 3: Very Unhealthy position (HF = 60%)
    /// health_factor = 600_000_000_000_000_000_000 (60%)
    /// base_protocol_fee = 100_000_000_000_000_000_000 (10%)
    /// distance = 400_000_000_000_000_000_000 (40%)
    /// health_factor_impact = 800_000_000_000_000_000_000 (80%)
    /// fee_reduction = min(80_000_000_000_000_000_000, 300_000_000_000_000_000_000) = 80_000_000_000_000_000_000
    /// max_allowed_reduction = 50_000_000_000_000_000_000 (5%)
    /// final_reduction = min(80_000_000_000_000_000_000, 50_000_000_000_000_000_000) = 50_000_000_000_000_000_000
    /// final_fee = 50_000_000_000_000_000_000 (5%)
    /// ```
    fn calculate_dynamic_protocol_fee(
        &self,
        health_factor: &BigUint,
        base_protocol_fee: BigUint,
    ) -> BigUint {
        let bp = BigUint::from(BP);
        let max_bonus = BigUint::from(MAX_BONUS); // 30%

        // Only start reducing fee when health factor < 100% of BP
        if health_factor >= &bp {
            return base_protocol_fee;
        }

        // Calculate how far below 100% the HF is
        let distance_from_threshold = &bp - health_factor;

        // Similar to bonus calculation, multiply by 2 for steeper reduction
        let health_factor_impact = distance_from_threshold.mul(2u64);

        // Calculate fee reduction based on how unhealthy the position is
        // More unhealthy = bigger fee reduction to incentivize quick liquidation
        let fee_reduction =
            BigUint::min(&health_factor_impact * &base_protocol_fee / bp, max_bonus);

        // Ensure we never reduce more than 50% of the base fee
        let max_allowed_reduction = base_protocol_fee.clone() / 2u32;
        let final_reduction = BigUint::min(fee_reduction, max_allowed_reduction);

        base_protocol_fee - final_reduction
    }
}
