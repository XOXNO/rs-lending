#![no_std]
#![allow(clippy::too_many_arguments)]

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

pub mod liq_math;
use common_constants::{BP, DECIMAL_PRECISION};
pub use liq_math::*;
pub mod contexts;
pub mod errors;
pub mod liquidity;
pub mod view;
pub use common_events::*;

pub mod liq_storage;
pub mod liq_utils;

#[multiversx_sc::contract]
pub trait LiquidityPool:
    liq_storage::StorageModule
    + common_events::EventsModule
    + liq_math::MathModule
    + liquidity::LiquidityModule
    + liq_utils::UtilsModule
    + view::ViewModule
{
    // Initialize the pool
    // # Parameters
    // - `asset`: The asset of the pool.
    // - `r_max`: The maximum borrow rate.
    // - `r_base`: The base borrow rate.
    // - `r_slope1`: The slope of the borrow rate before the optimal utilization.
    // - `r_slope2`: The slope of the borrow rate after the optimal utilization.
    // - `u_optimal`: The optimal utilization ratio.
    // - `reserve_factor`: The reserve factor.
    // - `decimals`: The number of decimals of the asset.
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

        self.protocol_revenue().set(BigUint::zero());
        self.last_update_timestamp()
            .set(self.blockchain().get_block_timestamp());
    }

    // Upgrade the pool
    // # Parameters
    // - `r_max`: The maximum borrow rate.
    // - `r_base`: The base borrow rate.
    // - `r_slope1`: The slope of the borrow rate before the optimal utilization.
    // - `r_slope2`: The slope of the borrow rate after the optimal utilization.
    // - `u_optimal`: The optimal utilization ratio.
    // - `reserve_factor`: The reserve factor.
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
            pool_params.r_slope1 =
                ManagedDecimal::from_raw_units(r_slope1.clone(), DECIMAL_PRECISION);
            pool_params.r_slope2 =
                ManagedDecimal::from_raw_units(r_slope2.clone(), DECIMAL_PRECISION);
            pool_params.u_optimal =
                ManagedDecimal::from_raw_units(u_optimal.clone(), DECIMAL_PRECISION);
            pool_params.reserve_factor =
                ManagedDecimal::from_raw_units(reserve_factor.clone(), DECIMAL_PRECISION);
        });
    }
}
