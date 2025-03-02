use common_structs::AssetExtendedConfigView;

use crate::{cache::Cache, helpers, oracle, storage, utils};

multiversx_sc::imports!();

#[multiversx_sc::module]
pub trait ViewsModule:
    storage::Storage
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
        let mut cache = Cache::new(self);
        let mut markets = ManagedVec::new();
        for asset in assets {
            let pool_address = self.pools_map(&asset).get();
            let feed = self.get_token_price(&asset, &mut cache);
            let usd = self.get_token_usd_value(&feed.price, &cache.egld_price_feed);

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
    /// - `account_nonce`: NFT nonce of the account position.
    ///
    /// # Returns
    /// - `bool`: `true` if the position can be liquidated.
    #[view(canBeLiquidated)]
    fn can_be_liquidated(&self, account_nonce: u64) -> bool {
        let health_factor = self.get_health_factor(account_nonce);
        health_factor < self.wad()
    }

    /// Computes the current health factor for an account position.
    /// Indicates position safety; lower values increase liquidation risk.
    ///
    /// # Arguments
    /// - `account_nonce`: NFT nonce of the account position.
    ///
    /// # Returns
    /// - Health factor as a `ManagedDecimal` in WAD precision.
    #[view(getHealthFactor)]
    fn get_health_factor(&self, account_nonce: u64) -> ManagedDecimal<Self::Api, NumDecimals> {
        let collateral_in_egld = self.get_liquidation_collateral_available(account_nonce);
        let borrowed_egld = self.get_total_borrow_in_egld(account_nonce);
        self.compute_health_factor(&collateral_in_egld, &borrowed_egld)
    }

    /// Retrieves the collateral amount for a specific token in an account position.
    /// Fails if the token is not part of the position’s collateral.
    ///
    /// # Arguments
    /// - `account_nonce`: NFT nonce of the account position.
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
        account_nonce: u64,
        token_id: &EgldOrEsdtTokenIdentifier,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        match self.deposit_positions(account_nonce).get(token_id) {
            Some(dp) => dp.get_total_amount(),
            None => sc_panic!("Token not existing in the account {}", token_id),
        }
    }

    /// Retrieves the borrowed amount for a specific token in an account position.
    /// Fails if the token is not part of the position’s borrows.
    ///
    /// # Arguments
    /// - `account_nonce`: NFT nonce of the account position.
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
        account_nonce: u64,
        token_id: &EgldOrEsdtTokenIdentifier,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        match self.borrow_positions(account_nonce).get(token_id) {
            Some(bp) => bp.get_total_amount(),
            None => sc_panic!("Token not existing in the account {}", token_id),
        }
    }

    /// Computes the total borrow value in EGLD for an account position.
    /// Sums the EGLD value of all borrowed assets.
    ///
    /// # Arguments
    /// - `account_nonce`: NFT nonce of the account position.
    ///
    /// # Returns
    /// - Total borrow value in EGLD as a `ManagedDecimal`.
    #[view(getTotalBorrowInEgld)]
    fn get_total_borrow_in_egld(
        &self,
        account_nonce: u64,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let mut cache = Cache::new(self);
        let borrow_positions = self.borrow_positions(account_nonce);

        self.calculate_total_borrow_in_egld(&borrow_positions.values().collect(), &mut cache)
    }

    /// Computes the total collateral value in EGLD for an account position.
    /// Sums the EGLD value of all collateral assets (unweighted).
    ///
    /// # Arguments
    /// - `account_nonce`: NFT nonce of the account position.
    ///
    /// # Returns
    /// - Total collateral value in EGLD as a `ManagedDecimal`.
    #[view(getTotalCollateralInEgld)]
    fn get_total_collateral_in_egld(
        &self,
        account_nonce: u64,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let deposit_positions = self.deposit_positions(account_nonce);

        let mut cache = Cache::new(self);
        cache.allow_unsafe_price = false;

        deposit_positions.values().fold(self.wad_zero(), |acc, dp| {
            let feed = self.get_token_price(&dp.asset_id, &mut cache);
            acc + self.get_token_egld_value(&dp.get_total_amount(), &feed.price)
        })
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

        let mut cache = Cache::new(self);

        let (weighted_collateral, _, _) =
            self.calculate_collateral_values(&deposit_positions.values().collect(), &mut cache);

        weighted_collateral
    }

    /// Computes the LTV-weighted collateral value in EGLD.
    /// Represents collateral value weighted by loan-to-value ratios.
    ///
    /// # Arguments
    /// - `account_nonce`: NFT nonce of the account position.
    ///
    /// # Returns
    /// - LTV-weighted collateral in EGLD as a `ManagedDecimal`.
    #[view(getLtvCollateralInEgld)]
    fn get_ltv_collateral_in_egld(
        &self,
        account_nonce: u64,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let deposit_positions = self.deposit_positions(account_nonce);

        let mut cache = Cache::new(self);

        let (_, _, ltv_collateral) =
            self.calculate_collateral_values(&deposit_positions.values().collect(), &mut cache);

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
        let mut cache = Cache::new(self);
        let data = self.get_token_price(token_id, &mut cache);

        self.get_token_usd_value(&data.price, &cache.egld_price_feed)
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
        let mut cache = Cache::new(self);
        let data = self.get_token_price(token_id, &mut cache);

        data.price
    }
}
