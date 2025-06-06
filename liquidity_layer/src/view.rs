multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use common_constants::RAY_PRECISION;

use crate::storage;

/// The ViewModule provides read-only endpoints for retrieving key market metrics.
#[multiversx_sc::module]
pub trait ViewModule:
    storage::Storage + common_math::SharedMathModule + common_rates::InterestRates
{
    /// Retrieves the current capital utilization of the pool.
    ///
    /// Capital utilization is defined as the ratio of borrowed tokens to the total supplied tokens.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The current utilization ratio.
    #[view(getCapitalUtilisation)]
    fn get_capital_utilisation(&self) -> ManagedDecimal<Self::Api, NumDecimals> {
        let params = self.params().get();
        let zero = self.to_decimal(BigUint::zero(), params.asset_decimals);
        let supplied = self.supplied().get();
        let borrowed = self.borrowed().get();
        let total_borrowed = self.mul_half_up(&borrowed, &self.borrow_index().get(), RAY_PRECISION);
        let total_supplied = self.mul_half_up(&supplied, &self.supply_index().get(), RAY_PRECISION);
        if total_supplied == zero {
            self.ray_zero()
        } else {
            self.div_half_up(&total_borrowed, &total_supplied, RAY_PRECISION)
        }
    }

    /// Retrieves the total actual balance of the asset held by the pool contract.
    /// This represents the current liquidity or reserves available in the pool.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The total reserves in the pool, scaled to the asset's decimals.
    #[view(getReserves)]
    fn get_reserves(&self) -> ManagedDecimal<Self::Api, NumDecimals> {
        let params = self.params().get();
        let pool_balance = self.blockchain().get_sc_balance(&params.asset_id, 0);
        self.to_decimal(pool_balance, params.asset_decimals)
    }

    /// Retrieves the current deposit rate for the pool.
    ///
    /// The deposit rate is derived from capital utilization, the borrow rate, and the reserve factor.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The current deposit rate.
    #[view(getDepositRate)]
    fn get_deposit_rate(&self) -> ManagedDecimal<Self::Api, NumDecimals> {
        let params = self.params().get();
        let utilization = self.get_capital_utilisation();
        let borrow_rate = self.calc_borrow_rate(utilization.clone(), params.clone());
        self.calc_deposit_rate(utilization, borrow_rate, params.reserve_factor.clone())
    }

    /// Retrieves the current borrow rate for the pool.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The current borrow rate.
    #[view(getBorrowRate)]
    fn get_borrow_rate(&self) -> ManagedDecimal<Self::Api, NumDecimals> {
        let params = self.params().get();
        let utilization = self.get_capital_utilisation();
        self.calc_borrow_rate(utilization, params)
    }

    /// Retrieves the time delta since the last update.
    ///
    /// # Returns
    /// - `u64`: The time delta in seconds.
    #[view(getDeltaTime)]
    fn get_delta_time(&self) -> u64 {
        (self.blockchain().get_block_timestamp() * 1000u64) - self.last_timestamp().get()
    }

    /// Retrieves the protocol revenue accrued from borrow interest fees, scaled to the asset's decimals.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The accumulated protocol revenue, scaled to the asset's decimals.
    #[view(getProtocolRevenue)]
    fn get_protocol_revenue(&self) -> ManagedDecimal<Self::Api, NumDecimals> {
        let revenue_scaled = self.revenue().get();
        let supply_index = self.supply_index().get();

        self.rescale_half_up(
            &self.mul_half_up(&revenue_scaled, &supply_index, RAY_PRECISION),
            self.params().get().asset_decimals,
        )
    }

    /// Retrieves the total amount supplied to the pool.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The total amount supplied.
    #[view(getSuppliedAmount)]
    fn get_supplied_amount(&self) -> ManagedDecimal<Self::Api, NumDecimals> {
        let supplied_scaled = self.supplied().get();
        let supply_index = self.supply_index().get();

        self.rescale_half_up(
            &self.mul_half_up(&supplied_scaled, &supply_index, RAY_PRECISION),
            self.params().get().asset_decimals,
        )
    }

    /// Retrieves the total amount borrowed from the pool.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The total amount borrowed.
    #[view(getBorrowedAmount)]
    fn get_borrowed_amount(&self) -> ManagedDecimal<Self::Api, NumDecimals> {
        let borrowed_scaled = self.borrowed().get();
        let borrow_index = self.borrow_index().get();

        self.rescale_half_up(
            &self.mul_half_up(&borrowed_scaled, &borrow_index, RAY_PRECISION),
            self.params().get().asset_decimals,
        )
    }
}
