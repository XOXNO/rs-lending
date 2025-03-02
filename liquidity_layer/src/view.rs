multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use crate::{cache::Cache, rates, storage};

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
        let cache = Cache::new(self);

        cache.get_utilization()
    }

    /// Retrieves the total capital of the pool.
    ///
    /// Total capital is defined as the sum of reserves and borrowed tokens.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The total capital.
    #[view(getTotalCapital)]
    fn get_total_capital(&self) -> ManagedDecimal<Self::Api, NumDecimals> {
        let cache = Cache::new(self);

        cache.get_total_capital()
    }

    /// Retrieves the current deposit rate for the pool.
    ///
    /// The deposit rate is derived from capital utilization, the borrow rate, and the reserve factor.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The current deposit rate.
    #[view(getDepositRate)]
    fn get_deposit_rate(&self) -> ManagedDecimal<Self::Api, NumDecimals> {
        let cache = Cache::new(self);
        let borrow_rate = self.calc_borrow_rate(&cache);

        self.calc_deposit_rate(
            cache.get_utilization(),
            borrow_rate,
            cache.params.reserve_factor.clone(),
        )
    }

    /// Retrieves the current borrow rate for the pool.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The current borrow rate.
    #[view(getBorrowRate)]
    fn get_borrow_rate(&self) -> ManagedDecimal<Self::Api, NumDecimals> {
        let cache = Cache::new(self);

        self.calc_borrow_rate(&cache)
    }
}
