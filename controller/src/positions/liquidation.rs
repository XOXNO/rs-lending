use common_constants::{BPS_PRECISION, RAY_PRECISION, WAD_PRECISION};
use common_structs::{AccountAttributes, AccountPosition, PriceFeedShort};

use crate::{cache::Cache, helpers, oracle, proxy_pool, storage, utils, validation};
use common_errors::ERROR_HEALTH_FACTOR;

use super::{account, borrow, emode, repay, update, vault, withdraw};

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

#[multiversx_sc::module]
pub trait PositionLiquidationModule:
    storage::Storage
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
    /// - `cache`: Mutable storage cache.
    ///
    /// # Returns
    /// - Tuple of (seized collateral details, repaid token details).
    fn execute_liquidation(
        &self,
        account_nonce: u64,
        debt_payments: &ManagedVec<EgldOrEsdtTokenPayment<Self::Api>>,
        caller: &ManagedAddress,
        cache: &mut Cache<Self>,
        account_attributes: &AccountAttributes<Self::Api>,
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
            self.sync_deposit_positions_interest(account_nonce, cache, false, &account_attributes);
        let (borrow_positions, map_debt_indexes) =
            self.sync_borrow_positions_interest(account_nonce, cache, false, true);

        let (debt_payment_in_egld, mut repaid_tokens) = self.calculate_repayment_amounts(
            debt_payments,
            &borrow_positions,
            &mut refunds,
            map_debt_indexes,
            cache,
        );

        let (liquidation_collateral, total_collateral, _) =
            self.calculate_collateral_values(&deposit_positions, cache);
        let (proportional_weighted, bonus_weighted) =
            self.calculate_seizure_proportions(&total_collateral, &deposit_positions, cache);
        let borrowed_egld = self.calculate_total_borrow_in_egld(&borrow_positions, cache);

        let health_factor =
            self.validate_liquidation_health_factor(&liquidation_collateral, &borrowed_egld);

        let (max_debt_to_repay, max_collateral_seized, bonus_rate) = self
            .calculate_liquidation_amounts(
                &borrowed_egld,
                &total_collateral,
                &liquidation_collateral,
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
            cache,
        );

        self.check_bad_debt_after_liquidation(
            cache,
            account_nonce,
            &borrowed_egld,
            &max_debt_to_repay,
            &total_collateral,
            &max_collateral_seized,
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
        let mut cache = Cache::new(self);
        cache.allow_unsafe_price = false;
        self.validate_liquidation_payments(debt_payments, caller);

        self.require_active_account(account_nonce);

        let account_attributes = self.account_attributes(account_nonce).get();

        let (seized_collaterals, repaid_tokens) = self.execute_liquidation(
            account_nonce,
            debt_payments,
            caller,
            &mut cache,
            &account_attributes,
        );

        for debt_payment_data in repaid_tokens {
            let (debt_payment, debt_egld_value, debt_price_feed) = debt_payment_data.into_tuple();
            self.process_repayment(
                account_nonce,
                &debt_payment.token_identifier,
                &self.to_decimal(debt_payment.amount, debt_price_feed.asset_decimals),
                caller,
                debt_egld_value,
                &debt_price_feed,
                &mut cache,
                &account_attributes,
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
                &mut cache,
                &account_attributes,
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

    /// Validates payments for liquidation operations.
    /// Ensures debt repayments are valid and the caller is authorized.
    ///
    /// # Arguments
    /// - `debt_repayments`: Vector of debt repayment payments.
    /// - `initial_caller`: Address initiating the liquidation.
    ///
    /// # Errors
    /// - Inherits errors from `validate_payment`.
    /// - `ERROR_ADDRESS_IS_ZERO`: If the caller address is zero.
    fn validate_liquidation_payments(
        &self,
        debt_repayments: &ManagedVec<EgldOrEsdtTokenPayment<Self::Api>>,
        initial_caller: &ManagedAddress,
    ) {
        for debt_payment in debt_repayments {
            self.validate_payment(&debt_payment);
        }
        self.require_non_zero_address(initial_caller);
    }

    /// Calculates collateral to seize based on debt repayment.
    /// Applies proportional seizure with bonus.
    ///
    /// # Arguments
    /// - `deposit_positions`: Borrower's deposit positions.
    /// - `total_collateral_value`: Total collateral in EGLD.
    /// - `debt_to_be_repaid`: Debt amount to repay.
    /// - `bonus_rate`: Liquidation bonus in BPS.
    /// - `cache`: Mutable storage cache.
    ///
    /// # Returns
    /// - Vector of (seized payment, protocol fee) tuples.
    fn calculate_seized_collateral(
        &self,
        deposit_positions: &ManagedVec<AccountPosition<Self::Api>>,
        total_collateral_value: &ManagedDecimal<Self::Api, NumDecimals>,
        debt_to_be_repaid: &ManagedDecimal<Self::Api, NumDecimals>,
        bonus_rate: &ManagedDecimal<Self::Api, NumDecimals>,
        cache: &mut Cache<Self>,
    ) -> ManagedVec<MultiValue2<EgldOrEsdtTokenPayment, ManagedDecimal<Self::Api, NumDecimals>>>
    {
        let mut seized_amounts_by_collateral = ManagedVec::new();

        for asset in deposit_positions {
            let total_amount = asset.get_total_amount();
            let asset_data = self.get_token_price(&asset.asset_id, cache);
            let asset_egld_value = self.get_token_egld_value(&total_amount, &asset_data.price);

            // Proportion = (asset_egld_value * WAD) / total_collateral_value
            let proportion = self.div_half_up(
                &(asset_egld_value * self.wad()),
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
            let bonus_bps = self.wad() + bonus_rate.clone();
            let numerator = self.mul_half_up(&seized_units, &bonus_bps, RAY_PRECISION);
            let seized_units_after_bonus = numerator.rescale(asset_data.asset_decimals);

            // Protocol fee = (bonus portion) * liquidation_fees / bps
            let protocol_fee = self
                .mul_half_up(
                    &(seized_units_after_bonus.clone() - seized_units.clone()),
                    &asset.liquidation_fees.clone(),
                    RAY_PRECISION,
                )
                .rescale(asset_data.asset_decimals);
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
    /// - `cache`: Mutable storage cache.
    ///
    /// # Returns
    /// - Tuple of (total repaid in EGLD, repaid token details).
    fn calculate_repayment_amounts(
        &self,
        repayments: &ManagedVec<EgldOrEsdtTokenPayment<Self::Api>>,
        borrows: &ManagedVec<AccountPosition<Self::Api>>,
        refunds: &mut ManagedVec<EgldOrEsdtTokenPayment<Self::Api>>,
        borrows_index_map: ManagedMapEncoded<Self::Api, EgldOrEsdtTokenIdentifier, usize>,
        cache: &mut Cache<Self>,
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
        let mut total_repaid = self.wad_zero();
        let mut repaid_tokens = ManagedVec::new();
        for payment_ref in repayments {
            let token_feed = self.get_token_price(&payment_ref.token_identifier, cache);
            let original_borrow = self.get_position_by_index(
                &payment_ref.token_identifier,
                borrows,
                &borrows_index_map,
            );
            let amount_dec = self.to_decimal(payment_ref.amount.clone(), token_feed.asset_decimals);

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
    /// - `cache`: Mutable storage cache.
    ///
    /// # Returns
    /// - Tuple of (proportional, bonus-weighted) values.
    fn calculate_seizure_proportions(
        &self,
        total_collateral_in_egld: &ManagedDecimal<Self::Api, NumDecimals>,
        positions: &ManagedVec<AccountPosition<Self::Api>>,
        cache: &mut Cache<Self>,
    ) -> (
        ManagedDecimal<Self::Api, NumDecimals>,
        ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        let mut proportion_seized = self.to_decimal_bps(BigUint::zero());
        let mut weighted_bonus = self.to_decimal_bps(BigUint::zero());

        for dp in positions {
            let feed = self.get_token_price(&dp.asset_id, cache);
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
    /// - Tuple of (max debt to repay,max_seized_collateral, bonus rate).
    fn calculate_liquidation_amounts(
        &self,
        total_debt_in_egld: &ManagedDecimal<Self::Api, NumDecimals>,
        total_collateral_in_egld: &ManagedDecimal<Self::Api, NumDecimals>,
        weighted_collateral_in_egld: &ManagedDecimal<Self::Api, NumDecimals>,
        proportion_seized: &ManagedDecimal<Self::Api, NumDecimals>,
        base_liquidation_bonus: &ManagedDecimal<Self::Api, NumDecimals>,
        health_factor: &ManagedDecimal<Self::Api, NumDecimals>,
        debt_payment: OptionalValue<ManagedDecimal<Self::Api, NumDecimals>>,
    ) -> (
        ManagedDecimal<Self::Api, NumDecimals>,
        ManagedDecimal<Self::Api, NumDecimals>,
        ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        let (max_repayable_debt, bonus) = self.estimate_liquidation_amount(
            weighted_collateral_in_egld,
            proportion_seized,
            total_collateral_in_egld,
            total_debt_in_egld,
            base_liquidation_bonus,
            health_factor,
        );

        if debt_payment.is_some() {
            let payment = debt_payment.into_option().unwrap();
            let max_debt_repay = if payment > max_repayable_debt {
                max_repayable_debt
            } else {
                payment
            };

            let max_collateral_seize = self.mul_half_up(
                &max_debt_repay,
                &(bonus.clone() + self.bps()),
                WAD_PRECISION,
            );
            (max_debt_repay, max_collateral_seize, bonus)
        } else {
            let max_collateral_seize = self.mul_half_up(
                &max_repayable_debt,
                &(bonus.clone() + self.bps()),
                WAD_PRECISION,
            );
            (max_repayable_debt, max_collateral_seize, bonus)
        }
    }

    /// Adjusts repayments and refunds for excess payments.
    /// Ensures accurate liquidation accounting.
    ///
    /// # Arguments
    /// - `repaid_tokens`: Mutable repaid token details.
    /// - `refunds`: Mutable refund vector.
    /// - `excess_in_egld`: Excess payment in EGLD.
    /// - `cache`: Mutable storage cache.
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
    ) {
        let mut remaining_excess = excess_in_egld;

        for index in 0..repaid_tokens.len() {
            if remaining_excess == self.wad_zero() {
                break;
            }

            let (mut debt_payment, mut egld_asset_amount, feed) =
                repaid_tokens.get(index).clone().into_tuple();

            if egld_asset_amount >= remaining_excess {
                let excess_in_original = self.convert_egld_to_tokens(&remaining_excess, &feed);
                debt_payment.amount -= excess_in_original.into_raw_units();
                egld_asset_amount -= &remaining_excess;

                refunds.push(EgldOrEsdtTokenPayment::new(
                    debt_payment.token_identifier.clone(),
                    0,
                    excess_in_original.into_raw_units().clone(),
                ));
                let _ = repaid_tokens.set(index, (debt_payment, egld_asset_amount, feed).into());

                remaining_excess = self.wad_zero();
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

    /// Checks if dust cleanup is needed immediately after liquidation and performs it if necessary.
    ///
    /// Uses values already calculated during the liquidation process to determine:
    /// 1. Remaining debt after liquidation
    /// 2. Remaining collateral after liquidation
    /// 3. Whether thresholds for dust cleanup are met
    ///
    /// # Arguments
    /// - `account_nonce`: Position NFT nonce
    /// - `borrowed_egld`: Total borrowed value before liquidation
    /// - `max_debt_repaid`: Amount of debt repaid during liquidation
    /// - `total_collateral`: Total collateral value before liquidation
    /// - `seized_collateral_egld`: Amount of collateral seized during liquidation
    /// - `cache`: Mutable storage cache
    /// - `account_attributes`: Account attributes
    fn check_bad_debt_after_liquidation(
        &self,
        cache: &mut Cache<Self>,
        account_nonce: u64,
        borrowed_egld: &ManagedDecimal<Self::Api, NumDecimals>,
        max_debt_repaid: &ManagedDecimal<Self::Api, NumDecimals>,
        total_collateral: &ManagedDecimal<Self::Api, NumDecimals>,
        seized_collateral_egld: &ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        // Calculate remaining debt and collateral after liquidation
        let remaining_debt_egld = if borrowed_egld > max_debt_repaid {
            borrowed_egld.clone() - max_debt_repaid.clone()
        } else {
            self.wad_zero() // All debt repaid
        };

        let remaining_collateral_egld = if total_collateral > seized_collateral_egld {
            total_collateral.clone() - seized_collateral_egld.clone()
        } else {
            self.wad_zero() // All collateral seized
        };

        let can_clean_bad_debt = self.can_clean_bad_debt_positions(
            cache,
            &remaining_debt_egld,
            &remaining_collateral_egld,
        );

        if can_clean_bad_debt {
            self.emit_trigger_clean_bad_debt(
                account_nonce,
                &remaining_debt_egld,
                &remaining_collateral_egld,
            );
        }
    }

    /// Checks if dust cleanup is needed immediately after liquidation and performs it if necessary.
    ///
    /// Uses values already calculated during the liquidation process to determine:
    /// 1. Remaining debt after liquidation
    /// 2. Remaining collateral after liquidation
    /// 3. Whether thresholds for dust cleanup are met
    ///
    fn can_clean_bad_debt_positions(
        &self,
        cache: &mut Cache<Self>,
        total_borrow: &ManagedDecimal<Self::Api, NumDecimals>,
        total_collateral: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> bool {
        let total_usd_debt = self.get_egld_usd_value(&total_borrow, &cache.egld_price_feed);
        let total_usd_collateral =
            self.get_egld_usd_value(&total_collateral, &cache.egld_price_feed);

        // 5 USD
        let min_collateral_threshold = self.mul_half_up(
            &self.wad(),
            &self.to_decimal(BigUint::from(5u64), 0),
            WAD_PRECISION,
        );

        let has_bad_debt = total_usd_debt > total_usd_collateral;
        let has_collateral_under_min_threshold = total_usd_collateral <= min_collateral_threshold;
        let has_bad_debt_above_min_threshold = total_usd_debt >= min_collateral_threshold;

        has_bad_debt && has_collateral_under_min_threshold && has_bad_debt_above_min_threshold
    }

    /// Executes dust cleanup by seizing all remaining collateral and adding bad debt
    /// to the respective liquidity pools.
    ///
    /// # Arguments
    /// - `account_nonce`: Position NFT nonce
    /// - `cache`: Mutable storage cache
    fn perform_bad_debt_cleanup(&self, account_nonce: u64, cache: &mut Cache<Self>) {
        let caller = self.blockchain().get_caller();
        let mut account_attributes = self.account_attributes(account_nonce).get();
        // If the account is a vault, toggle it to non-vault to move funds to the shared liquidity pool
        if account_attributes.is_vault() {
            account_attributes.is_vault_position = false;

            self.process_vault_toggle(account_nonce, false, cache, &account_attributes, &caller);

            self.update_account_attributes(account_nonce, &account_attributes);
        }

        // Add all remaining debt as bad debt, clean isolated debt if any
        let borrow_positions = self.borrow_positions(account_nonce);
        for (token_id, mut position) in borrow_positions.iter() {
            let feed = self.get_token_price(&token_id, cache);
            let pool_address = cache.get_cached_pool_address(&token_id);
            if account_attributes.is_isolated() {
                self.clear_position_isolated_debt(&mut position, &feed, &account_attributes, cache);
            }

            // Call the add_bad_debt function on the liquidity pool
            let updated_position = self
                .tx()
                .to(pool_address)
                .typed(proxy_pool::LiquidityPoolProxy)
                .add_bad_debt(position.clone(), feed.price.clone())
                .returns(ReturnsResult)
                .sync_call();

            self.update_position_event(
                &position.zero_decimal(),
                &updated_position,
                OptionalValue::Some(feed.price.clone()),
                OptionalValue::Some(&caller),
                OptionalValue::Some(&account_attributes),
            );
        }

        // Seize all remaining collateral + interest
        let deposit_positions = self.deposit_positions(account_nonce);
        for (token_id, position) in deposit_positions.iter() {
            let feed = self.get_token_price(&token_id, cache);
            let pool_address = cache.get_cached_pool_address(&token_id);
            // Call the seize_dust_collateral function on the liquidity pool
            let updated_position = self
                .tx()
                .to(pool_address)
                .typed(proxy_pool::LiquidityPoolProxy)
                .seize_dust_collateral(position.clone(), feed.price.clone())
                .returns(ReturnsResult)
                .sync_call();

            self.update_position_event(
                &position.zero_decimal(),
                &updated_position,
                OptionalValue::Some(feed.price.clone()),
                OptionalValue::Some(&caller),
                OptionalValue::Some(&account_attributes),
            );
        }

        self.borrow_positions(account_nonce).clear();
        self.deposit_positions(account_nonce).clear();
        self.account_positions().swap_remove(&account_nonce);
        self.account_attributes(account_nonce).clear();
    }
}
