multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use common_structs::PoolParams;

#[multiversx_sc::module]
pub trait StorageModule {
    /// Returns the pool asset.
    ///
    /// # Returns
    /// - `EgldOrEsdtTokenIdentifier`: The pool asset.
    #[view(getPoolAsset)]
    #[storage_mapper("pool_asset")]
    fn pool_asset(&self) -> SingleValueMapper<EgldOrEsdtTokenIdentifier>;

    /// Returns the reserves.
    /// Reserves are the amount of tokens that are currently in the pool available for borrowing or withdrawing.
    ///
    /// # Returns
    /// - `BigUint`: The reserves.
    #[view(getReserves)]
    #[storage_mapper("reserves")]
    fn reserves(&self) -> SingleValueMapper<BigUint>;

    /// Returns the supplied amount.
    /// Supplied amount is the amount of tokens that were supplied to the pool.
    ///
    /// # Returns
    /// - `BigUint`: The supplied amount.
    #[view(getSuppliedAmount)]
    #[storage_mapper("supplied_amount")]
    fn supplied_amount(&self) -> SingleValueMapper<BigUint>;

    /// Returns the rewards reserves.
    /// Rewards reserves are the amount of tokens that were earned by the protocol from the borrowers debt repayments.
    ///
    /// # Returns
    /// - `BigUint`: The rewards reserves.
    #[view(getProtocolRevenue)]
    #[storage_mapper("protocol_revenue")]
    fn protocol_revenue(&self) -> SingleValueMapper<BigUint>;

    /// Returns the borrowed amount.
    /// Borrowed amount is the amount of tokens that were borrowed from the pool.
    ///
    /// # Returns
    /// - `BigUint`: The borrowed amount.
    #[view(getTotalBorrow)]
    #[storage_mapper("borrowed_amount")]
    fn borrowed_amount(&self) -> SingleValueMapper<BigUint>;

    /// Returns the pool parameters.
    /// Pool parameters are the parameters of the pool.
    ///
    /// # Returns
    /// - `PoolParams<Self::Api>`: The pool parameters.
    #[view(getPoolParams)]
    #[storage_mapper("pool_params")]
    fn pool_params(&self) -> SingleValueMapper<PoolParams<Self::Api>>;

    /// Returns the borrow index.
    /// Borrow index is the index of the borrow rate.
    /// It is used to calculate the debt accrued by the borrowers.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The borrow index.
    #[view(getBorrowIndex)]
    #[storage_mapper("borrow_index")]
    fn borrow_index(&self) -> SingleValueMapper<ManagedDecimal<Self::Api, NumDecimals>>;

    /// Returns the supply index.
    /// Supply index is the index of the supply rate.
    /// It is used to calculate the interest earned by the suppliers.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The supply index.
    #[view(getSupplyIndex)]
    #[storage_mapper("supply_index")]
    fn supply_index(&self) -> SingleValueMapper<ManagedDecimal<Self::Api, NumDecimals>>;

    /// Returns the last update timestamp.
    /// Last update timestamp is the last time when the indexes were updated.
    ///
    /// # Returns
    /// - `u64`: The last update timestamp.
    #[view(getLastUpdateTimestamp)]
    #[storage_mapper("last_update_timestamp")]
    fn last_update_timestamp(&self) -> SingleValueMapper<u64>;
}
