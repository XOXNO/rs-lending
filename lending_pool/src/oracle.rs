multiversx_sc::imports!();
use common_constants::{
    BPS, BPS_PRECISION, EGLD_TICKER, PRICE_AGGREGATOR_ROUNDS_STORAGE_KEY,
    PRICE_AGGREGATOR_STATUS_STORAGE_KEY, SECONDS_PER_HOUR, SECONDS_PER_MINUTE,
    STATE_PAIR_STORAGE_KEY, USD_TICKER, WAD_PRECISION, WEGLD_TICKER,
};
use common_events::{ExchangeSource, OracleProvider, OracleType, PriceFeedShort, PricingMethod};
use multiversx_sc::storage::StorageKey;

use price_aggregator::{
    errors::{PAUSED_ERROR, TOKEN_PAIR_NOT_FOUND_ERROR},
    structs::{TimestampedPrice, TokenPair},
};

use crate::{
    contexts::base::StorageCache,
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
    storage::LendingStorageModule + helpers::math::MathsModule + common_math::SharedMathModule
{
    /// Get token amount in egld
    ///
    /// This function is used to get the amount of a token in egld from the price
    /// It uses the price data of the token to convert the amount to egld
    fn get_token_amount_in_egld(
        &self,
        token_id: &EgldOrEsdtTokenIdentifier,
        amount: &ManagedDecimal<Self::Api, NumDecimals>,
        storage_cache: &mut StorageCache<Self>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let feed = self.get_token_price(token_id, storage_cache);
        self.get_token_amount_in_egld_raw(amount, &feed.price)
    }

    /// Get token price data
    /// It has a cache mechanism to store the price and decimals of a token
    /// If the price and decimals are already cached, it returns the cached price and decimals
    /// If the price and decimals are not cached, it calculates the price and decimals
    /// It also handles the case when the token is EGLD/WEGLD to early return
    /// If the token is not found in the oracle, it returns an error
    fn get_token_price(
        &self,
        token_id: &EgldOrEsdtTokenIdentifier,
        storage_cache: &mut StorageCache<Self>,
    ) -> PriceFeedShort<Self::Api> {
        let ticker = self.get_token_ticker(token_id);
        let egld_ticker = ManagedBuffer::new_from_bytes(EGLD_TICKER);
        let is_egld = ticker == egld_ticker;
        if is_egld {
            return PriceFeedShort {
                decimals: WAD_PRECISION,
                price: storage_cache.wad_dec.clone(),
            };
        }
        if storage_cache.prices_cache.contains(&token_id) {
            let feed = storage_cache.prices_cache.get(&token_id);
            return feed;
        }

        let oracle_data = self.token_oracle(token_id);

        require!(!oracle_data.is_empty(), ERROR_ORACLE_TOKEN_NOT_FOUND);
        let data = oracle_data.get();
        let price = self.find_price_feed(&data, token_id, storage_cache);
        let feed = PriceFeedShort {
            decimals: data.decimals,
            price: price,
        };

        storage_cache.prices_cache.put(token_id, &feed);

        feed
    }

    /// Find price feed
    ///
    /// This function is used to find the price feed of a token
    /// It uses the token type to determine the price feed
    /// If the token type is derived, it uses the derived price
    /// If the token type is lp, it uses the lp price
    /// If the token type is normal, it uses the normal price
    fn find_price_feed(
        &self,
        configs: &OracleProvider<Self::Api>,
        original_market_token: &EgldOrEsdtTokenIdentifier,
        storage_cache: &mut StorageCache<Self>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        match configs.token_type {
            OracleType::Derived => self.get_derived_price(configs, storage_cache),
            OracleType::Lp => self.get_safe_lp_price(configs, storage_cache),
            OracleType::Normal => {
                self.get_normal_price_in_egld(configs, original_market_token, storage_cache)
            }
            _ => sc_panic!(ERROR_INVALID_ORACLE_TOKEN_TYPE),
        }
    }

    fn get_safe_lp_price(
        &self,
        configs: &OracleProvider<Self::Api>,
        storage_cache: &mut StorageCache<Self>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let short_interval = self.get_lp_price(configs, SECONDS_PER_MINUTE * 10, storage_cache);
        let long_interval = self.get_lp_price(configs, SECONDS_PER_HOUR, storage_cache);

        let tolerances = &configs.tolerance;

        let final_price = {
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
                require!(
                    storage_cache.allow_unsafe_price,
                    ERROR_UN_SAFE_PRICE_NOT_ALLOWED
                );
                long_interval
            }
        };

        final_price
    }

    // Derived Price Functions
    fn get_derived_price(
        &self,
        configs: &OracleProvider<Self::Api>,
        storage_cache: &mut StorageCache<Self>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        match configs.source {
            ExchangeSource::XEGLD => self.get_xegld_derived_price(configs),
            ExchangeSource::LEGLD => self.get_legld_derived_price(configs),
            ExchangeSource::LXOXNO => self.get_lxoxno_derived_price(configs, storage_cache),
            _ => sc_panic!(ERROR_INVALID_EXCHANGE_SOURCE),
        }
    }

    fn get_legld_derived_price(
        &self,
        configs: &OracleProvider<Self::Api>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let ratio = self
            .tx()
            .to(&configs.contract_address)
            .typed(proxy_legld::SalsaContractProxy)
            .token_price()
            .returns(ReturnsResult)
            .sync_call_readonly();

        self.to_decimal_wad(ratio)
    }

    fn get_xegld_derived_price(
        &self,
        configs: &OracleProvider<Self::Api>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let ratio = self
            .tx()
            .to(&configs.contract_address)
            .typed(xegld_proxy::LiquidStakingProxy)
            .get_exchange_rate()
            .returns(ReturnsResult)
            .sync_call_readonly();

        self.to_decimal_wad(ratio)
    }

    fn get_lxoxno_derived_price(
        &self,
        configs: &OracleProvider<Self::Api>,
        storage_cache: &mut StorageCache<Self>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let ratio = self
            .tx()
            .to(&configs.contract_address)
            .typed(lxoxno_proxy::RsLiquidXoxnoProxy)
            .get_exchange_rate()
            .returns(ReturnsResult)
            .sync_call_readonly();
        let ratio_dec = ManagedDecimal::from_raw_units(ratio, configs.decimals as usize);
        let main_price = self.get_token_price(&configs.first_token_id, storage_cache);
        let egld_price = self.get_token_amount_in_egld_raw(&ratio_dec, &main_price.price);

        egld_price
    }

    fn get_pair_state(&self, pair: &ManagedAddress) -> State {
        SingleValueMapper::<_, _, ManagedAddress>::new_from_address(
            pair.clone(),
            StorageKey::new(STATE_PAIR_STORAGE_KEY),
        )
        .get()
    }

    // Safe Price Functions
    fn get_safe_price(
        &self,
        configs: &OracleProvider<Self::Api>,
        token_id: &EgldOrEsdtTokenIdentifier,
        storage_cache: &mut StorageCache<Self>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let egld_ticker = ManagedBuffer::new_from_bytes(EGLD_TICKER);

        let one_token = BigUint::from(10u64).pow(configs.decimals as u32);
        let pair_status = self.get_pair_state(&configs.contract_address);

        require!(pair_status == State::Active, ERROR_PAIR_NOT_ACTIVE);

        let result = self
            .safe_price_proxy(self.safe_price_view().get())
            .get_safe_price_by_timestamp_offset(
                &configs.contract_address,
                SECONDS_PER_HOUR,
                EsdtTokenPayment::new(token_id.clone().unwrap_esdt(), 0, one_token),
            )
            .returns(ReturnsResult)
            .sync_call_readonly();

        let new_token_id = EgldOrEsdtTokenIdentifier::esdt(result.token_identifier.clone());
        let result_ticker = self.get_token_ticker(&new_token_id);

        if result_ticker == egld_ticker {
            return self.to_decimal_wad(result.amount);
        }

        self.get_token_price(&new_token_id, storage_cache).price
    }

    /// Get normal price egld price
    ///
    /// This function is used to get the price of a token in egld from the normal price calculation
    /// It uses the aggregator or mix pricing method to get the price
    /// If the pricing method is aggregator, it uses the aggregator to get the price
    /// If the pricing method is safe, it uses the safe price to get the price
    /// If the pricing method is mix, it uses the aggregator and safe price to get the price
    fn get_normal_price_in_egld(
        &self,
        configs: &OracleProvider<Self::Api>,
        original_market_token: &EgldOrEsdtTokenIdentifier,
        storage_cache: &mut StorageCache<Self>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let token_price_in_egld_opt = if configs.pricing_method == PricingMethod::Aggregator
            || configs.pricing_method == PricingMethod::Mix
        {
            OptionalValue::Some(
                self.get_token_price_in_egld_from_aggregator(original_market_token, storage_cache),
            )
        } else {
            OptionalValue::None
        };

        let safe_price = if configs.pricing_method == PricingMethod::Safe
            || configs.pricing_method == PricingMethod::Mix
        {
            OptionalValue::Some(self.get_safe_price(configs, original_market_token, storage_cache))
        } else {
            OptionalValue::None
        };

        let final_price = if safe_price.is_some() && token_price_in_egld_opt.is_some() {
            let token_price_in_egld = token_price_in_egld_opt.into_option().unwrap();
            let anchor_price = safe_price.into_option().unwrap();
            let avg_price = (token_price_in_egld.clone() + anchor_price.clone()) / 2;
            let tolerances = &configs.tolerance;

            if self.is_within_anchor(
                &token_price_in_egld,
                &anchor_price,
                &tolerances.first_upper_ratio,
                &tolerances.first_lower_ratio,
            ) {
                token_price_in_egld
            } else if self.is_within_anchor(
                &token_price_in_egld,
                &anchor_price,
                &tolerances.last_upper_ratio,
                &tolerances.last_lower_ratio,
            ) {
                avg_price
            } else {
                require!(
                    storage_cache.allow_unsafe_price,
                    ERROR_UN_SAFE_PRICE_NOT_ALLOWED
                );
                anchor_price
            }
        } else if token_price_in_egld_opt.is_some() {
            token_price_in_egld_opt.into_option().unwrap()
        } else if safe_price.is_some() {
            safe_price.into_option().unwrap()
        } else {
            sc_panic!(ERROR_NO_LAST_PRICE_FOUND);
        };

        final_price
    }

    /// Get token price in egld from aggregator
    ///
    /// This function is used to get the price of a token in egld from the aggregator
    /// It uses the USD price of both the token and EGLD to calculate the price in egld
    fn get_token_price_in_egld_from_aggregator(
        &self,
        token_id: &EgldOrEsdtTokenIdentifier,
        storage_cache: &mut StorageCache<Self>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        // Get price feeds
        let token_price =
            self.get_aggregator_price_feed(token_id, &storage_cache.price_aggregator_sc);

        // For other tokens, continue with normal calculation
        self.div_half_up(&token_price, &storage_cache.egld_price_feed, WAD_PRECISION)
    }

    /// Check if the price is within the anchor
    ///
    /// This function compares the price of a token with the safe price and the aggregator price.
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

    /// Get token ticker
    ///
    /// This function is used to get the ticker of a token.
    /// It handles both EGLD and ESDT tokens.
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

    fn get_lp_price(
        &self,
        configs: &OracleProvider<Self::Api>,
        time_offest: u64,
        storage_cache: &mut StorageCache<Self>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        if configs.source == ExchangeSource::XExchange {
            let tokens = self
                .safe_price_proxy(self.safe_price_view().get())
                .get_lp_tokens_safe_price_by_timestamp_offset(
                    configs.contract_address.clone(),
                    time_offest,
                    &BigUint::from(10u64).pow(configs.decimals as u32),
                )
                .returns(ReturnsResult)
                .sync_call_readonly();

            let (first_token, second_token) = tokens.into_tuple();

            let first = &EgldOrEsdtTokenIdentifier::esdt(first_token.token_identifier);
            let second = &EgldOrEsdtTokenIdentifier::esdt(second_token.token_identifier);

            let first_token_data = self.get_token_price(first, storage_cache);
            let second_token_data = self.get_token_price(second, storage_cache);

            let first_token_egld_price = self.get_token_amount_in_egld_raw(
                &ManagedDecimal::from_raw_units(
                    first_token.amount,
                    first_token_data.decimals as usize,
                ),
                &first_token_data.price,
            );
            let second_token_egld_price = self.get_token_amount_in_egld_raw(
                &ManagedDecimal::from_raw_units(
                    second_token.amount,
                    second_token_data.decimals as usize,
                ),
                &second_token_data.price,
            );

            // TODO: Add anchor checks and more ways of getting the LP price
            first_token_egld_price + second_token_egld_price
        } else {
            sc_panic!(ERROR_INVALID_EXCHANGE_SOURCE);
        }
    }

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
            !self.get_aggregator_status(price_aggregator_sc),
            PAUSED_ERROR
        );

        let token_pair = TokenPair {
            from: from_ticker,
            to: ManagedBuffer::new_from_bytes(USD_TICKER),
        };

        let round_values =
            self.token_oracle_prices_round(&token_pair.from, &token_pair.to, price_aggregator_sc);

        require!(!round_values.is_empty(), TOKEN_PAIR_NOT_FOUND_ERROR);

        let price_feed = self.make_price_feed(token_pair, round_values.get());

        self.to_decimal_wad(price_feed.price)
    }

    fn token_oracle_prices_round(
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

    fn get_aggregator_status(&self, address: &ManagedAddress) -> bool {
        SingleValueMapper::<_, _, ManagedAddress>::new_from_address(
            address.clone(),
            StorageKey::new(PRICE_AGGREGATOR_STATUS_STORAGE_KEY),
        )
        .get()
    }

    fn make_price_feed(
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
            decimals: last_price.decimals,
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
        fn get_safe_price_by_timestamp_offset(
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
