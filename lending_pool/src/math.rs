multiversx_sc::imports!();

use common_events::MAX_BONUS;
use common_structs::BP;

use crate::{oracle, proxy_price_aggregator::PriceFeed, storage, ERROR_NO_COLLATERAL_TOKEN};

#[multiversx_sc::module]
pub trait LendingMathModule: storage::LendingStorageModule + oracle::OracleModule {
    /// Computes the health factor for a position based on weighted collateral and borrowed value
    /// 
    /// # Arguments
    /// * `weighted_collateral_in_dollars` - Total USD value of collateral weighted by liquidation thresholds
    /// * `borrowed_value_in_dollars` - Total USD value of borrowed assets
    /// 
    /// # Returns
    /// * `BigUint` - Health factor in basis points (10000 = 100%)
    /// 
    /// # Examples
    /// ```
    /// // Example 1: No borrows
    /// weighted_collateral = 1000 USD
    /// borrowed_value = 0 USD
    /// health_factor = u128::MAX (effectively infinite)
    /// 
    /// // Example 2: Healthy position
    /// weighted_collateral = 150 USD
    /// borrowed_value = 100 USD
    /// health_factor = 15000 (150%)
    /// 
    /// // Example 3: Unhealthy position
    /// weighted_collateral = 90 USD
    /// borrowed_value = 100 USD
    /// health_factor = 9000 (90%)
    /// ```
    fn compute_health_factor(
        &self,
        weighted_collateral_in_dollars: &BigUint,
        borrowed_value_in_dollars: &BigUint,
    ) -> BigUint {
        // If there's no borrowed value, health factor is "infinite" (represented by max value)
        if borrowed_value_in_dollars == &BigUint::zero() {
            return BigUint::from(u128::MAX);
        }

        let health_factor = weighted_collateral_in_dollars
            .mul(&BigUint::from(BP))
            .div(borrowed_value_in_dollars);

        health_factor
    }

    /// Calculates the maximum amount that can be liquidated in a single transaction
    /// 
    /// # Arguments
    /// * `total_debt_in_dollars` - Total USD value of user's debt
    /// * `weighted_collateral_in_dollars` - Total USD value of collateral weighted by liquidation thresholds
    /// * `liquidation_bonus` - Bonus percentage for liquidators in basis points
    /// 
    /// # Returns
    /// * `BigUint` - Maximum USD value that can be liquidated
    /// 
    /// # Examples
    /// ```
    /// // Example 1: Position slightly unhealthy
    /// total_debt = 100 USD
    /// weighted_collateral = 95 USD
    /// liquidation_bonus = 500 (5%)
    /// target_hf = 1.05
    /// required_repayment ≈ 20 USD
    /// 
    /// // Example 2: Position deeply unhealthy
    /// total_debt = 100 USD
    /// weighted_collateral = 50 USD
    /// liquidation_bonus = 500 (5%)
    /// target_hf = 1.05
    /// required_repayment = 100 USD (full liquidation)
    /// ```
    fn calculate_max_liquidatable_amount_in_dollars(
        &self,
        total_debt_in_dollars: &BigUint,
        weighted_collateral_in_dollars: &BigUint,
        liquidation_bonus: &BigUint,
    ) -> BigUint {
        let bp = BigUint::from(BP);
        // Target HF = 1.05 for all liquidations
        let target_hf = bp.clone() + (bp.clone() / &BigUint::from(20u32)); // 1.05

        // Calculate required repayment to reach target HF
        let adjusted_debt = total_debt_in_dollars * &target_hf / &bp;
        // Since HF < 1.0, total_debt > weighted_collateral, and we multiply by 1.05,
        // adjusted_debt is always > weighted_collateral
        let debt_surplus = adjusted_debt.clone() - weighted_collateral_in_dollars;
        let required_repayment = &debt_surplus * &bp / &(liquidation_bonus + &(&target_hf - &bp));

        // If required repayment is more than total debt, it means we can't restore HF > 1.05
        // In this case, allow maximum liquidation (100% of debt)
        if &required_repayment > total_debt_in_dollars {
            total_debt_in_dollars.clone()
        } else {
            required_repayment
        }
    }

    /// Calculates the maximum amount of a specific collateral asset that can be liquidated
    /// 
    /// # Arguments
    /// * `total_debt_in_dollars` - Total USD value of user's debt
    /// * `total_collateral_in_dollars` - Total USD value of all collateral
    /// * `token_to_liquidate` - Token identifier of collateral to liquidate
    /// * `token_price_data` - Price feed data for the collateral token
    /// * `liquidatee_account_nonce` - NFT nonce of the account being liquidated
    /// * `debt_payment_in_usd` - Optional USD value of debt being repaid
    /// * `liquidation_bonus` - Bonus percentage for liquidators in basis points
    /// 
    /// # Returns
    /// * `BigUint` - Maximum USD value of the specific collateral that can be liquidated
    /// 
    /// # Examples
    /// ```
    /// // Example 1: Single collateral liquidation
    /// total_debt = 100 USD
    /// total_collateral = 95 USD
    /// available_collateral = 95 USD (EGLD)
    /// debt_payment = None
    /// max_liquidatable = min(20 USD, 95 USD) = 20 USD
    /// 
    /// // Example 2: Partial liquidation with payment limit
    /// total_debt = 100 USD
    /// total_collateral = 95 USD
    /// available_collateral = 95 USD (EGLD)
    /// debt_payment = Some(10 USD)
    /// max_liquidatable = min(20 USD, 95 USD, 10 USD) = 10 USD
    /// ```
    fn calculate_single_asset_liquidation_amount(
        &self,
        total_debt_in_dollars: &BigUint,
        total_collateral_in_dollars: &BigUint,
        token_to_liquidate: &EgldOrEsdtTokenIdentifier,
        token_price_data: &PriceFeed<Self::Api>,
        liquidatee_account_nonce: u64,
        debt_payment_in_usd: OptionalValue<BigUint>,
        liquidation_bonus: &BigUint,
    ) -> BigUint {
        // Get the available collateral value for this specific asset
        let deposit_position = self
            .deposit_positions(liquidatee_account_nonce)
            .get(token_to_liquidate)
            .unwrap_or_else(|| sc_panic!(ERROR_NO_COLLATERAL_TOKEN));

        let available_collateral_value = self.get_token_amount_in_dollars_raw(
            &deposit_position.get_total_amount(),
            token_price_data,
        );

        let max_liquidatable_amount = self.calculate_max_liquidatable_amount_in_dollars(
            total_debt_in_dollars,
            total_collateral_in_dollars,
            liquidation_bonus,
        );

        if debt_payment_in_usd.is_some() {
            // Take the minimum between what we need and what's available and what the liquidator is paying
            BigUint::min(max_liquidatable_amount, available_collateral_value)
                .min(debt_payment_in_usd.into_option().unwrap())
        } else {
            BigUint::min(max_liquidatable_amount, available_collateral_value)
        }
    }

    /// Calculates a dynamic liquidation bonus based on position health
    /// 
    /// # Arguments
    /// * `health_factor` - Current health factor in basis points
    /// * `initial_bonus` - Base liquidation bonus in basis points
    /// 
    /// # Returns
    /// * `BigUint` - Final liquidation bonus in basis points
    /// 
    /// # Examples
    /// ```
    /// // Example 1: Slightly unhealthy position
    /// health_factor = 9500 (95%)
    /// initial_bonus = 500 (5%)
    /// health_factor_impact = (10000 - 9500) * 2 = 1000
    /// bonus_increase = min(1000 * 500 / 10000, 3000) = 50
    /// final_bonus = 550 (5.5%)
    /// 
    /// // Example 2: Very unhealthy position
    /// health_factor = 5000 (50%)
    /// initial_bonus = 500 (5%)
    /// health_factor_impact = (10000 - 5000) * 2 = 10000
    /// bonus_increase = min(10000 * 500 / 10000, 3000) = 500
    /// final_bonus = 1000 (10%)
    /// ```
    fn calculate_dynamic_liquidation_bonus(
        &self,
        health_factor: &BigUint,
        initial_bonus: BigUint,
    ) -> BigUint {
        // Example: If health factor is 0.95 (95%), position is slightly unhealthy
        // If health factor is 0.5 (50%), position is very unhealthy

        // BP - health_factor gives us how far below 1.0 the HF is
        // HF 0.95 -> 1.0 - 0.95 = 0.05 (5% below healthy)
        // HF 0.50 -> 1.0 - 0.50 = 0.50 (50% below healthy)
        let bp = BigUint::from(BP);

        // Multiply by 2 to increase the bonus for more unhealthy positions
        let health_factor_impact = (&bp - health_factor).mul(2u64);

        // Max bonus increase is 30% (3000 basis points)
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
    /// * `health_factor` - Current health factor in basis points
    /// * `base_protocol_fee` - Base protocol fee in basis points
    /// 
    /// # Returns
    /// * `BigUint` - Final protocol fee in basis points
    /// 
    /// # Examples
    /// ```
    /// // Example 1: Healthy position (HF > 75%)
    /// health_factor = 8000 (80%)
    /// base_protocol_fee = 1000 (10%)
    /// final_fee = 1000 (10%, no reduction)
    /// 
    /// // Example 2: Unhealthy position
    /// health_factor = 6000 (60%)
    /// base_protocol_fee = 1000 (10%)
    /// distance = 7500 - 6000 = 1500
    /// health_factor_impact = 1500 * 2 = 3000
    /// fee_reduction = min(3000 * 1000 / 10000, 2500) = 300
    /// final_fee = 700 (7%)
    /// ```
    fn calculate_dynamic_protocol_fee(
        &self,
        health_factor: &BigUint,
        base_protocol_fee: BigUint,
    ) -> BigUint {
        let bp = BigUint::from(BP);
        let max_bonus = BigUint::from(MAX_BONUS / 4); // 25%

        // Only start reducing fee when health factor < 75% of BP
        let threshold = &bp - &max_bonus; // 10_000 - 25_000 = 7_500

        if health_factor >= &threshold {
            return base_protocol_fee;
        }

        // Calculate how far below threshold the HF is
        let distance_from_threshold = &threshold - health_factor;

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
