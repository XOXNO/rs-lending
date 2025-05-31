use common_constants::{BPS_PRECISION, RAY_PRECISION, WAD_PRECISION};
use common_structs::{AccountPosition, AccountPositionType, PriceFeedShort};

use crate::{cache::Cache, helpers, oracle, proxy_pool, storage, utils, validation};
use common_errors::ERROR_HEALTH_FACTOR;

use super::{account, borrow, emode, repay, update, withdraw};

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

#[multiversx_sc::module]
pub trait PositionLiquidationModule:
    storage::Storage
    + validation::ValidationModule
    + oracle::OracleModule
    + common_events::EventsModule
    + utils::LendingUtilsModule
    + helpers::MathsModule
    + account::PositionAccountModule
    + repay::PositionRepayModule
    + withdraw::PositionWithdrawModule
    + update::PositionUpdateModule
    + borrow::PositionBorrowModule
    + common_math::SharedMathModule
    + common_rates::InterestRates
    + emode::EModeModule
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
        cache: &mut Cache<Self>,
    ) -> (
        ManagedVec<MultiValue2<EgldOrEsdtTokenPayment, ManagedDecimal<Self::Api, NumDecimals>>>,
        ManagedVec<
            MultiValue3<
                EgldOrEsdtTokenPayment,
                ManagedDecimal<Self::Api, NumDecimals>,
                PriceFeedShort<Self::Api>,
            >,
        >,
        ManagedVec<EgldOrEsdtTokenPayment>,
        ManagedDecimal<Self::Api, NumDecimals>,
        ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        let mut refunds = ManagedVec::new();
        let deposit_positions = self
            .positions(account_nonce, AccountPositionType::Deposit)
            .values()
            .collect();

        let (borrow_positions, map_debt_indexes) = self.get_borrow_positions(account_nonce, true);

        let (debt_payment_in_egld_ray, mut repaid_tokens) = self.calculate_repayment_amounts(
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

        let (max_debt_to_repay_ray, max_debt_to_repay_wad, max_collateral_seized, bonus_rate) =
            self.calculate_liquidation_amounts(
                &borrowed_egld,
                &total_collateral,
                &liquidation_collateral,
                &proportional_weighted,
                &bonus_weighted,
                &health_factor,
                &debt_payment_in_egld_ray,
            );

        let seized_collaterals = self.calculate_seized_collateral(
            &deposit_positions,
            &total_collateral,
            &max_debt_to_repay_ray,
            &bonus_rate,
            cache,
        );

        self.check_bad_debt_after_liquidation(
            cache,
            account_nonce,
            &borrowed_egld,
            &max_debt_to_repay_ray,
            &total_collateral,
            &max_collateral_seized,
        );

        let user_paid_more = debt_payment_in_egld_ray > max_debt_to_repay_ray;
        // User paid more than the max debt to repay, so we need to refund the excess.
        if user_paid_more {
            let excess_payment_ray = debt_payment_in_egld_ray - max_debt_to_repay_ray.clone();
            self.process_excess_payment(&mut repaid_tokens, &mut refunds, excess_payment_ray);
        }

        (
            seized_collaterals,
            repaid_tokens,
            refunds,
            max_debt_to_repay_wad,
            bonus_rate,
        )
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
        self.reentrancy_guard(cache.flash_loan_ongoing);
        cache.allow_unsafe_price = false;
        self.validate_liquidation_payments(debt_payments, caller);

        self.require_active_account(account_nonce);

        let account_attributes = self.account_attributes(account_nonce).get();

        let (seized_collaterals, repaid_tokens, refunds, _, _) =
            self.execute_liquidation(account_nonce, debt_payments, &mut cache);

        if !refunds.is_empty() {
            self.tx()
                .to(caller)
                .payment(refunds)
                .transfer_if_not_empty();
        }

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
            let mut deposit_position =
                self.get_deposit_position(account_nonce, &seized_collateral.token_identifier);
            let feed = self.get_token_price(&deposit_position.asset_id, &mut cache);
            let amount = deposit_position
                .make_amount_decimal(&seized_collateral.amount, feed.asset_decimals);
            let _ = self.process_withdrawal(
                account_nonce,
                amount,
                caller,
                true,
                Some(protocol_fee),
                &mut cache,
                &account_attributes,
                &mut deposit_position,
                &feed,
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
        debt_to_be_repaid_ray: &ManagedDecimal<Self::Api, NumDecimals>,
        bonus_rate: &ManagedDecimal<Self::Api, NumDecimals>,
        cache: &mut Cache<Self>,
    ) -> ManagedVec<MultiValue2<EgldOrEsdtTokenPayment, ManagedDecimal<Self::Api, NumDecimals>>>
    {
        let mut seized_amounts_by_collateral = ManagedVec::new();

        // Pre-calculate bonus multiplier
        let bonus_multiplier = self.bps() + bonus_rate.clone();

        for position in deposit_positions {
            let asset_data = self.get_token_price(&position.asset_id, cache);
            let total_amount = self.get_total_amount(&position, &asset_data, cache);
            let asset_egld_value_ray =
                self.get_token_egld_value_ray(&total_amount, &asset_data.price);

            // Calculate proportion in RAY precision
            let proportion_ray =
                self.div_half_up(&asset_egld_value_ray, total_collateral_value, RAY_PRECISION);

            // Calculate seized EGLD amount
            let seized_egld_ray =
                self.mul_half_up(&proportion_ray, debt_to_be_repaid_ray, RAY_PRECISION);

            // Apply liquidation bonus
            let seized_egld_with_bonus_ray =
                self.mul_half_up(&seized_egld_ray, &bonus_multiplier, RAY_PRECISION);

            // Convert back to token units
            let seized_units_with_bonus_ray =
                self.convert_egld_to_tokens_ray(&seized_egld_with_bonus_ray, &asset_data);

            // Calculate protocol fee on the bonus portion
            let seized_base_units_ray = self.div_half_up(
                &seized_units_with_bonus_ray,
                &bonus_multiplier,
                RAY_PRECISION,
            );
            let bonus_units_ray = seized_units_with_bonus_ray.clone() - seized_base_units_ray;

            // Protocol fee calculation
            let protocol_fee_ray =
                self.mul_half_up(&bonus_units_ray, &position.liquidation_fees, RAY_PRECISION);
            let protocol_fee = self.rescale_half_up(&protocol_fee_ray, asset_data.asset_decimals);

            // Final rescale to token decimals - this is where unavoidable precision loss occurs
            // due to finite decimal precision of individual tokens (6 decimals for USDC, etc.)
            let final_amount = self.get_min(
                self.rescale_half_up(&seized_units_with_bonus_ray, asset_data.asset_decimals),
                total_amount,
            );

            let seized_asset = EgldOrEsdtTokenPayment::new(
                position.asset_id.clone(),
                0,
                final_amount.into_raw_units().clone(),
            );
            seized_amounts_by_collateral.push((seized_asset, protocol_fee).into());
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
        let mut total_repaid = self.ray_zero();
        let mut repaid_tokens = ManagedVec::new();
        for payment_ref in repayments {
            let token_feed = self.get_token_price(&payment_ref.token_identifier, cache);
            let original_borrow = self.get_position_by_index(
                &payment_ref.token_identifier,
                borrows,
                &borrows_index_map,
            );
            let amount_dec = self.to_decimal(payment_ref.amount.clone(), token_feed.asset_decimals);

            let token_egld_amount = self.get_token_egld_value_ray(&amount_dec, &token_feed.price);

            let amount = self.get_total_amount(&original_borrow, &token_feed, cache);
            let borrowed_egld_amount = self.get_token_egld_value_ray(&amount, &token_feed.price);
            let mut payment = payment_ref.clone();
            if token_egld_amount > borrowed_egld_amount {
                let egld_excess = token_egld_amount - borrowed_egld_amount.clone();
                let original_excess_paid = self.convert_egld_to_tokens(&egld_excess, &token_feed);
                let token_excess_amount = original_excess_paid.into_raw_units().clone();
                payment.amount -= &token_excess_amount;

                refunds.push(EgldOrEsdtTokenPayment::new(
                    payment_ref.token_identifier.clone(),
                    payment_ref.token_nonce,
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
        let mut proportion_seized = self.bps_zero();
        let mut weighted_bonus = self.bps_zero();

        for dp in positions {
            let feed = self.get_token_price(&dp.asset_id, cache);
            let amount = self.get_total_amount(&dp, &feed, cache);
            let egld_amount = self.get_token_egld_value(&amount, &feed.price);
            let fraction = self.rescale_half_up(
                &self.div_half_up(&egld_amount, total_collateral_in_egld, RAY_PRECISION),
                BPS_PRECISION,
            );
            proportion_seized += self.rescale_half_up(
                &self.mul_half_up(&fraction, &dp.liquidation_threshold, RAY_PRECISION),
                BPS_PRECISION,
            );
            weighted_bonus += self.rescale_half_up(
                &self.mul_half_up(&fraction, &dp.liquidation_bonus, RAY_PRECISION),
                BPS_PRECISION,
            );
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
        egld_payment: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> (
        ManagedDecimal<Self::Api, NumDecimals>,
        ManagedDecimal<Self::Api, NumDecimals>,
        ManagedDecimal<Self::Api, NumDecimals>,
        ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        let (estimated_max_repayable_debt_ray, bonus) = self.estimate_liquidation_amount(
            weighted_collateral_in_egld,
            proportion_seized,
            total_collateral_in_egld,
            total_debt_in_egld,
            base_liquidation_bonus,
            health_factor,
        );
        let egld_payment_ray = egld_payment.rescale(RAY_PRECISION);
        let final_repayment_amount_ray = if egld_payment_ray > self.ray_zero() {
            self.get_min(egld_payment_ray, estimated_max_repayable_debt_ray)
        } else {
            estimated_max_repayable_debt_ray.clone()
        };

        let liquidation_premium = bonus.clone() + self.bps();

        let collateral_to_seize = self.mul_half_up(
            &final_repayment_amount_ray,
            &liquidation_premium,
            WAD_PRECISION,
        );
        let final_repayment_amount =
            self.rescale_half_up(&final_repayment_amount_ray, WAD_PRECISION);
        (
            final_repayment_amount_ray,
            final_repayment_amount,
            collateral_to_seize,
            bonus,
        )
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
        let mut index = repaid_tokens.len();
        while index > 0 && remaining_excess > self.ray_zero() {
            index -= 1;

            let (mut debt_payment, mut egld_asset_amount_ray, feed) =
                repaid_tokens.get(index).clone().into_tuple();

            if egld_asset_amount_ray >= remaining_excess {
                let excess_in_original = self.convert_egld_to_tokens(&remaining_excess, &feed);
                debt_payment.amount -= excess_in_original.into_raw_units();
                egld_asset_amount_ray -= &remaining_excess;

                refunds.push(EgldOrEsdtTokenPayment::new(
                    debt_payment.token_identifier.clone(),
                    0,
                    excess_in_original.into_raw_units().clone(),
                ));
                let _ =
                    repaid_tokens.set(index, (debt_payment, egld_asset_amount_ray, feed).into());

                remaining_excess = self.ray_zero();
            } else {
                refunds.push(debt_payment);
                repaid_tokens.remove(index);
                remaining_excess -= egld_asset_amount_ray;
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
        let total_usd_debt = self.get_egld_usd_value(total_borrow, &cache.egld_usd_price);
        let total_usd_collateral = self.get_egld_usd_value(total_collateral, &cache.egld_usd_price);

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
        let account_attributes = self.account_attributes(account_nonce).get();

        // Add all remaining debt as bad debt, clean isolated debt if any
        let borrow_positions = self.positions(account_nonce, AccountPositionType::Borrow);
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

            self.emit_position_update_event(
                &position.zero_decimal(),
                &updated_position,
                feed.price.clone(),
                &caller,
                &account_attributes,
            );
        }

        // Seize all remaining collateral + interest
        let deposit_positions = self.positions(account_nonce, AccountPositionType::Deposit);
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

            self.emit_position_update_event(
                &position.zero_decimal(),
                &updated_position,
                feed.price,
                &caller,
                &account_attributes,
            );
        }

        self.positions(account_nonce, AccountPositionType::Borrow)
            .clear();
        self.positions(account_nonce, AccountPositionType::Deposit)
            .clear();
        self.accounts().swap_remove(&account_nonce);
        self.account_attributes(account_nonce).clear();
    }
}
