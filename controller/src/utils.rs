multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use crate::cache::Cache;
use crate::{helpers, oracle, proxy_pool, storage, ERROR_NO_POOL_FOUND, WAD_PRECISION};
use common_errors::*;
use common_structs::*;

#[multiversx_sc::module]
pub trait LendingUtilsModule:
    storage::Storage
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
    /// - `cache`: Mutable reference to the storage cache for efficiency.
    ///
    /// # Returns
    /// - Tuple of:
    ///   - Weighted collateral in EGLD (based on liquidation thresholds).
    ///   - Total collateral in EGLD (unweighted).
    ///   - LTV-weighted collateral in EGLD (based on loan-to-value ratios).
    fn calculate_collateral_values(
        &self,
        positions: &ManagedVec<AccountPosition<Self::Api>>,
        cache: &mut Cache<Self>,
    ) -> (
        ManagedDecimal<Self::Api, NumDecimals>,
        ManagedDecimal<Self::Api, NumDecimals>,
        ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        let mut weighted_collateral = self.wad_zero();
        let mut total_collateral = self.wad_zero();
        let mut ltv_collateral = self.wad_zero();

        for position in positions {
            let feed = self.get_token_price(&position.asset_id, cache);
            let collateral_in_egld =
                self.get_token_egld_value(&position.get_total_amount(), &feed.price);

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
    /// - `cache`: Mutable reference to the storage cache.
    ///
    /// # Returns
    /// - Total borrow value in EGLD as a `ManagedDecimal`.
    fn calculate_total_borrow_in_egld(
        &self,
        positions: &ManagedVec<AccountPosition<Self::Api>>,
        cache: &mut Cache<Self>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        positions.iter().fold(self.wad_zero(), |acc, position| {
            let feed = self.get_token_price(&position.asset_id, cache);
            acc + self.get_token_egld_value(&position.get_total_amount(), &feed.price)
        })
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
        if amount_in_usd.eq(&self.wad_zero()) {
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
                };
                // If dust remains under 1$ globally just erase the tracker
                if debt.into_raw_units() < self.wad().into_raw_units() {
                    *debt = self.wad_zero();
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

    /// Validates the endpoint for flash loans.
    fn validate_flash_loan_endpoint(&self, endpoint: &ManagedBuffer<Self::Api>) {
        require!(
            !self.blockchain().is_builtin_function(endpoint) && !endpoint.is_empty(),
            ERROR_INVALID_ENDPOINT
        );
    }

    /// Updates bulk borrow positions in the borrow list.
    fn update_bulk_borrow_positions(
        &self,
        borrows: &mut ManagedVec<AccountPosition<Self::Api>>,
        borrow_index_mapper: &mut ManagedMapEncoded<Self::Api, EgldOrEsdtTokenIdentifier, usize>,
        updated_position: AccountPosition<Self::Api>,
        is_bulk_borrow: bool,
    ) {
        if !is_bulk_borrow {
            return;
        }

        let existing_borrow = borrow_index_mapper.contains(&updated_position.asset_id);
        if existing_borrow {
            let safe_index = borrow_index_mapper.get(&updated_position.asset_id);
            let index = safe_index - 1;
            let token_id = &borrows.get(index).asset_id.clone();
            require!(
                token_id == &updated_position.asset_id,
                ERROR_INVALID_BULK_BORROW_TICKER
            );
            let _ = borrows.set(index, updated_position);
        } else {
            let safe_index = borrows.len() + 1;
            borrow_index_mapper.put(&updated_position.asset_id, &safe_index);
            borrows.push(updated_position);
        }
    }

    /// Updates the interest index for a specific asset.
    fn update_asset_index(
        &self,
        asset_id: &EgldOrEsdtTokenIdentifier<Self::Api>,
        cache: &mut Cache<Self>,
    ) {
        let pool_address = self.get_pool_address(asset_id);
        let asset_price = self.get_token_price(asset_id, cache);
        self.tx()
            .to(pool_address)
            .typed(proxy_pool::LiquidityPoolProxy)
            .update_indexes(asset_price.price)
            .sync_call();
    }

    /// Validates health factor post-withdrawal.
    /// Ensures position remains safe after withdrawal.
    ///
    /// # Arguments
    /// - `account_nonce`: Position NFT nonce.
    /// - `is_liquidation`: Liquidation flag.
    /// - `cache`: Mutable storage cache.
    /// - `safety_factor`: Optional safety factor.
    fn validate_is_healthy(
        &self,
        account_nonce: u64,
        cache: &mut Cache<Self>,
        safety_factor: Option<ManagedDecimal<Self::Api, NumDecimals>>,
    ) {
        let borrow_positions = self.borrow_positions(account_nonce);
        if borrow_positions.is_empty() {
            return;
        }

        let deposit_positions = self.deposit_positions(account_nonce);
        let (collateral, _, _) =
            self.calculate_collateral_values(&deposit_positions.values().collect(), cache);
        let borrowed =
            self.calculate_total_borrow_in_egld(&borrow_positions.values().collect(), cache);
        let health_factor = self.compute_health_factor(&collateral, &borrowed);

        let min_health_factor = if let Some(safety_factor_value) = safety_factor {
            self.wad() + (self.wad() / safety_factor_value)
        } else {
            self.wad()
        };

        require!(
            health_factor >= min_health_factor,
            ERROR_HEALTH_FACTOR_WITHDRAW
        );
    }
}
