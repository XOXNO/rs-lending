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
        r_max: &BigUint,
        r_base: &BigUint,
        r_slope1: &BigUint,
        r_slope2: &BigUint,
        u_optimal: &BigUint,
        reserve_factor: &BigUint,
        decimals: usize,
    ) {
        self.pool_asset().set(asset);
        self.pool_params().set(&PoolParams {
            r_max: ManagedDecimal::from_raw_units(r_max.clone(), DECIMAL_PRECISION),
            r_base: ManagedDecimal::from_raw_units(r_base.clone(), DECIMAL_PRECISION),
            r_slope1: ManagedDecimal::from_raw_units(r_slope1.clone(), DECIMAL_PRECISION),
            r_slope2: ManagedDecimal::from_raw_units(r_slope2.clone(), DECIMAL_PRECISION),
            u_optimal: ManagedDecimal::from_raw_units(u_optimal.clone(), DECIMAL_PRECISION),
            reserve_factor: ManagedDecimal::from_raw_units(
                reserve_factor.clone(),
                DECIMAL_PRECISION,
            ),
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
        r_max: BigUint,
        r_base: BigUint,
        r_slope1: BigUint,
        r_slope2: BigUint,
        u_optimal: BigUint,
        reserve_factor: BigUint,
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
            pool_params.r_max = ManagedDecimal::from_raw_units(r_max.clone(), DECIMAL_PRECISION);
            pool_params.r_base = ManagedDecimal::from_raw_units(r_base.clone(), DECIMAL_PRECISION);
            pool_params.r_slope1 = ManagedDecimal::from_raw_units(r_slope1.clone(), DECIMAL_PRECISION);
            pool_params.r_slope2 = ManagedDecimal::from_raw_units(r_slope2.clone(), DECIMAL_PRECISION);
            pool_params.u_optimal = ManagedDecimal::from_raw_units(u_optimal.clone(), DECIMAL_PRECISION);
            pool_params.reserve_factor = ManagedDecimal::from_raw_units(reserve_factor.clone(), DECIMAL_PRECISION);
        });
    }
}
