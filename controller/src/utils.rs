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
    /// Retrieves the liquidity pool address for a given asset.
    /// Ensures the asset has an associated pool; errors if not found.
    ///
    /// # Arguments
    /// - `asset`: The token identifier (EGLD or ESDT) of the asset.
    ///
    /// # Returns
    /// - `ManagedAddress`: The address of the liquidity pool.
    ///
    /// # Errors
    /// - `ERROR_NO_POOL_FOUND`: If no pool exists for the asset.
    #[view(getPoolAddress)]
    fn get_pool_address(&self, asset: &EgldOrEsdtTokenIdentifier) -> ManagedAddress {
        let pool_address = self.pools_map(asset).get();
        require!(!pool_address.is_zero(), ERROR_NO_POOL_FOUND);
        pool_address
    }

    /// Calculates the total weighted collateral, total collateral, and LTV-weighted collateral in EGLD.
    /// Used for assessing position health and borrow capacity.
    ///
    /// # Arguments
    /// - `positions`: Vector of account positions.
    /// - `storage_cache`: Mutable reference to the storage cache for efficiency.
    ///
    /// # Returns
    /// - Tuple of:
    ///   - Weighted collateral in EGLD (based on liquidation thresholds).
    ///   - Total collateral in EGLD (unweighted).
    ///   - LTV-weighted collateral in EGLD (based on loan-to-value ratios).
    fn calculate_collateral_values(
        &self,
        positions: &ManagedVec<AccountPosition<Self::Api>>,
        storage_cache: &mut StorageCache<Self>,
    ) -> (
        ManagedDecimal<Self::Api, NumDecimals>,
        ManagedDecimal<Self::Api, NumDecimals>,
        ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        let mut weighted_collateral = self.to_decimal_wad(BigUint::zero());
        let mut total_collateral = self.to_decimal_wad(BigUint::zero());
        let mut ltv_collateral = self.to_decimal_wad(BigUint::zero());

        for position in positions {
            let collateral_in_egld = self.get_asset_egld_value(
                &position.asset_id,
                &position.get_total_amount(),
                storage_cache,
            );
            total_collateral += &collateral_in_egld;
            weighted_collateral += self.mul_half_up(
                &collateral_in_egld,
                &position.liquidation_threshold,
                WAD_PRECISION,
            );
            ltv_collateral +=
                self.mul_half_up(&collateral_in_egld, &position.loan_to_value, WAD_PRECISION);
        }

        (weighted_collateral, total_collateral, ltv_collateral)
    }

    /// Calculates the total borrow value in EGLD for a set of positions.
    /// Sums the EGLD value of all borrowed assets.
    ///
    /// # Arguments
    /// - `positions`: Vector of account positions.
    /// - `storage_cache`: Mutable reference to the storage cache.
    ///
    /// # Returns
    /// - Total borrow value in EGLD as a `ManagedDecimal`.
    fn calculate_total_borrow_in_egld(
        &self,
        positions: &ManagedVec<AccountPosition<Self::Api>>,
        storage_cache: &mut StorageCache<Self>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let mut total_borrow = self.to_decimal_wad(BigUint::zero());
        for position in positions {
            total_borrow += self.get_asset_egld_value(
                &position.asset_id,
                &position.get_total_amount(),
                storage_cache,
            );
        }
        total_borrow
    }

    /// Adjusts the isolated debt tracking for an asset in USD.
    /// Updates the debt ceiling based on borrowing or repayment.
    ///
    /// # Arguments
    /// - `asset_id`: Token identifier (EGLD or ESDT) of the asset.
    /// - `amount_in_usd`: USD value to adjust the debt by.
    /// - `is_increase`: Flag to indicate increase (`true`) or decrease (`false`).
    ///
    /// # Notes
    /// - Skips adjustment if the amount is zero.
    /// - Emits a debt ceiling update event.
    fn adjust_isolated_debt_usd(
        &self,
        asset_id: &EgldOrEsdtTokenIdentifier,
        amount_in_usd: ManagedDecimal<Self::Api, NumDecimals>,
        is_increase: bool,
    ) {
        if amount_in_usd.eq(&self.to_decimal_wad(BigUint::zero())) {
            return;
        }

        let debt_mapper = self.isolated_asset_debt_usd(asset_id);
        if is_increase {
            debt_mapper.update(|debt| *debt += amount_in_usd);
        } else {
            debt_mapper.update(|debt| {
                *debt -= if debt.into_raw_units() > amount_in_usd.into_raw_units() {
                    amount_in_usd
                } else {
                    debt.clone()
                }
            });
        }
        self.update_debt_ceiling_event(asset_id, debt_mapper.get());
    }

    /// Updates an account position with the latest interest data from the liquidity pool.
    /// Syncs interest accruals for accurate position tracking.
    ///
    /// # Arguments
    /// - `asset_address`: Address of the asset’s liquidity pool.
    /// - `position`: Mutable reference to the account position to update.
    /// - `price`: Optional current price of the asset for calculations.
    ///
    /// # Notes
    /// - Performs a synchronous call to the liquidity pool proxy.
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
            .sync_position_interest(position.clone(), price)
            .returns(ReturnsResult)
            .sync_call();
    }

    /// Calculates the EGLD value of a specified asset amount.
    /// Converts token amounts to EGLD for standardized valuation.
    ///
    /// # Arguments
    /// - `asset_id`: Token identifier (EGLD or ESDT) of the asset.
    /// - `amount`: Amount of the asset as a `ManagedDecimal`.
    /// - `storage_cache`: Mutable reference to the storage cache.
    ///
    /// # Returns
    /// - EGLD value of the asset amount as a `ManagedDecimal`.
    fn get_asset_egld_value(
        &self,
        asset_id: &EgldOrEsdtTokenIdentifier,
        amount: &ManagedDecimal<Self::Api, NumDecimals>,
        storage_cache: &mut StorageCache<Self>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        self.get_token_amount_in_egld(asset_id, amount, storage_cache)
    }
}
