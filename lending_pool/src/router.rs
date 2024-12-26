#![allow(clippy::too_many_arguments)]

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use common_events::AssetConfig;

use crate::{
    contexts::base::StorageCache, math, oracle, proxy_accumulator, proxy_pool, storage, utils,
    ERROR_ASSET_ALREADY_SUPPORTED, ERROR_INVALID_TICKER, ERROR_NO_ACCUMULATOR_FOUND,
    ERROR_NO_POOL_FOUND,
};

use super::factory;

#[multiversx_sc::module]
pub trait RouterModule:
    factory::FactoryModule
    + storage::LendingStorageModule
    + common_events::EventsModule
    + oracle::OracleModule
    + utils::LendingUtilsModule
    + math::LendingMathModule
{
    #[allow_multiple_var_args]
    #[only_owner]
    #[endpoint(createLiquidityPool)]
    fn create_liquidity_pool(
        &self,
        base_asset: EgldOrEsdtTokenIdentifier,
        r_max: BigUint,
        r_base: BigUint,
        r_slope1: BigUint,
        r_slope2: BigUint,
        u_optimal: BigUint,
        reserve_factor: BigUint,
        ltv: BigUint,
        liquidation_threshold: BigUint,
        liquidation_base_bonus: BigUint,
        liquidation_max_fee: BigUint,
        can_be_collateral: bool,
        can_be_borrowed: bool,
        is_isolated: bool,
        debt_ceiling_usd: BigUint,
        flash_loan_fee: BigUint,
        is_siloed: bool,
        flashloan_enabled: bool,
        can_borrow_in_isolation: bool,
        borrow_cap: OptionalValue<BigUint>,
        supply_cap: OptionalValue<BigUint>,
    ) -> ManagedAddress {
        require!(
            self.pools_map(&base_asset).is_empty(),
            ERROR_ASSET_ALREADY_SUPPORTED
        );
        require!(base_asset.is_valid(), ERROR_INVALID_TICKER);

        let address = self.create_pool(
            &base_asset,
            &r_max,
            &r_base,
            &r_slope1,
            &r_slope2,
            &u_optimal,
            &reserve_factor,
        );

        self.require_non_zero_address(&address);

        self.pools_map(&base_asset).set(address.clone());
        self.pools_allowed().insert(address.clone());

        let asset_config = &AssetConfig {
            ltv,
            liquidation_threshold,
            liquidation_base_bonus,
            liquidation_max_fee,
            borrow_cap: borrow_cap.into_option(),
            supply_cap: supply_cap.into_option(),
            can_be_collateral,
            can_be_borrowed,
            is_e_mode_enabled: false,
            is_isolated,
            debt_ceiling_usd,
            flash_loan_fee,
            is_siloed,
            flashloan_enabled,
            can_borrow_in_isolation,
        };

        self.asset_config(&base_asset).set(asset_config);

        self.create_market_params_event(
            &base_asset,
            &r_max,
            &r_base,
            &r_slope1,
            &r_slope2,
            &u_optimal,
            &reserve_factor,
            &address,
            asset_config,
        );
        address
    }

    #[only_owner]
    #[endpoint(upgradeLiquidityPool)]
    fn upgrade_liquidity_pool(
        &self,
        base_asset: &EgldOrEsdtTokenIdentifier,
        r_max: BigUint,
        r_base: BigUint,
        r_slope1: BigUint,
        r_slope2: BigUint,
        u_optimal: BigUint,
        reserve_factor: BigUint,
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
        );
    }

    /// Claim revenue from the accumulator
    ///
    /// This function is used to claim the revenue from the liquidity pools
    /// It iterates over the assets and claims the revenue
    /// The revenue is deposited into the accumulator
    #[only_owner]
    #[endpoint(claimRevenue)]
    fn claim_revenue(&self, assets: MultiValueEncoded<EgldOrEsdtTokenIdentifier>) {
        let mut storage_cache = StorageCache::new(self);
        let accumulator_address_mapper = self.accumulator_address();

        require!(
            !accumulator_address_mapper.is_empty(),
            ERROR_NO_ACCUMULATOR_FOUND
        );

        let accumulator_address = accumulator_address_mapper.get();
        for asset in assets {
            let pool_address = self.get_pool_address(&asset);
            let data = self.get_token_price_data(&asset, &mut storage_cache);
            let revenue = self
                .tx()
                .to(pool_address)
                .typed(proxy_pool::LiquidityPoolProxy)
                .claim_revenue(&data.price)
                .returns(ReturnsResult)
                .sync_call();

            if revenue.amount > 0 {
                self.tx()
                    .to(&accumulator_address)
                    .typed(proxy_accumulator::AccumulatorProxy)
                    .deposit()
                    .payment(revenue)
                    .returns(ReturnsResult)
                    .sync_call();
            }
        }
    }
}
