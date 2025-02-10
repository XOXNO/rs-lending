multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use crate::{contexts::base::StorageCache, rates, storage};

/// The ViewModule provides read-only endpoints for retrieving key market metrics.
#[multiversx_sc::module]
pub trait ViewModule: rates::InterestRateMath + storage::StorageModule {
    /// Retrieves the current capital utilization of the pool.
    ///
    /// Capital utilization is defined as the ratio of borrowed tokens to the total supplied tokens.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The current utilization ratio.
    #[view(getCapitalUtilisation)]
    fn get_capital_utilisation(&self) -> ManagedDecimal<Self::Api, NumDecimals> {
        let mut storage_cache = StorageCache::new(self);

        self.get_capital_utilisation_internal(&mut storage_cache)
    }

    /// Internal function to compute the capital utilization.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The computed utilization ratio.
    fn get_capital_utilisation_internal(
        &self,
        storage_cache: &mut StorageCache<Self>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        self.compute_capital_utilisation(
            storage_cache.borrowed_amount.clone(),
            storage_cache.supplied_amount.clone(),
            storage_cache.zero.clone(),
        )
    }

    /// Retrieves the total capital of the pool.
    ///
    /// Total capital is defined as the sum of reserves and borrowed tokens.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The total capital.
    #[view(getTotalCapital)]
    fn get_total_capital(&self) -> ManagedDecimal<Self::Api, NumDecimals> {
        let mut storage_cache = StorageCache::new(self);

        self.get_total_capital_internal(&mut storage_cache)
    }

    /// Internal function to compute total capital.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The sum of reserves and borrowed tokens.
    fn get_total_capital_internal(
        &self,
        storage_cache: &mut StorageCache<Self>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let reserve_amount = storage_cache.reserves_amount.clone();
        let borrowed_amount = storage_cache.borrowed_amount.clone();

        reserve_amount + borrowed_amount
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

    /// Retrieves the current deposit rate for the pool.
    ///
    /// The deposit rate is derived from capital utilization, the borrow rate, and the reserve factor.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The current deposit rate.
    #[view(getDepositRate)]
    fn get_deposit_rate(&self) -> ManagedDecimal<Self::Api, NumDecimals> {
        let mut storage_cache = StorageCache::new(self);

        self.get_deposit_rate_internal(&mut storage_cache)
    }

    /// Internal function to compute the deposit rate.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The computed deposit rate.
    fn get_deposit_rate_internal(
        &self,
        storage_cache: &mut StorageCache<Self>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let capital_utilisation = self.get_capital_utilisation_internal(storage_cache);
        let borrow_rate = self.get_borrow_rate_internal(storage_cache);

        self.compute_deposit_rate(
            capital_utilisation,
            borrow_rate,
            storage_cache.pool_params.reserve_factor.clone(),
        )
    }

    /// Retrieves the current borrow rate for the pool.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The current borrow rate.
    #[view(getBorrowRate)]
    fn get_borrow_rate(&self) -> ManagedDecimal<Self::Api, NumDecimals> {
        let mut storage_cache = StorageCache::new(self);

        self.get_borrow_rate_internal(&mut storage_cache)
    }

    /// Internal function to compute the borrow rate.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The computed borrow rate.
    fn get_borrow_rate_internal(
        &self,
        storage_cache: &mut StorageCache<Self>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let capital_utilisation = self.get_capital_utilisation_internal(storage_cache);

        self.compute_borrow_rate(storage_cache.pool_params.clone(), capital_utilisation)
    }
}
