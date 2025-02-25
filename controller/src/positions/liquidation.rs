use common_constants::{RAY_PRECISION, WAD_PRECISION};
use common_events::BPS_PRECISION;
use common_structs::{AccountPosition, PriceFeedShort};

use crate::{contexts::base::StorageCache, helpers, oracle, storage, utils, validation};
use common_errors::ERROR_HEALTH_FACTOR;

use super::{account, borrow, repay, update, withdraw, vault, emode};

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
    + emode::EModeModule
    + vault::PositionVaultModule
{
    /// Executes core liquidation logic for an account.
    /// Manages debt repayment and collateral seizure.
    ///
    /// # Arguments
    /// - `account_nonce`: Position NFT nonce.
    /// - `debt_payments`: Debt repayment payments.
    /// - `caller`: Liquidator's address.
    /// - `storage_cache`: Mutable storage cache.
    ///
    /// # Returns
    /// - Tuple of (seized collateral details, repaid token details).
    fn execute_liquidation(
        &self,
        account_nonce: u64,
        debt_payments: &ManagedVec<EgldOrEsdtTokenPayment<Self::Api>>,
        caller: &ManagedAddress,
        storage_cache: &mut StorageCache<Self>,
    ) -> (
        ManagedVec<MultiValue2<EgldOrEsdtTokenPayment, ManagedDecimal<Self::Api, NumDecimals>>>,
        ManagedVec<
            MultiValue3<
                EgldOrEsdtTokenPayment,
                ManagedDecimal<Self::Api, NumDecimals>,
                PriceFeedShort<Self::Api>,
            >,
        >,
    ) {
        let mut refunds = ManagedVec::new();
        let deposit_positions =
            self.sync_deposit_positions_interest(account_nonce, storage_cache, false);
        let (borrow_positions, map_debt_indexes) =
            self.sync_borrow_positions_interest(account_nonce, storage_cache, false, true);

        let (debt_payment_in_egld, mut repaid_tokens) = self.calculate_repayment_amounts(
            debt_payments,
            &borrow_positions,
            &mut refunds,
            map_debt_indexes,
            storage_cache,
        );

        let (liquidation_collateral, total_collateral, _) =
            self.calculate_collateral_values(&deposit_positions, storage_cache);
        let (proportional_weighted, bonus_weighted) = self.calculate_seizure_proportions(
            &total_collateral,
            &deposit_positions,
            storage_cache,
        );
        let borrowed_egld = self.calculate_total_borrow_in_egld(&borrow_positions, storage_cache);
        let health_factor =
            self.validate_liquidation_health_factor(&liquidation_collateral, &borrowed_egld);

        let (max_debt_to_repay, bonus_rate) = self.calculate_liquidation_amounts(
            &borrowed_egld,
            &total_collateral,
            liquidation_collateral,
            &proportional_weighted,
            &bonus_weighted,
            &health_factor,
            OptionalValue::Some(debt_payment_in_egld.clone()),
        );

        if debt_payment_in_egld > max_debt_to_repay {
            self.process_excess_payment(
                &mut repaid_tokens,
                &mut refunds,
                debt_payment_in_egld - max_debt_to_repay.clone(),
                storage_cache,
            );
        }

        if !refunds.is_empty() {
            self.tx()
                .to(caller)
                .payment(refunds)
                .transfer_if_not_empty();
        }

        let seized_collaterals = self.calculate_seized_collateral(
            &deposit_positions,
            &total_collateral,
            &max_debt_to_repay,
            &bonus_rate,
            storage_cache,
        );

        (seized_collaterals, repaid_tokens)
    }

    /// Orchestrates the full liquidation process.
    /// Coordinates repayments and collateral seizures.
    ///
    /// # Arguments
    /// - `account_nonce`: Position NFT nonce.
    /// - `debt_payments`: Debt repayment payments.
    /// - `caller`: Liquidator's address.
    fn process_liquidation(
        &self,
        account_nonce: u64,
        debt_payments: &ManagedVec<EgldOrEsdtTokenPayment<Self::Api>>,
        caller: &ManagedAddress,
    ) {
        let mut storage_cache = StorageCache::new(self);
        storage_cache.allow_unsafe_price = false;
        let account = self.account_attributes(account_nonce).get();

        let (seized_collaterals, repaid_tokens) =
            self.execute_liquidation(account_nonce, debt_payments, caller, &mut storage_cache);

        for debt_payment_data in repaid_tokens {
            let (debt_payment, debt_egld_value, debt_price_feed) = debt_payment_data.into_tuple();
            self.process_repayment(
                account_nonce,
                &debt_payment.token_identifier,
                &ManagedDecimal::from_raw_units(
                    debt_payment.amount,
                    debt_price_feed.asset_decimals,
                ),
                caller,
                debt_egld_value,
                &debt_price_feed,
                &mut storage_cache,
                &account,
            );
        }

        for collateral_data in seized_collaterals {
            let (seized_collateral, protocol_fee) = collateral_data.into_tuple();
            self.process_withdrawal(
                account_nonce,
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

    /// Validates if the health factor permits liquidation.
    /// Ensures the position is unhealthy enough to liquidate.
    ///
    /// # Arguments
    /// - `collateral_in_egld`: Collateral value in EGLD.
    /// - `borrowed_egld`: Borrowed value in EGLD.
    ///
    /// # Returns
    /// - Current health factor.
    fn validate_liquidation_health_factor(
        &self,
        collateral_in_egld: &ManagedDecimal<Self::Api, NumDecimals>,
        borrowed_egld: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let health_factor = self.compute_health_factor(collateral_in_egld, borrowed_egld);
        require!(health_factor < self.wad(), ERROR_HEALTH_FACTOR);
        health_factor
    }

    /// Calculates collateral to seize based on debt repayment.
    /// Applies proportional seizure with bonus.
    ///
    /// # Arguments
    /// - `deposit_positions`: Borrower's deposit positions.
    /// - `total_collateral_value`: Total collateral in EGLD.
    /// - `debt_to_be_repaid`: Debt amount to repay.
    /// - `bonus_rate`: Liquidation bonus in BPS.
    /// - `storage_cache`: Mutable storage cache.
    ///
    /// # Returns
    /// - Vector of (seized payment, protocol fee) tuples.
    fn calculate_seized_collateral(
        &self,
        deposit_positions: &ManagedVec<AccountPosition<Self::Api>>,
        total_collateral_value: &ManagedDecimal<Self::Api, NumDecimals>,
        debt_to_be_repaid: &ManagedDecimal<Self::Api, NumDecimals>,
        bonus_rate: &ManagedDecimal<Self::Api, NumDecimals>,
        storage_cache: &mut StorageCache<Self>,
    ) -> ManagedVec<MultiValue2<EgldOrEsdtTokenPayment, ManagedDecimal<Self::Api, NumDecimals>>>
    {
        let mut seized_amounts_by_collateral = ManagedVec::new();

        for asset in deposit_positions {
            let total_amount = asset.get_total_amount();
            let asset_data = self.get_token_price(&asset.asset_id, storage_cache);
            let asset_egld_value = self.get_token_egld_value(&total_amount, &asset_data.price);

            // Proportion = (asset_egld_value * WAD) / total_collateral_value
            let proportion = self.div_half_up(
                &(asset_egld_value * storage_cache.wad_dec.clone()),
                total_collateral_value,
                WAD_PRECISION,
            );

            // Seized EGLD = proportion * debt_to_be_repaid (rescaled from RAY to WAD precision)
            let seized_egld_numerator_ray =
                self.mul_half_up(&proportion, debt_to_be_repaid, RAY_PRECISION);
            let seized_egld = seized_egld_numerator_ray.rescale(WAD_PRECISION);

            // Convert seized EGLD to token units
            let seized_units = self.convert_egld_to_tokens(&seized_egld, &asset_data);

            // Apply bonus: seized_units_after_bonus = seized_units * (bps + bonus_rate) / bps
            let bonus_bps = storage_cache.bps_dec.clone() + bonus_rate.clone();
            let numerator = self.mul_half_up(&seized_units, &bonus_bps, RAY_PRECISION);
            let seized_units_after_bonus = numerator.rescale(asset_data.asset_decimals as usize);

            // Protocol fee = (bonus portion) * liquidation_fees / bps
            let protocol_fee = (seized_units_after_bonus.clone() - seized_units.clone())
                * asset.liquidation_fees.clone()
                / storage_cache.bps_dec.clone();

            let final_amount = BigUint::min(
                seized_units_after_bonus.into_raw_units().clone(),
                total_amount.into_raw_units().clone(),
            );

            seized_amounts_by_collateral.push(MultiValue2::from((
                EgldOrEsdtTokenPayment::new(asset.asset_id.clone(), 0, final_amount),
                protocol_fee,
            )));
        }

        seized_amounts_by_collateral
    }

    /// Computes total repaid debt and token details.
    /// Handles excess payments with refunds.
    ///
    /// # Arguments
    /// - `repayments`: Debt repayment payments.
    /// - `borrows`: Borrow positions.
    /// - `refunds`: Mutable refund vector.
    /// - `borrows_index_map`: Token-to-index mapping.
    /// - `storage_cache`: Mutable storage cache.
    ///
    /// # Returns
    /// - Tuple of (total repaid in EGLD, repaid token details).
    fn calculate_repayment_amounts(
        &self,
        repayments: &ManagedVec<EgldOrEsdtTokenPayment<Self::Api>>,
        borrows: &ManagedVec<AccountPosition<Self::Api>>,
        refunds: &mut ManagedVec<EgldOrEsdtTokenPayment<Self::Api>>,
        borrows_index_map: ManagedMapEncoded<Self::Api, EgldOrEsdtTokenIdentifier, usize>,
        storage_cache: &mut StorageCache<Self>,
    ) -> (
        ManagedDecimal<Self::Api, NumDecimals>,
        ManagedVec<
            MultiValue3<
                EgldOrEsdtTokenPayment,
                ManagedDecimal<Self::Api, NumDecimals>,
                PriceFeedShort<Self::Api>,
            >,
        >,
    ) {
        let mut total_repaid = self.to_decimal_wad(BigUint::zero());
        let mut repaid_tokens = ManagedVec::new();
        for payment_ref in repayments {
            let token_feed = self.get_token_price(&payment_ref.token_identifier, storage_cache);
            let original_borrow = self.get_position_by_index(
                &payment_ref.token_identifier,
                borrows,
                &borrows_index_map,
            );
            let amount_dec = ManagedDecimal::from_raw_units(
                payment_ref.amount.clone(),
                token_feed.asset_decimals as usize,
            );

            let token_egld_amount = self.get_token_egld_value(&amount_dec, &token_feed.price);

            let borrowed_egld_amount =
                self.get_token_egld_value(&original_borrow.get_total_amount(), &token_feed.price);
            let mut payment = payment_ref.clone();
            if token_egld_amount > borrowed_egld_amount {
                let egld_excess = token_egld_amount - borrowed_egld_amount.clone();
                let original_excess_paid = self.convert_egld_to_tokens(&egld_excess, &token_feed);
                let token_excess_amount = original_excess_paid.into_raw_units().clone();

                payment.amount -= &token_excess_amount;

                refunds.push(EgldOrEsdtTokenPayment::new(
                    payment_ref.token_identifier.clone(),
                    payment_ref.token_nonce.clone(),
                    token_excess_amount,
                ));

                total_repaid += &borrowed_egld_amount;
                repaid_tokens.push((payment, borrowed_egld_amount, token_feed).into());
            } else {
                total_repaid += &token_egld_amount;
                repaid_tokens.push((payment, token_egld_amount, token_feed).into());
            }
        }

        (total_repaid, repaid_tokens)
    }

    /// Calculates proportional and bonus-weighted seizure amounts.
    /// Determines seizure proportions for liquidation.
    ///
    /// # Arguments
    /// - `total_collateral_in_egld`: Total collateral in EGLD.
    /// - `positions`: Deposit positions.
    /// - `storage_cache`: Mutable storage cache.
    ///
    /// # Returns
    /// - Tuple of (proportional, bonus-weighted) values.
    fn calculate_seizure_proportions(
        &self,
        total_collateral_in_egld: &ManagedDecimal<Self::Api, NumDecimals>,
        positions: &ManagedVec<AccountPosition<Self::Api>>,
        storage_cache: &mut StorageCache<Self>,
    ) -> (
        ManagedDecimal<Self::Api, NumDecimals>,
        ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        let mut proportion_seized = self.to_decimal_bps(BigUint::zero());
        let mut weighted_bonus = self.to_decimal_bps(BigUint::zero());

        for dp in positions {
            let feed = self.get_token_price(&dp.asset_id, storage_cache);
            let collateral_in_egld = self.get_token_egld_value(&dp.get_total_amount(), &feed.price);
            let fraction = self
                .div_half_up(&collateral_in_egld, total_collateral_in_egld, RAY_PRECISION)
                .rescale(BPS_PRECISION);
            proportion_seized += self
                .mul_half_up(&fraction, &dp.liquidation_threshold, RAY_PRECISION)
                .rescale(BPS_PRECISION);
            weighted_bonus += self
                .mul_half_up(&fraction, &dp.liquidation_bonus, RAY_PRECISION)
                .rescale(BPS_PRECISION);
        }

        (proportion_seized, weighted_bonus)
    }

    /// Calculates maximum debt to repay and bonus rate.
    /// Uses a Dutch auction mechanism for liquidation amounts.
    ///
    /// # Arguments
    /// - `total_debt_in_egld`: Total debt in EGLD.
    /// - `total_collateral_in_egld`: Total collateral in EGLD.
    /// - `weighted_collateral_in_egld`: Weighted collateral in EGLD.
    /// - `proportion_seized`: Seizure proportion.
    /// - `base_liquidation_bonus`: Base bonus.
    /// - `health_factor`: Current health factor.
    /// - `debt_payment`: Optional debt payment in EGLD.
    ///
    /// # Returns
    /// - Tuple of (max debt to repay, bonus rate).
    fn calculate_liquidation_amounts(
        &self,
        total_debt_in_egld: &ManagedDecimal<Self::Api, NumDecimals>,
        total_collateral_in_egld: &ManagedDecimal<Self::Api, NumDecimals>,
        weighted_collateral_in_egld: ManagedDecimal<Self::Api, NumDecimals>,
        proportion_seized: &ManagedDecimal<Self::Api, NumDecimals>,
        base_liquidation_bonus: &ManagedDecimal<Self::Api, NumDecimals>,
        health_factor: &ManagedDecimal<Self::Api, NumDecimals>,
        debt_payment: OptionalValue<ManagedDecimal<Self::Api, NumDecimals>>,
    ) -> (
        ManagedDecimal<Self::Api, NumDecimals>,
        ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        let (max_repayable_debt, bonus) = self.estimate_liquidation_amount(
            weighted_collateral_in_egld.into_raw_units(),
            proportion_seized.into_raw_units(),
            total_collateral_in_egld.into_raw_units(),
            total_debt_in_egld.into_raw_units(),
            base_liquidation_bonus.into_raw_units(),
            health_factor.into_raw_units(),
        );

        if debt_payment.is_some() {
            (
                self.to_decimal_wad(BigUint::min(
                    debt_payment.into_option().unwrap().into_raw_units().clone(),
                    max_repayable_debt,
                )),
                self.to_decimal_bps(bonus),
            )
        } else {
            (
                self.to_decimal_wad(max_repayable_debt),
                self.to_decimal_bps(bonus),
            )
        }
    }

    /// Adjusts repayments and refunds for excess payments.
    /// Ensures accurate liquidation accounting.
    ///
    /// # Arguments
    /// - `repaid_tokens`: Mutable repaid token details.
    /// - `refunds`: Mutable refund vector.
    /// - `excess_in_egld`: Excess payment in EGLD.
    /// - `storage_cache`: Mutable storage cache.
    fn process_excess_payment(
        &self,
        repaid_tokens: &mut ManagedVec<
            MultiValue3<
                EgldOrEsdtTokenPayment,
                ManagedDecimal<Self::Api, NumDecimals>,
                PriceFeedShort<Self::Api>,
            >,
        >,
        refunds: &mut ManagedVec<EgldOrEsdtTokenPayment<Self::Api>>,
        excess_in_egld: ManagedDecimal<Self::Api, NumDecimals>,
        storage_cache: &mut StorageCache<Self>,
    ) {
        let mut remaining_excess = excess_in_egld;

        for index in 0..repaid_tokens.len() {
            if remaining_excess == storage_cache.wad_dec_zero {
                break;
            }

            let (mut debt_payment, mut egld_asset_amount, price_feed) =
                repaid_tokens.get(index).clone().into_tuple();

            if egld_asset_amount >= remaining_excess {
                let excess_in_original =
                    self.convert_egld_to_tokens(&remaining_excess, &price_feed);
                debt_payment.amount -= excess_in_original.into_raw_units();
                egld_asset_amount -= &remaining_excess;

                refunds.push(EgldOrEsdtTokenPayment::new(
                    debt_payment.token_identifier.clone(),
                    0,
                    excess_in_original.into_raw_units().clone(),
                ));
                let _ =
                    repaid_tokens.set(index, (debt_payment, egld_asset_amount, price_feed).into());

                remaining_excess = storage_cache.wad_dec_zero.clone();
            } else {
                refunds.push(debt_payment);
                let _ = repaid_tokens.remove(index);
                remaining_excess -= egld_asset_amount;
            }
        }
    }

    /// Retrieves a borrow position by token index.
    /// Uses an index map for efficient lookup.
    ///
    /// # Arguments
    /// - `key_token`: Token identifier.
    /// - `borrows`: Borrow positions vector.
    /// - `borrows_index_map`: Token-to-index mapping.
    ///
    /// # Returns
    /// - Borrow position.
    fn get_position_by_index(
        &self,
        key_token: &EgldOrEsdtTokenIdentifier,
        borrows: &ManagedVec<AccountPosition<Self::Api>>,
        borrows_index_map: &ManagedMapEncoded<Self::Api, EgldOrEsdtTokenIdentifier, usize>,
    ) -> AccountPosition<Self::Api> {
        require!(
            borrows_index_map.contains(key_token),
            "Token {} is not part of the mapper",
            key_token
        );
        let safe_index = borrows_index_map.get(key_token);
        // -1 is required to by pass the issue of index = 0 which will throw at the above .contains
        let index = safe_index - 1;
        let position = borrows.get(index).clone();

        position
    }
}
