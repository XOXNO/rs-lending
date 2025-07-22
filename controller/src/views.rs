use common_constants::WAD_PRECISION;
use common_structs::{
    AccountPositionType, AssetExtendedConfigView, LiquidationEstimate, MarketIndexView,
};

use crate::{cache::Cache, helpers, oracle, positions, storage, utils, validation};

multiversx_sc::imports!();

#[multiversx_sc::module]
pub trait ViewsModule:
    storage::Storage
    + oracle::OracleModule
    + utils::LendingUtilsModule
    + common_events::EventsModule
    + helpers::MathsModule
    + common_math::SharedMathModule
    + common_rates::InterestRates
    + positions::liquidation::PositionLiquidationModule
    + positions::repay::PositionRepayModule
    + positions::withdraw::PositionWithdrawModule
    + positions::update::PositionUpdateModule
    + positions::borrow::PositionBorrowModule
    + positions::account::PositionAccountModule
    + positions::emode::EModeModule
    + validation::ValidationModule
{
    #[view(liquidationEstimations)]
    fn liquidation_estimations(
        &self,
        account_nonce: u64,
        debt_payments: &ManagedVec<EgldOrEsdtTokenPayment<Self::Api>>,
    ) -> LiquidationEstimate<Self::Api> {
        let mut cache = Cache::new(self);
        self.require_active_account(account_nonce);

        let (collaterals, _, refunds, max_egld_payment, bonus_rate) =
            self.execute_liquidation(account_nonce, debt_payments, &mut cache);

        let mut seized_collaterals = ManagedVec::new();
        let mut protocol_fees = ManagedVec::new();
        for collateral in collaterals {
            let (seized_collateral, fees) = collateral.into_tuple();
            let collateral_view = EgldOrEsdtTokenPayment::new(
                seized_collateral.token_identifier.clone(),
                seized_collateral.token_nonce,
                seized_collateral.amount,
            );
            let protocol_fees_view = EgldOrEsdtTokenPayment::new(
                seized_collateral.token_identifier,
                seized_collateral.token_nonce,
                fees.into_raw_units().clone(),
            );
            seized_collaterals.push(collateral_view);
            protocol_fees.push(protocol_fees_view);
        }

        LiquidationEstimate {
            seized_collaterals,
            protocol_fees,
            refunds,
            max_egld_payment,
            bonus_rate,
        }
    }

    #[view(getAllMarketIndexes)]
    fn get_all_market_indexes(
        &self,
        assets: MultiValueEncoded<EgldOrEsdtTokenIdentifier>,
    ) -> ManagedVec<MarketIndexView<Self::Api>> {
        let mut cache = Cache::new(self);
        let mut markets = ManagedVec::new();
        for asset in assets {
            let indexes = self.update_asset_index(&asset, &mut cache, true);
            let feed = self.get_token_price(&asset, &mut cache);
            let usd = self.get_egld_usd_value(&feed.price, &cache.egld_usd_price);

            markets.push(MarketIndexView {
                asset_id: asset,
                supply_index: indexes.supply_index,
                borrow_index: indexes.borrow_index,
                egld_price: feed.price,
                usd_price: usd,
            });
        }
        markets
    }

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
            let usd = self.get_egld_usd_value(&feed.price, &cache.egld_usd_price);

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
        health_factor < self.ray()
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
        let mut cache = Cache::new(self);
        let deposit_positions = self.positions(account_nonce, AccountPositionType::Deposit);

        let (weighted_collateral, _, _) =
            self.calculate_collateral_values(&deposit_positions.values().collect(), &mut cache);

        let borrow_positions = self
            .positions(account_nonce, AccountPositionType::Borrow)
            .values()
            .collect();

        let total_borrow_ray = self.calculate_total_borrow_in_egld(&borrow_positions, &mut cache);

        self.compute_health_factor(&weighted_collateral, &total_borrow_ray)
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
        let mut cache = Cache::new(self);
        let feed = self.get_token_price(token_id, &mut cache);
        match self
            .positions(account_nonce, AccountPositionType::Deposit)
            .get(token_id)
        {
            Some(dp) => self.get_total_amount(&dp, &feed, &mut cache),
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
        let mut cache = Cache::new(self);
        cache.allow_unsafe_price = false;
        let feed = self.get_token_price(token_id, &mut cache);
        match self
            .positions(account_nonce, AccountPositionType::Borrow)
            .get(token_id)
        {
            Some(bp) => self.get_total_amount(&bp, &feed, &mut cache),
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
        let borrow_positions = self
            .positions(account_nonce, AccountPositionType::Borrow)
            .values()
            .collect();
        let total_borrow_ray = self.calculate_total_borrow_in_egld(&borrow_positions, &mut cache);

        self.rescale_half_up(&total_borrow_ray, WAD_PRECISION)
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
        let deposit_positions = self.positions(account_nonce, AccountPositionType::Deposit);

        let mut cache = Cache::new(self);
        cache.allow_unsafe_price = false;

        deposit_positions.values().fold(self.wad_zero(), |acc, dp| {
            let feed = self.get_token_price(&dp.asset_id, &mut cache);
            let amount = self.get_total_amount(&dp, &feed, &mut cache);
            acc + self.get_token_egld_value(&amount, &feed.price)
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
        let deposit_positions = self.positions(account_nonce, AccountPositionType::Deposit);

        let mut cache = Cache::new(self);

        let (weighted_collateral, _, _) =
            self.calculate_collateral_values(&deposit_positions.values().collect(), &mut cache);

        self.rescale_half_up(&weighted_collateral, WAD_PRECISION)
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
        let deposit_positions = self.positions(account_nonce, AccountPositionType::Deposit);

        let mut cache = Cache::new(self);

        let (_, _, ltv_collateral) =
            self.calculate_collateral_values(&deposit_positions.values().collect(), &mut cache);

        self.rescale_half_up(&ltv_collateral, WAD_PRECISION)
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

        self.get_egld_usd_value(&data.price, &cache.egld_usd_price)
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
