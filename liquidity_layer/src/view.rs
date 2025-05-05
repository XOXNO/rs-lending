multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use common_events::RAY_PRECISION;

use crate::{rates, storage};

/// The ViewModule provides read-only endpoints for retrieving key market metrics.
#[multiversx_sc::module]
pub trait ViewModule:
    rates::InterestRates + storage::Storage + common_math::SharedMathModule
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
        let total_borrower = self.mul_half_up(&borrowed, &self.borrow_index().get(), RAY_PRECISION);
        let total_supplied = self.mul_half_up(&supplied, &self.supply_index().get(), RAY_PRECISION);
        if supplied == zero {
            self.ray_zero()
        } else {
            self.div_half_up(&total_borrower, &total_supplied, RAY_PRECISION)
        }
    }

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
        self.blockchain().get_block_timestamp() - self.last_timestamp().get()
    }

    /// Retrieves the protocol revenue accrued from borrow interest fees.
    ///
    /// # Returns
    /// - `BigUint`: The accumulated protocol revenue.
    #[view(getProtocolRevenue)]
    fn get_protocol_revenue(&self) -> ManagedDecimal<Self::Api, NumDecimals> {
        let revenue_scaled = self.revenue().get();
        
        revenue_scaled.rescale(self.params().get().asset_decimals)
    }

    /// Retrieves the total amount supplied to the pool.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The total amount supplied.
    #[view(getSuppliedAmount)]
    fn get_supplied_amount(&self) -> ManagedDecimal<Self::Api, NumDecimals> {
        let supplied_scaled = self.supplied().get();
        
        self
            .mul_half_up(&supplied_scaled, &self.supply_index().get(), RAY_PRECISION)
            .rescale(self.params().get().asset_decimals)
    }

    /// Retrieves the total amount borrowed from the pool.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The total amount borrowed.
    #[view(getBorrowedAmount)]
    fn get_borrowed_amount(&self) -> ManagedDecimal<Self::Api, NumDecimals> {
        let borrowed_scaled = self.borrowed().get();
        
        self
            .mul_half_up(&borrowed_scaled, &self.borrow_index().get(), RAY_PRECISION)
            .rescale(self.params().get().asset_decimals)
    }
}
