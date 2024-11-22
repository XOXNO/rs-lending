#![allow(clippy::too_many_arguments)]

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use common_events::UpdateAssetParamsType;

use crate::{
    storage, ERROR_ASSET_ALREADY_SUPPORTED, ERROR_INVALID_LIQUIDATION_THRESHOLD, ERROR_INVALID_LTV,
    ERROR_INVALID_TICKER, ERROR_NO_POOL_FOUND,
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
        r_max: BigUint,
        r_base: BigUint,
        r_slope1: BigUint,
        r_slope2: BigUint,
        u_optimal: BigUint,
        reserve_factor: BigUint,
        ltv: &BigUint,
        liquidation_threshold: &BigUint,
        liq_bonus: &BigUint,
        protocol_liquidation_fee: &BigUint,
        borrow_cap: &BigUint,
        supply_cap: &BigUint,
    ) -> ManagedAddress {
        require!(
            !self.pools_map(&base_asset).is_empty(),
            ERROR_ASSET_ALREADY_SUPPORTED
        );
        require!(base_asset.is_valid_esdt_identifier(), ERROR_INVALID_TICKER);

        let address = self.create_pool(
            &base_asset,
            &r_max,
            &r_base,
            &r_slope1,
            &r_slope2,
            &u_optimal,
            &reserve_factor,
            &protocol_liquidation_fee,
            &borrow_cap,
            &supply_cap,
        );

        self.require_non_zero_address(&address);

        self.pools_map(&base_asset).set(address.clone());
        self.pools_allowed().insert(address.clone());

        self.set_asset_loan_to_value(&base_asset, ltv);
        self.set_asset_liquidation_bonus(&base_asset, liq_bonus);
        self.set_asset_liquidation_threshold(&base_asset, liquidation_threshold);
        self.create_market_params_event(
            &base_asset,
            &r_max,
            &r_base,
            &r_slope1,
            &r_slope2,
            &u_optimal,
            &reserve_factor,
            &protocol_liquidation_fee,
            &address,
            &ltv,
            &liquidation_threshold,
            &liq_bonus,
            &borrow_cap,
            &supply_cap,
        );
        address
    }

    #[only_owner]
    #[endpoint(upgradeLiquidityPool)]
    fn upgrade_liquidity_pool(
        &self,
        base_asset: &TokenIdentifier,
        r_max: BigUint,
        r_base: BigUint,
        r_slope1: BigUint,
        r_slope2: BigUint,
        u_optimal: BigUint,
        reserve_factor: BigUint,
        protocol_liquidation_fee: BigUint,
        borrow_cap: BigUint,
        supply_cap: BigUint,
    ) {
        require!(!self.pools_map(base_asset).is_empty(), ERROR_NO_POOL_FOUND);

        let pool_address = self.get_pool_address(base_asset);
        self.upgrade_pool(
            pool_address,
            r_max,
            r_base,
            r_slope1,
            r_slope2,
            u_optimal,
            reserve_factor,
            protocol_liquidation_fee,
            borrow_cap,
            supply_cap,
        );
    }

    #[only_owner]
    #[endpoint(setAggregator)]
    fn set_aggregator(&self, aggregator: ManagedAddress) {
        self.price_aggregator_address().set(&aggregator);
    }

    #[only_owner]
    #[endpoint(setAssetLoanToValue)]
    fn set_asset_loan_to_value(&self, asset: &TokenIdentifier, loan_to_value: &BigUint) {
        let liquidation_threshold = self.asset_liquidation_threshold(asset);

        if !liquidation_threshold.is_empty() {
            require!(
                loan_to_value < &liquidation_threshold.get(),
                ERROR_INVALID_LTV
            );
        }
        self.asset_loan_to_value(asset).set(loan_to_value);
        self.update_asset_params_event(asset, loan_to_value, UpdateAssetParamsType::LTV);
    }

    #[only_owner]
    #[endpoint(setAssetLiquidationBonus)]
    fn set_asset_liquidation_bonus(&self, asset: &TokenIdentifier, liq_bonus: &BigUint) {
        self.asset_liquidation_bonus(asset).set(liq_bonus);
        self.update_asset_params_event(asset, liq_bonus, UpdateAssetParamsType::LiquidationBonus);
    }

    #[only_owner]
    #[endpoint(setAssetLiquidationThreshold)]
    fn set_asset_liquidation_threshold(
        &self,
        asset: &TokenIdentifier,
        liquidation_threshold: &BigUint,
    ) {
        let ltv = self.asset_loan_to_value(asset).get();
        require!(
            liquidation_threshold < &ltv,
            ERROR_INVALID_LIQUIDATION_THRESHOLD
        );

        self.asset_liquidation_threshold(asset)
            .set(liquidation_threshold);

        self.update_asset_params_event(
            asset,
            liquidation_threshold,
            UpdateAssetParamsType::LiquidationThreshold,
        );
    }

    #[view(getPoolAddress)]
    fn get_pool_address(&self, asset: &TokenIdentifier) -> ManagedAddress {
        let pool_address = self.pools_map(asset).get();

        require!(!pool_address.is_zero(), ERROR_NO_POOL_FOUND);

        pool_address
    }
}
