use common_constants::BP;
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
    pub price_aggregator_sc: ManagedAddress<C::Api>,
    pub allow_unsafe_price: bool,
    pub bp: BigUint<C::Api>,
}

impl<'a, C> StorageCache<'a, C>
where
    C: crate::oracle::OracleModule,
{
    pub fn new(sc_ref: &'a C) -> Self {
        let price_aggregator = sc_ref.price_aggregator_address().get();
        StorageCache {
            _sc_ref: sc_ref,
            prices: ManagedMap::new(),
            decimals: ManagedMap::new(),
            egld_price_feed: sc_ref
                .get_aggregator_price_feed(&EgldOrEsdtTokenIdentifier::egld(), &price_aggregator),
            price_aggregator_sc: price_aggregator,
            allow_unsafe_price: true,
            bp: BigUint::from(BP),
        }
    }
}
