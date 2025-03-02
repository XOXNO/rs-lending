use common_structs::{AssetConfig, PriceFeedShort};

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

pub struct Cache<'a, C>
where
    C: crate::oracle::OracleModule + crate::storage::Storage,
{
    _sc_ref: &'a C,
    pub prices_cache:
        ManagedMapEncoded<C::Api, EgldOrEsdtTokenIdentifier<C::Api>, PriceFeedShort<C::Api>>,
    pub asset_configs:
        ManagedMapEncoded<C::Api, EgldOrEsdtTokenIdentifier<C::Api>, AssetConfig<C::Api>>,
    pub asset_pools:
        ManagedMapEncoded<C::Api, EgldOrEsdtTokenIdentifier<C::Api>, ManagedAddress<C::Api>>,
    pub egld_price_feed: ManagedDecimal<C::Api, NumDecimals>,
    pub price_aggregator_sc: ManagedAddress<C::Api>,
    pub allow_unsafe_price: bool,
}

impl<'a, C> Cache<'a, C>
where
    C: crate::oracle::OracleModule + crate::storage::Storage,
{
    pub fn new(sc_ref: &'a C) -> Self {
        let price_aggregator = sc_ref.price_aggregator_address().get();
        Cache {
            _sc_ref: sc_ref,
            prices_cache: ManagedMapEncoded::<
                C::Api,
                EgldOrEsdtTokenIdentifier<C::Api>,
                PriceFeedShort<C::Api>,
            >::new(),
            asset_configs: ManagedMapEncoded::<
                C::Api,
                EgldOrEsdtTokenIdentifier<C::Api>,
                AssetConfig<C::Api>,
            >::new(),
            asset_pools: ManagedMapEncoded::<
                C::Api,
                EgldOrEsdtTokenIdentifier<C::Api>,
                ManagedAddress<C::Api>,
            >::new(),
            egld_price_feed: sc_ref
                .get_aggregator_price_feed(&EgldOrEsdtTokenIdentifier::egld(), &price_aggregator),
            price_aggregator_sc: price_aggregator,
            allow_unsafe_price: true,
        }
    }

    /// Retrieves or caches asset configuration data.
    /// Reduces gas costs by caching frequently accessed asset info.
    ///
    /// # Arguments
    /// - `token_id`: Token identifier.
    ///
    /// # Returns
    /// - `AssetConfig` for the specified token.
    pub fn get_cached_asset_info(
        &mut self,
        token_id: &EgldOrEsdtTokenIdentifier<C::Api>,
    ) -> AssetConfig<C::Api> {
        let existing = self.asset_configs.contains(token_id);
        if existing {
            return self.asset_configs.get(token_id);
        }
        let new = self._sc_ref.asset_config(&token_id).get();
        self.asset_configs.put(token_id, &new);
        new
    }

    /// Retrieves or caches the liquidity pool address for a token.
    /// Optimizes repeated pool address lookups.
    ///
    /// # Arguments
    /// - `token_id`: Token identifier.
    ///
    /// # Returns
    /// - Pool address for the token.
    pub fn get_cached_pool_address(
        &mut self,
        token_id: &EgldOrEsdtTokenIdentifier<C::Api>,
    ) -> ManagedAddress<C::Api> {
        let existing = self.asset_pools.contains(token_id);
        if existing {
            return self.asset_pools.get(token_id);
        }
        let address = self._sc_ref.pools_map(&token_id).get();
        self.asset_pools.put(token_id, &address);

        address
    }
}
