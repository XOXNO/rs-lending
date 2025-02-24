multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use crate::{contexts::base::Cache, rates, storage};

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

    /// Computes the total accrued interest on a borrow position.
    ///
    /// The interest is computed based on the difference between the current and the initial borrow index.
    ///
    /// # Parameters
    /// - `amount`: The principal amount borrowed.
    /// - `initial_borrow_index`: The borrow index at the time of borrowing.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The accrued interest.
    #[view(getDebtInterest)]
    fn get_debt_interest(
        &self,
        amount: ManagedDecimal<Self::Api, NumDecimals>,
        initial_borrow_index: ManagedDecimal<Self::Api, NumDecimals>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let current_borrow_index = self.borrow_index().get();
        let borrow_index_diff = current_borrow_index - initial_borrow_index;

        amount * borrow_index_diff
    }

    /// Computes the total accrued interest on a supply position.
    ///
    /// The interest is computed based on the difference between the current and the initial supply index.
    ///
    /// # Parameters
    /// - `amount`: The principal amount supplied.
    /// - `initial_supply_index`: The supply index at the time of supplying.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The accrued interest.
    #[view(getSupplyInterest)]
    fn get_supply_interest(
        &self,
        amount: ManagedDecimal<Self::Api, NumDecimals>,
        initial_supply_index: ManagedDecimal<Self::Api, NumDecimals>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let current_supply_index = self.supply_index().get();
        let supply_index_diff = current_supply_index - initial_supply_index;

        amount * supply_index_diff
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
            cache.pool_params.reserve_factor.clone(),
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
