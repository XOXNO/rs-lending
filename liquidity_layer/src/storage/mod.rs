multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use common_structs::MarketParams;

/// The Storage trait provides on-chain storage mappers and view functions
/// for accessing the core state variables of the liquidity pool.
#[multiversx_sc::module]
pub trait Storage {
    /// Returns the pool asset identifier.
    ///
    /// # Returns
    /// - `EgldOrEsdtTokenIdentifier`: The asset managed by this pool.
    #[view(getPoolAsset)]
    #[storage_mapper("pool_asset")]
    fn pool_asset(&self) -> SingleValueMapper<EgldOrEsdtTokenIdentifier>;

    /// Retrieves the current reserves available in the pool.
    ///
    /// Reserves represent tokens held in the pool that are available for borrowing or withdrawal.
    ///
    /// # Returns
    /// - `BigUint`: The current reserves.
    #[view(getReserves)]
    #[storage_mapper("reserves")]
    fn reserves(&self) -> SingleValueMapper<ManagedDecimal<Self::Api, NumDecimals>>;

    /// Retrieves the total amount supplied to the pool.
    ///
    /// # Returns
    /// - `BigUint`: The total supplied tokens.
    #[view(getSuppliedAmount)]
    #[storage_mapper("supplied")]
    fn supplied(&self) -> SingleValueMapper<ManagedDecimal<Self::Api, NumDecimals>>;

    /// Retrieves the protocol revenue accrued from borrow interest fees.
    ///
    /// # Returns
    /// - `BigUint`: The accumulated protocol revenue.
    #[view(getProtocolRevenue)]
    #[storage_mapper("revenue")]
    fn revenue(&self) -> SingleValueMapper<ManagedDecimal<Self::Api, NumDecimals>>;

    /// Retrieves the total borrowed amount from the pool.
    ///
    /// # Returns
    /// - `BigUint`: The total tokens borrowed.
    #[view(getTotalBorrow)]
    #[storage_mapper("borrowed")]
    fn borrowed(&self) -> SingleValueMapper<ManagedDecimal<Self::Api, NumDecimals>>;

    /// Retrieves the total bad debt from the pool.
    ///
    /// # Returns
    /// - `BigUint`: The total bad debt pending to be collected.
    #[view(getBadDebt)]
    #[storage_mapper("bad_debt")]
    fn bad_debt(&self) -> SingleValueMapper<ManagedDecimal<Self::Api, NumDecimals>>;

    /// Returns the market parameters.
    ///
    /// These include interest rate parameters and asset asset_decimals.
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
    /// - `u64`: The timestamp when indexes were last updated.
    #[view(getLastTimestamp)]
    #[storage_mapper("last_timestamp")]
    fn last_timestamp(&self) -> SingleValueMapper<u64>;
}
