multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use crate::contexts::base::StorageCache;
use crate::{helpers, oracle, proxy_pool, storage, ERROR_NO_COLLATERAL_TOKEN, ERROR_NO_POOL_FOUND};
use common_constants::EGLD_IDENTIFIER;
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

    fn get_multi_payments(&self) -> ManagedVec<EgldOrEsdtTokenPaymentNew<Self::Api>> {
        let payments = self.call_value().all_esdt_transfers();

        let mut valid_payments = ManagedVec::new();
        for i in 0..payments.len() {
            let payment = payments.get(i);
            // EGLD sent as multi-esdt payment
            if payment.token_identifier.clone().into_managed_buffer()
                == ManagedBuffer::from(EGLD_IDENTIFIER)
                || payment.token_identifier.clone().into_managed_buffer()
                    == ManagedBuffer::from("EGLD")
            {
                valid_payments.push(EgldOrEsdtTokenPaymentNew {
                    token_identifier: EgldOrEsdtTokenIdentifier::egld(),
                    token_nonce: 0,
                    amount: payment.amount.clone(),
                });
            } else {
                valid_payments.push(EgldOrEsdtTokenPaymentNew {
                    token_identifier: EgldOrEsdtTokenIdentifier::esdt(
                        payment.token_identifier.clone(),
                    ),
                    token_nonce: payment.token_nonce,
                    amount: payment.amount.clone(),
                });
            }
        }

        valid_payments
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
    fn calculate_single_asset_liquidation_amount(
        &self,
        total_debt_in_egld: &BigUint,
        total_collateral_in_egld: &BigUint,
        token_to_liquidate: &EgldOrEsdtTokenIdentifier,
        liquidatee_account_nonce: u64,
        debt_payment: OptionalValue<BigUint>,
        base_liquidation_bonus: &BigUint,
        health_factor: &BigUint,
        collateral_feed: &PriceFeedShort<Self::Api>,
    ) -> (BigUint, BigUint) {
        // Get the available collateral value for this specific asset
        let deposit_position = self
            .deposit_positions(liquidatee_account_nonce)
            .get(token_to_liquidate)
            .unwrap_or_else(|| sc_panic!(ERROR_NO_COLLATERAL_TOKEN));

        let total_position_egld_value = self
            .get_token_amount_in_egld_raw(&deposit_position.get_total_amount(), collateral_feed);

        let (max_repayable_debt, bonus) = self.estimate_liquidation_amount(
            &total_position_egld_value,
            total_collateral_in_egld,
            total_debt_in_egld,
            &deposit_position.entry_liquidation_threshold,
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
