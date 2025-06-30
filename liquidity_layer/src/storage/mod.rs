multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use common_structs::MarketParams;

/// The Storage trait provides on-chain storage mappers and view functions
/// for accessing the core state variables of the liquidity pool.
#[multiversx_sc::module]
pub trait Storage {
    /// Retrieves the total scaled amount supplied to the pool.
    /// This value represents the sum of all supplied principals, each divided by the supply index at the time of their deposit.
    /// It is stored RAY-scaled.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The total scaled amount supplied, RAY-scaled.
    #[view(getSuppliedScaled)]
    #[storage_mapper("supplied")]
    fn supplied(&self) -> SingleValueMapper<ManagedDecimal<Self::Api, NumDecimals>>;

    /// Retrieves the protocol revenue accrued from borrow interest fees.
    /// This value is stored RAY-scaled.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The accumulated protocol revenue, RAY-scaled.
    #[view(getRevenueScaled)]
    #[storage_mapper("revenue")]
    fn revenue(&self) -> SingleValueMapper<ManagedDecimal<Self::Api, NumDecimals>>;

    /// Retrieves the total scaled borrowed amount from the pool.
    /// This value represents the sum of all borrowed principals, each divided by the borrow index at the time of their borrowing.
    /// It is stored RAY-scaled.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The total scaled borrowed amount, RAY-scaled.
    #[view(getBorrowedScaled)]
    #[storage_mapper("borrowed")]
    fn borrowed(&self) -> SingleValueMapper<ManagedDecimal<Self::Api, NumDecimals>>;

    /// Returns the market parameters.
    ///
    /// These include interest rate parameters and asset decimals.
    ///
    /// # Returns
    /// - `MarketParams<Self::Api>`: The market configuration.
    #[view(getParams)]
    #[storage_mapper("params")]
    fn params(&self) -> SingleValueMapper<MarketParams<Self::Api>>;

    /// Retrieves the current borrow index.
    ///
    /// The borrow index is used to calculate accrued interest on borrow positions.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The current borrow index.
    #[view(getBorrowIndex)]
    #[storage_mapper("borrow_index")]
    fn borrow_index(&self) -> SingleValueMapper<ManagedDecimal<Self::Api, NumDecimals>>;

    /// Retrieves the current supply index.
    ///
    /// The supply index is used to compute the yield for suppliers.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The current supply index.
    #[view(getSupplyIndex)]
    #[storage_mapper("supply_index")]
    fn supply_index(&self) -> SingleValueMapper<ManagedDecimal<Self::Api, NumDecimals>>;

    /// Retrieves the last update timestamp for the interest indexes.
    ///
    /// # Returns
    /// - `u64`: The timestamp when indexes were last updated, stored in milliseconds since Unix epoch.
    #[view(getLastTimestamp)]
    #[storage_mapper("last_timestamp")]
    fn last_timestamp(&self) -> SingleValueMapper<u64>;
}
