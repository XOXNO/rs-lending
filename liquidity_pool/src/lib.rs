#![no_std]

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

pub mod rates;
use common_constants::{BP, DECIMAL_PRECISION};
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
