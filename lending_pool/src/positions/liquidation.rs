use common_constants::BP;
use common_events::PriceFeedShort;

use crate::{
    contexts::base::StorageCache, helpers, oracle, storage, utils, validation, ERROR_HEALTH_FACTOR,
};

use super::{account, borrow, repay, update, withdraw};

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

#[multiversx_sc::module]
pub trait PositionLiquidationModule:
    storage::LendingStorageModule
    + validation::ValidationModule
    + oracle::OracleModule
    + common_events::EventsModule
    + utils::LendingUtilsModule
    + helpers::math::MathsModule
    + account::PositionAccountModule
    + repay::PositionRepayModule
    + withdraw::PositionWithdrawModule
    + update::PositionUpdateModule
    + borrow::PositionBorrowModule
{
    /// Handles core liquidation logic
    ///
    /// # Arguments
    /// * `liquidatee_account_nonce` - NFT nonce of account being liquidated
    /// * `debt_payment` - Payment to cover debt
    /// * `collateral_to_receive` - Collateral token to receive
    /// * `caller` - Address initiating liquidation
    /// * `asset_config_collateral` - Configuration of collateral asset
    ///
    /// Calculates liquidation amounts, handles excess payments,
    /// and determines collateral to receive including bonus.
    fn handle_liquidation(
        &self,
        account_nonce: u64,
        payments: &ManagedVec<EgldOrEsdtTokenPayment<Self::Api>>,
        caller: &ManagedAddress,
        storage_cache: &mut StorageCache<Self>,
    ) -> (
        ManagedVec<MultiValue2<EgldOrEsdtTokenPayment, BigUint>>, // Collateral seized
        ManagedVec<MultiValue3<EgldOrEsdtTokenPayment, BigUint, PriceFeedShort<Self::Api>>>, // Repaid tokens, egld value, price feed of each
    ) {
        let mut refunds = ManagedVec::new();
        let collaterals = self.update_interest(account_nonce, storage_cache, false);
        let (borrows, map_debt_indexes) =
            self.update_debt(account_nonce, storage_cache, false, true);

        let (debt_payment_in_egld, mut repaid_tokens) = self.sum_repayments(
            payments,
            &borrows,
            &mut refunds,
            map_debt_indexes,
            storage_cache,
        );

        let (liquidation_collateral, total_collateral, _) =
            self.sum_collaterals(&collaterals, storage_cache);

        let (proportional_weighted, bonus_weighted) =
            self.proportion_of_weighted_seized(&total_collateral, &collaterals, storage_cache);

        let borrowed_egld = self.sum_borrows(&borrows, storage_cache);

        let health_factor = self.validate_can_liquidate(&liquidation_collateral, &borrowed_egld);

        // Calculate liquidation amount using Dutch auction mechanism
        let (max_debt_to_repay, bonus_rate) = self.calculate_max_debt_repayment(
            &borrowed_egld,
            &total_collateral,
            liquidation_collateral,
            &proportional_weighted,
            &bonus_weighted,
            &health_factor,
            OptionalValue::Some(debt_payment_in_egld.clone()),
        );

        // Handle excess debt payment if any
        // User plateste in total cu 2 tokens 100 EGLD,
        // Option 1: totalul de 100 EGLD este mai mare decat suma necesara sa faca pozitia healthy again = over paid
        if debt_payment_in_egld > max_debt_to_repay {
            // Excess este total platit - max required in EGLD to recover debt = 150 - 10 EGLD = over paid 140 EGLD
            let mut excess_in_egld = &debt_payment_in_egld - &max_debt_to_repay;
            // Token 1 = EGLD de 100 amount
            // Token 2 = USDC de 50 EGLD echivalent = 1000 USDC
            // debt_payment_in_egld = 150 EGLD
            for index in 0..repaid_tokens.len() {
                if excess_in_egld == BigUint::zero() {
                    break;
                }
                // Token 1 repaid = EGLD = 100 paid
                // Token 2 repaud = 50 EDLD
                let (mut debt_payment, mut egld_asset_amount, price_feed) =
                    repaid_tokens.get(index).clone().into_tuple();

                // Daca 100 >= 140 (Token 1)
                // Daca 50 >= 40 (Token 2)
                if egld_asset_amount >= excess_in_egld {
                    // This flow is when the amount repaid is higher than the maximum possible
                    // We calculate how much we repaid in EGLD, then we convert to the original token
                    // We deduct from the repayment vec the amount and push to the refunds vec what needs to be sent back.
                    // Get the USDC echivalent of the excess EGLD
                    let excess_in_original =
                        self.compute_amount_in_tokens(&excess_in_egld, &price_feed);

                    debt_payment.amount -= &excess_in_original; // Reseteaza plata de 100 de EGLD sa ramana la 100 - 90 = 10 EGLD care teoretic face match cu max_debt to repay
                    egld_asset_amount -= &excess_in_egld; // Ramana ca tokenul valoreaza in EGLD exact 50 - 40 = 10 EGLD
                    refunds.push(EgldOrEsdtTokenPayment::new(
                        debt_payment.token_identifier.clone(),
                        0,
                        excess_in_original,
                    ));
                    let _ = repaid_tokens
                        .set(index, (debt_payment, egld_asset_amount, price_feed).into());

                    excess_in_egld = BigUint::zero();
                } else {
                    // This flow is when the excess amount is more than the entire amount of this token, then refund the entire token sent
                    // it can happen only when there is a bulk repayment of different debts in the same position
                    refunds.push(debt_payment);
                    let _ = repaid_tokens.remove(index);
                    excess_in_egld -= egld_asset_amount; // Ramane la 140 - 100 = 40;
                }
            }
        };

        // Return excess if any
        if !refunds.is_empty() {
            self.tx()
                .to(caller)
                .payment(refunds)
                .transfer_if_not_empty();
        }

        let seized_collaterals = self.seize_collateral_proportionally(
            &collaterals,
            &total_collateral,
            &max_debt_to_repay,
            &bonus_rate,
            storage_cache,
        );

        (seized_collaterals, repaid_tokens)
    }

    /// Processes complete liquidation operation
    ///
    /// # Arguments
    /// * `liquidatee_account_nonce` - NFT nonce of account being liquidated
    /// * `debt_payment` - Payment to cover debt
    /// * `collateral_to_receive` - Collateral token to receive
    /// * `caller` - Address initiating liquidation
    ///
    /// Orchestrates the entire liquidation flow:
    /// 1. Calculates liquidation amounts
    /// 2. Repays debt
    /// 3. Calculates and applies protocol fee
    /// 4. Transfers collateral to liquidator
    fn process_liquidation(
        &self,
        liquidatee_account_nonce: u64,
        debt_payments: &ManagedVec<EgldOrEsdtTokenPayment<Self::Api>>,
        caller: &ManagedAddress,
    ) {
        let mut storage_cache = StorageCache::new(self);
        storage_cache.allow_unsafe_price = false;

        let account = self.account_attributes(liquidatee_account_nonce).get();

        let (seized_collaterals, repaid_tokens) = self.handle_liquidation(
            liquidatee_account_nonce,
            debt_payments,
            caller,
            &mut storage_cache,
        );

        for debt_payment_data in repaid_tokens {
            let (debt_payment, debt_egld_value, debt_price_feeed) = debt_payment_data.into_tuple();
            // // Repay debt
            self.internal_repay(
                liquidatee_account_nonce,
                &debt_payment.token_identifier,
                &debt_payment.amount,
                caller,
                debt_egld_value,
                &debt_price_feeed,
                &mut storage_cache,
                &account,
            );
        }

        for collateral_data in seized_collaterals {
            let (seized_collateral, protocol_fee) = collateral_data.into_tuple();
            // // Process withdrawal with protocol fee
            self.internal_withdraw(
                liquidatee_account_nonce,
                seized_collateral,
                caller,
                true,
                &protocol_fee,
                &mut storage_cache,
                &account,
            );
        }
    }

    /// Validates liquidation health factor
    ///
    /// # Arguments
    /// * `collateral_in_egld` - EGLD value of collateral
    /// * `borrowed_egld` - EGLD value of borrows
    ///
    /// # Returns
    /// * `BigUint` - Current health factor
    ///
    /// Calculates health factor and ensures it's below liquidation threshold
    fn validate_can_liquidate(
        &self,
        collateral_in_egld: &BigUint,
        borrowed_egld: &BigUint,
    ) -> BigUint {
        let health_factor = self.compute_health_factor(collateral_in_egld, borrowed_egld);
        require!(health_factor < BigUint::from(BP), ERROR_HEALTH_FACTOR);
        health_factor
    }
}
