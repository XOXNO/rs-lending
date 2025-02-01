multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use crate::contexts::base::StorageCache;
use crate::{helpers, oracle, proxy_pool, storage, ERROR_NO_POOL_FOUND};
use common_structs::*;

#[multiversx_sc::module]
pub trait LendingUtilsModule:
    storage::LendingStorageModule
    + oracle::OracleModule
    + common_events::EventsModule
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
    ) -> (BigUint, BigUint, BigUint) {
        let mut weighted_collateral_in_egld = BigUint::zero();
        let mut total_collateral_in_egld = BigUint::zero();
        let mut total_ltv_collateral_in_egld = BigUint::zero();

        for dp in positions {
            let collateral_in_egld =
                self.get_token_amount_in_egld(&dp.token_id, &dp.get_total_amount(), storage_cache);
            weighted_collateral_in_egld +=
                &collateral_in_egld * &dp.entry_liquidation_threshold / &storage_cache.bp;
            total_ltv_collateral_in_egld += &collateral_in_egld * &dp.entry_ltv / &storage_cache.bp;
            total_collateral_in_egld += collateral_in_egld;
        }

        (
            weighted_collateral_in_egld,
            total_collateral_in_egld,
            total_ltv_collateral_in_egld,
        )
    }

    fn proportion_of_weighted_seized(
        &self,
        total_collateral_in_egld: &BigUint,
        positions: &ManagedVec<AccountPosition<Self::Api>>,
        storage_cache: &mut StorageCache<Self>,
    ) -> (BigUint, BigUint) {
        let mut proportion_of_weighted_seized = BigUint::zero();
        let mut weighted_bonus = BigUint::zero();

        for dp in positions {
            let collateral_in_egld =
                self.get_token_amount_in_egld(&dp.token_id, &dp.get_total_amount(), storage_cache);
            let fraction = collateral_in_egld * &storage_cache.bp / total_collateral_in_egld;
            proportion_of_weighted_seized +=
                &fraction * &dp.entry_liquidation_threshold / &storage_cache.bp;
            weighted_bonus += fraction * &dp.entry_liquidation_bonus / &storage_cache.bp;
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
    ) -> BigUint {
        let mut total_borrow_in_egld = BigUint::zero();

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
        BigUint,
        ManagedVec<MultiValue3<EgldOrEsdtTokenPayment, BigUint, PriceFeedShort<Self::Api>>>,
    ) {
        let mut total_repaid = BigUint::zero();
        let mut tokens_egld_value = ManagedVec::new();
        for mut payment in repayments.clone() {
            let token_feed = self.get_token_price(&payment.token_identifier, storage_cache);
            let original_borrow =
                self.get_position_by_index(&payment.token_identifier, borrows, &borrows_index_map);

            let token_egld_amount = self.get_token_amount_in_egld_raw(&payment.amount, &token_feed);

            let borrowed_egld_amount =
                self.get_token_amount_in_egld_raw(&original_borrow.get_total_amount(), &token_feed);

            if token_egld_amount > borrowed_egld_amount {
                total_repaid += &borrowed_egld_amount;
                let egld_excess = token_egld_amount - &borrowed_egld_amount;
                let original_excess_paid = self.compute_amount_in_tokens(&egld_excess, &token_feed);
                payment.amount -= &original_excess_paid;
                tokens_egld_value.push((payment.clone(), borrowed_egld_amount, token_feed).into());
                refunds.push(EgldOrEsdtTokenPayment::new(
                    payment.token_identifier,
                    payment.token_nonce,
                    original_excess_paid,
                ));
            } else {
                total_repaid += &token_egld_amount;
                tokens_egld_value.push((payment.clone(), token_egld_amount, token_feed).into());
            }
        }

        (total_repaid, tokens_egld_value)
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
        amount_in_usd: &BigUint,
        is_increase: bool,
    ) {
        if amount_in_usd.eq(&BigUint::zero()) {
            return;
        }

        let map = self.isolated_asset_debt_usd(token_id);

        if is_increase {
            map.update(|debt| *debt += amount_in_usd);
        } else {
            map.update(|debt| *debt -= amount_in_usd.min(&debt.clone()));
        }

        self.update_debt_ceiling_event(token_id, map.get());
    }

    /// Gets NFT attributes for an account position
    ///
    /// # Arguments
    /// * `account_nonce` - NFT nonce of the position
    /// * `token_id` - NFT token identifier
    ///
    /// # Returns
    /// * `NftAccountAttributes` - Decoded NFT attributes
    fn nft_attributes(
        &self,
        account_nonce: u64,
        token_id: &TokenIdentifier<Self::Api>,
    ) -> NftAccountAttributes {
        let data = self.blockchain().get_esdt_token_data(
            &self.blockchain().get_sc_address(),
            token_id,
            account_nonce,
        );

        data.decode_attributes::<NftAccountAttributes>()
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
        price: OptionalValue<BigUint>,
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
        total_debt_in_egld: &BigUint,
        total_collateral_in_egld: &BigUint,
        weighted_collateral_in_egld: BigUint,
        proportion_of_weighted_seized: &BigUint,
        base_liquidation_bonus: &BigUint,
        health_factor: &BigUint,
        debt_payment: OptionalValue<BigUint>,
    ) -> (BigUint, BigUint) {
        let (max_repayable_debt, bonus) = self.estimate_liquidation_amount(
            weighted_collateral_in_egld,
            proportion_of_weighted_seized,
            total_collateral_in_egld,
            total_debt_in_egld,
            base_liquidation_bonus,
            health_factor,
        );

        if debt_payment.is_some() {
            // Take the minimum between what we need and what's available and what the liquidator is paying
            (
                BigUint::min(debt_payment.into_option().unwrap(), max_repayable_debt),
                bonus,
            )
        } else {
            (max_repayable_debt, bonus)
        }
    }
}
