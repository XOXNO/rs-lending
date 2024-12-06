multiversx_sc::imports!();

use common_events::MAX_BONUS;
use common_structs::BP;

use crate::{oracle, proxy_price_aggregator::PriceFeed, storage, ERROR_NO_COLLATERAL_TOKEN};

#[multiversx_sc::module]
pub trait LendingMathModule: storage::LendingStorageModule + oracle::OracleModule {
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

    fn compute_amount_in_tokens(
        &self,
        amount_to_return_to_liquidator_in_dollars: &BigUint, // amount to return to the liquidator with bonus
        token_price_data: &PriceFeed<Self::Api>,
    ) -> BigUint {
        self.get_usd_amount_in_tokens_raw(
            amount_to_return_to_liquidator_in_dollars,
            token_price_data,
        )
    }

    fn get_usd_amount_in_tokens_raw(
        &self,
        amount_in_dollars: &BigUint,
        token_data: &PriceFeed<Self::Api>,
    ) -> BigUint {
        amount_in_dollars
            .mul(&BigUint::from(BP))
            .div(&token_data.price)
            .mul(BigUint::from(10u64).pow(token_data.decimals as u32))
            .div(&BigUint::from(BP))
    }

    #[inline]
    fn get_token_amount_in_dollars(
        &self,
        token_id: &EgldOrEsdtTokenIdentifier,
        amount: &BigUint,
    ) -> BigUint {
        let token_data = self.get_token_price_data(token_id);

        amount
            .mul(&BigUint::from(BP))
            .mul(&token_data.price)
            .div(BigUint::from(10u64).pow(token_data.decimals as u32))
            .div(&BigUint::from(BP))
    }

    fn get_token_amount_in_dollars_raw(
        &self,
        amount: &BigUint,
        token_data: &PriceFeed<Self::Api>,
    ) -> BigUint {
        amount
            .mul(&BigUint::from(BP))
            .mul(&token_data.price)
            .div(BigUint::from(10u64).pow(token_data.decimals as u32))
            .div(&BigUint::from(BP))
    }

    fn calculate_max_liquidatable_amount_in_dollars(
        &self,
        health_factor: &BigUint,
        total_debt_in_dollars: &BigUint,
        weighted_collateral_in_dollars: &BigUint,
        liquidation_bonus: &BigUint,
    ) -> BigUint {
        let bp = BigUint::from(BP);
        
        // Target HF = 1.05 for all liquidations
        let target_hf = bp.clone() * 105u32 / 100u32; // 1.05
        
        sc_print!("target_hf:           {}", target_hf);
        sc_print!("current_hf:          {}", health_factor);
        
        // Calculate required repayment to reach target HF
        let adjusted_debt = total_debt_in_dollars * &target_hf / &bp;
        // Since HF < 1.0, total_debt > weighted_collateral, and we multiply by 1.05,
        // adjusted_debt is always > weighted_collateral
        let debt_surplus = adjusted_debt.clone() - weighted_collateral_in_dollars;
        let required_repayment = &debt_surplus * &bp / &(liquidation_bonus + &(&target_hf - &bp));
        
        sc_print!("total_debt:          {}", total_debt_in_dollars);
        sc_print!("weighted_collateral: {}", weighted_collateral_in_dollars);
        sc_print!("adjusted_debt:       {}", adjusted_debt);
        sc_print!("debt_surplus:        {}", debt_surplus);
        sc_print!("required_repayment:  {}", required_repayment);
        
        // If required repayment is more than total debt, it means we can't restore HF > 1.05
        // In this case, allow maximum liquidation (100% of debt)
        if &required_repayment > total_debt_in_dollars {
            sc_print!("reason:              {}", 1); // Can't restore HF, liquidate maximum
            total_debt_in_dollars.clone()
        } else {
            sc_print!("reason:              {}", 2); // Can restore HF with partial liquidation
            required_repayment
        }
    }

    fn calculate_single_asset_liquidation_amount(
        &self,
        health_factor: &BigUint,
        total_debt_in_dollars: &BigUint,
        total_collateral_in_dollars: &BigUint,
        token_to_liquidate: &EgldOrEsdtTokenIdentifier,
        token_price_data: &PriceFeed<Self::Api>,
        liquidatee_account_nonce: u64,
        debt_payment_in_usd: &BigUint,
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
            health_factor,
            total_debt_in_dollars,
            total_collateral_in_dollars,
            liquidation_bonus,
        );

        sc_print!("available_collateral_value: {}", available_collateral_value);
        sc_print!("max_liquidatable_amount:    {}", max_liquidatable_amount);
        sc_print!("debt_payment_in_usd:        {}", debt_payment_in_usd);
        // Take the minimum between what we need and what's available and what the liquidator is paying
        BigUint::min(max_liquidatable_amount, available_collateral_value)
            .min(debt_payment_in_usd.clone())
    }

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
