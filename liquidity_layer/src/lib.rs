#![no_std]

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

pub mod rates;
use common_errors::{
    ERROR_INVALID_BORROW_RATE_PARAMS, ERROR_INVALID_RESERVE_FACTOR,
    ERROR_INVALID_UTILIZATION_RANGE, ERROR_OPTIMAL_UTILIZATION_TOO_HIGH,
};
pub use rates::*;
pub mod cache;
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
    /// 2. Configures interest rate parameters (`max_borrow_rate`, `base_borrow_rate`, `slope1`, `slope2`, `slope3`, `mid_utilization`, `optimal_utilization`, `reserve_factor`)
    ///    by converting `BigUint` inputs to `ManagedDecimal` with appropriate scaling (RAY for rates, BPS for reserve factor).
    /// 3. Initializes the borrow and supply indexes to `RAY` (representing 1.0 in the system's precision).
    /// 4. Sets initial values for supplied, reserves, borrowed, and revenue to zero, using the asset's decimal precision.
    /// 5. Records the current blockchain timestamp as the last update time.
    ///
    /// ### Parameters
    /// - `asset`: The asset identifier (`EgldOrEsdtTokenIdentifier`) for the pool.
    /// - `max_borrow_rate`: Maximum borrow rate (`BigUint`), scaled to RAY precision.
    /// - `base_borrow_rate`: Base borrow rate (`BigUint`), scaled to RAY precision.
    /// - `slope1`: Slope before optimal utilization (`BigUint`), scaled to RAY precision.
    /// - `slope2`: Slope after optimal utilization (`BigUint`), scaled to RAY precision.
    /// - `slope3`: Slope for high utilization (`BigUint`), scaled to RAY precision.
    /// - `mid_utilization`: Midpoint utilization ratio (`BigUint`), scaled to RAY precision.
    /// - `optimal_utilization`: Optimal utilization ratio (`BigUint`), scaled to RAY precision.
    /// - `reserve_factor`: Fraction of interest reserved as protocol fee (`BigUint`), scaled to BPS precision.
    /// - `asset_decimals`: Number of asset_decimals for the asset (`usize`).
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
        max_borrow_rate: BigUint,
        base_borrow_rate: BigUint,
        slope1: BigUint,
        slope2: BigUint,
        slope3: BigUint,
        mid_utilization: BigUint,
        optimal_utilization: BigUint,
        reserve_factor: BigUint,
        asset_decimals: usize,
    ) {
        self.pool_asset().set(asset);
        let params = &MarketParams {
            max_borrow_rate: self.to_decimal_ray(max_borrow_rate),
            base_borrow_rate: self.to_decimal_ray(base_borrow_rate),
            slope1: self.to_decimal_ray(slope1),
            slope2: self.to_decimal_ray(slope2),
            slope3: self.to_decimal_ray(slope3),
            mid_utilization: self.to_decimal_ray(mid_utilization),
            optimal_utilization: self.to_decimal_ray(optimal_utilization),
            reserve_factor: self.to_decimal_bps(reserve_factor),
            asset_decimals,
        };

        require!(
            params.max_borrow_rate > params.base_borrow_rate,
            ERROR_INVALID_BORROW_RATE_PARAMS
        );
        require!(
            params.optimal_utilization > params.mid_utilization,
            ERROR_INVALID_UTILIZATION_RANGE
        );
        require!(
            params.optimal_utilization < self.ray(),
            ERROR_OPTIMAL_UTILIZATION_TOO_HIGH
        );
        require!(
            params.reserve_factor < self.bps(),
            ERROR_INVALID_RESERVE_FACTOR
        );

        self.params().set(params);
        self.borrow_index().set(self.ray());
        self.supply_index().set(self.ray());

        self.supplied()
            .set(self.ray_zero());

        self.borrowed()
            .set(self.ray_zero());

        self.revenue()
            .set(self.to_decimal(BigUint::zero(), asset_decimals));

        self.bad_debt()
            .set(self.to_decimal(BigUint::zero(), asset_decimals));

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
    /// - `max_borrow_rate`: New maximum borrow rate (`BigUint`), scaled to RAY precision.
    /// - `base_borrow_rate`: New base borrow rate (`BigUint`), scaled to RAY precision.
    /// - `slope1`: New slope before optimal utilization (`BigUint`), scaled to RAY precision.
    /// - `slope2`: New slope after optimal utilization (`BigUint`), scaled to RAY precision.
    /// - `slope3`: New slope for high utilization (`BigUint`), scaled to RAY precision.
    /// - `mid_utilization`: New midpoint utilization ratio (`BigUint`), scaled to RAY precision.
    /// - `optimal_utilization`: New optimal utilization ratio (`BigUint`), scaled to RAY precision.
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
        max_borrow_rate: BigUint,
        base_borrow_rate: BigUint,
        slope1: BigUint,
        slope2: BigUint,
        slope3: BigUint,
        mid_utilization: BigUint,
        optimal_utilization: BigUint,
        reserve_factor: BigUint,
    ) {
        self.market_params_event(
            &self.pool_asset().get(),
            &max_borrow_rate,
            &base_borrow_rate,
            &slope1,
            &slope2,
            &slope3,
            &mid_utilization,
            &optimal_utilization,
            &reserve_factor,
        );

        self.params().update(|params| {
            params.max_borrow_rate = self.to_decimal_ray(max_borrow_rate);
            params.base_borrow_rate = self.to_decimal_ray(base_borrow_rate);
            params.slope1 = self.to_decimal_ray(slope1);
            params.slope2 = self.to_decimal_ray(slope2);
            params.slope3 = self.to_decimal_ray(slope3);
            params.mid_utilization = self.to_decimal_ray(mid_utilization);
            params.optimal_utilization = self.to_decimal_ray(optimal_utilization);
            params.reserve_factor = self.to_decimal_bps(reserve_factor);
            require!(
                params.max_borrow_rate > params.base_borrow_rate,
                ERROR_INVALID_BORROW_RATE_PARAMS
            );
            require!(
                params.optimal_utilization > params.mid_utilization,
                ERROR_INVALID_UTILIZATION_RANGE
            );
            require!(
                params.optimal_utilization < self.ray(),
                ERROR_OPTIMAL_UTILIZATION_TOO_HIGH
            );
            require!(
                params.reserve_factor < self.bps(),
                ERROR_INVALID_RESERVE_FACTOR
            );
        });

        // let current_balance = self
        //     .blockchain()
        //     .get_sc_balance(&self.pool_asset().get(), 0);

        // if current_balance > BigUint::zero() {
        //     self.tx()
        //         .to(self.blockchain().get_caller())
        //         .egld_or_single_esdt(&self.pool_asset().get(), 0, &current_balance)
        //         .transfer();
        // }
    }
}
