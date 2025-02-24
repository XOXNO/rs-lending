use common_constants::{RAY_PRECISION, WAD_PRECISION};
use common_structs::{AccountPosition, PriceFeedShort};

use crate::{contexts::base::StorageCache, helpers, oracle, storage, utils, validation};
use common_errors::ERROR_HEALTH_FACTOR;

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
    + common_math::SharedMathModule
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
        ManagedVec<MultiValue2<EgldOrEsdtTokenPayment, ManagedDecimal<Self::Api, NumDecimals>>>, // Collateral seized
        ManagedVec<
            MultiValue3<
                EgldOrEsdtTokenPayment,
                ManagedDecimal<Self::Api, NumDecimals>,
                PriceFeedShort<Self::Api>,
            >,
        >, // Repaid tokens, egld value, price feed of each
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
            liquidation_collateral.clone(),
            &proportional_weighted,
            &bonus_weighted,
            &health_factor,
            OptionalValue::Some(debt_payment_in_egld.clone()),
        );
        sc_print!("debt_payment_in_egld     {}", debt_payment_in_egld);
        sc_print!("max_debt_to_repay        {}", max_debt_to_repay);
        // sc_print!("borrowed_egld            {}", borrowed_egld);
        // sc_print!("liquidation_collateral   {}", liquidation_collateral);
        // sc_print!("bonus_rate               {}", bonus_rate);
        // Calculate the excess amount paid over the required debt repayment.
        if debt_payment_in_egld > max_debt_to_repay {
            let mut excess_in_egld = debt_payment_in_egld - max_debt_to_repay.clone();

            for index in 0..repaid_tokens.len() {
                if excess_in_egld == storage_cache.wad_dec_zero {
                    break;
                }
                // Retrieve the token repayment details.
                let (mut debt_payment, mut egld_asset_amount, price_feed) =
                    repaid_tokens.get(index).clone().into_tuple();
                // sc_print!("egld_asset_amount {}", egld_asset_amount);
                // Check if this token can cover the remaining excess.
                if egld_asset_amount >= excess_in_egld {
                    // Convert the excess EGLD amount to the token's native units.
                    let excess_in_original =
                        self.compute_egld_in_tokens(&excess_in_egld, &price_feed);
                    // Adjust the repayment amount and asset value.
                    debt_payment.amount -= excess_in_original.into_raw_units();
                    // sc_print!("debt_payment.amount {}", debt_payment.amount);

                    egld_asset_amount -= &excess_in_egld;
                    // sc_print!("egld_asset_amount {}", egld_asset_amount);

                    // Record the refund for this token.
                    refunds.push(EgldOrEsdtTokenPayment::new(
                        debt_payment.token_identifier.clone(),
                        0,
                        excess_in_original.into_raw_units().clone(),
                    ));
                    let _ = repaid_tokens
                        .set(index, (debt_payment, egld_asset_amount, price_feed).into());

                    excess_in_egld = storage_cache.wad_dec_zero.clone();
                } else {
                    // This flow is when the excess amount is more than the entire amount of this token, then refund the entire token sent
                    // it can happen only when there is a bulk repayment of different debts in the same position
                    refunds.push(debt_payment);
                    let _ = repaid_tokens.remove(index);
                    excess_in_egld -= egld_asset_amount;
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
                &ManagedDecimal::from_raw_units(
                    debt_payment.amount,
                    debt_price_feeed.decimals as usize,
                ),
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
                Some(protocol_fee),
                &mut storage_cache,
                &account,
                false,
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
        collateral_in_egld: &ManagedDecimal<Self::Api, NumDecimals>,
        borrowed_egld: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let health_factor = self.compute_health_factor(collateral_in_egld, borrowed_egld);

        require!(health_factor < self.wad(), ERROR_HEALTH_FACTOR);

        health_factor
    }

    /// Seizes collateral proportionally from a borrower's collateral basket during liquidation.
    ///
    /// This function calculates, for each collateral asset supplied by a borrower, the amount
    /// (in token units) to be seized in order to cover the debt being repaid. The seizure is performed
    /// proportionally based on the asset’s contribution to the total collateral value (expressed in EGLD).
    /// A liquidation bonus is applied to incentivize liquidators, and a protocol fee is computed based on
    /// the bonus portion. The function returns a vector of payment instructions (with associated protocol fees)
    /// for each collateral asset.
    ///
    /// # Arguments
    /// * `collaterals` - A vector of the borrower’s collateral positions.
    /// * `total_collateral_value` - The total value of all collateral (in EGLD, expressed in WAD precision).
    /// * `debt_to_be_repaid` - The total debt amount (in EGLD, in WAD precision) that is to be repaid during liquidation.
    /// * `bonus` - The liquidation bonus expressed in basis points (BPS).
    /// * `storage_cache` - A mutable reference to the storage cache for retrieving token prices and other data.
    ///
    /// # Returns
    /// A ManagedVec containing, for each collateral asset, a tuple with:
    /// - An `EgldOrEsdtTokenPayment` specifying the token ID and final seized amount (in token units).
    /// - The protocol fee (in token units) applied on the bonus portion.
    fn seize_collateral_proportionally(
        &self,
        collaterals: &ManagedVec<AccountPosition<Self::Api>>,
        total_collateral_value: &ManagedDecimal<Self::Api, NumDecimals>,
        debt_to_be_repaid: &ManagedDecimal<Self::Api, NumDecimals>,
        bonus: &ManagedDecimal<Self::Api, NumDecimals>,
        storage_cache: &mut StorageCache<Self>,
    ) -> ManagedVec<MultiValue2<EgldOrEsdtTokenPayment, ManagedDecimal<Self::Api, NumDecimals>>>
    {
        // This vector will accumulate the result for each collateral asset.
        let mut seized_amounts_by_collateral = ManagedVec::new();

        // Constants for fixed-point arithmetic:

        // Loop over each collateral asset in the borrower's position.
        for asset in collaterals {
            // Retrieve the total amount of this asset supplied by the borrower.
            let total_amount = asset.get_total_amount();
            // sc_print!("total_amount {}", total_amount);

            // Get the asset's pricing data from the storage cache.
            let asset_data = self.get_token_price(&asset.token_id, storage_cache);

            // Convert the total amount of this asset into its equivalent EGLD value.
            let asset_egld_value =
                self.get_token_amount_in_egld_raw(&total_amount, &asset_data.price.clone());
            // sc_print!("asset_egld_value {}", asset_egld_value);
            // Compute the asset's proportion of the total collateral value.
            // This is calculated as: (asset_egld_value / total_collateral_value) in WAD precision.
            let proportion = self.div_half_up(
                &(asset_egld_value * storage_cache.wad_dec.clone()),
                total_collateral_value,
                WAD_PRECISION,
            );
            // sc_print!("proportion {}", proportion);
            // Determine the amount of EGLD value to seize from this asset.
            // The seizure amount in EGLD is the asset's proportion times the total debt to be repaid.
            let seized_egld_numerator_ray =
                self.mul_half_up(&proportion, debt_to_be_repaid, RAY_PRECISION);
            let seized_egld = seized_egld_numerator_ray.rescale(WAD_PRECISION);
            sc_print!("seized_egld {}", seized_egld);
            // Convert the seized EGLD value back into token units for this asset.
            let seized_units = self.compute_egld_in_tokens(&seized_egld, &asset_data);
            sc_print!("seized_units {}", seized_units);

            // Apply the liquidation bonus.
            // The bonus is given in basis points, so the seized amount is increased by (bps + bonus)/bps.
            let bonus_bps = storage_cache.bps_dec.clone() + bonus.clone();
            let numerator = self.mul_half_up(&seized_units, &bonus_bps, RAY_PRECISION);
            let seized_units_after_bonus = numerator.rescale(asset_data.decimals as usize); // seized_units.clone() * (storage_cache.bps_dec.clone() + bonus.clone()) / storage_cache.bps_dec.clone();
            sc_print!("seized_units_after_bonus {}", seized_units_after_bonus);
            // Calculate the protocol fee, which is charged on the bonus portion only.
            // The fee is computed on the difference between the bonus-adjusted units and the original seized units.
            let protocol_fee = (seized_units_after_bonus.clone() - seized_units.clone())
                * asset.entry_liquidation_fees.clone()
                / storage_cache.bps_dec.clone();

            sc_print!("protocol_fee {}", protocol_fee);
            // Ensure that the final seized amount does not exceed the total available collateral for this asset.
            let final_amount = BigUint::min(
                seized_units_after_bonus.into_raw_units().clone(),
                total_amount.into_raw_units().clone(),
            );
            sc_print!("final_amount {}", final_amount);

            // Record the result for this asset:
            // - The payment instruction specifies the token and the final seized amount.
            // - The protocol fee associated with the seizure is also included.
            seized_amounts_by_collateral.push(MultiValue2::from((
                EgldOrEsdtTokenPayment::new(asset.token_id.clone(), 0, final_amount),
                protocol_fee,
            )));
        }

        // Return the vector of seized collateral amounts along with their protocol fees.
        seized_amounts_by_collateral
    }
}
