multiversx_sc::imports!();
multiversx_sc::derive_imports!();

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

        if supplied == zero {
            self.to_decimal_ray(BigUint::zero())
        } else {
            self.div_half_up(&borrowed, &supplied, common_constants::RAY_PRECISION)
        }
    }

    /// Retrieves the total capital of the pool.
    ///
    /// Total capital is defined as the sum of reserves and borrowed tokens.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The total capital.
    #[view(getTotalCapital)]
    fn get_total_capital(&self) -> ManagedDecimal<Self::Api, NumDecimals> {
        self.reserves().get() + self.borrowed().get()
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
}
