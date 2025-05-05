use common_events::{MarketIndex, OracleProvider};
use common_structs::{AssetConfig, PriceFeedShort};

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

pub struct Cache<'a, C>
where
    C: crate::oracle::OracleModule + crate::storage::Storage,
{
    sc_ref: &'a C,

    pub prices_cache:
        ManagedMapEncoded<C::Api, EgldOrEsdtTokenIdentifier<C::Api>, PriceFeedShort<C::Api>>,
    pub asset_configs:
        ManagedMapEncoded<C::Api, EgldOrEsdtTokenIdentifier<C::Api>, AssetConfig<C::Api>>,
    pub asset_pools:
        ManagedMapEncoded<C::Api, EgldOrEsdtTokenIdentifier<C::Api>, ManagedAddress<C::Api>>,
    pub asset_oracles:
        ManagedMapEncoded<C::Api, EgldOrEsdtTokenIdentifier<C::Api>, OracleProvider<C::Api>>,
    pub market_indexes:
        ManagedMapEncoded<C::Api, EgldOrEsdtTokenIdentifier<C::Api>, MarketIndex<C::Api>>,
    pub egld_usd_price: ManagedDecimal<C::Api, NumDecimals>,
    pub price_aggregator_sc: ManagedAddress<C::Api>,
    pub egld_ticker: ManagedBuffer<C::Api>,
    pub allow_unsafe_price: bool,
    pub flash_loan_ongoing: bool,
}

impl<'a, C> Cache<'a, C>
where
    C: crate::oracle::OracleModule + crate::storage::Storage,
{
    pub fn new(sc_ref: &'a C) -> Self {
        let price_aggregator = sc_ref.price_aggregator_address().get();
        let egld_token_id = EgldOrEsdtTokenIdentifier::egld();
        let egld_provider = sc_ref.token_oracle(&egld_token_id).get();
        let mut asset_oracles = ManagedMapEncoded::new();
        asset_oracles.put(&egld_token_id, &egld_provider);
        let egld_price_feed = sc_ref.get_aggregator_price_feed(
            egld_token_id.clone().into_name(),
            &price_aggregator,
            egld_provider.max_price_stale_seconds,
        );
        let egld_usd_price = sc_ref.to_decimal_wad(egld_price_feed.price);

        Cache {
            sc_ref,
            prices_cache: ManagedMapEncoded::new(),
            asset_configs: ManagedMapEncoded::new(),
            asset_pools: ManagedMapEncoded::new(),
            asset_oracles,
            market_indexes: ManagedMapEncoded::new(),
            egld_usd_price,
            price_aggregator_sc: price_aggregator,
            egld_ticker: egld_token_id.into_name(),
            allow_unsafe_price: true,
            flash_loan_ongoing: sc_ref.flash_loan_ongoing().get(),
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

        let new = self.sc_ref.asset_config(token_id).get();
        self.asset_configs.put(token_id, &new);

        new
    }

    pub fn get_cached_market_index(
        &mut self,
        token_id: &EgldOrEsdtTokenIdentifier<C::Api>,
    ) -> MarketIndex<C::Api> {
        let existing = self.market_indexes.contains(token_id);
        if existing {
            return self.market_indexes.get(token_id);
        }

        let new = self.sc_ref.update_asset_index(token_id, self);
        self.market_indexes.put(token_id, &new);

        new
    }

    pub fn get_cached_oracle(
        &mut self,
        token_id: &EgldOrEsdtTokenIdentifier<C::Api>,
    ) -> OracleProvider<C::Api> {
        let existing = self.asset_oracles.contains(token_id);
        if existing {
            return self.asset_oracles.get(token_id);
        }

        let new = self.sc_ref.token_oracle(token_id).get();
        self.asset_oracles.put(token_id, &new);

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

        let address = self.sc_ref.pools_map(token_id).get();
        self.asset_pools.put(token_id, &address);

        address
    }
}
