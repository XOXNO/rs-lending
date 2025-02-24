#![no_std]

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

pub mod rates;
pub use rates::*;
pub mod contexts;
pub mod liquidity;
pub mod view;
pub use common_events::*;

pub mod storage;
pub mod utils;

#[multiversx_sc::contract]
pub trait LiquidityPool:
    storage::Storage
    + common_events::EventsModule
    + rates::InterestRates
    + liquidity::LiquidityModule
    + utils::UtilsModule
    + common_math::SharedMathModule
    + view::ViewModule
{
    /// Initializes the liquidity pool for a specific asset.
    ///
    /// **Purpose**: Sets up the initial state of the liquidity pool, including the asset, interest rate parameters,
    /// supply and borrow indexes, and other key variables, preparing it for lending operations.
    ///
    /// **Process**:
    /// 1. Stores the pool's asset identifier.
    /// 2. Configures interest rate parameters (`r_max`, `r_base`, `r_slope1`, `r_slope2`, `r_slope3`, `u_mid`, `u_optimal`, `reserve_factor`)
    ///    by converting `BigUint` inputs to `ManagedDecimal` with appropriate scaling (RAY for rates, BPS for reserve factor).
    /// 3. Initializes the borrow and supply indexes to `RAY` (representing 1.0 in the system's precision).
    /// 4. Sets initial values for supplied, reserves, borrowed, and revenue to zero, using the asset's decimal precision.
    /// 5. Records the current blockchain timestamp as the last update time.
    ///
    /// ### Parameters
    /// - `asset`: The asset identifier (`EgldOrEsdtTokenIdentifier`) for the pool.
    /// - `r_max`: Maximum borrow rate (`BigUint`), scaled to RAY precision.
    /// - `r_base`: Base borrow rate (`BigUint`), scaled to RAY precision.
    /// - `r_slope1`: Slope before optimal utilization (`BigUint`), scaled to RAY precision.
    /// - `r_slope2`: Slope after optimal utilization (`BigUint`), scaled to RAY precision.
    /// - `r_slope3`: Slope for high utilization (`BigUint`), scaled to RAY precision.
    /// - `u_mid`: Midpoint utilization ratio (`BigUint`), scaled to RAY precision.
    /// - `u_optimal`: Optimal utilization ratio (`BigUint`), scaled to RAY precision.
    /// - `reserve_factor`: Fraction of interest reserved as protocol fee (`BigUint`), scaled to BPS precision.
    /// - `decimals`: Number of decimals for the asset (`usize`).
    ///
    /// ### Returns
    /// - Nothing (void function).
    ///
    /// **Security Considerations**:
    /// - Ensures all critical state variables (asset, parameters, indexes, etc.) are initialized to prevent uninitialized storage vulnerabilities.
    /// - Uses precise decimal conversions (`to_decimal_ray` and `to_decimal_bps`) to maintain calculation accuracy.
    #[init]
    fn init(
        &self,
        asset: &EgldOrEsdtTokenIdentifier,
        r_max: BigUint,
        r_base: BigUint,
        r_slope1: BigUint,
        r_slope2: BigUint,
        r_slope3: BigUint,
        u_mid: BigUint,
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
            r_slope3: self.to_decimal_ray(r_slope3),
            u_mid: self.to_decimal_ray(u_mid),
            u_optimal: self.to_decimal_ray(u_optimal),
            reserve_factor: self.to_decimal_bps(reserve_factor),
            decimals,
        });
        self.borrow_index().set(self.ray());

        self.supply_index().set(self.ray());

        self.supplied()
            .set(ManagedDecimal::from_raw_units(BigUint::zero(), decimals));

        self.reserves()
            .set(ManagedDecimal::from_raw_units(BigUint::zero(), decimals));

        self.borrowed()
            .set(ManagedDecimal::from_raw_units(BigUint::zero(), decimals));

        self.revenue()
            .set(ManagedDecimal::from_raw_units(BigUint::zero(), decimals));

        let timestamp = self.blockchain().get_block_timestamp();
        self.last_timestamp().set(timestamp);
    }

    /// Upgrades the liquidity pool parameters.
    ///
    /// **Purpose**: Updates the pool's interest rate parameters and reserve factor to adapt to changing market conditions
    /// or protocol requirements, ensuring flexibility in pool management.
    ///
    /// **Process**:
    /// 1. Emits an event (`market_params_event`) with the new parameters for transparency and auditability.
    /// 2. Updates the existing pool parameters by converting `BigUint` inputs to `ManagedDecimal` with appropriate scaling.
    ///
    /// ### Parameters
    /// - `r_max`: New maximum borrow rate (`BigUint`), scaled to RAY precision.
    /// - `r_base`: New base borrow rate (`BigUint`), scaled to RAY precision.
    /// - `r_slope1`: New slope before optimal utilization (`BigUint`), scaled to RAY precision.
    /// - `r_slope2`: New slope after optimal utilization (`BigUint`), scaled to RAY precision.
    /// - `r_slope3`: New slope for high utilization (`BigUint`), scaled to RAY precision.
    /// - `u_mid`: New midpoint utilization ratio (`BigUint`), scaled to RAY precision.
    /// - `u_optimal`: New optimal utilization ratio (`BigUint`), scaled to RAY precision.
    /// - `reserve_factor`: New fraction of interest reserved as protocol fee (`BigUint`), scaled to BPS precision.
    ///
    /// ### Returns
    /// - Nothing (void function).
    ///
    /// **Security Considerations**:
    /// - Restricted to the contract owner (via the `#[upgrade]` attribute) to prevent unauthorized modifications.
    /// - Uses precise decimal conversions (`to_decimal_ray` and `to_decimal_bps`) to ensure consistency in calculations.
    /// - Logs changes via an event, enabling tracking and verification of updates.
    #[upgrade]
    fn upgrade(
        &self,
        r_max: BigUint,
        r_base: BigUint,
        r_slope1: BigUint,
        r_slope2: BigUint,
        r_slope3: BigUint,
        u_mid: BigUint,
        u_optimal: BigUint,
        reserve_factor: BigUint,
    ) {
        self.market_params_event(
            &self.pool_asset().get(),
            &r_max,
            &r_base,
            &r_slope1,
            &r_slope2,
            &r_slope3,
            &u_mid,
            &u_optimal,
            &reserve_factor,
        );

        self.pool_params().update(|pool_params| {
            pool_params.r_max = self.to_decimal_ray(r_max);
            pool_params.r_base = self.to_decimal_ray(r_base);
            pool_params.r_slope1 = self.to_decimal_ray(r_slope1);
            pool_params.r_slope2 = self.to_decimal_ray(r_slope2);
            pool_params.r_slope3 = self.to_decimal_ray(r_slope3);
            pool_params.u_mid = self.to_decimal_ray(u_mid);
            pool_params.u_optimal = self.to_decimal_ray(u_optimal);
            pool_params.reserve_factor = self.to_decimal_bps(reserve_factor);
        });
    }
}
