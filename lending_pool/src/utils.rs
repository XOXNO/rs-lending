multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use crate::contexts::base::StorageCache;
use crate::{helpers, oracle, proxy_pool, storage, ERROR_NO_POOL_FOUND, WAD_PRECISION};
use common_structs::*;

#[multiversx_sc::module]
pub trait LendingUtilsModule:
    storage::LendingStorageModule
    + oracle::OracleModule
    + common_events::EventsModule
    + common_math::SharedMathModule
    + helpers::math::MathsModule
{
    /// Gets the liquidity pool address for a given asset
    ///
    /// # Arguments
    /// * `asset` - Token identifier of the asset
    ///
    /// # Returns
    /// * `ManagedAddress` - Address of the liquidity pool
    ///
    /// # Errors
    /// * `ERROR_NO_POOL_FOUND` - If no pool exists for the asset
    #[view(getPoolAddress)]
    fn get_pool_address(&self, asset: &EgldOrEsdtTokenIdentifier) -> ManagedAddress {
        let pool_address = self.pools_map(asset).get();

        require!(!pool_address.is_zero(), ERROR_NO_POOL_FOUND);

        pool_address
    }

    /// Calculates total weighted collateral value in EGLD for liquidation
    ///
    /// # Arguments
    /// * `positions` - Vector of account positions
    ///
    /// # Returns
    /// * `BigUint` - Total EGLD value weighted by liquidation thresholds
    ///
    /// ```
    fn sum_collaterals(
        &self,
        positions: &ManagedVec<AccountPosition<Self::Api>>,
        storage_cache: &mut StorageCache<Self>,
    ) -> (
        ManagedDecimal<Self::Api, NumDecimals>,
        ManagedDecimal<Self::Api, NumDecimals>,
        ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        let mut weighted_collateral_in_egld = self.to_decimal_wad(BigUint::zero());
        let mut total_collateral_in_egld = self.to_decimal_wad(BigUint::zero());
        let mut total_ltv_collateral_in_egld = self.to_decimal_wad(BigUint::zero());

        for dp in positions {
            let collateral_in_egld =
                self.get_token_amount_in_egld(&dp.token_id, &dp.get_total_amount(), storage_cache);

            total_collateral_in_egld += &collateral_in_egld;
            weighted_collateral_in_egld += self.mul_half_up(
                &collateral_in_egld,
                &dp.entry_liquidation_threshold,
                WAD_PRECISION,
            );
            total_ltv_collateral_in_egld +=
                self.mul_half_up(&collateral_in_egld, &dp.entry_ltv, WAD_PRECISION);
        }

        (
            weighted_collateral_in_egld,
            total_collateral_in_egld,
            total_ltv_collateral_in_egld,
        )
    }

    fn proportion_of_weighted_seized(
        &self,
        total_collateral_in_egld: &ManagedDecimal<Self::Api, NumDecimals>,
        positions: &ManagedVec<AccountPosition<Self::Api>>,
        storage_cache: &mut StorageCache<Self>,
    ) -> (
        ManagedDecimal<Self::Api, NumDecimals>,
        ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        let mut proportion_of_weighted_seized = self.to_decimal_bps(BigUint::zero());
        let mut weighted_bonus = self.to_decimal_bps(BigUint::zero());

        for dp in positions {
            let collateral_in_egld =
                self.get_token_amount_in_egld(&dp.token_id, &dp.get_total_amount(), storage_cache);
            let fraction = self
                .div_half_up(&collateral_in_egld, total_collateral_in_egld, RAY_PRECISION)
                .rescale(BPS_PRECISION);
            proportion_of_weighted_seized += self
                .mul_half_up(&fraction, &dp.entry_liquidation_threshold, RAY_PRECISION)
                .rescale(BPS_PRECISION);
            weighted_bonus += self
                .mul_half_up(&fraction, &dp.entry_liquidation_bonus, RAY_PRECISION)
                .rescale(BPS_PRECISION);
        }

        (proportion_of_weighted_seized, weighted_bonus)
    }

    /// Calculates total borrow value in USD
    ///
    /// # Arguments
    /// * `positions` - Vector of account positions
    ///
    /// # Returns
    /// * `BigUint` - Total USD value of borrowed assets
    ///
    /// ```
    fn sum_borrows(
        &self,
        positions: &ManagedVec<AccountPosition<Self::Api>>,
        storage_cache: &mut StorageCache<Self>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let mut total_borrow_in_egld = self.to_decimal_wad(BigUint::zero());

        for bp in positions {
            total_borrow_in_egld +=
                self.get_token_amount_in_egld(&bp.token_id, &bp.get_total_amount(), storage_cache);
        }

        total_borrow_in_egld
    }

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

    fn sum_repayments(
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
                token_feed.decimals as usize,
            );

            let token_egld_amount =
                self.get_token_amount_in_egld_raw(&amount_dec, &token_feed.price);

            let borrowed_egld_amount = self.get_token_amount_in_egld_raw(
                &original_borrow.get_total_amount(),
                &token_feed.price,
            );
            let mut payment = payment_ref.clone();
            if token_egld_amount > borrowed_egld_amount {
                let egld_excess = token_egld_amount - borrowed_egld_amount.clone();
                let original_excess_paid = self.compute_egld_in_tokens(&egld_excess, &token_feed);
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

    /// Updates isolated asset debt tracking
    ///
    /// # Arguments
    /// * `token_id` - Token identifier
    /// * `amount_in_usd` - USD value to add/subtract
    /// * `is_increase` - Whether to increase or decrease debt
    ///
    /// # Flow
    /// 1. Skips if amount is zero
    /// 2. Updates debt tracking storage
    /// 3. Emits debt ceiling event
    /// ```
    fn update_isolated_debt_usd(
        &self,
        token_id: &EgldOrEsdtTokenIdentifier,
        amount_in_usd: ManagedDecimal<Self::Api, NumDecimals>,
        is_increase: bool,
    ) {
        if amount_in_usd.eq(&self.to_decimal_wad(BigUint::zero())) {
            return;
        }

        let map = self.isolated_asset_debt_usd(token_id);

        if is_increase {
            map.update(|debt| *debt += amount_in_usd);
        } else {
            map.update(|debt| {
                *debt -= if debt.into_raw_units() > amount_in_usd.into_raw_units() {
                    amount_in_usd
                } else {
                    debt.clone()
                }
            });
        }

        self.update_debt_ceiling_event(token_id, map.get());
    }

    /// Updates account position with interest
    /// - Updates position's interest amount
    /// - Updates position's last interest update timestamp
    /// - Updates position's total amount
    ///
    /// # Arguments
    /// * `asset_address` - Address of the asset
    /// * `position` - Account position to update
    /// * `price` - Current price of the asset
    ///
    /// # Returns
    /// * `AccountPosition` - Edits the position in place
    ///
    fn update_position(
        &self,
        asset_address: &ManagedAddress,
        position: &mut AccountPosition<Self::Api>,
        price: OptionalValue<ManagedDecimal<Self::Api, NumDecimals>>,
    ) {
        *position = self
            .tx()
            .to(asset_address)
            .typed(proxy_pool::LiquidityPoolProxy)
            .update_position_with_interest(position.clone(), price)
            .returns(ReturnsResult)
            .sync_call();
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
    /// * `(BigUint, BigUint)` - Maximum EGLD value of the specific collateral that can be liquidated and the bonus
    fn calculate_max_debt_repayment(
        &self,
        total_debt_in_egld: &ManagedDecimal<Self::Api, NumDecimals>,
        total_collateral_in_egld: &ManagedDecimal<Self::Api, NumDecimals>,
        weighted_collateral_in_egld: ManagedDecimal<Self::Api, NumDecimals>,
        proportion_of_weighted_seized: &ManagedDecimal<Self::Api, NumDecimals>,
        base_liquidation_bonus: &ManagedDecimal<Self::Api, NumDecimals>,
        health_factor: &ManagedDecimal<Self::Api, NumDecimals>,
        debt_payment: OptionalValue<ManagedDecimal<Self::Api, NumDecimals>>,
    ) -> (
        ManagedDecimal<Self::Api, NumDecimals>,
        ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        let (max_repayable_debt, bonus) = self.estimate_liquidation_amount(
            weighted_collateral_in_egld.into_raw_units(),
            proportion_of_weighted_seized.into_raw_units(),
            total_collateral_in_egld.into_raw_units(),
            total_debt_in_egld.into_raw_units(),
            base_liquidation_bonus.into_raw_units(),
            health_factor.into_raw_units(),
        );

        if debt_payment.is_some() {
            // Take the minimum between what we need and what's available and what the liquidator is paying
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
}
