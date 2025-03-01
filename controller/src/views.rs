use common_structs::AssetExtendedConfigView;

use crate::{contexts::base::StorageCache, helpers, oracle, storage, utils};

multiversx_sc::imports!();

#[multiversx_sc::module]
pub trait ViewsModule:
    storage::LendingStorageModule
    + oracle::OracleModule
    + utils::LendingUtilsModule
    + common_events::EventsModule
    + helpers::math::MathsModule
    + common_math::SharedMathModule
{
    /// Retrieves extended configuration views for multiple assets.
    /// Includes market addresses and current prices in EGLD and USD.
    ///
    /// # Arguments
    /// - `assets`: List of token identifiers (EGLD or ESDT) to query.
    ///
    /// # Returns
    /// - Vector of `AssetExtendedConfigView` structs for each asset.
    #[view(getAllMarkets)]
    fn get_all_markets(
        &self,
        assets: MultiValueEncoded<EgldOrEsdtTokenIdentifier>,
    ) -> ManagedVec<AssetExtendedConfigView<Self::Api>> {
        let mut storage_cache = StorageCache::new(self);
        let mut markets = ManagedVec::new();
        for asset in assets {
            let pool_address = self.pools_map(&asset).get();
            let feed = self.get_token_price(&asset, &mut storage_cache);
            let usd = self.get_token_usd_value(&feed.price, &storage_cache.egld_price_feed);

            markets.push(AssetExtendedConfigView {
                asset_id: asset,
                market_contract_address: pool_address,
                price_in_egld: feed.price,
                price_in_usd: usd,
            });
        }
        markets
    }

    /// Determines if an account position is eligible for liquidation.
    /// Checks if the health factor is below 1 (100% in WAD precision).
    ///
    /// # Arguments
    /// - `account_position`: NFT nonce of the account position.
    ///
    /// # Returns
    /// - `bool`: `true` if the position can be liquidated.
    #[view(canBeLiquidated)]
    fn can_be_liquidated(&self, account_position: u64) -> bool {
        let health_factor = self.get_health_factor(account_position);
        health_factor < self.wad()
    }

    /// Computes the current health factor for an account position.
    /// Indicates position safety; lower values increase liquidation risk.
    ///
    /// # Arguments
    /// - `account_position`: NFT nonce of the account position.
    ///
    /// # Returns
    /// - Health factor as a `ManagedDecimal` in WAD precision.
    #[view(getHealthFactor)]
    fn get_health_factor(&self, account_position: u64) -> ManagedDecimal<Self::Api, NumDecimals> {
        let collateral_in_egld = self.get_liquidation_collateral_available(account_position);
        let borrowed_egld = self.get_total_borrow_in_egld(account_position);
        self.compute_health_factor(&collateral_in_egld, &borrowed_egld)
    }

    /// Retrieves the collateral amount for a specific token in an account position.
    /// Fails if the token is not part of the position’s collateral.
    ///
    /// # Arguments
    /// - `account_position`: NFT nonce of the account position.
    /// - `token_id`: Token identifier (EGLD or ESDT) to query.
    ///
    /// # Returns
    /// - Collateral amount as a `ManagedDecimal`.
    ///
    /// # Panics
    /// - If the token is not in the account’s collateral.
    #[view(getCollateralAmountForToken)]
    fn get_collateral_amount_for_token(
        &self,
        account_position: u64,
        token_id: &EgldOrEsdtTokenIdentifier,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        match self.deposit_positions(account_position).get(token_id) {
            Some(dp) => dp.get_total_amount(),
            None => sc_panic!("Token not existing in the account {}", token_id),
        }
    }

    /// Retrieves the borrowed amount for a specific token in an account position.
    /// Fails if the token is not part of the position’s borrows.
    ///
    /// # Arguments
    /// - `account_position`: NFT nonce of the account position.
    /// - `token_id`: Token identifier (EGLD or ESDT) to query.
    ///
    /// # Returns
    /// - Borrowed amount as a `ManagedDecimal`.
    ///
    /// # Panics
    /// - If the token is not in the account’s borrows.
    #[view(getBorrowAmountForToken)]
    fn get_borrow_amount_for_token(
        &self,
        account_position: u64,
        token_id: &EgldOrEsdtTokenIdentifier,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        match self.borrow_positions(account_position).get(token_id) {
            Some(bp) => bp.get_total_amount(),
            None => sc_panic!("Token not existing in the account {}", token_id),
        }
    }

    /// Computes the total borrow value in EGLD for an account position.
    /// Sums the EGLD value of all borrowed assets.
    ///
    /// # Arguments
    /// - `account_position`: NFT nonce of the account position.
    ///
    /// # Returns
    /// - Total borrow value in EGLD as a `ManagedDecimal`.
    #[view(getTotalBorrowInEgld)]
    fn get_total_borrow_in_egld(
        &self,
        account_position: u64,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let mut storage_cache = StorageCache::new(self);
        let borrow_positions = self.borrow_positions(account_position);

        self.calculate_total_borrow_in_egld(
            &borrow_positions.values().collect(),
            &mut storage_cache,
        )
    }

    /// Computes the total collateral value in EGLD for an account position.
    /// Sums the EGLD value of all collateral assets (unweighted).
    ///
    /// # Arguments
    /// - `account_position`: NFT nonce of the account position.
    ///
    /// # Returns
    /// - Total collateral value in EGLD as a `ManagedDecimal`.
    #[view(getTotalCollateralInEgld)]
    fn get_total_collateral_in_egld(
        &self,
        account_position: u64,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let mut deposited_amount_in_egld = self.to_decimal_wad(BigUint::zero());
        let deposit_positions = self.deposit_positions(account_position);

        let mut storage_cache = StorageCache::new(self);
        storage_cache.allow_unsafe_price = false;

        for dp in deposit_positions.values() {
            deposited_amount_in_egld += self.get_token_amount_in_egld(
                &dp.asset_id,
                &dp.get_total_amount(),
                &mut storage_cache,
            );
        }

        deposited_amount_in_egld
    }

    /// Computes the liquidation collateral available in EGLD.
    /// Represents collateral value weighted by liquidation thresholds.
    ///
    /// # Arguments
    /// - `account_nonce`: NFT nonce of the account position.
    ///
    /// # Returns
    /// - Liquidation collateral in EGLD as a `ManagedDecimal`.
    #[view(getLiquidationCollateralAvailable)]
    fn get_liquidation_collateral_available(
        &self,
        account_nonce: u64,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let deposit_positions = self.deposit_positions(account_nonce);

        let mut storage_cache = StorageCache::new(self);

        let (weighted_collateral, _, _) = self
            .calculate_collateral_values(&deposit_positions.values().collect(), &mut storage_cache);

        weighted_collateral
    }

    /// Computes the LTV-weighted collateral value in EGLD.
    /// Represents collateral value weighted by loan-to-value ratios.
    ///
    /// # Arguments
    /// - `account_position`: NFT nonce of the account position.
    ///
    /// # Returns
    /// - LTV-weighted collateral in EGLD as a `ManagedDecimal`.
    #[view(getLtvCollateralInEgld)]
    fn get_ltv_collateral_in_egld(
        &self,
        account_position: u64,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let deposit_positions = self.deposit_positions(account_position);

        let mut storage_cache = StorageCache::new(self);

        let (_, _, ltv_collateral) = self
            .calculate_collateral_values(&deposit_positions.values().collect(), &mut storage_cache);

        ltv_collateral
    }

    /// Retrieves the USD price of a token using oracle data.
    /// Converts the token’s EGLD price to USD for standardization.
    ///
    /// # Arguments
    /// - `token_id`: Token identifier (EGLD or ESDT) to query.
    ///
    /// # Returns
    /// - USD price of the token as a `ManagedDecimal`.
    #[view(getTokenPriceUSD)]
    fn get_usd_price(
        &self,
        token_id: &EgldOrEsdtTokenIdentifier,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let mut storage_cache = StorageCache::new(self);
        let data = self.get_token_price(token_id, &mut storage_cache);

        self.get_token_usd_value(&data.price, &storage_cache.egld_price_feed)
    }

    /// Retrieves the EGLD price of a token using oracle data.
    /// Accesses the token’s price feed directly.
    ///
    /// # Arguments
    /// - `token_id`: Token identifier (EGLD or ESDT) to query.
    ///
    /// # Returns
    /// - EGLD price of the token as a `ManagedDecimal`.
    #[view(getTokenPriceEGLD)]
    fn get_egld_price(
        &self,
        token_id: &EgldOrEsdtTokenIdentifier,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let mut storage_cache = StorageCache::new(self);
        let data = self.get_token_price(token_id, &mut storage_cache);

        data.price
    }
}
