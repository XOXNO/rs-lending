use common_structs::PoolParams;

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

pub struct StorageCache<'a, C>
where
    C: crate::liq_storage::StorageModule,
{
    sc_ref: &'a C,
    pub supplied_amount: BigUint<C::Api>,
    pub reserves_amount: BigUint<C::Api>,
    pub borrowed_amount: BigUint<C::Api>,
    pub rewards_reserve: BigUint<C::Api>,
    pub timestamp: u64,
    pub pool_asset: EgldOrEsdtTokenIdentifier<C::Api>,
    pub pool_params: PoolParams<C::Api>,
    pub borrow_index: BigUint<C::Api>,
    pub supply_index: BigUint<C::Api>,
    pub borrow_index_last_update_timestamp: u64,
}

impl<'a, C> StorageCache<'a, C>
where
    C: crate::liq_storage::StorageModule,
{
    pub fn new(sc_ref: &'a C) -> Self {
        StorageCache {
            supplied_amount: sc_ref.supplied_amount().get(),
            reserves_amount: sc_ref.reserves().get(),
            borrowed_amount: sc_ref.borrowed_amount().get(),
            rewards_reserve: sc_ref.rewards_reserves().get(),
            timestamp: sc_ref.blockchain().get_block_timestamp(),
            pool_asset: sc_ref.pool_asset().get(),
            pool_params: sc_ref.pool_params().get(),
            borrow_index: sc_ref.borrow_index().get(),
            supply_index: sc_ref.supply_index().get(),
            borrow_index_last_update_timestamp: sc_ref.borrow_index_last_update_timestamp().get(),
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
        self.sc_ref.supplied_amount().set(&self.supplied_amount);
        self.sc_ref.reserves().set(&self.reserves_amount);
        self.sc_ref.borrowed_amount().set(&self.borrowed_amount);
        self.sc_ref.rewards_reserves().set(&self.rewards_reserve);
        self.sc_ref.borrow_index().set(&self.borrow_index);
        self.sc_ref.supply_index().set(&self.supply_index);
        self.sc_ref
            .borrow_index_last_update_timestamp()
            .set(&self.borrow_index_last_update_timestamp);
    }
}
