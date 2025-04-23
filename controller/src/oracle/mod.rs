multiversx_sc::imports!();
use common_constants::{
    EGLD_TICKER, PRICE_AGGREGATOR_ROUNDS_STORAGE_KEY, PRICE_AGGREGATOR_STATUS_STORAGE_KEY,
    SECONDS_PER_MINUTE, STATE_PAIR_ONEDEX_STORAGE_KEY, STATE_PAIR_STORAGE_KEY, USD_TICKER,
    WAD_HALF_PRECISION, WAD_PRECISION, WEGLD_TICKER,
};
use common_errors::ERROR_UN_SAFE_PRICE_NOT_ALLOWED;
use common_proxies::proxy_xexchange_pair;
use common_structs::{ExchangeSource, OracleProvider, OracleType, PriceFeedShort, PricingMethod};
use multiversx_sc::storage::StorageKey;

use price_aggregator::{
    errors::{PAUSED_ERROR, TOKEN_PAIR_NOT_FOUND_ERROR},
    structs::{TimestampedPrice, TokenPair},
};

use crate::{
    cache::Cache,
    helpers, proxy_legld, proxy_lxoxno,
    proxy_onedex::{self, State as StateOnedex},
    proxy_price_aggregator::PriceFeed,
    proxy_xegld,
    proxy_xexchange_pair::State as StateXExchange,
    storage, ERROR_INVALID_EXCHANGE_SOURCE, ERROR_INVALID_ORACLE_TOKEN_TYPE,
    ERROR_NO_LAST_PRICE_FOUND, ERROR_ORACLE_TOKEN_NOT_FOUND, ERROR_PAIR_NOT_ACTIVE,
    ERROR_PRICE_AGGREGATOR_NOT_SET,
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

        if cache.prices_cache.contains(token_id) {
            return cache.prices_cache.get(token_id);
        }

        let oracle_data = self.token_oracle(token_id);
        require!(!oracle_data.is_empty(), ERROR_ORACLE_TOKEN_NOT_FOUND);

        let data = oracle_data.get();

        let price = self.find_price_feed(&data, token_id, cache);
        let feed = PriceFeedShort {
            asset_decimals: data.price_decimals,
            price,
        };

        cache.prices_cache.put(token_id, &feed);

        feed
    }

    /// Find price feed based on token type (derived, LP, normal).
    fn find_price_feed(
        &self,
        configs: &OracleProvider<Self::Api>,
        original_market_token: &EgldOrEsdtTokenIdentifier,
        cache: &mut Cache<Self>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        match configs.oracle_type {
            OracleType::Derived => self.get_derived_price(configs, cache, false),
            OracleType::Lp => self.get_safe_lp_price(configs, cache),
            OracleType::Normal => {
                self.get_normal_price_in_egld(configs, original_market_token, cache)
            },
            _ => sc_panic!(ERROR_INVALID_ORACLE_TOKEN_TYPE),
        }
    }

    /// Compute safe LP price using Arda LP price formula with anchor price checks.
    fn get_safe_lp_price(
        &self,
        configs: &OracleProvider<Self::Api>,
        cache: &mut Cache<Self>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let (reserve_0, reserve_1, total_supply) =
            self.get_reserves(&configs.oracle_contract_address);

        let safe_first_token_feed = self.get_token_price(&configs.base_token_id, cache);
        let safe_second_token_feed = self.get_token_price(&configs.quote_token_id, cache);

        // Convert to decimals with proper precision
        let reserve_first = self.to_decimal(reserve_0, safe_first_token_feed.asset_decimals);
        let reserve_second = self.to_decimal(reserve_1, safe_second_token_feed.asset_decimals);
        let total_supply = self.to_decimal(total_supply, configs.price_decimals);

        let safe_lp_price = self.get_lp_price(
            configs,
            &reserve_first,
            safe_first_token_feed.asset_decimals,
            &reserve_second,
            safe_second_token_feed.asset_decimals,
            &total_supply,
            &safe_first_token_feed.price,
            &safe_second_token_feed.price,
        );

        let config_base_token_id = cache.get_cached_oracle(&configs.base_token_id);
        let config_quote_token_id = cache.get_cached_oracle(&configs.quote_token_id);

        let off_chain_first_egld_price = self.find_token_price_in_egld_from_aggregator(
            &config_base_token_id,
            &configs.base_token_id,
            cache,
        );

        let off_chain_second_egld_price = self.find_token_price_in_egld_from_aggregator(
            &config_quote_token_id,
            &configs.quote_token_id,
            cache,
        );

        let off_chain_lp_price = self.get_lp_price(
            configs,
            &reserve_first,
            safe_first_token_feed.asset_decimals,
            &reserve_second,
            safe_second_token_feed.asset_decimals,
            &total_supply,
            &off_chain_first_egld_price,
            &off_chain_second_egld_price,
        );

        let tolerances = &configs.tolerance;
        let avg_price = (safe_lp_price.clone() + off_chain_lp_price.clone()) / 2;

        if self.is_within_anchor(
            &safe_lp_price,
            &off_chain_lp_price,
            &tolerances.first_upper_ratio,
            &tolerances.first_lower_ratio,
        ) {
            off_chain_lp_price
        } else if self.is_within_anchor(
            &safe_lp_price,
            &off_chain_lp_price,
            &tolerances.last_upper_ratio,
            &tolerances.last_lower_ratio,
        ) {
            avg_price
        } else {
            require!(cache.allow_unsafe_price, ERROR_UN_SAFE_PRICE_NOT_ALLOWED);
            avg_price
        }
    }

    // --- Derived Price Functions ---
    fn get_derived_price(
        &self,
        configs: &OracleProvider<Self::Api>,
        cache: &mut Cache<Self>,
        safe_price_check: bool,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        match configs.exchange_source {
            ExchangeSource::XEGLD => self.get_xegld_derived_price(configs),
            ExchangeSource::LEGLD => self.get_legld_derived_price(configs),
            ExchangeSource::LXOXNO => {
                self.get_lxoxno_derived_price(configs, cache, safe_price_check)
            },
            _ => sc_panic!(ERROR_INVALID_EXCHANGE_SOURCE),
        }
    }

    fn get_legld_derived_price(
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

    fn get_xegld_derived_price(
        &self,
        configs: &OracleProvider<Self::Api>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let ratio = self
            .tx()
            .to(&configs.oracle_contract_address)
            .typed(proxy_xegld::LiquidStakingProxy)
            .get_exchange_rate()
            .returns(ReturnsResult)
            .sync_call_readonly();
        self.to_decimal_wad(ratio)
    }

    fn get_lxoxno_derived_price(
        &self,
        configs: &OracleProvider<Self::Api>,
        cache: &mut Cache<Self>,
        safe_price: bool,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let ratio = self
            .tx()
            .to(&configs.oracle_contract_address)
            .typed(proxy_lxoxno::RsLiquidXoxnoProxy)
            .get_exchange_rate()
            .returns(ReturnsResult)
            .sync_call_readonly();
        let ratio_dec = self.to_decimal(ratio, configs.price_decimals);
        let main_price = if safe_price {
            self.get_token_price(&configs.base_token_id, cache).price
        } else {
            self.get_token_price_in_egld_from_aggregator(&configs.base_token_id, cache)
        };
        self.get_token_egld_value(&ratio_dec, &main_price)
    }

    // --- Utility Functions ---
    fn get_pair_state(&self, pair: &ManagedAddress) -> StateXExchange {
        SingleValueMapper::<_, _, ManagedAddress>::new_from_address(
            pair.clone(),
            StorageKey::new(STATE_PAIR_STORAGE_KEY),
        )
        .get()
    }

    fn get_pair_state_onedex(&self, pair: &ManagedAddress, pair_id: usize) -> StateOnedex {
        let mut key = StorageKey::new(STATE_PAIR_ONEDEX_STORAGE_KEY);
        key.append_item(&pair_id);
        SingleValueMapper::<_, _, ManagedAddress>::new_from_address(pair.clone(), key).get()
    }

    // --- Safe Price Functions ---
    fn get_safe_price(
        &self,
        configs: &OracleProvider<Self::Api>,
        token_id: &EgldOrEsdtTokenIdentifier,
        cache: &mut Cache<Self>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let egld_ticker = ManagedBuffer::new_from_bytes(EGLD_TICKER);
        let one_token = BigUint::from(10u64).pow(configs.price_decimals as u32);

        let result = if configs.exchange_source == ExchangeSource::Onedex {
            let pair_status = self
                .get_pair_state_onedex(&configs.oracle_contract_address, configs.onedex_pair_id);
            require!(pair_status == StateOnedex::Active, ERROR_PAIR_NOT_ACTIVE);
            let from_identifier = token_id.clone().unwrap_esdt();
            let to_identifier = if from_identifier == configs.quote_token_id.clone().unwrap_esdt() {
                configs.base_token_id.clone()
            } else {
                configs.quote_token_id.clone()
            };
            self
                .tx()
                .to(&configs.oracle_contract_address)
                .typed(proxy_onedex::OneDexProxy)
                .get_safe_price_by_timestamp_offset(
                    from_identifier.clone(),
                    to_identifier.clone().unwrap_esdt(),
                    SECONDS_PER_MINUTE * 15,
                    EsdtTokenPayment::new(from_identifier, 0, one_token),
                )
                .returns(ReturnsResult)
                .sync_call_readonly()
        } else if configs.exchange_source == ExchangeSource::XExchange {
            let pair_status = self.get_pair_state(&configs.oracle_contract_address);
            require!(pair_status == StateXExchange::Active, ERROR_PAIR_NOT_ACTIVE);

            self
                .safe_price_proxy(self.safe_price_view().get())
                .get_safe_price_by_timestamp_offset(
                    &configs.oracle_contract_address,
                    SECONDS_PER_MINUTE * 15,
                    EsdtTokenPayment::new(token_id.clone().unwrap_esdt(), 0, one_token),
                )
                .returns(ReturnsResult)
                .sync_call_readonly()
        } else {
            sc_panic!(ERROR_INVALID_EXCHANGE_SOURCE)
        };

        let new_token_id = EgldOrEsdtTokenIdentifier::esdt(result.token_identifier.clone());
        let result_ticker = self.get_token_ticker(&new_token_id);
        if result_ticker == egld_ticker {
            self.to_decimal_wad(result.amount)
        } else {
            self.get_token_price(&new_token_id, cache).price
        }
    }

    /// Compute normal price in EGLD using aggregator, safe, or mixed pricing methods.
    fn get_normal_price_in_egld(
        &self,
        configs: &OracleProvider<Self::Api>,
        original_market_token: &EgldOrEsdtTokenIdentifier,
        cache: &mut Cache<Self>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let aggregator_price =
            self.get_aggregator_price_if_applicable(configs, original_market_token, cache);
        let safe_price = self.get_safe_price_if_applicable(configs, original_market_token, cache);
        self.calculate_final_price(aggregator_price, safe_price, configs, cache)
    }

    fn get_aggregator_price_if_applicable(
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

    fn get_safe_price_if_applicable(
        &self,
        configs: &OracleProvider<Self::Api>,
        original_market_token: &EgldOrEsdtTokenIdentifier,
        cache: &mut Cache<Self>,
    ) -> OptionalValue<ManagedDecimal<Self::Api, NumDecimals>> {
        if configs.pricing_method == PricingMethod::Safe
            || configs.pricing_method == PricingMethod::Mix
        {
            OptionalValue::Some(self.get_safe_price(configs, original_market_token, cache))
        } else {
            OptionalValue::None
        }
    }

    fn calculate_final_price(
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
                    safe_price
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

    fn find_token_price_in_egld_from_aggregator(
        &self,
        configs: &OracleProvider<Self::Api>,
        token_id: &EgldOrEsdtTokenIdentifier,
        cache: &mut Cache<Self>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        if configs.oracle_type == OracleType::Derived {
            self.get_derived_price(configs, cache, true)
        } else {
            self.get_token_price_in_egld_from_aggregator(token_id, cache)
        }
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
    fn get_lp_price(
        &self,
        configs: &OracleProvider<Self::Api>,
        reserve_first: &ManagedDecimal<Self::Api, NumDecimals>, // Amount of Token A (scaled by WAD)
        _reserve_first_decimals: usize,
        reserve_second: &ManagedDecimal<Self::Api, NumDecimals>, // Amount of Token B (scaled by WAD)
        _reserve_second_decimals: usize,
        total_supply: &ManagedDecimal<Self::Api, NumDecimals>, // Amount of LP token (scaled by LP decimals)
        first_token_egld_price: &ManagedDecimal<Self::Api, NumDecimals>, // Price A (EGLD/UnitA, scaled WAD)
        second_token_egld_price: &ManagedDecimal<Self::Api, NumDecimals>, // Price B (EGLD/UnitB, scaled WAD)
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        if configs.exchange_source != ExchangeSource::XExchange {
            sc_panic!(ERROR_INVALID_EXCHANGE_SOURCE);
        }

        // Ensure inputs are WAD - assuming reserve inputs and prices are already WAD based on caller
        let price_a = first_token_egld_price;
        let price_b = second_token_egld_price;

        // Calculate constant product using reserve amounts (scaled WAD)
        // Result: AmountA*AmountB * WAD
        let constant_product = self.mul_half_up(reserve_first, reserve_second, WAD_PRECISION);

        // Calculate price ratios (unitless, WAD)
        let price_ratio_x = self.div_half_up(price_b, price_a, WAD_PRECISION); // pB / pA
        let price_ratio_y = self.div_half_up(price_a, price_b, WAD_PRECISION); // pA / pB

        // Calculate intermediate values for sqrt
        // Inner = (AmountA*AmountB * WAD) * (Unitless * WAD) / WAD = AmountA*AmountB * WAD
        let inner_x = self.mul_half_up(&constant_product, &price_ratio_x, WAD_PRECISION);
        let inner_y = self.mul_half_up(&constant_product, &price_ratio_y, WAD_PRECISION);

        // --- Calculate modified reserve AMOUNT X' (x_prime) scaled WAD ---
        // 1. Take BigUint sqrt
        let sqrt_raw_x = inner_x.into_raw_units().sqrt(); // sqrt(AmtA*AmtB * 10^18) = sqrt(AmtA*AmtB) * 10^9

        // 2. Create decimal from sqrt_raw with 9 decimals (half WAD)
        let sqrt_decimal_temp_x = self.to_decimal(sqrt_raw_x, WAD_HALF_PRECISION);

        // 3. Create scaling factor 10^9 with 9 decimals (half WAD)
        let ten_pow_9 = BigUint::from(10u64).pow(WAD_HALF_PRECISION as u32);
        let sqrt_wad_factor = self.to_decimal(ten_pow_9, WAD_HALF_PRECISION);

        // 4. Multiply to rescale back to WAD (18 decimals)
        // Input scales are 9, target scale is 18.
        let x_prime = self.mul_half_up(&sqrt_decimal_temp_x, &sqrt_wad_factor, WAD_PRECISION); // Amount Token A * WAD

        // --- Calculate modified reserve AMOUNT Y' (y_prime) scaled WAD ---
        let sqrt_raw_y = inner_y.into_raw_units().sqrt();
        let sqrt_decimal_temp_y = self.to_decimal(sqrt_raw_y, WAD_HALF_PRECISION);
        // Re-use sqrt_wad_factor
        let y_prime = self.mul_half_up(&sqrt_decimal_temp_y, &sqrt_wad_factor, WAD_PRECISION); // Amount Token B * WAD

        // --- Calculate total LP value in EGLD ---
        // ValueA = (AmountA * WAD) * (PriceA * WAD) / WAD = ValueA * WAD
        let value_a = self.mul_half_up(&x_prime, price_a, WAD_PRECISION);
        // ValueB = (AmountB * WAD) * (PriceB * WAD) / WAD = ValueB * WAD
        let value_b = self.mul_half_up(&y_prime, price_b, WAD_PRECISION);

        let lp_total_value_egld = value_a + value_b; // Total Value * WAD

        // --- Calculate final LP price in EGLD per LP token ---
        // Ensure total_supply is scaled to WAD before division
        let total_supply_wad = total_supply.rescale(WAD_PRECISION);
        // Price = (Total Value * WAD) / (LP Supply * WAD) * WAD = Price * WAD
        self.div_half_up(&lp_total_value_egld, &total_supply_wad, WAD_PRECISION)
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

        let feed = self.make_price_feed(token_pair, round_values.get());
        self.to_decimal_wad(feed.price)
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
        }
    }

    fn get_reserves(&self, oracle_address: &ManagedAddress) -> (BigUint, BigUint, BigUint) {
        self.tx()
            .to(oracle_address)
            .typed(proxy_xexchange_pair::PairProxy)
            .get_reserves_and_total_supply()
            .returns(ReturnsResult)
            .sync_call_readonly()
            .into_tuple()
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
