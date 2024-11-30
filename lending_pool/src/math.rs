multiversx_sc::imports!();

use common_events::{MAX_BONUS, MAX_THRESHOLD};
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
            return BigUint::from(BigUint::from(BP));
        }

        let health_factor = weighted_collateral_in_dollars
            .mul(&BigUint::from(BP))
            .div(borrowed_value_in_dollars);

        health_factor
    }

    fn calculate_liquidation_amount(
        &self,
        health_factor: &BigUint,
        total_debt: &BigUint,
    ) -> BigUint {
        // Only liquidate enough to bring position back to health
        let bp = BigUint::from(BP);
        let target_health_factor = &bp + &(&bp / &BigUint::from(5u32)); // 120%

        let required_debt_reduction = (total_debt * &(&target_health_factor - health_factor)) / &bp;

        BigUint::min(
            required_debt_reduction,
            total_debt * &BigUint::from(MAX_THRESHOLD) / &bp,
        )
    }

    fn compute_amount_in_tokens(
        &self,
        liquidatee_account_nonce: u64,
        token_to_liquidate: &EgldOrEsdtTokenIdentifier, // collateral token of the debt position
        amount_to_return_to_liquidator_in_dollars: BigUint, // amount to return to the liquidator with bonus
        token_price_data: &PriceFeed<Self::Api>,
    ) -> BigUint {
        require!(
            self.deposit_positions(liquidatee_account_nonce)
                .contains_key(token_to_liquidate),
            ERROR_NO_COLLATERAL_TOKEN
        );

        self.get_usd_amount_in_tokens_raw(
            &amount_to_return_to_liquidator_in_dollars,
            &token_price_data,
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

    fn get_usd_amount_in_tokens(
        &self,
        token_id: &EgldOrEsdtTokenIdentifier,
        amount_in_dollars: &BigUint,
    ) -> BigUint {
        // Take the USD price of the token that the liquidator will receive
        let token_data = self.get_token_price_data(token_id);
        // Convert the amount to return to the liquidator with bonus to the token amount
        self.get_usd_amount_in_tokens_raw(amount_in_dollars, &token_data)
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

    fn calculate_single_asset_liquidation_amount(
        &self,
        health_factor: &BigUint,
        total_debt: &BigUint,
        token_to_liquidate: &EgldOrEsdtTokenIdentifier,
        token_price_data: &PriceFeed<Self::Api>,
        liquidatee_account_nonce: u64,
    ) -> BigUint {
        let full_liquidation_amount = self.calculate_liquidation_amount(health_factor, total_debt);

        // Get the available collateral value for this specific asset
        let deposit_position = self
            .deposit_positions(liquidatee_account_nonce)
            .get(token_to_liquidate)
            .unwrap_or_else(|| sc_panic!(ERROR_NO_COLLATERAL_TOKEN));

        let available_collateral_value =
            self.get_token_amount_in_dollars_raw(&deposit_position.amount, token_price_data);

        // Take the minimum between what we need and what's available
        BigUint::min(full_liquidation_amount, available_collateral_value)
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
        let max_bonus = BigUint::from(MAX_BONUS / 5); // 12%

        // Only start reducing fee when health factor < 70% of BP
        let threshold = &bp - &max_bonus; // 10_000 - 15_000 = 8_500

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
