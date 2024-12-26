use common_constants::BP;
use common_events::{AssetExtendedConfigView, PriceFeedShort};

use crate::{contexts::base::StorageCache, oracle, storage, utils, ERROR_HEALTH_FACTOR};

multiversx_sc::imports!();

#[multiversx_sc::module]
pub trait ViewsModule:
    storage::LendingStorageModule
    + oracle::OracleModule
    + crate::math::LendingMathModule
    + utils::LendingUtilsModule
    + common_events::EventsModule
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
            let egld_price = self.get_token_price_data(&token, &mut storage_cache);
            let usd_price = self
                .get_token_amount_in_dollars_raw(&egld_price.price, &storage_cache.egld_price_feed);

            markets.push(AssetExtendedConfigView {
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

    /// Calculates maximum amount of collateral that can be liquidated
    ///
    /// # Arguments
    /// * `account_position` - NFT nonce of the account position
    /// * `collateral_asset` - Token identifier of collateral to liquidate
    /// * `in_usd` - Whether to return amount in USD (true) or token units (false)
    ///
    /// # Returns
    /// * `BigUint` - Maximum liquidatable amount in USD or token units
    ///
    /// # Errors
    /// * `ERROR_HEALTH_FACTOR` - If position is not liquidatable (HF >= 100%)
    ///
    /// # Example
    /// ```
    /// // Position:
    /// // Collateral: 100 EGLD @ $100 each = $10,000
    /// // Borrows: 9000 USDC @ $1 each = $9,000
    /// // Health Factor: 90% (unhealthy)
    /// // Liquidation Bonus: 10%
    ///
    /// // In USD:
    /// get_max_liquidate_amount_for_collateral(1, "EGLD-123456", true) = 5000
    /// // Can liquidate $5,000 worth of collateral
    ///
    /// // In EGLD:
    /// get_max_liquidate_amount_for_collateral(1, "EGLD-123456", false) = 50
    /// // Can liquidate 50 EGLD
    /// ```
    // #[view(getMaxLiquidateAmountForCollateral)]
    // fn get_max_liquidate_amount_for_collateral(
    //     &self,
    //     account_position: u64,
    //     collateral_asset: &EgldOrEsdtTokenIdentifier,
    //     in_egld: bool,
    // ) -> BigUint {
    //     let bp = BigUint::from(BP);

    //     let borrowed_egld = self.get_total_borrow_in_egld(account_position);
    //     let collateral_in_egld = self.get_liquidation_collateral_available(account_position);
    //     let health_factor = self.compute_health_factor(&collateral_in_egld, &borrowed_egld);

    //     require!(health_factor < bp, ERROR_HEALTH_FACTOR);

    //     let asset_config = self.asset_config(collateral_asset).get();
    //     let nft_attributes = self.account_attributes(account_position).get();
    //     // Calculate collateral to receive with bonus

    //     let mut storage_cache = StorageCache::new(self);
    //     let feed = self.get_token_price_data(collateral_asset, &mut storage_cache);

    //     // Calculate liquidation amount using Dutch auction mechanism
    //     let (liquidation_amount_egld, liq_bonus) = self.calculate_single_asset_liquidation_amount(
    //         &borrowed_egld,
    //         &collateral_in_egld,
    //         collateral_asset,
    //         &feed,
    //         account_position,
    //         OptionalValue::None,
    //         &asset_config.liquidation_base_bonus,
    //         &health_factor,
    //     );
    //     // Convert USD value to collateral token amount
    //     let collateral_amount_before_bonus =
    //         self.compute_amount_in_tokens(&liquidation_amount_egld, &feed);

    //     if in_egld {
    //         liquidation_amount_egld
    //     } else {
    //         collateral_amount_before_bonus
    //     }
    // }

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
        self.get_total_borrow_in_egld_vec(&borrow_positions.values().collect(), &mut storage_cache)
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
        let (weighted_collateral, _, _) = self.get_summary_collateral_in_egld_vec(
            &deposit_positions.values().collect(),
            &mut storage_cache,
        );
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
        let (_, _, ltv_collateral_in_egld) = self.get_summary_collateral_in_egld_vec(
            &deposit_positions.values().collect(),
            &mut storage_cache,
        );
        ltv_collateral_in_egld
    }

    #[view(getTokenPriceData)]
    fn get_token_price_data_view(
        &self,
        token_id: &EgldOrEsdtTokenIdentifier,
    ) -> PriceFeedShort<Self::Api> {
        let mut storage_cache = StorageCache::new(self);
        self.get_token_price_data(token_id, &mut storage_cache)
    }

    #[view(getTokenPriceUSD)]
    fn get_usd_price(&self, token_id: &EgldOrEsdtTokenIdentifier) -> BigUint {
        let mut storage_cache = StorageCache::new(self);
        let data = self.get_token_price_data(token_id, &mut storage_cache);
        self.get_token_amount_in_dollars_raw(&data.price, &storage_cache.egld_price_feed)
    }

    #[view(getTokenPriceEGLD)]
    fn get_egld_price(&self, token_id: &EgldOrEsdtTokenIdentifier) -> BigUint {
        let mut storage_cache = StorageCache::new(self);
        let data = self.get_token_price_data(token_id, &mut storage_cache);
        data.price
    }
}
