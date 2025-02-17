#![no_std]

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

pub mod rates;
pub use rates::*;
pub mod contexts;
pub mod errors;
pub mod liquidity;
pub mod view;
pub use common_events::*;

pub mod storage;
pub mod utils;

#[multiversx_sc::contract]
pub trait LiquidityPool:
    storage::StorageModule
    + common_events::EventsModule
    + rates::InterestRateMath
    + liquidity::LiquidityModule
    + utils::UtilsModule
    + common_math::SharedMathModule
    + view::ViewModule
{
    /// Initializes the liquidity pool for a specific asset.
    ///
    /// This function sets the asset for the pool, initializes the interest rate parameters
    /// (maximum rate, base rate, slopes, optimal utilization, reserve factor) using a given decimal precision,
    /// and initializes both the borrow and supply indexes to the base point (BP). It also sets the protocol revenue
    /// to zero and records the current blockchain timestamp.
    ///
    /// # Parameters
    /// - `asset`: The asset identifier (EgldOrEsdtTokenIdentifier) for the pool.
    /// - `r_max`: The maximum borrow rate.
    /// - `r_base`: The base borrow rate.
    /// - `r_slope1`: The slope before optimal utilization.
    /// - `r_slope2`: The slope after optimal utilization.
    /// - `u_optimal`: The optimal utilization ratio.
    /// - `reserve_factor`: The fraction (reserve factor) of accrued interest reserved as protocol fee.
    /// - `decimals`: The number of decimals for the underlying asset.
    ///
    /// # Returns
    /// - Nothing.
    #[init]
    fn init(
        &self,
        asset: &EgldOrEsdtTokenIdentifier,
        r_max: BigUint,
        r_base: BigUint,
        r_slope1: BigUint,
        r_slope2: BigUint,
        u_optimal: BigUint,
        reserve_factor: BigUint,
        decimals: usize,
    ) {
        self.pool_asset().set(asset);
        self.pool_params().set(&PoolParams {
            r_max: self.to_decimal_ray(r_max),
            r_base: self.to_decimal_ray(r_base),
            r_slope1: self.to_decimal_ray(r_slope1),
            r_slope2: self.to_decimal_ray(r_slope2),
            u_optimal: self.to_decimal_ray(u_optimal),
            reserve_factor: self.to_decimal_bps(reserve_factor),
            decimals,
        });
        self.borrow_index().set(self.ray());

        self.supply_index().set(self.ray());

        self.supplied_amount()
            .set(ManagedDecimal::from_raw_units(BigUint::zero(), decimals));

        self.reserves()
            .set(ManagedDecimal::from_raw_units(BigUint::zero(), decimals));

        self.borrowed_amount()
            .set(ManagedDecimal::from_raw_units(BigUint::zero(), decimals));

        self.protocol_revenue()
            .set(ManagedDecimal::from_raw_units(BigUint::zero(), decimals));

        let timestamp = self.blockchain().get_block_timestamp();
        self.last_timestamp().set(timestamp);
    }

    /// Upgrades the liquidity pool parameters.
    ///
    /// This function updates the pool's interest rate parameters and reserve factor. It emits an event
    /// reflecting the new parameters, and then updates the on-chain pool parameters accordingly.
    ///
    /// # Parameters
    /// - `r_max`: The new maximum borrow rate.
    /// - `r_base`: The new base borrow rate.
    /// - `r_slope1`: The new slope before optimal utilization.
    /// - `r_slope2`: The new slope after optimal utilization.
    /// - `u_optimal`: The new optimal utilization ratio.
    /// - `reserve_factor`: The new reserve factor.
    ///
    /// # Returns
    /// - Nothing.
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
            pool_params.r_max = self.to_decimal_ray(r_max);
            pool_params.r_base = self.to_decimal_ray(r_base);
            pool_params.r_slope1 = self.to_decimal_ray(r_slope1);
            pool_params.r_slope2 = self.to_decimal_ray(r_slope2);
            pool_params.u_optimal = self.to_decimal_ray(u_optimal);
            pool_params.reserve_factor = self.to_decimal_bps(reserve_factor);
        });
    }
}
