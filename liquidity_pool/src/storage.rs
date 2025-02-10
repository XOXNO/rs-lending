multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use common_structs::PoolParams;

/// The StorageModule trait provides on-chain storage mappers and view functions
/// for accessing the core state variables of the liquidity pool.
#[multiversx_sc::module]
pub trait StorageModule {
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
    fn reserves(&self) -> SingleValueMapper<BigUint>;

    /// Retrieves the total amount supplied to the pool.
    ///
    /// # Returns
    /// - `BigUint`: The total supplied tokens.
    #[view(getSuppliedAmount)]
    #[storage_mapper("supplied_amount")]
    fn supplied_amount(&self) -> SingleValueMapper<BigUint>;

    /// Retrieves the protocol revenue accrued from borrow interest fees.
    ///
    /// # Returns
    /// - `BigUint`: The accumulated protocol revenue.
    #[view(getProtocolRevenue)]
    #[storage_mapper("protocol_revenue")]
    fn protocol_revenue(&self) -> SingleValueMapper<BigUint>;

    /// Retrieves the total borrowed amount from the pool.
    ///
    /// # Returns
    /// - `BigUint`: The total tokens borrowed.
    #[view(getTotalBorrow)]
    #[storage_mapper("borrowed_amount")]
    fn borrowed_amount(&self) -> SingleValueMapper<BigUint>;

    /// Returns the pool parameters.
    ///
    /// These include interest rate parameters and asset decimals.
    ///
    /// # Returns
    /// - `PoolParams<Self::Api>`: The pool configuration.
    #[view(getPoolParams)]
    #[storage_mapper("pool_params")]
    fn pool_params(&self) -> SingleValueMapper<PoolParams<Self::Api>>;

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
    #[view(getLastUpdateTimestamp)]
    #[storage_mapper("last_update_timestamp")]
    fn last_update_timestamp(&self) -> SingleValueMapper<u64>;
}
