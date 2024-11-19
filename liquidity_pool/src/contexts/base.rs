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
    pub round: u64,
    pub pool_asset: TokenIdentifier<C::Api>,
    pub pool_params: PoolParams<C::Api>,
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
            round: sc_ref.blockchain().get_block_round(),
            pool_asset: sc_ref.pool_asset().get(),
            pool_params: sc_ref.pool_params().get(),
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
    }
}
