use common_events::PriceFeedShort;

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

pub struct StorageCache<'a, C>
where
    C: crate::oracle::OracleModule + crate::storage::LendingStorageModule,
{
    _sc_ref: &'a C,
    pub prices: ManagedMap<C::Api>,
    pub decimals: ManagedMap<C::Api>,
    pub egld_price_feed: PriceFeedShort<C::Api>,
}

impl<'a, C> StorageCache<'a, C>
where
    C: crate::oracle::OracleModule,
{
    pub fn new(sc_ref: &'a C) -> Self {
        StorageCache {
            _sc_ref: sc_ref,
            prices: ManagedMap::new(),
            decimals: ManagedMap::new(),
            egld_price_feed: sc_ref.get_aggregator_price_feed(&EgldOrEsdtTokenIdentifier::egld()),
        }
    }
}
