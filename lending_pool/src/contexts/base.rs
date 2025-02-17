use common_constants::{BPS, BPS_PRECISION, WAD, WAD_PRECISION};
use common_events::PriceFeedShort;

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

pub struct StorageCache<'a, C>
where
    C: crate::oracle::OracleModule + crate::storage::LendingStorageModule,
{
    _sc_ref: &'a C,
    pub prices_cache:
        ManagedMapEncoded<C::Api, EgldOrEsdtTokenIdentifier<C::Api>, PriceFeedShort<C::Api>>,
    pub egld_price_feed: ManagedDecimal<C::Api, NumDecimals>,
    pub price_aggregator_sc: ManagedAddress<C::Api>,
    pub allow_unsafe_price: bool,
    pub wad: BigUint<C::Api>,
    pub wad_dec: ManagedDecimal<C::Api, NumDecimals>,
    pub bps: BigUint<C::Api>,
    pub bps_dec: ManagedDecimal<C::Api, NumDecimals>,
    pub bps_dec_zero: ManagedDecimal<C::Api, NumDecimals>,
    pub wad_dec_zero: ManagedDecimal<C::Api, NumDecimals>,
}

impl<'a, C> StorageCache<'a, C>
where
    C: crate::oracle::OracleModule,
{
    pub fn new(sc_ref: &'a C) -> Self {
        let price_aggregator = sc_ref.price_aggregator_address().get();
        StorageCache {
            _sc_ref: sc_ref,
            prices_cache: ManagedMapEncoded::<
                C::Api,
                EgldOrEsdtTokenIdentifier<C::Api>,
                PriceFeedShort<C::Api>,
            >::new(),
            egld_price_feed: sc_ref
                .get_aggregator_price_feed(&EgldOrEsdtTokenIdentifier::egld(), &price_aggregator),
            price_aggregator_sc: price_aggregator,
            allow_unsafe_price: true,
            wad: BigUint::from(WAD),
            bps: BigUint::from(BPS),
            bps_dec: ManagedDecimal::from_raw_units(BigUint::from(BPS), BPS_PRECISION),
            wad_dec: ManagedDecimal::from_raw_units(BigUint::from(WAD), WAD_PRECISION),
            bps_dec_zero: ManagedDecimal::from_raw_units(BigUint::zero(), BPS_PRECISION),
            wad_dec_zero: ManagedDecimal::from_raw_units(BigUint::zero(), WAD_PRECISION),
        }
    }
}
