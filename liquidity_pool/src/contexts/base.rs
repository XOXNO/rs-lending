use common_structs::PoolParams;

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

pub struct StorageCache<'a, C>
where
    C: crate::liq_storage::StorageModule,
{
    sc_ref: &'a C,
    pub supplied_amount: ManagedDecimal<C::Api, NumDecimals>,
    pub reserves_amount: ManagedDecimal<C::Api, NumDecimals>,
    pub borrowed_amount: ManagedDecimal<C::Api, NumDecimals>,
    pub rewards_reserve: ManagedDecimal<C::Api, NumDecimals>,
    pub timestamp: u64,
    pub pool_asset: EgldOrEsdtTokenIdentifier<C::Api>,
    pub pool_params: PoolParams<C::Api>,
    pub borrow_index: ManagedDecimal<C::Api, NumDecimals>,
    pub supply_index: ManagedDecimal<C::Api, NumDecimals>,
    pub last_update_timestamp: u64,
}

impl<'a, C> StorageCache<'a, C>
where
    C: crate::liq_storage::StorageModule,
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
            rewards_reserve: ManagedDecimal::from_raw_units(
                sc_ref.rewards_reserves().get(),
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
    C: crate::liq_storage::StorageModule,
{
    fn drop(&mut self) {
        // commit changes to storage for the mutable fields
        self.sc_ref.supplied_amount().set(&self.supplied_amount.into_raw_units().clone());
        self.sc_ref.reserves().set(&self.reserves_amount.into_raw_units().clone());
        self.sc_ref.borrowed_amount().set(&self.borrowed_amount.into_raw_units().clone());
        self.sc_ref.rewards_reserves().set(&self.rewards_reserve.into_raw_units().clone());
        self.sc_ref.borrow_index().set(&self.borrow_index);
        self.sc_ref.supply_index().set(&self.supply_index);
        self.sc_ref
            .last_update_timestamp()
            .set(&self.last_update_timestamp);
    }
}
