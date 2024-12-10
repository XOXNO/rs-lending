use common_events::{ExchangeSource, OracleProvider, OracleType, PricingMethod, BP};

use crate::{
    proxies::{lxoxno_proxy, xegld_proxy},
    proxy_price_aggregator::{PriceAggregatorProxy, PriceFeed},
    storage, ERROR_INVALID_EXCHANGE_SOURCE, ERROR_INVALID_ORACLE_OVERRIDE_TYPE,
    ERROR_ORACLE_OVERRIDE_NOT_FOUND, ERROR_PRICE_AGGREGATOR_NOT_SET, ERROR_TOKEN_TICKER_FETCH,
};

multiversx_sc::imports!();

const TOKEN_ID_SUFFIX_LEN: usize = 7; // "dash" + 6 random bytes
const DOLLAR_TICKER: &[u8] = b"USD";
const EGLD_TICKER: &[u8] = b"EGLD";
const WEGLD_TICKER: &[u8] = b"WEGLD";

#[multiversx_sc::module]
pub trait OracleModule: storage::LendingStorageModule {
    fn compute_amount_in_tokens(
        &self,
        amount_in_dollars: &BigUint,
        token_data: &PriceFeed<Self::Api>,
    ) -> BigUint {
        amount_in_dollars
            .mul(&BigUint::from(BP))
            .div(&token_data.price)
            .mul(BigUint::from(10u64).pow(token_data.decimals as u32))
            .div(&BigUint::from(BP))
    }

    fn get_token_amount_in_dollars(
        &self,
        token_id: &EgldOrEsdtTokenIdentifier,
        amount: &BigUint,
    ) -> BigUint {
        let token_data: PriceFeed<<Self as ContractBase>::Api> =
            self.get_token_price_data(token_id);

        self.get_token_amount_in_dollars_raw(amount, &token_data)
    }

    fn get_token_amount_in_dollars_raw(
        &self,
        amount: &BigUint,
        token_data: &PriceFeed<Self::Api>,
    ) -> BigUint {
        amount
            .mul(&BigUint::from(BP))
            .mul(&token_data.price)
            .div(BigUint::from(10u64).pow(token_data.decimals as u32))
            .div(&BigUint::from(BP))
    }

    fn get_token_price_in_egld(
        &self,
        token_id: &EgldOrEsdtTokenIdentifier,
    ) -> (BigUint, PriceFeed<Self::Api>) {
        // Get price feeds
        let token_price_feed = self.get_aggregator_price_feed(token_id);
        let egld_price_feed = self.get_aggregator_price_feed(&EgldOrEsdtTokenIdentifier::egld());

        if token_id.is_egld() {
            // For EGLD, return 1/EGLD_PRICE in USD
            // Example: if EGLD is $40, return 0.025 EGLD per USD
            let one_usd = BigUint::from(BP); // 1$ with 21 decimals
            (
                self.compute_amount_in_tokens(&one_usd, &egld_price_feed),
                egld_price_feed,
            )
        } else {
            // For other tokens, continue with normal calculation
            let one_token = BigUint::from(10u64).pow(token_price_feed.decimals as u32);
            let one_token_in_usd =
                self.get_token_amount_in_dollars_raw(&one_token, &token_price_feed);

            (
                self.compute_amount_in_tokens(&one_token_in_usd, &egld_price_feed),
                egld_price_feed,
            )
        }
    }

    // WEGLD
    fn get_token_price_data(&self, token_id: &EgldOrEsdtTokenIdentifier) -> PriceFeed<Self::Api> {
        // In case of token_id is WEGLD, we need to use the EGLD oracle the swap happens in the ticker function
        let ticker = self.get_token_ticker(token_id);
        let egld = &EgldOrEsdtTokenIdentifier::egld();
        let egld_ticker = ManagedBuffer::new_from_bytes(EGLD_TICKER);
        let is_egld = ticker == egld_ticker;
        // Force direction to EGLD for WEGLD or normal EGLD
        let override_price = if is_egld {
            // WEGLD is EGLD
            self.token_oracle(egld)
        } else {
            // Fallback the original token_id when not EGLD or WEGLD
            self.token_oracle(token_id)
        };
        require!(!override_price.is_empty(), ERROR_ORACLE_OVERRIDE_NOT_FOUND);
        // WEGLD is EGLD
        self.find_price_feed(&override_price.get(), if is_egld { egld } else { token_id })
    }

    fn find_price_feed(
        &self,
        configs: &OracleProvider<Self::Api>,               // EGLD
        original_market_token: &EgldOrEsdtTokenIdentifier, // EGLD,
    ) -> PriceFeed<Self::Api> {
        if configs.token_type == OracleType::Derived {
            return self.get_derived_price(configs, original_market_token);
        }

        if configs.token_type == OracleType::Lp {
            return self.get_lp_price(configs, original_market_token);
        }

        // EGLD
        if configs.token_type == OracleType::Normal {
            return self.get_normal_price_usd_price(configs, original_market_token);
        }

        sc_panic!(ERROR_INVALID_ORACLE_OVERRIDE_TYPE);
    }

    fn get_aggregator_price_feed(
        &self,
        token_id: &EgldOrEsdtTokenIdentifier,
    ) -> PriceFeed<Self::Api> {
        let from_ticker = self.get_token_ticker(token_id); // XOXNO
        let price_aggregator_address = self.price_aggregator_address();

        require!(
            !price_aggregator_address.is_empty(),
            ERROR_PRICE_AGGREGATOR_NOT_SET
        );
        let feed = self
            .tx()
            .to(self.price_aggregator_address().get())
            .typed(PriceAggregatorProxy)
            .latest_price_feed(from_ticker, ManagedBuffer::new_from_bytes(DOLLAR_TICKER))
            .returns(ReturnsResult)
            .sync_call();

        feed
    }

    fn get_normal_price_usd_price(
        &self,
        configs: &OracleProvider<Self::Api>,               // EGLD
        original_market_token: &EgldOrEsdtTokenIdentifier, // EGLD
    ) -> PriceFeed<Self::Api> {
        let result = self.get_normal_price_egld_price(configs, original_market_token);
        let (egld_equivalent, egld_price_feed) = result.into_tuple();

        let token_price_in_usd =
            self.get_token_amount_in_dollars_raw(&egld_equivalent, &egld_price_feed);

        PriceFeed {
            price: token_price_in_usd,
            decimals: configs.decimals,
            from: self.get_token_ticker(original_market_token),
            to: ManagedBuffer::new_from_bytes(DOLLAR_TICKER),
            round_id: self.blockchain().get_block_round() as u32,
            timestamp: self.blockchain().get_block_timestamp(),
        }
    }

    fn get_normal_price_egld_price(
        &self,
        configs: &OracleProvider<Self::Api>,               // EGLD
        original_market_token: &EgldOrEsdtTokenIdentifier, // EGLD
    ) -> MultiValue2<BigUint, PriceFeed<Self::Api>> {
        let (token_price_in_egld, egld_price_feed) =
            self.get_token_price_in_egld(original_market_token);

        let safe_price = if configs.pricing_method == PricingMethod::Safe
            || configs.pricing_method == PricingMethod::Mix
        {
            OptionalValue::Some(self.get_safe_price(
                configs,
                original_market_token,
                &egld_price_feed,
            ))
        } else {
            OptionalValue::None
        };

        (token_price_in_egld, egld_price_feed).into()
    }

    fn get_safe_price(
        &self,
        configs: &OracleProvider<Self::Api>,
        token_id: &EgldOrEsdtTokenIdentifier,
        egld_price_feed: &PriceFeed<Self::Api>,
    ) -> MultiValue2<BigUint, PriceFeed<Self::Api>> {
        let egld_ticker = ManagedBuffer::new_from_bytes(EGLD_TICKER);
        let first_token_ticker = self.get_token_ticker(&configs.first_token_id);
        let second_token_ticker = self.get_token_ticker(&configs.second_token_id);
        let is_first_token_id_egld = first_token_ticker == egld_ticker;
        let is_second_token_id_egld = second_token_ticker == egld_ticker;

        let (token_in, decimals) = if is_first_token_id_egld && token_id.is_egld() {
            let token_data = self.token_oracle(&configs.second_token_id);

            require!(!token_data.is_empty(), ERROR_ORACLE_OVERRIDE_NOT_FOUND);

            let decimals = token_data.get().decimals;

            (configs.second_token_id.clone(), decimals)
        } else if is_second_token_id_egld && token_id.is_egld() {
            let token_data = self.token_oracle(&configs.first_token_id);

            require!(!token_data.is_empty(), ERROR_ORACLE_OVERRIDE_NOT_FOUND);

            let decimals = token_data.get().decimals;

            (configs.first_token_id.clone(), decimals)
        } else {
            (token_id.clone(), configs.decimals)
        };

        let one_token = BigUint::from(10u64).pow(decimals as u32);
        let result = self
            .safe_price_proxy(self.safe_price_view().get())
            .get_safe_price_by_default_offset(
                configs.contract_address.clone(),
                EsdtTokenPayment::new(token_in.unwrap_esdt(), 0, one_token),
            )
            .returns(ReturnsResult)
            .sync_call();

        let result_ticker = self.get_token_ticker(&EgldOrEsdtTokenIdentifier::esdt(
            result.token_identifier.clone(),
        ));

        if result_ticker == egld_ticker {
            return (result.amount, egld_price_feed.clone()).into();
        }

        let new_config = self.token_oracle(token_id);
        require!(!new_config.is_empty(), ERROR_ORACLE_OVERRIDE_NOT_FOUND);

        // Can not be WEGLD
        return self.get_normal_price_egld_price(
            &new_config.get(),
            &EgldOrEsdtTokenIdentifier::esdt(result.token_identifier),
        );
    }

    fn get_derived_price(
        &self,
        configs: &OracleProvider<Self::Api>,
        original_market_token: &EgldOrEsdtTokenIdentifier,
    ) -> PriceFeed<Self::Api> {
        if configs.source == ExchangeSource::XEGLD {
            // Ratio is the amount of EGLD worth 1 XEGLD
            let ratio = self
                .tx()
                .to(configs.contract_address.clone())
                .typed(xegld_proxy::LiquidStakingProxy)
                .get_exchange_rate()
                .returns(ReturnsResult)
                .sync_call();
            // Derived token price is the price of the token in EGLD
            // First token is the token that is being derived from
            let token_data: PriceFeed<<Self as ContractBase>::Api> =
                self.get_token_price_data(&configs.first_token_id);
            // Use the ratio to convert the price of the derived token to USD
            let dollar_price = self.get_token_amount_in_dollars_raw(&ratio, &token_data);
            return PriceFeed {
                price: dollar_price,
                decimals: token_data.decimals,
                from: self.get_token_ticker(original_market_token),
                to: ManagedBuffer::new_from_bytes(DOLLAR_TICKER),
                round_id: self.blockchain().get_block_round() as u32,
                timestamp: self.blockchain().get_block_timestamp(),
            };
        }

        if configs.source == ExchangeSource::LXOXNO {
            let ratio = self
                .tx()
                .to(configs.contract_address.clone())
                .typed(lxoxno_proxy::RsLiquidXoxnoProxy)
                .get_exchange_rate()
                .returns(ReturnsResult)
                .sync_call();
            let token_data: PriceFeed<<Self as ContractBase>::Api> =
                self.get_token_price_data(&configs.first_token_id);
            let dollar_price = self.get_token_amount_in_dollars_raw(&ratio, &token_data);
            return PriceFeed {
                price: dollar_price,
                decimals: token_data.decimals,
                from: self.get_token_ticker(original_market_token),
                to: ManagedBuffer::new_from_bytes(b"USD"),
                round_id: self.blockchain().get_block_round() as u32,
                timestamp: self.blockchain().get_block_timestamp(),
            };
        }
        sc_panic!(ERROR_INVALID_EXCHANGE_SOURCE);
    }

    fn get_lp_price(
        &self,
        configs: &OracleProvider<Self::Api>,
        original_market_token: &EgldOrEsdtTokenIdentifier,
    ) -> PriceFeed<Self::Api> {
        if configs.source == ExchangeSource::XExchange {
            let tokens = self
                .safe_price_proxy(self.safe_price_view().get())
                .get_lp_tokens_safe_price_by_default_offset(
                    configs.contract_address.clone(),
                    BigUint::from(1u64).mul(&BigUint::from(10u64).pow(configs.decimals as u32)),
                )
                .returns(ReturnsResult)
                .sync_call();

            let (first_token, second_token) = tokens.into_tuple();
            let first_token_data: PriceFeed<<Self as ContractBase>::Api> = self
                .get_token_price_data(&EgldOrEsdtTokenIdentifier::esdt(
                    first_token.token_identifier,
                ));
            let second_token_data: PriceFeed<<Self as ContractBase>::Api> = self
                .get_token_price_data(&EgldOrEsdtTokenIdentifier::esdt(
                    second_token.token_identifier,
                ));

            return PriceFeed {
                price: first_token_data.price + second_token_data.price,
                decimals: configs.decimals,
                from: self.get_token_ticker(original_market_token),
                to: ManagedBuffer::new_from_bytes(DOLLAR_TICKER),
                round_id: self.blockchain().get_block_round() as u32,
                timestamp: self.blockchain().get_block_timestamp(),
            };
        }

        sc_panic!(ERROR_INVALID_EXCHANGE_SOURCE);
    }

    fn get_token_ticker(&self, token_id: &EgldOrEsdtTokenIdentifier) -> ManagedBuffer {
        let egld_ticker = ManagedBuffer::new_from_bytes(EGLD_TICKER);
        if token_id.is_egld() {
            return egld_ticker;
        }

        let as_buffer = token_id.clone().into_name();

        let ticker_start_index = 0;
        let ticker_end_index = as_buffer.len() - TOKEN_ID_SUFFIX_LEN;

        let result = as_buffer.copy_slice(ticker_start_index, ticker_end_index);

        match result {
            Some(r) => {
                if r == ManagedBuffer::new_from_bytes(WEGLD_TICKER) {
                    return egld_ticker;
                } else {
                    return r;
                }
            }
            None => sc_panic!(ERROR_TOKEN_TICKER_FETCH),
        }
    }

    #[proxy]
    fn safe_price_proxy(&self, sc_address: ManagedAddress) -> safe_price_proxy::ProxyTo<Self::Api>;
}

mod safe_price_proxy {
    multiversx_sc::imports!();

    #[multiversx_sc::proxy]
    pub trait SafePriceContract {
        #[view(getSafePriceByDefaultOffset)]
        fn get_safe_price_by_default_offset(
            &self,
            pair_address: ManagedAddress,
            input_payment: EsdtTokenPayment,
        ) -> EsdtTokenPayment;

        #[view(getLpTokensSafePriceByDefaultOffset)]
        fn get_lp_tokens_safe_price_by_default_offset(
            &self,
            pair_address: ManagedAddress,
            liquidity: BigUint,
        ) -> MultiValue2<EsdtTokenPayment, EsdtTokenPayment>;
    }
}
