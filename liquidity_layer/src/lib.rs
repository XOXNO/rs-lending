#![no_std]

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use cache::Cache;
use common_errors::{
    ERROR_INVALID_BORROW_RATE_PARAMS, ERROR_INVALID_RESERVE_FACTOR,
    ERROR_INVALID_UTILIZATION_RANGE, ERROR_OPTIMAL_UTILIZATION_TOO_HIGH,
};
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
    + common_rates::InterestRates
    + liquidity::LiquidityModule
    + utils::UtilsModule
    + common_math::SharedMathModule
    + view::ViewModule
{
    /// Handles contract upgrade with empty implementation.
    /// Allows code updates without state migration requirements.
    #[upgrade]
    fn upgrade(&self) {}
    /// Initializes a new liquidity pool with asset configuration and interest rate parameters.
    /// Sets up initial indexes (RAY), validates rate parameters, and records pool asset details.
    /// All supplied/borrowed/revenue amounts start at zero.
    #[init]
    fn init(
        &self,
        asset: EgldOrEsdtTokenIdentifier,
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
        let parameters = &MarketParams {
            max_borrow_rate_ray: self.to_decimal_ray(max_borrow_rate),
            base_borrow_rate_ray: self.to_decimal_ray(base_borrow_rate),
            slope1_ray: self.to_decimal_ray(slope1),
            slope2_ray: self.to_decimal_ray(slope2),
            slope3_ray: self.to_decimal_ray(slope3),
            mid_utilization_ray: self.to_decimal_ray(mid_utilization),
            optimal_utilization_ray: self.to_decimal_ray(optimal_utilization),
            reserve_factor_bps: self.to_decimal_bps(reserve_factor),
            asset_id: asset,
            asset_decimals,
        };

        require!(
            parameters.max_borrow_rate_ray > parameters.base_borrow_rate_ray,
            ERROR_INVALID_BORROW_RATE_PARAMS
        );
        require!(
            parameters.optimal_utilization_ray > parameters.mid_utilization_ray,
            ERROR_INVALID_UTILIZATION_RANGE
        );
        require!(
            parameters.optimal_utilization_ray < self.ray(),
            ERROR_OPTIMAL_UTILIZATION_TOO_HIGH
        );
        require!(
            parameters.reserve_factor_bps < self.bps(),
            ERROR_INVALID_RESERVE_FACTOR
        );

        self.parameters().set(parameters);
        self.borrow_index().set(self.ray());
        self.supply_index().set(self.ray());

        self.supplied().set(self.ray_zero());

        self.borrowed().set(self.ray_zero());

        self.revenue().set(self.ray_zero());

        let timestamp_ms = self.blockchain().get_block_timestamp_ms();
        self.last_timestamp().set(timestamp_ms);
    }

    /// Updates pool interest rate parameters and reserve factor.
    /// Validates new parameters and emits event for transparency.
    /// Only callable by owner.
    #[only_owner]
    #[endpoint(updateParams)]
    fn update_params(
        &self,
        max_borrow_rate: BigUint,
        base_borrow_rate: BigUint,
        slope1: BigUint,
        slope2: BigUint,
        slope3: BigUint,
        mid_utilization: BigUint,
        optimal_utilization: BigUint,
        reserve_factor: BigUint,
        asset_price: ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        let mut cache = Cache::new(self);
        self.global_sync(&mut cache);
        self.emit_market_update(&cache, &asset_price);

        self.parameters().update(|parameters| {
            self.market_params_event(
                &parameters.asset_id,
                &max_borrow_rate,
                &base_borrow_rate,
                &slope1,
                &slope2,
                &slope3,
                &mid_utilization,
                &optimal_utilization,
                &reserve_factor,
            );
            parameters.max_borrow_rate_ray = self.to_decimal_ray(max_borrow_rate);
            parameters.base_borrow_rate_ray = self.to_decimal_ray(base_borrow_rate);
            parameters.slope1_ray = self.to_decimal_ray(slope1);
            parameters.slope2_ray = self.to_decimal_ray(slope2);
            parameters.slope3_ray = self.to_decimal_ray(slope3);
            parameters.mid_utilization_ray = self.to_decimal_ray(mid_utilization);
            parameters.optimal_utilization_ray = self.to_decimal_ray(optimal_utilization);
            parameters.reserve_factor_bps = self.to_decimal_bps(reserve_factor);
            require!(
                parameters.max_borrow_rate_ray > parameters.base_borrow_rate_ray,
                ERROR_INVALID_BORROW_RATE_PARAMS
            );
            require!(
                parameters.optimal_utilization_ray > parameters.mid_utilization_ray,
                ERROR_INVALID_UTILIZATION_RANGE
            );
            require!(
                parameters.optimal_utilization_ray < self.ray(),
                ERROR_OPTIMAL_UTILIZATION_TOO_HIGH
            );
            require!(
                parameters.reserve_factor_bps < self.bps(),
                ERROR_INVALID_RESERVE_FACTOR
            );
        });
    }
}
