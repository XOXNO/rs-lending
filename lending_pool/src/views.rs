use common_constants::BP;
use common_events::{AssetExtendedConfigView, PriceFeedShort};

use crate::{contexts::base::StorageCache, helpers, oracle, storage, utils};

multiversx_sc::imports!();

#[multiversx_sc::module]
pub trait ViewsModule:
    storage::LendingStorageModule
    + oracle::OracleModule
    + utils::LendingUtilsModule
    + common_events::EventsModule
    + helpers::math::MathsModule
{
    #[view(getAllMarkets)]
    fn get_all_markets(
        &self,
        tokens: MultiValueEncoded<EgldOrEsdtTokenIdentifier>,
    ) -> ManagedVec<AssetExtendedConfigView<Self::Api>> {
        let mut storage_cache = StorageCache::new(self);
        let mut markets = ManagedVec::new();
        for token in tokens {
            let pool_address = self.pools_map(&token).get();
            let pool = self.asset_config(&token).get();
            let egld_price = self.get_token_price(&token, &mut storage_cache);
            let usd_price = self
                .get_token_amount_in_dollars_raw(&egld_price.price, &storage_cache.egld_price_feed);

            markets.push(AssetExtendedConfigView {
                token,
                asset_config: pool,
                market_address: pool_address,
                egld_price: egld_price.price,
                usd_price: usd_price,
            });
        }
        markets
    }

    /// Checks if an account position can be liquidated
    ///
    /// # Arguments
    /// * `account_position` - NFT nonce of the account position
    ///
    /// # Returns
    /// * `bool` - True if position can be liquidated (health factor < 100%)
    ///
    /// # Example
    /// ```
    /// // Position 1: Healthy
    /// // Collateral: $150 weighted
    /// // Borrows: $100
    /// // Health Factor: 150% (15000 bp)
    /// can_be_liquidated(1) = false
    ///
    /// // Position 2: Unhealthy
    /// // Collateral: $90 weighted
    /// // Borrows: $100
    /// // Health Factor: 90% (9000 bp)
    /// can_be_liquidated(2) = true
    /// ```
    #[view(canBeLiquidated)]
    fn can_be_liquidated(&self, account_position: u64) -> bool {
        let bp = BigUint::from(BP);
        let health_factor = self.get_health_factor(account_position);
        health_factor < bp
    }

    /// Gets the current health factor for an account position
    ///
    /// # Arguments
    /// * `account_position` - NFT nonce of the account position
    ///
    /// # Returns
    /// * `BigUint` - Health factor in basis points (10000 = 100%)
    ///
    /// # Example
    /// ```
    /// // Position with:
    /// // Collateral: 100 EGLD @ $100 each = $10,000
    /// // Liquidation threshold: 80%
    /// // Weighted collateral: $8,000
    ///
    /// // Borrows: 5000 USDC @ $1 each = $5,000
    ///
    /// // Health Factor = $8,000 * 10000 / $5,000 = 16000 (160%)
    /// get_health_factor(1) = 16000
    /// ```
    #[view(getHealthFactor)]
    fn get_health_factor(&self, account_position: u64) -> BigUint {
        let collateral_in_egld = self.get_liquidation_collateral_available(account_position);
        let borrowed_egld = self.get_total_borrow_in_egld(account_position);
        self.compute_health_factor(&collateral_in_egld, &borrowed_egld)
    }

    /// Gets the collateral amount for a specific token
    ///
    /// # Arguments
    /// * `account_position` - NFT nonce of the account position
    /// * `token_id` - Token identifier to check
    ///
    /// # Returns
    /// * `BigUint` - Amount of token supplied as collateral
    ///
    /// # Example
    /// ```
    /// // Position has:
    /// // - 100 EGLD supplied
    /// // - 1000 USDC supplied
    ///
    /// get_collateral_amount_for_token(1, "EGLD-123456") = 100_000_000_000_000_000_000
    /// get_collateral_amount_for_token(1, "USDC-123456") = 1_000_000_000
    /// get_collateral_amount_for_token(1, "USDT-123456") = 0 // No USDT supplied
    /// ```
    #[view(getCollateralAmountForToken)]
    fn get_collateral_amount_for_token(
        &self,
        account_position: u64,
        token_id: &EgldOrEsdtTokenIdentifier,
    ) -> BigUint {
        match self.deposit_positions(account_position).get(token_id) {
            Some(dp) => dp.get_total_amount(),
            None => BigUint::zero(),
        }
    }

    /// Gets the borrowed amount for a specific token
    ///
    /// # Arguments
    /// * `account_position` - NFT nonce of the account position
    /// * `token_id` - Token identifier to check
    ///
    /// # Returns
    /// * `BigUint` - Amount of token borrowed
    ///
    /// # Example
    /// ```
    /// // Position has:
    /// // - 50 EGLD borrowed
    /// // - 500 USDC borrowed
    ///
    /// get_borrow_amount_for_token(1, "EGLD-123456") = 50_000_000_000_000_000_000
    /// get_borrow_amount_for_token(1, "USDC-123456") = 500_000_000
    /// get_borrow_amount_for_token(1, "USDT-123456") = 0 // No USDT borrowed
    /// ```
    #[view(getBorrowAmountForToken)]
    fn get_borrow_amount_for_token(
        &self,
        account_position: u64,
        token_id: &EgldOrEsdtTokenIdentifier,
    ) -> BigUint {
        match self.borrow_positions(account_position).get(token_id) {
            Some(bp) => bp.get_total_amount(),
            None => BigUint::zero(),
        }
    }

    /// Gets total value of borrowed assets in USD
    ///
    /// # Arguments
    /// * `account_position` - NFT nonce of the account position
    ///
    /// # Returns
    /// * `BigUint` - Total USD value of all borrowed assets
    ///
    /// # Example
    /// ```
    /// // Position has:
    /// // - 50 EGLD borrowed @ $100 each = $5,000
    /// // - 500 USDC borrowed @ $1 each = $500
    ///
    /// get_total_borrow_in_egld(1) = 5_500_000_000 // $5,500
    /// ```
    #[view(getTotalBorrowInEgld)]
    fn get_total_borrow_in_egld(&self, account_position: u64) -> BigUint {
        let mut storage_cache = StorageCache::new(self);
        let borrow_positions = self.borrow_positions(account_position);
        self.sum_borrows(&borrow_positions.values().collect(), &mut storage_cache)
    }

    /// Gets total value of collateral assets in USD
    ///
    /// # Arguments
    /// * `account_position` - NFT nonce of the account position
    ///
    /// # Returns
    /// * `BigUint` - Total USD value of all collateral assets (unweighted)
    ///
    /// # Example
    /// ```
    /// // Position has:
    /// // - 100 EGLD supplied @ $100 each = $10,000
    /// // - 1000 USDC supplied @ $1 each = $1,000
    ///
    /// get_total_collateral_in_dollars(1) = 11_000_000_000 // $11,000
    /// ```
    #[view(getTotalCollateralInEgld)]
    fn get_total_collateral_in_egld(&self, account_position: u64) -> BigUint {
        let mut deposited_amount_in_egld = BigUint::zero();
        let deposit_positions = self.deposit_positions(account_position);
        let mut storage_cache = StorageCache::new(self);
        storage_cache.allow_unsafe_price = false;

        for dp in deposit_positions.values() {
            deposited_amount_in_egld += self.get_token_amount_in_egld(
                &dp.token_id,
                &dp.get_total_amount(),
                &mut storage_cache,
            );
        }

        deposited_amount_in_egld
    }

    /// Gets total value of collateral available for liquidation in USD
    ///
    /// # Arguments
    /// * `account_nonce` - NFT nonce of the account position
    ///
    /// # Returns
    /// * `BigUint` - Total USD value of collateral weighted by liquidation thresholds
    ///
    /// # Example
    /// ```
    /// // Position has:
    /// // - 100 EGLD @ $100 each = $10,000, threshold 80% = $8,000
    /// // - 1000 USDC @ $1 each = $1,000, threshold 85% = $850
    ///
    /// get_liquidation_collateral_available(1) = 8_850_000_000 // $8,850
    /// ```
    #[view(getLiquidationCollateralAvailable)]
    fn get_liquidation_collateral_available(&self, account_nonce: u64) -> BigUint {
        let deposit_positions = self.deposit_positions(account_nonce);
        let mut storage_cache = StorageCache::new(self);
        let (weighted_collateral, _, _) =
            self.sum_collaterals(&deposit_positions.values().collect(), &mut storage_cache);

        weighted_collateral
    }

    /// Gets total value of collateral weighted by LTV ratios in USD
    ///
    /// # Arguments
    /// * `account_position` - NFT nonce of the account position
    ///
    /// # Returns
    /// * `BigUint` - Total USD value of collateral weighted by LTV ratios
    ///
    /// # Example
    /// ```
    /// // Position has:
    /// // - 100 EGLD @ $100 each = $10,000, LTV 75% = $7,500
    /// // - 1000 USDC @ $1 each = $1,000, LTV 80% = $800
    ///
    /// get_ltv_collateral_in_dollars(1) = 8_300_000_000 // $8,300
    /// ```
    #[view(getLtvCollateralInEgld)]
    fn get_ltv_collateral_in_egld(&self, account_position: u64) -> BigUint {
        let deposit_positions = self.deposit_positions(account_position);
        let mut storage_cache = StorageCache::new(self);
        let (_, _, ltv_collateral) =
            self.sum_collaterals(&deposit_positions.values().collect(), &mut storage_cache);
        ltv_collateral
    }

    #[view(getTokenPriceData)]
    fn get_token_price_data_view(
        &self,
        token_id: &EgldOrEsdtTokenIdentifier,
    ) -> PriceFeedShort<Self::Api> {
        let mut storage_cache = StorageCache::new(self);
        self.get_token_price(token_id, &mut storage_cache)
    }

    #[view(getTokenPriceUSD)]
    fn get_usd_price(&self, token_id: &EgldOrEsdtTokenIdentifier) -> BigUint {
        let mut storage_cache = StorageCache::new(self);
        let data = self.get_token_price(token_id, &mut storage_cache);
        self.get_token_amount_in_dollars_raw(&data.price, &storage_cache.egld_price_feed)
    }

    #[view(getTokenPriceEGLD)]
    fn get_egld_price(&self, token_id: &EgldOrEsdtTokenIdentifier) -> BigUint {
        let mut storage_cache = StorageCache::new(self);
        let data = self.get_token_price(token_id, &mut storage_cache);
        data.price
    }
}
