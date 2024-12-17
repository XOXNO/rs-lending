multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use crate::{contexts::base::StorageCache, liq_math, liq_storage};

#[multiversx_sc::module]
pub trait ViewModule: liq_math::MathModule + liq_storage::StorageModule {
    /// Returns the capital utilisation of the pool.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The capital utilisation.
    #[view(getCapitalUtilisation)]
    fn get_capital_utilisation(&self) -> ManagedDecimal<Self::Api, NumDecimals> {
        let mut storage_cache = StorageCache::new(self);

        self.get_capital_utilisation_internal(&mut storage_cache)
    }

    /// Returns the capital utilisation of the pool.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The capital utilisation.
    fn get_capital_utilisation_internal(
        &self,
        storage_cache: &mut StorageCache<Self>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        self.compute_capital_utilisation(
            storage_cache.borrowed_amount.clone(),
            storage_cache.supplied_amount.clone(),
            storage_cache.pool_params.decimals,
        )
    }

    /// Returns the total capital of the pool.
    /// Total capital is the sum of the reserves and the borrowed amount.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The total capital.
    #[view(getTotalCapital)]
    fn get_total_capital(&self) -> ManagedDecimal<Self::Api, NumDecimals> {
        let mut storage_cache = StorageCache::new(self);

        self.get_total_capital_internal(&mut storage_cache)
    }

    /// Returns the total capital of the pool.
    /// Total capital is the sum of the reserves and the borrowed amount.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The total capital.
    fn get_total_capital_internal(
        &self,
        storage_cache: &mut StorageCache<Self>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let reserve_amount = storage_cache.reserves_amount.clone();
        let borrowed_amount = storage_cache.borrowed_amount.clone();

        reserve_amount + borrowed_amount
    }

    /// Returns the total interest earned (compound) for the borrowers.
    ///
    /// # Parameters
    /// - `amount`: The amount of tokens to calculate the interest for.
    /// - `initial_borrow_index`: The initial borrow index, which is the index at the time of the borrow from the position metadata.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The total interest earned.
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

    /// Returns the deposit rate of the pool.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The deposit rate.
    #[view(getDepositRate)]
    fn get_deposit_rate(&self) -> ManagedDecimal<Self::Api, NumDecimals> {
        let mut storage_cache = StorageCache::new(self);

        self.get_deposit_rate_internal(&mut storage_cache)
    }

    /// Returns the deposit rate of the pool.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The deposit rate.
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

    /// Returns the borrow rate of the pool.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The borrow rate.
    #[view(getBorrowRate)]
    fn get_borrow_rate(&self) -> ManagedDecimal<Self::Api, NumDecimals> {
        let mut storage_cache = StorageCache::new(self);

        self.get_borrow_rate_internal(&mut storage_cache)
    }

    /// Returns the borrow rate of the pool.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The borrow rate.
    fn get_borrow_rate_internal(
        &self,
        storage_cache: &mut StorageCache<Self>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let capital_utilisation = self.get_capital_utilisation_internal(storage_cache);

        self.compute_borrow_rate(
            storage_cache.pool_params.r_max.clone(),
            storage_cache.pool_params.r_base.clone(),
            storage_cache.pool_params.r_slope1.clone(),
            storage_cache.pool_params.r_slope2.clone(),
            storage_cache.pool_params.u_optimal.clone(),
            capital_utilisation,
        )
    }
}
