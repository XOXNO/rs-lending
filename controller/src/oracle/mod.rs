multiversx_sc::imports!();
use common_constants::{
    EGLD_TICKER, PRICE_AGGREGATOR_ROUNDS_STORAGE_KEY, PRICE_AGGREGATOR_STATUS_STORAGE_KEY,
    SECONDS_PER_HOUR, SECONDS_PER_MINUTE, STATE_PAIR_STORAGE_KEY, USD_TICKER, WAD_PRECISION,
    WEGLD_TICKER,
};
use common_structs::{ExchangeSource, OracleProvider, OracleType, PriceFeedShort, PricingMethod};
use multiversx_sc::storage::StorageKey;

use price_aggregator::{
    errors::{PAUSED_ERROR, TOKEN_PAIR_NOT_FOUND_ERROR},
    structs::{TimestampedPrice, TokenPair},
};

use crate::{
    cache::Cache,
    helpers,
    proxies::{lxoxno_proxy, proxy_legld, xegld_proxy},
    proxy_price_aggregator::PriceFeed,
    proxy_xexchange_pair::State,
    storage, ERROR_INVALID_EXCHANGE_SOURCE, ERROR_INVALID_ORACLE_TOKEN_TYPE,
    ERROR_NO_LAST_PRICE_FOUND, ERROR_ORACLE_TOKEN_NOT_FOUND, ERROR_PAIR_NOT_ACTIVE,
    ERROR_PRICE_AGGREGATOR_NOT_SET, ERROR_UN_SAFE_PRICE_NOT_ALLOWED,
};

#[multiversx_sc::module]
pub trait OracleModule:
    storage::Storage + helpers::math::MathsModule + common_math::SharedMathModule
{
    /// Get token price data
    /// Retrieves price data with caching; handles EGLD/WEGLD cases early and errors if token is not found.
    fn get_token_price(
        &self,
        token_id: &EgldOrEsdtTokenIdentifier,
        cache: &mut Cache<Self>,
    ) -> PriceFeedShort<Self::Api> {
        let ticker = self.get_token_ticker(token_id);
        let egld_ticker = ManagedBuffer::new_from_bytes(EGLD_TICKER);
        if ticker == egld_ticker {
            return PriceFeedShort {
                asset_decimals: WAD_PRECISION,
                price: self.wad(),
            };
        }

        if cache.prices_cache.contains(&token_id) {
            let feed = cache.prices_cache.get(&token_id);
            return feed;
        }

        let oracle_data = self.token_oracle(token_id);
        require!(!oracle_data.is_empty(), ERROR_ORACLE_TOKEN_NOT_FOUND);

        let data = oracle_data.get();

        let price = self._find_price_feed(&data, token_id, cache);
        let feed = PriceFeedShort {
            asset_decimals: data.price_decimals,
            price,
        };

        cache.prices_cache.put(token_id, &feed);

        feed
    }

    /// Find price feed based on token type (derived, LP, normal).
    fn _find_price_feed(
        &self,
        configs: &OracleProvider<Self::Api>,
        original_market_token: &EgldOrEsdtTokenIdentifier,
        cache: &mut Cache<Self>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        match configs.oracle_type {
            OracleType::Derived => self._get_derived_price(configs, cache),
            OracleType::Lp => self._get_safe_lp_price(configs, cache),
            OracleType::Normal => {
                self._get_normal_price_in_egld(configs, original_market_token, cache)
            },
            _ => sc_panic!(ERROR_INVALID_ORACLE_TOKEN_TYPE),
        }
    }

    /// Compute safe LP price using short/long interval checks with tolerances.
    fn _get_safe_lp_price(
        &self,
        configs: &OracleProvider<Self::Api>,
        cache: &mut Cache<Self>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let short_interval = self._get_lp_price(configs, SECONDS_PER_MINUTE * 10, cache);
        let long_interval = self._get_lp_price(configs, SECONDS_PER_HOUR, cache);
        let tolerances = &configs.tolerance;
        let avg_price = (short_interval.clone() + long_interval.clone()) / 2;

        if self.is_within_anchor(
            &short_interval,
            &long_interval,
            &tolerances.first_upper_ratio,
            &tolerances.first_lower_ratio,
        ) {
            short_interval
        } else if self.is_within_anchor(
            &short_interval,
            &long_interval,
            &tolerances.last_upper_ratio,
            &tolerances.last_lower_ratio,
        ) {
            avg_price
        } else {
            require!(cache.allow_unsafe_price, ERROR_UN_SAFE_PRICE_NOT_ALLOWED);
            long_interval
        }
    }

    // --- Derived Price Functions ---
    fn _get_derived_price(
        &self,
        configs: &OracleProvider<Self::Api>,
        cache: &mut Cache<Self>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        match configs.exchange_source {
            ExchangeSource::XEGLD => self._get_xegld_derived_price(configs),
            ExchangeSource::LEGLD => self._get_legld_derived_price(configs),
            ExchangeSource::LXOXNO => self._get_lxoxno_derived_price(configs, cache),
            _ => sc_panic!(ERROR_INVALID_EXCHANGE_SOURCE),
        }
    }

    fn _get_legld_derived_price(
        &self,
        configs: &OracleProvider<Self::Api>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let ratio = self
            .tx()
            .to(&configs.oracle_contract_address)
            .typed(proxy_legld::SalsaContractProxy)
            .token_price()
            .returns(ReturnsResult)
            .sync_call_readonly();
        self.to_decimal_wad(ratio)
    }

    fn _get_xegld_derived_price(
        &self,
        configs: &OracleProvider<Self::Api>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let ratio = self
            .tx()
            .to(&configs.oracle_contract_address)
            .typed(xegld_proxy::LiquidStakingProxy)
            .get_exchange_rate()
            .returns(ReturnsResult)
            .sync_call_readonly();
        self.to_decimal_wad(ratio)
    }

    fn _get_lxoxno_derived_price(
        &self,
        configs: &OracleProvider<Self::Api>,
        cache: &mut Cache<Self>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let ratio = self
            .tx()
            .to(&configs.oracle_contract_address)
            .typed(lxoxno_proxy::RsLiquidXoxnoProxy)
            .get_exchange_rate()
            .returns(ReturnsResult)
            .sync_call_readonly();
        let ratio_dec = ManagedDecimal::from_raw_units(ratio, configs.price_decimals);
        let main_price = self.get_token_price(&configs.base_token_id, cache);
        self.get_token_egld_value(&ratio_dec, &main_price.price)
    }

    // --- Utility Functions ---
    fn get_pair_state(&self, pair: &ManagedAddress) -> State {
        SingleValueMapper::<_, _, ManagedAddress>::new_from_address(
            pair.clone(),
            StorageKey::new(STATE_PAIR_STORAGE_KEY),
        )
        .get()
    }

    // --- Safe Price Functions ---
    fn _get_safe_price(
        &self,
        configs: &OracleProvider<Self::Api>,
        token_id: &EgldOrEsdtTokenIdentifier,
        cache: &mut Cache<Self>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let egld_ticker = ManagedBuffer::new_from_bytes(EGLD_TICKER);
        let one_token = BigUint::from(10u64).pow(configs.price_decimals as u32);
        let pair_status = self.get_pair_state(&configs.oracle_contract_address);
        require!(pair_status == State::Active, ERROR_PAIR_NOT_ACTIVE);

        let result = self
            .safe_price_proxy(self.safe_price_view().get())
            ._get_safe_price_by_timestamp_offset(
                &configs.oracle_contract_address,
                SECONDS_PER_MINUTE * 15,
                EsdtTokenPayment::new(token_id.clone().unwrap_esdt(), 0, one_token),
            )
            .returns(ReturnsResult)
            .sync_call_readonly();

        let new_token_id = EgldOrEsdtTokenIdentifier::esdt(result.token_identifier.clone());
        let result_ticker = self.get_token_ticker(&new_token_id);
        if result_ticker == egld_ticker {
            self.to_decimal_wad(result.amount)
        } else {
            self.get_token_price(&new_token_id, cache).price
        }
    }

    /// Compute normal price in EGLD using aggregator, safe, or mixed pricing methods.
    fn _get_normal_price_in_egld(
        &self,
        configs: &OracleProvider<Self::Api>,
        original_market_token: &EgldOrEsdtTokenIdentifier,
        cache: &mut Cache<Self>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let aggregator_price =
            self._get_aggregator_price_if_applicable(configs, original_market_token, cache);
        let safe_price = self._get_safe_price_if_applicable(configs, original_market_token, cache);
        self._calculate_final_price(aggregator_price, safe_price, configs, cache)
    }

    fn _get_aggregator_price_if_applicable(
        &self,
        configs: &OracleProvider<Self::Api>,
        original_market_token: &EgldOrEsdtTokenIdentifier,
        cache: &mut Cache<Self>,
    ) -> OptionalValue<ManagedDecimal<Self::Api, NumDecimals>> {
        if configs.pricing_method == PricingMethod::Aggregator
            || configs.pricing_method == PricingMethod::Mix
        {
            OptionalValue::Some(
                self.get_token_price_in_egld_from_aggregator(original_market_token, cache),
            )
        } else {
            OptionalValue::None
        }
    }

    fn _get_safe_price_if_applicable(
        &self,
        configs: &OracleProvider<Self::Api>,
        original_market_token: &EgldOrEsdtTokenIdentifier,
        cache: &mut Cache<Self>,
    ) -> OptionalValue<ManagedDecimal<Self::Api, NumDecimals>> {
        if configs.pricing_method == PricingMethod::Safe
            || configs.pricing_method == PricingMethod::Mix
        {
            OptionalValue::Some(self._get_safe_price(configs, original_market_token, cache))
        } else {
            OptionalValue::None
        }
    }

    fn _calculate_final_price(
        &self,
        aggregator_price_opt: OptionalValue<ManagedDecimal<Self::Api, NumDecimals>>,
        safe_price_opt: OptionalValue<ManagedDecimal<Self::Api, NumDecimals>>,
        configs: &OracleProvider<Self::Api>,
        cache: &mut Cache<Self>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        match (aggregator_price_opt, safe_price_opt) {
            (OptionalValue::Some(aggregator_price), OptionalValue::Some(safe_price)) => {
                let tolerances = &configs.tolerance;
                if self.is_within_anchor(
                    &aggregator_price,
                    &safe_price,
                    &tolerances.first_upper_ratio,
                    &tolerances.first_lower_ratio,
                ) {
                    aggregator_price
                } else if self.is_within_anchor(
                    &aggregator_price,
                    &safe_price,
                    &tolerances.last_upper_ratio,
                    &tolerances.last_lower_ratio,
                ) {
                    (aggregator_price + safe_price) / 2
                } else {
                    require!(cache.allow_unsafe_price, ERROR_UN_SAFE_PRICE_NOT_ALLOWED);
                    safe_price
                }
            },
            (OptionalValue::Some(aggregator_price), OptionalValue::None) => aggregator_price,
            (OptionalValue::None, OptionalValue::Some(safe_price)) => safe_price,
            (OptionalValue::None, OptionalValue::None) => {
                sc_panic!(ERROR_NO_LAST_PRICE_FOUND)
            },
        }
    }

    /// Get token price in EGLD from aggregator using USD prices.
    fn get_token_price_in_egld_from_aggregator(
        &self,
        token_id: &EgldOrEsdtTokenIdentifier,
        cache: &mut Cache<Self>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let token_price = self.get_aggregator_price_feed(token_id, &cache.price_aggregator_sc);
        self.div_half_up(&token_price, &cache.egld_price_feed, WAD_PRECISION)
    }

    /// Check if price is within tolerance bounds relative to anchor price.
    fn is_within_anchor(
        &self,
        aggregator_price: &ManagedDecimal<Self::Api, NumDecimals>,
        safe_price: &ManagedDecimal<Self::Api, NumDecimals>,
        upper_bound_ratio: &ManagedDecimal<Self::Api, NumDecimals>,
        lower_bound_ratio: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> bool {
        let anchor_ratio = safe_price.clone() * self.bps() / aggregator_price.clone();
        &anchor_ratio <= upper_bound_ratio && &anchor_ratio >= lower_bound_ratio
    }

    /// Get token ticker, handling EGLD and WEGLD cases.
    fn get_token_ticker(&self, token_id: &EgldOrEsdtTokenIdentifier) -> ManagedBuffer {
        let egld_ticker = ManagedBuffer::new_from_bytes(EGLD_TICKER);
        if token_id.is_egld() || token_id.clone().into_name() == egld_ticker {
            return egld_ticker;
        }
        let result = token_id.as_esdt_option().unwrap().ticker();
        if result == ManagedBuffer::new_from_bytes(WEGLD_TICKER) {
            egld_ticker
        } else {
            result
        }
    }

    /// Calculate LP price based on underlying assets.
    fn _get_lp_price(
        &self,
        configs: &OracleProvider<Self::Api>,
        time_offset: u64,
        cache: &mut Cache<Self>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        if configs.exchange_source != ExchangeSource::XExchange {
            sc_panic!(ERROR_INVALID_EXCHANGE_SOURCE);
        }

        let tokens = self
            .safe_price_proxy(self.safe_price_view().get())
            .get_lp_tokens_safe_price_by_timestamp_offset(
                configs.oracle_contract_address.clone(),
                time_offset,
                BigUint::from(10u64).pow(configs.price_decimals as u32),
            )
            .returns(ReturnsResult)
            .sync_call_readonly();

        let (first_token, second_token) = tokens.into_tuple();
        let first = EgldOrEsdtTokenIdentifier::esdt(first_token.token_identifier);
        let second = EgldOrEsdtTokenIdentifier::esdt(second_token.token_identifier);

        let first_token_data = self.get_token_price(&first, cache);
        let second_token_data = self.get_token_price(&second, cache);

        let first_token_egld_price = self.get_token_egld_value(
            &ManagedDecimal::from_raw_units(first_token.amount, first_token_data.asset_decimals),
            &first_token_data.price,
        );
        let second_token_egld_price = self.get_token_egld_value(
            &ManagedDecimal::from_raw_units(second_token.amount, second_token_data.asset_decimals),
            &second_token_data.price,
        );

        // TODO: Add anchor checks and additional LP price calculation methods
        first_token_egld_price + second_token_egld_price
    }

    /// Fetch price feed from aggregator, converting to decimal format.
    fn get_aggregator_price_feed(
        &self,
        token_id: &EgldOrEsdtTokenIdentifier,
        price_aggregator_sc: &ManagedAddress,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let from_ticker = self.get_token_ticker(token_id);
        require!(
            !price_aggregator_sc.is_zero(),
            ERROR_PRICE_AGGREGATOR_NOT_SET
        );
        require!(
            !self._get_aggregator_status(price_aggregator_sc),
            PAUSED_ERROR
        );

        let token_pair = TokenPair {
            from: from_ticker,
            to: ManagedBuffer::new_from_bytes(USD_TICKER),
        };
        let round_values =
            self._token_oracle_prices_round(&token_pair.from, &token_pair.to, price_aggregator_sc);
        require!(!round_values.is_empty(), TOKEN_PAIR_NOT_FOUND_ERROR);

        let feed = self._make_price_feed(token_pair, round_values.get());
        self.to_decimal_wad(feed.price)
    }

    fn _token_oracle_prices_round(
        &self,
        from: &ManagedBuffer,
        to: &ManagedBuffer,
        address: &ManagedAddress,
    ) -> SingleValueMapper<TimestampedPrice<Self::Api>, ManagedAddress> {
        let mut key = StorageKey::new(PRICE_AGGREGATOR_ROUNDS_STORAGE_KEY);
        key.append_item(from);
        key.append_item(to);
        SingleValueMapper::<_, _, ManagedAddress>::new_from_address(address.clone(), key)
    }

    fn _get_aggregator_status(&self, address: &ManagedAddress) -> bool {
        SingleValueMapper::<_, _, ManagedAddress>::new_from_address(
            address.clone(),
            StorageKey::new(PRICE_AGGREGATOR_STATUS_STORAGE_KEY),
        )
        .get()
    }

    fn _make_price_feed(
        &self,
        token_pair: TokenPair<Self::Api>,
        last_price: TimestampedPrice<Self::Api>,
    ) -> PriceFeed<Self::Api> {
        PriceFeed {
            round_id: last_price.round,
            from: token_pair.from,
            to: token_pair.to,
            timestamp: last_price.timestamp,
            price: last_price.price,
        }
    }

    #[proxy]
    fn safe_price_proxy(&self, sc_address: ManagedAddress) -> safe_price_proxy::ProxyTo<Self::Api>;
}

mod safe_price_proxy {
    multiversx_sc::imports!();

    #[multiversx_sc::proxy]
    pub trait SafePriceContract {
        #[view(getSafePriceByTimestampOffset)]
        fn _get_safe_price_by_timestamp_offset(
            &self,
            pair_address: ManagedAddress,
            timestamp_offset: u64,
            input_payment: EsdtTokenPayment,
        ) -> EsdtTokenPayment;

        #[view(getLpTokensSafePriceByTimestampOffset)]
        fn get_lp_tokens_safe_price_by_timestamp_offset(
            &self,
            pair_address: ManagedAddress,
            timestamp_offset: u64,
            liquidity: BigUint,
        ) -> MultiValue2<EsdtTokenPayment, EsdtTokenPayment>;
    }
}
