#![allow(clippy::too_many_arguments)]

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use crate::{
    proxy_pool, storage, ERROR_ASSET_ALREADY_SUPPORTED, ERROR_INVALID_TICKER, ERROR_NO_POOL_FOUND,
};

use super::factory;

#[multiversx_sc::module]
pub trait RouterModule:
    factory::FactoryModule
    + common_checks::ChecksModule
    + storage::LendingStorageModule
    + common_events::EventsModule
{
    #[only_owner]
    #[endpoint(createLiquidityPool)]
    fn create_liquidity_pool(
        &self,
        base_asset: TokenIdentifier,
        r_base: BigUint,
        r_slope1: BigUint,
        r_slope2: BigUint,
        u_optimal: BigUint,
        reserve_factor: BigUint,
        liquidation_threshold: BigUint,
    ) -> ManagedAddress {
        require!(
            !self.pools_map(&base_asset).is_empty(),
            ERROR_ASSET_ALREADY_SUPPORTED
        );
        require!(base_asset.is_valid_esdt_identifier(), ERROR_INVALID_TICKER);

        let address = self.create_pool(
            &base_asset,
            &r_base,
            &r_slope1,
            &r_slope2,
            &u_optimal,
            &reserve_factor,
            &liquidation_threshold,
        );

        self.require_non_zero_address(&address);

        self.pools_map(&base_asset).set(address.clone());
        self.pools_allowed().insert(address.clone());
        address
    }

    #[only_owner]
    #[endpoint(upgradeLiquidityPool)]
    fn upgrade_liquidity_pool(
        &self,
        base_asset: TokenIdentifier,
        r_base: BigUint,
        r_slope1: BigUint,
        r_slope2: BigUint,
        u_optimal: BigUint,
        reserve_factor: BigUint,
        liquidation_threshold: BigUint,
    ) {
        require!(!self.pools_map(&base_asset).is_empty(), ERROR_NO_POOL_FOUND);

        let pool_address = self.get_pool_address(&base_asset);
        self.upgrade_pool(
            pool_address,
            r_base,
            r_slope1,
            r_slope2,
            u_optimal,
            reserve_factor,
            liquidation_threshold,
        );
    }

    #[only_owner]
    #[endpoint(setAggregator)]
    fn set_aggregator(&self, pool_asset_id: TokenIdentifier, aggregator: ManagedAddress) {
        let pool_address = self.get_pool_address(&pool_asset_id);

        let _: IgnoreValue = self
            .tx()
            .to(pool_address)
            .typed(proxy_pool::LiquidityPoolProxy)
            .set_price_aggregator_address(aggregator)
            .execute_on_dest_context();
    }

    #[only_owner]
    #[endpoint(setAssetLoanToValue)]
    fn set_asset_loan_to_value(&self, asset: TokenIdentifier, loan_to_value: BigUint) {
        self.asset_loan_to_value(&asset).set(&loan_to_value);
    }

    #[only_owner]
    #[endpoint(setAssetLiquidationBonus)]
    fn set_asset_liquidation_bonus(&self, asset: TokenIdentifier, liq_bonus: BigUint) {
        self.asset_liquidation_bonus(&asset).set(&liq_bonus);
    }

    #[view(getPoolAddress)]
    fn get_pool_address(&self, asset: &TokenIdentifier) -> ManagedAddress {
        let pool_address = self.pools_map(asset).get();

        require!(!pool_address.is_zero(), ERROR_NO_POOL_FOUND);

        pool_address
    }
}
