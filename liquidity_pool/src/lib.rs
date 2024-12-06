#![no_std]
#![allow(clippy::too_many_arguments)]

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

pub mod liq_math;
pub use liq_math::*;
pub mod contexts;
pub mod errors;
pub mod liquidity;
pub mod view;
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
    + common_checks::ChecksModule
{
    #[init]
    fn init(
        &self,
        asset: &EgldOrEsdtTokenIdentifier,
        r_max: &ManagedDecimal<Self::Api, NumDecimals>,
        r_base: &ManagedDecimal<Self::Api, NumDecimals>,
        r_slope1: &ManagedDecimal<Self::Api, NumDecimals>,
        r_slope2: &ManagedDecimal<Self::Api, NumDecimals>,
        u_optimal: &ManagedDecimal<Self::Api, NumDecimals>,
        reserve_factor: &ManagedDecimal<Self::Api, NumDecimals>,
        decimals: usize,
    ) {
        self.pool_asset().set(asset);
        self.pool_params().set(&PoolParams {
            r_max: r_max.clone(),
            r_base: r_base.clone(),
            r_slope1: r_slope1.clone(),
            r_slope2: r_slope2.clone(),
            u_optimal: u_optimal.clone(),
            reserve_factor: reserve_factor.clone(),
            decimals,
        });
        self.borrow_index().set(ManagedDecimal::from_raw_units(
            BigUint::from(BP),
            DECIMAL_PRECISION,
        ));
        self.supply_index().set(ManagedDecimal::from_raw_units(
            BigUint::from(BP),
            DECIMAL_PRECISION,
        ));

        self.rewards_reserves().set(BigUint::from(0u64));
        self.last_update_timestamp().set(0);
    }

    #[upgrade]
    fn upgrade(
        &self,
        r_max: ManagedDecimal<Self::Api, NumDecimals>,
        r_base: ManagedDecimal<Self::Api, NumDecimals>,
        r_slope1: ManagedDecimal<Self::Api, NumDecimals>,
        r_slope2: ManagedDecimal<Self::Api, NumDecimals>,
        u_optimal: ManagedDecimal<Self::Api, NumDecimals>,
        reserve_factor: ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        self.market_params_event(
            &self.pool_asset().get(),
            &r_max,
            &r_base,
            &r_slope1,
            &r_slope2,
            &u_optimal,
            &reserve_factor,
        );

        self.pool_params().update(|pool_params| {
            pool_params.r_max = r_max;
            pool_params.r_base = r_base;
            pool_params.r_slope1 = r_slope1;
            pool_params.r_slope2 = r_slope2;
            pool_params.u_optimal = u_optimal;
            pool_params.reserve_factor = reserve_factor;
        });
    }
}
