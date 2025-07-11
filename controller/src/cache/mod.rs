use common_structs::{AssetConfig, MarketIndex, OracleProvider, PriceFeedShort};

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

pub struct Cache<'a, C>
where
    C: crate::oracle::OracleModule + crate::storage::Storage + common_rates::InterestRates,
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
    pub safe_price_view: ManagedAddress<C::Api>,
    pub current_timestamp: u64,
}

impl<'a, C> Cache<'a, C>
where
    C: crate::oracle::OracleModule + crate::storage::Storage + common_rates::InterestRates,
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
        let safe_price_view = sc_ref.safe_price_view().get();

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
            safe_price_view,
            current_timestamp: sc_ref.blockchain().get_block_timestamp_ms(),
        }
    }

    // Clean the prices cache to have a fresh value after the swaps to prevent a bad HF
    // This is used in the strategy to prevent a bad HF after the swaps
    pub fn clean_prices_cache(&mut self) {
        self.prices_cache = ManagedMapEncoded::new();
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

        let new = self.sc_ref.update_asset_index(token_id, self, true);
        self.market_indexes.put(token_id, &new);

        new
    }

    pub fn get_cached_oracle(
        &mut self,
        token_id: &EgldOrEsdtTokenIdentifier<C::Api>,
    ) -> OracleProvider<C::Api> {
        let canonical_token_id = if self.sc_ref.get_token_ticker(token_id, self) == self.egld_ticker
        {
            EgldOrEsdtTokenIdentifier::egld()
        } else {
            token_id.clone()
        };
        let existing = self.asset_oracles.contains(&canonical_token_id);
        if existing {
            return self.asset_oracles.get(&canonical_token_id);
        }

        let new = self.sc_ref.token_oracle(&canonical_token_id).get();
        self.asset_oracles.put(&canonical_token_id, &new);

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
