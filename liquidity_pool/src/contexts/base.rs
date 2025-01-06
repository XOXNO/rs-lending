use common_structs::PoolParams;

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

pub struct StorageCache<'a, C>
where
    C: crate::storage::StorageModule,
{
    sc_ref: &'a C,
    /// The amount of the asset supplied.
    pub supplied_amount: ManagedDecimal<C::Api, NumDecimals>,
    /// The amount of the asset reserved.
    pub reserves_amount: ManagedDecimal<C::Api, NumDecimals>,
    /// The amount of the asset borrowed.
    pub borrowed_amount: ManagedDecimal<C::Api, NumDecimals>,
    /// The amount of the asset reserved for protocol revenue.
    pub protocol_revenue: ManagedDecimal<C::Api, NumDecimals>,
    /// The timestamp of the block.
    pub timestamp: u64,
    /// The asset of the pool.
    pub pool_asset: EgldOrEsdtTokenIdentifier<C::Api>,
    /// The parameters of the pool.
    pub pool_params: PoolParams<C::Api>,
    /// The borrow index.
    pub borrow_index: ManagedDecimal<C::Api, NumDecimals>,
    /// The supply index.
    pub supply_index: ManagedDecimal<C::Api, NumDecimals>,
    /// The timestamp of the last update.
    pub last_update_timestamp: u64,
}

impl<'a, C> StorageCache<'a, C>
where
    C: crate::storage::StorageModule,
{
    pub fn new(sc_ref: &'a C) -> Self {
        let params = sc_ref.pool_params().get();
        StorageCache {
            supplied_amount: ManagedDecimal::from_raw_units(
                sc_ref.supplied_amount().get(),
                params.decimals,
            ),
            reserves_amount: ManagedDecimal::from_raw_units(
                sc_ref.reserves().get(),
                params.decimals,
            ),
            borrowed_amount: ManagedDecimal::from_raw_units(
                sc_ref.borrowed_amount().get(),
                params.decimals,
            ),
            protocol_revenue: ManagedDecimal::from_raw_units(
                sc_ref.protocol_revenue().get(),
                params.decimals,
            ),
            timestamp: sc_ref.blockchain().get_block_timestamp(),
            pool_asset: sc_ref.pool_asset().get(),
            pool_params: params,
            borrow_index: sc_ref.borrow_index().get(),
            supply_index: sc_ref.supply_index().get(),
            last_update_timestamp: sc_ref.last_update_timestamp().get(),
            sc_ref,
        }
    }
}

impl<'a, C> Drop for StorageCache<'a, C>
where
    C: crate::storage::StorageModule,
{
    fn drop(&mut self) {
        // commit changes to storage for the mutable fields
        self.sc_ref
            .supplied_amount()
            .set(&self.supplied_amount.into_raw_units().clone());
        self.sc_ref
            .reserves()
            .set(&self.reserves_amount.into_raw_units().clone());
        self.sc_ref
            .borrowed_amount()
            .set(&self.borrowed_amount.into_raw_units().clone());
        self.sc_ref
            .protocol_revenue()
            .set(&self.protocol_revenue.into_raw_units().clone());
        self.sc_ref.borrow_index().set(&self.borrow_index);
        self.sc_ref.supply_index().set(&self.supply_index);
        self.sc_ref
            .last_update_timestamp()
            .set(&self.last_update_timestamp);
    }
}

impl<'a, C> StorageCache<'a, C>
where
    C: crate::storage::StorageModule,
{
    /// Returns the reserves of the pool.
    /// This is the amount of the asset that is not reserved for protocol revenue.
    /// Important as it protects the revenue from being borrowed by protocol users.
    /// # Returns
    /// - `ManagedDecimal<C::Api, NumDecimals>`: The reserves of the pool.
    pub fn get_reserves(&self) -> ManagedDecimal<C::Api, NumDecimals> {
        if self.reserves_amount >= self.protocol_revenue {
            self.reserves_amount.clone() - self.protocol_revenue.clone()
        } else {
            ManagedDecimal::from_raw_units(BigUint::zero(), self.pool_params.decimals)
        }
    }

    /// Returns the available revenue of the pool.
    /// This is the amount of the asset that is reserved for protocol revenue.
    /// If the reserves are less than the protocol revenue, the available revenue is 0. (Can happen when the debt was not paid back for a long time)
    /// # Returns
    /// - `ManagedDecimal<C::Api, NumDecimals>`: The available revenue of the pool.
    pub fn available_revenue(&self) -> ManagedDecimal<C::Api, NumDecimals> {
        ManagedDecimal::from_raw_units(
            BigUint::min(
                self.protocol_revenue.into_raw_units().clone(),
                self.reserves_amount.into_raw_units().clone(),
            ),
            self.pool_params.decimals,
        )
    }
}
