#![no_std]
#![allow(clippy::too_many_arguments)]

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

pub mod liq_math;
pub use liq_math::*;
pub mod contexts;
pub mod liquidity;
pub mod view;
pub mod errors;
pub use common_structs::*;
pub use common_events::*;
pub use common_tokens::*;

pub mod liq_storage;
pub mod liq_utils;

#[multiversx_sc::contract]
pub trait LiquidityPool:
    liq_storage::StorageModule
    + common_tokens::AccountTokenModule
    + common_events::EventsModule
    + liq_math::MathModule
    + liquidity::LiquidityModule
    + liq_utils::UtilsModule
    + view::ViewModule
    + price_aggregator_proxy::PriceAggregatorModule
    + common_checks::ChecksModule
{
    #[init]
    fn init(
        &self,
        asset: &TokenIdentifier,
        r_base: &BigUint,
        r_slope1: &BigUint,
        r_slope2: &BigUint,
        u_optimal: &BigUint,
        reserve_factor: &BigUint,
        liquidation_threshold: &BigUint,
    ) {
        self.pool_asset().set(asset);
        self.pool_params().set(&PoolParams {
            r_base: r_base.clone(),
            r_slope1: r_slope1.clone(),
            r_slope2: r_slope2.clone(),
            u_optimal: u_optimal.clone(),
            reserve_factor: reserve_factor.clone(),
        });
        self.liquidation_threshold().set(liquidation_threshold);
        self.borrow_index().set(BigUint::from(BP));
        self.supply_index().set(BigUint::from(BP));
        self.rewards_reserves().set(BigUint::zero());
        self.borrow_index_last_update_round().set(0);
    }

    #[upgrade]
    fn upgrade(
        &self,
        r_base: BigUint,
        r_slope1: BigUint,
        r_slope2: BigUint,
        u_optimal: BigUint,
        reserve_factor: BigUint,
        liquidation_threshold: &BigUint,
    ) {
        self.pool_params().set(&PoolParams {
            r_base: r_base.clone(),
            r_slope1: r_slope1.clone(),
            r_slope2: r_slope2.clone(),
            u_optimal: u_optimal.clone(),
            reserve_factor: reserve_factor.clone(),
        });
        self.liquidation_threshold().set(liquidation_threshold);

        self.upgrade_market_event(
            &self.pool_asset().get(),
            &r_base,
            &r_slope1,
            &r_slope2,
            &u_optimal,
            &reserve_factor,
            &liquidation_threshold,
            &self.blockchain().get_sc_address(),
        );
    }
}
