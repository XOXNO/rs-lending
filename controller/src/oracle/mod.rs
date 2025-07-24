//! # Oracle Module - MultiversX Lending Protocol
//!
//! This module implements a sophisticated multi-layered oracle system that provides secure,
//! manipulation-resistant price feeds for the MultiversX lending protocol. The system combines
//! multiple price sources and validation mechanisms to ensure accurate asset pricing.
//!
//! ## Core Components
//!
//! ### Multi-Source Price Validation
//! - **Aggregator feeds:** Off-chain USD price data with staleness checks
//! - **Safe prices:** On-chain 15-minute TWAP from DEX contracts
//! - **Mixed validation:** Cross-validation between sources with tolerance bounds
//!
//! ### Token Type Support
//! - **Normal tokens:** Standard ERC-20 tokens with direct price feeds
//! - **LP tokens:** Liquidity pool tokens using Arda LP pricing formula
//! - **Derived tokens:** Liquid staking derivatives (xEGLD, LEGLD, LXOXNO)
//!
//! ### Security Features
//!
//! #### Price Deviation Protection
//! - **First tolerance bounds:** Strict validation (±2%) for normal operations
//! - **Second tolerance bounds:** Relaxed validation (±5%) with averaged pricing
//! - **Unsafe price blocking:** Prevents exploitable operations during high deviation
//!
//! #### TWAP Protection
//! - **15-minute freshness:** Prevents short-term price manipulation
//! - **DEX validation:** Only active trading pairs provide valid prices
//! - **Atomic resistance:** Time-weighted averages smooth manipulation attempts
//!
//! #### LP Token Security (Arda Formula)
//! - **Reserve manipulation resistance:** Mathematical formula prevents pool balance attacks
//! - **Anchor price validation:** Cross-validates LP prices against underlying assets
//! - **Impermanent loss accounting:** Fair valuation regardless of pool imbalances
//!
//! ## Mathematical Formulas
//!
//! ### Arda LP Pricing
//! ```
//! K = Reserve_A × Reserve_B
//! X' = sqrt(K × Price_B/Price_A)
//! Y' = sqrt(K × Price_A/Price_B)
//! LP_Value = X' × Price_A + Y' × Price_B
//! LP_Price = LP_Value ÷ Total_Supply
//! ```
//!
//! ### Price Tolerance Validation
//! ```
//! ratio = safe_price ÷ aggregator_price
//! valid = (lower_bound ≤ ratio ≤ upper_bound)
//! ```
//!
//! ### Derived Token Pricing
//! ```
//! LXOXNO_Price = LXOXNO_to_XOXNO_Rate × XOXNO_Price_in_EGLD
//! xEGLD_Price = Hatom_Exchange_Rate (direct EGLD rate)
//! LEGLD_Price = Salsa_Exchange_Rate (direct EGLD rate)
//! ```
//!
//! ## Operation Safety Matrix
//!
//! | Operation | First Tolerance | Second Tolerance | High Deviation |
//! |-----------|----------------|------------------|----------------|
//! | Supply    | ✅ Safe price   | ✅ Average price  | ✅ Average price|
//! | Repay     | ✅ Safe price   | ✅ Average price  | ✅ Average price|
//! | Borrow    | ✅ Safe price   | ✅ Average price  | ❌ Blocked      |
//! | Withdraw  | ✅ Safe price   | ✅ Average price  | ❌ Blocked      |
//! | Liquidate | ✅ Safe price   | ✅ Average price  | ❌ Blocked      |
//!
//! ## Supported Exchanges
//!
//! - **xExchange:** Primary DEX for TWAP and LP token pricing
//! - **Onedx:** Alternative DEX for safe price validation
//! - **Price Aggregator:** Off-chain oracle for USD-denominated feeds
//!
//! ## Cache Strategy
//!
//! - **Transaction-level caching:** Prevents oracle manipulation within single transaction
//! - **EGLD optimization:** Direct WAD precision for native token
//! - **Ticker normalization:** WEGLD treated as EGLD for pricing consistency

multiversx_sc::imports!();
use common_constants::{
    BPS_PRECISION, RAY_PRECISION, SECONDS_PER_MINUTE, USD_TICKER, WAD_HALF_PRECISION,
    WAD_PRECISION, WEGLD_TICKER,
};
use common_errors::{ERROR_PRICE_FEED_STALE, ERROR_UN_SAFE_PRICE_NOT_ALLOWED};
use common_proxies::{proxy_pool, proxy_xexchange_pair};
use common_structs::{
    ExchangeSource, MarketIndex, OracleProvider, OracleType, PriceFeedShort, PricingMethod,
};

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
    storage::Storage
    + helpers::MathsModule
    + common_math::SharedMathModule
    + common_rates::InterestRates
{
    /// Updates the interest index for a specific asset in the lending pool.
    ///
    /// **Purpose:** Synchronizes interest accrual for borrowers and lenders by updating
    /// the cumulative interest indices. These indices track how much interest has
    /// accumulated since the pool's inception.
    ///
    /// **How it works:**
    /// - In simulation mode: Calculates theoretical indices without state changes
    /// - In normal mode: Fetches current asset price and triggers pool index update
    /// - Uses current timestamp to calculate time-based interest accumulation
    ///
    /// **Security considerations:**
    /// - Relies on accurate price feeds from the oracle system
    /// - Price manipulation could affect interest calculations
    /// - Simulation mode prevents state corruption during view calls
    ///
    /// **Returns:** MarketIndex containing updated borrow and supply indices
    ///
    /// **Mathematical formula:**
    /// ```
    /// new_index = old_index * (1 + interest_rate * time_delta)
    /// ```
    fn update_asset_index(
        &self,
        asset_id: &EgldOrEsdtTokenIdentifier<Self::Api>,
        cache: &mut Cache<Self>,
        simulate: bool,
    ) -> MarketIndex<Self::Api> {
        let pool_address = cache.get_cached_pool_address(asset_id);
        if simulate {
            let last_timestamp = self.last_timestamp(pool_address.clone()).get();
            let borrowed = self.borrowed(pool_address.clone()).get();
            let current_borrowed_index = self.borrow_index(pool_address.clone()).get();
            let supplied = self.supplied(pool_address.clone()).get();
            let current_supply_index = self.supply_index(pool_address.clone()).get();
            let params = self.params(pool_address.clone()).get();
            self.simulate_update_indexes(
                cache.current_timestamp,
                last_timestamp,
                borrowed,
                current_borrowed_index,
                supplied,
                current_supply_index,
                params,
            )
        } else {
            let asset_price = self.get_token_price(asset_id, cache);
            self.tx()
                .to(pool_address)
                .typed(proxy_pool::LiquidityPoolProxy)
                .update_indexes(asset_price.price)
                .returns(ReturnsResult)
                .sync_call()
        }
    }

    /// Retrieves comprehensive price data for any supported token with intelligent caching.
    ///
    /// **Purpose:** Central entry point for all price queries in the lending protocol.
    /// Provides cached, validated price feeds with proper decimal scaling.
    ///
    /// **How it works:**
    /// 1. **EGLD optimization:** Returns WAD precision (10^18) immediately for EGLD
    /// 2. **Cache lookup:** Checks in-memory cache to avoid redundant oracle calls
    /// 3. **Oracle validation:** Ensures token has configured oracle provider
    /// 4. **Price resolution:** Delegates to specialized pricing functions based on token type
    /// 5. **Cache storage:** Stores result for subsequent queries within same transaction
    ///
    /// **Security considerations:**
    /// - Validates oracle configuration exists before proceeding
    /// - Cache prevents oracle manipulation within single transaction
    /// - EGLD special handling prevents oracle dependency for native token
    ///
    /// **Returns:** PriceFeedShort containing:
    /// - asset_decimals: Token's decimal precision
    /// - price: Current price in EGLD (WAD precision)
    fn get_token_price(
        &self,
        token_id: &EgldOrEsdtTokenIdentifier,
        cache: &mut Cache<Self>,
    ) -> PriceFeedShort<Self::Api> {
        let ticker = self.get_token_ticker(token_id, cache);
        if ticker == cache.egld_ticker {
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
            asset_decimals: data.asset_decimals,
            price,
        };

        cache.prices_cache.put(token_id, &feed);

        feed
    }

    /// Routes price discovery to appropriate method based on oracle token type.
    ///
    /// **Purpose:** Dispatches price calculation to specialized functions based on
    /// the token's characteristics and underlying value derivation method.
    ///
    /// **How it works:**
    /// - **Derived tokens:** Liquid staking derivatives (xEGLD, LEGLD, LXOXNO)
    /// - **LP tokens:** Liquidity pool tokens requiring reserve-based pricing
    /// - **Normal tokens:** Standard tokens with direct price feeds
    ///
    /// **Security considerations:**
    /// - Each token type has specific validation and security checks
    /// - Prevents incorrect pricing method application
    /// - Validates oracle type configuration
    ///
    /// **Returns:** Token price in EGLD with WAD precision (10^18)
    fn find_price_feed(
        &self,
        configs: &OracleProvider<Self::Api>,
        original_market_token: &EgldOrEsdtTokenIdentifier,
        cache: &mut Cache<Self>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        match configs.oracle_type {
            OracleType::Derived => self.get_derived_price(configs, cache, true),
            OracleType::Lp => self.get_safe_lp_price(configs, cache),
            OracleType::Normal => {
                self.get_normal_price_in_egld(configs, original_market_token, cache)
            },
            _ => sc_panic!(ERROR_INVALID_ORACLE_TOKEN_TYPE),
        }
    }

    /// Computes secure LP token price using the Arda LP pricing formula with multi-layered validation.
    ///
    /// **Purpose:** Calculates fair value for LP tokens while preventing price manipulation
    /// attacks through anchor price validation and tolerance checking.
    ///
    /// **How it works:**
    /// 1. **Reserve fetching:** Gets current pool reserves and total LP supply
    /// 2. **Safe price calculation:** Uses on-chain TWAP data for underlying assets
    /// 3. **Off-chain validation:** Fetches aggregator prices for comparison
    /// 4. **Anchor checking:** Validates prices are within acceptable deviation ranges
    /// 5. **Fallback logic:** Uses averaged price if within secondary tolerance bounds
    ///
    /// **Security considerations:**
    /// - **First tolerance check:** Strict bounds (e.g., ±2%) for normal operation
    /// - **Second tolerance check:** Wider bounds (e.g., ±5%) with averaged pricing
    /// - **Unsafe price protection:** Blocks dangerous operations (liquidations, borrows, withdrawals)
    /// - **Safe operations:** Allows supplies/repays even with price deviations (no exploit risk)
    ///
    /// **Mathematical formula (Arda LP):**
    /// ```
    /// LP_Price = (sqrt(Reserve_A * Reserve_B * Price_A / Price_B) * Price_A +
    ///             sqrt(Reserve_A * Reserve_B * Price_B / Price_A) * Price_B) / Total_Supply
    /// ```
    ///
    /// **Returns:** LP token price in EGLD per LP token (WAD precision)
    fn get_safe_lp_price(
        &self,
        configs: &OracleProvider<Self::Api>,
        cache: &mut Cache<Self>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let (reserve_0, reserve_1, total_supply) =
            self.get_reserves(&configs.oracle_contract_address);

        let safe_first_token_feed = self.get_token_price(&configs.base_token_id, cache);
        let safe_second_token_feed = self.get_token_price(&configs.quote_token_id, cache);

        // Convert raw BigUint reserves to ManagedDecimal with token-specific precision
        // This ensures consistent decimal arithmetic across different token denominations
        let reserve_first = self.to_decimal(reserve_0, safe_first_token_feed.asset_decimals);
        let reserve_second = self.to_decimal(reserve_1, safe_second_token_feed.asset_decimals);
        let total_supply = self.to_decimal(total_supply, configs.asset_decimals);

        let safe_lp_price = self.get_lp_price(
            configs,
            &reserve_first,
            &reserve_second,
            &total_supply,
            &safe_first_token_feed.price,
            &safe_second_token_feed.price,
        );

        let oracle_base_token_id = cache.get_cached_oracle(&configs.base_token_id);
        let oracle_quote_token_id = cache.get_cached_oracle(&configs.quote_token_id);

        let off_chain_first_egld_price = self.find_token_price_in_egld_from_aggregator(
            &oracle_base_token_id,
            &configs.base_token_id,
            cache,
        );

        let off_chain_second_egld_price = self.find_token_price_in_egld_from_aggregator(
            &oracle_quote_token_id,
            &configs.quote_token_id,
            cache,
        );

        let off_chain_lp_price = self.get_lp_price(
            configs,
            &reserve_first,
            &reserve_second,
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
            safe_lp_price
        } else if self.is_within_anchor(
            &safe_lp_price,
            &off_chain_lp_price,
            &tolerances.last_upper_ratio,
            &tolerances.last_lower_ratio,
        ) {
            avg_price
        } else {
            // SECURITY: Block dangerous operations (liquidation, borrow, withdraw) when prices deviate significantly
            // Allow safe operations (supply, repay) since they send funds TO the protocol (no exploitation risk)
            // This asymmetric safety model prevents oracle attacks while maintaining protocol functionality
            require!(cache.allow_unsafe_price, ERROR_UN_SAFE_PRICE_NOT_ALLOWED);
            avg_price
        }
    }

    // --- Derived Price Functions ---

    /// Calculates price for liquid staking derivative tokens.
    ///
    /// **Purpose:** Determines fair value for staking derivatives that represent
    /// claims on underlying staked assets with accumulated rewards.
    ///
    /// **How it works:**
    /// - Routes to exchange-specific pricing based on configured source
    /// - Each derivative has unique exchange rate mechanisms
    /// - Safe price check can be disabled for nested LP token calculations
    ///
    /// **Security considerations:**
    /// - Relies on trusted staking contract exchange rates
    /// - Safe price validation prevents manipulation in most contexts
    /// - Exchange rate contracts must be audited and secure
    ///
    /// **Supported derivatives:**
    /// - **xEGLD:** Hatom liquid staking (exchange rate from staking contract)
    /// - **LEGLD:** Salsa liquid staking (token price from contract)
    /// - **LXOXNO:** XOXNO liquid staking (rate × underlying XOXNO price)
    ///
    /// **Returns:** Derivative token price in EGLD (WAD precision)
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

    /// Calculates LEGLD price using Salsa liquid staking contract.
    ///
    /// **Purpose:** Determines LEGLD value based on the current exchange rate
    /// provided by the Salsa staking protocol.
    ///
    /// **How it works:**
    /// - Queries Salsa contract for current LEGLD → EGLD exchange rate
    /// - Rate includes accumulated staking rewards since inception
    /// - Direct conversion with proper decimal scaling
    ///
    /// **Security considerations:**
    /// - Relies on Salsa contract's exchange rate accuracy
    /// - No additional validation layers (trusts contract)
    /// - Contract should have governance and security measures
    ///
    /// **Mathematical formula:**
    /// ```
    /// LEGLD_Price = Salsa_Exchange_Rate (already in EGLD terms)
    /// ```
    ///
    /// **Returns:** LEGLD price in EGLD per LEGLD token (WAD precision)
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

        self.to_decimal(ratio, configs.asset_decimals)
    }

    /// Calculates xEGLD price using Hatom liquid staking exchange rate.
    ///
    /// **Purpose:** Determines xEGLD value based on the current exchange rate
    /// from the Hatom liquid staking protocol.
    ///
    /// **How it works:**
    /// - Queries Hatom liquid staking contract for exchange rate
    /// - Rate reflects accumulated staking rewards over time
    /// - Monotonically increasing rate (never decreases)
    ///
    /// **Security considerations:**
    /// - Trusts Hatom liquid staking contract's rate calculation
    /// - Exchange rate should only increase or remain stable
    /// - Contract governance protects against manipulation
    ///
    /// **Mathematical formula:**
    /// ```
    /// xEGLD_Price = Hatom_Exchange_Rate (EGLD per xEGLD)
    /// ```
    ///
    /// **Returns:** xEGLD price in EGLD per xEGLD token (WAD precision)
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

        self.to_decimal(ratio, configs.asset_decimals)
    }

    /// Calculates LXOXNO price using liquid staking rate and underlying XOXNO price.
    ///
    /// **Purpose:** Determines LXOXNO value by combining the staking exchange rate
    /// with the current market price of the underlying XOXNO token.
    ///
    /// **How it works:**
    /// 1. **Exchange rate:** Fetches LXOXNO → XOXNO rate from staking contract
    /// 2. **Underlying price:** Gets XOXNO price (safe vs aggregator based on context)
    /// 3. **Price calculation:** Multiplies rate by underlying price
    ///
    /// **Security considerations:**
    /// - **Safe price mode:** Uses secure TWAP for XOXNO price (normal operation)
    /// - **Aggregator mode:** Direct price feed (for LP token calculations)
    /// - Dual validation prevents manipulation of composite price
    /// - Staking contract rate assumed secure
    ///
    /// **Mathematical formula:**
    /// ```
    /// LXOXNO_Price = LXOXNO_to_XOXNO_Rate × XOXNO_Price_in_EGLD
    /// ```
    ///
    /// **Returns:** LXOXNO price in EGLD per LXOXNO token (WAD precision)
    fn get_lxoxno_derived_price(
        &self,
        configs: &OracleProvider<Self::Api>,
        cache: &mut Cache<Self>,
        safe_price_check: bool,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        // Step 1: Fetch the current LXOXNO → XOXNO exchange rate from the liquid staking contract
        // This rate includes all accumulated staking rewards since inception
        let ratio = self
            .tx()
            .to(&configs.oracle_contract_address)
            .typed(proxy_lxoxno::RsLiquidXoxnoProxy)
            .get_exchange_rate()
            .returns(ReturnsResult)
            .sync_call_readonly();
        let ratio_dec = self.to_decimal(ratio, configs.asset_decimals);

        let main_price = if safe_price_check {
            self.get_token_price(&configs.base_token_id, cache).price
        } else {
            // CONTEXT: When LXOXNO is used in LP tokens, we need aggregator pricing to avoid circular dependencies
            // Safe price check disabled to prevent infinite recursion in LP token pricing calculations
            // Aggregator provides direct XOXNO/USD price without TWAP validation
            self.get_token_price_in_egld_from_aggregator(
                &configs.base_token_id,
                configs.max_price_stale_seconds,
                cache,
            )
        };

        self.get_token_egld_value(&ratio_dec, &main_price)
    }

    // --- Safe Price Functions ---

    /// Retrieves TWAP-based safe price from DEX contracts with 15-minute freshness requirement.
    ///
    /// **Purpose:** Provides manipulation-resistant pricing using Time-Weighted Average Prices
    /// from decentralized exchanges, protecting against flash loan and MEV attacks.
    ///
    /// **How it works:**
    /// 1. **Exchange validation:** Ensures trading pair is active
    /// 2. **TWAP query:** Fetches 15-minute time-weighted average price
    /// 3. **Direction handling:** Manages token pair direction (A→B or B→A)
    /// 4. **Result processing:** Converts output to EGLD terms if needed
    ///
    /// **Security considerations:**
    /// - **15-minute TWAP:** Prevents short-term price manipulation
    /// - **Pair status check:** Only active pairs provide valid prices
    /// - **Exchange validation:** Supports Onedx and xExchange protocols
    /// - **Atomic resistance:** TWAP smooths out single-block manipulation
    ///
    /// **Supported exchanges:**
    /// - **Onedx:** Direct safe price query with pair validation
    /// - **xExchange:** Safe price via dedicated view contract
    ///
    /// **Returns:** Token price in EGLD based on 15-minute TWAP (WAD precision)
    fn get_safe_price(
        &self,
        configs: &OracleProvider<Self::Api>,
        token_id: &EgldOrEsdtTokenIdentifier,
        cache: &mut Cache<Self>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let one_token = BigUint::from(10u64).pow(configs.asset_decimals as u32);

        let result = if configs.exchange_source == ExchangeSource::Onedex {
            let pair_status = self
                .onedex_pair_state(
                    configs.oracle_contract_address.clone(),
                    configs.onedex_pair_id,
                )
                .get();
            require!(pair_status == StateOnedex::Active, ERROR_PAIR_NOT_ACTIVE);
            let from_identifier = token_id.clone().unwrap_esdt();
            let to_identifier = if from_identifier == configs.quote_token_id.clone().unwrap_esdt() {
                configs.base_token_id.clone()
            } else {
                configs.quote_token_id.clone()
            };
            self.tx()
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
            let pair_status = self
                .xexchange_pair_state(configs.oracle_contract_address.clone())
                .get();
            require!(pair_status == StateXExchange::Active, ERROR_PAIR_NOT_ACTIVE);

            self.safe_price_proxy(cache.safe_price_view.clone())
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
        let result_ticker = self.get_token_ticker(&new_token_id, cache);
        if result_ticker == cache.egld_ticker {
            self.to_decimal_wad(result.amount)
        } else {
            self.get_token_price(&new_token_id, cache).price
        }
    }

    /// Computes normal token price using sophisticated multi-source validation strategy.
    ///
    /// **Purpose:** Implements the core pricing logic that combines aggregator feeds
    /// and on-chain TWAP data with tolerance-based validation to prevent manipulation.
    ///
    /// **How it works:**
    /// 1. **Source gathering:** Collects prices from applicable sources (aggregator/safe)
    /// 2. **Tolerance checking:** Validates price consistency within bounds
    /// 3. **Fallback logic:** Uses averaged prices for moderate deviations
    /// 4. **Safety enforcement:** Blocks unsafe operations during high deviation
    ///
    /// **Pricing methods:**
    /// - **Aggregator:** Off-chain price feeds (fast, comprehensive)
    /// - **Safe:** On-chain TWAP (manipulation-resistant, slower)
    /// - **Mix:** Both sources with cross-validation
    ///
    /// **Security considerations:**
    /// - **First tolerance:** Tight bounds for normal operation
    /// - **Second tolerance:** Wider bounds with averaged pricing
    /// - **Unsafe price blocking:** Prevents exploitable operations
    /// - **Source independence:** Aggregator and DEX provide different attack vectors
    ///
    /// **Returns:** Validated token price in EGLD (WAD precision)
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

    /// Conditionally fetches aggregator price based on configured pricing method.
    ///
    /// **Purpose:** Retrieves off-chain oracle price when aggregator pricing
    /// is enabled in the token's configuration.
    ///
    /// **How it works:**
    /// - Checks if pricing method includes aggregator (Aggregator or Mix)
    /// - Returns OptionalValue to handle method-based conditional logic
    /// - Delegates to aggregator price fetching with staleness checks
    ///
    /// **Security considerations:**
    /// - Respects configured pricing method restrictions
    /// - Aggregator prices subject to staleness validation
    /// - Off-chain data requires independent validation
    ///
    /// **Returns:** OptionalValue<Price> - Some if aggregator enabled, None otherwise
    fn get_aggregator_price_if_applicable(
        &self,
        configs: &OracleProvider<Self::Api>,
        original_market_token: &EgldOrEsdtTokenIdentifier,
        cache: &mut Cache<Self>,
    ) -> OptionalValue<ManagedDecimal<Self::Api, NumDecimals>> {
        if configs.pricing_method == PricingMethod::Aggregator
            || configs.pricing_method == PricingMethod::Mix
        {
            OptionalValue::Some(self.get_token_price_in_egld_from_aggregator(
                original_market_token,
                configs.max_price_stale_seconds,
                cache,
            ))
        } else {
            OptionalValue::None
        }
    }

    /// Conditionally fetches safe TWAP price based on configured pricing method.
    ///
    /// **Purpose:** Retrieves on-chain TWAP price when safe pricing
    /// is enabled in the token's configuration.
    ///
    /// **How it works:**
    /// - Checks if pricing method includes safe price (Safe or Mix)
    /// - Returns OptionalValue for conditional processing
    /// - Delegates to TWAP price fetching with DEX validation
    ///
    /// **Security considerations:**
    /// - Honors configured pricing method settings
    /// - TWAP prices resist short-term manipulation
    /// - On-chain data provides manipulation resistance
    ///
    /// **Returns:** OptionalValue<Price> - Some if safe pricing enabled, None otherwise
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

    /// Determines final price using tolerance-based validation between multiple sources.
    ///
    /// **Purpose:** Implements sophisticated price validation logic that prevents
    /// oracle manipulation while maintaining system functionality during market volatility.
    ///
    /// **How it works:**
    /// 1. **Both sources:** Validates prices within tolerance bounds
    /// 2. **First tolerance:** Uses safe price if within tight bounds
    /// 3. **Second tolerance:** Uses averaged price for moderate deviations
    /// 4. **High deviation:** Blocks unsafe operations, allows safe operations
    /// 5. **Single source:** Uses available price directly
    ///
    /// **Security considerations:**
    /// - **Tolerance validation:** Prevents accepting manipulated prices
    /// - **Operation safety:** Blocks exploitable actions during price uncertainty
    /// - **Market continuity:** Allows non-exploitable operations to continue
    /// - **Source redundancy:** Multiple price sources increase security
    ///
    /// **Tolerance bounds:**
    /// - **First bounds:** Strict tolerance (e.g., ±2%)
    /// - **Last bounds:** Relaxed tolerance (e.g., ±5%)
    ///
    /// **Returns:** Final validated price in EGLD (WAD precision)
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
                    // SECURITY: Block dangerous operations (liquidation, borrow, withdraw) when prices deviate significantly
                    // Allow safe operations (supply, repay) since they send funds TO the protocol (no exploitation risk)
                    // This asymmetric safety model prevents oracle attacks while maintaining protocol functionality
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

    /// Converts USD-denominated aggregator price to EGLD terms with staleness validation.
    ///
    /// **Purpose:** Transforms off-chain USD price feeds into EGLD-denominated prices
    /// required by the lending protocol, with comprehensive staleness checks.
    ///
    /// **How it works:**
    /// 1. **Price fetching:** Gets USD price from aggregator with staleness check
    /// 2. **USD conversion:** Divides token/USD by EGLD/USD to get token/EGLD
    /// 3. **Precision handling:** Maintains WAD precision throughout calculation
    /// 4. **Scaling:** Ensures result matches protocol's precision requirements
    ///
    /// **Security considerations:**
    /// - **Staleness validation:** Rejects outdated price feeds
    /// - **Aggregator status:** Checks if price aggregator is operational
    /// - **Precision safety:** Prevents precision loss in calculations
    /// - **USD dependency:** Requires stable EGLD/USD reference price
    ///
    /// **Mathematical formula:**
    /// ```
    /// Token_Price_EGLD = Token_Price_USD ÷ EGLD_Price_USD
    /// ```
    ///
    /// **Returns:** Token price in EGLD per token unit (WAD precision)
    fn get_token_price_in_egld_from_aggregator(
        &self,
        token_id: &EgldOrEsdtTokenIdentifier,
        max_seconds_stale: u64,
        cache: &mut Cache<Self>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let ticker = self.get_token_ticker(token_id, cache);
        let feed =
            self.get_aggregator_price_feed(ticker, &cache.price_aggregator_sc, max_seconds_stale);
        let token_usd_price = self.to_decimal_wad(feed.price);
        self.rescale_half_up(
            &self.div_half_up(&token_usd_price, &cache.egld_usd_price, RAY_PRECISION),
            WAD_PRECISION,
        )
    }

    /// Routes aggregator price fetching based on token type (normal vs derived).
    ///
    /// **Purpose:** Provides appropriate aggregator pricing for both standard tokens
    /// and derived tokens, handling their different price calculation requirements.
    ///
    /// **How it works:**
    /// - **Derived tokens:** Uses specialized derived price calculation
    /// - **Normal tokens:** Direct aggregator price conversion to EGLD
    /// - Disables safe price check for derived tokens in LP contexts
    ///
    /// **Security considerations:**
    /// - Maintains token type-specific security measures
    /// - Derived tokens use appropriate exchange rate validation
    /// - Normal tokens use standard aggregator validation
    ///
    /// **Returns:** Token price in EGLD from aggregator sources (WAD precision)
    fn find_token_price_in_egld_from_aggregator(
        &self,
        configs: &OracleProvider<Self::Api>,
        token_id: &EgldOrEsdtTokenIdentifier,
        cache: &mut Cache<Self>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        if configs.oracle_type == OracleType::Derived {
            self.get_derived_price(configs, cache, false)
        } else {
            self.get_token_price_in_egld_from_aggregator(
                token_id,
                configs.max_price_stale_seconds,
                cache,
            )
        }
    }
    /// Validates price deviation against anchor price using configurable tolerance bounds.
    ///
    /// **Purpose:** Implements the core price validation logic that prevents acceptance
    /// of manipulated prices while allowing reasonable market movements.
    ///
    /// **How it works:**
    /// 1. **Ratio calculation:** Computes safe_price/aggregator_price ratio
    /// 2. **Bound checking:** Validates ratio falls within [lower, upper] bounds
    /// 3. **Precision handling:** Uses BPS precision for accurate comparisons
    ///
    /// **Security considerations:**
    /// - **Symmetric validation:** Protects against manipulation in both directions
    /// - **Configurable bounds:** Allows protocol-specific tolerance settings
    /// - **Precision safety:** Uses high precision for accurate boundary checks
    ///
    /// **Mathematical formula:**
    /// ```
    /// ratio = safe_price ÷ aggregator_price
    /// valid = (lower_bound ≤ ratio ≤ upper_bound)
    /// ```
    ///
    /// **Example bounds:**
    /// - First: 9800-10200 BPS (±2%)
    /// - Last: 9500-10500 BPS (±5%)
    ///
    /// **Returns:** true if price deviation is within acceptable bounds

    fn is_within_anchor(
        &self,
        aggregator_price: &ManagedDecimal<Self::Api, NumDecimals>,
        safe_price: &ManagedDecimal<Self::Api, NumDecimals>,
        upper_bound_ratio: &ManagedDecimal<Self::Api, NumDecimals>,
        lower_bound_ratio: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> bool {
        let anchor_ratio = self.rescale_half_up(
            &self.div_half_up(safe_price, aggregator_price, RAY_PRECISION),
            BPS_PRECISION,
        );
        &anchor_ratio <= upper_bound_ratio && &anchor_ratio >= lower_bound_ratio
    }

    /// Retrieves token ticker with special handling for EGLD and WEGLD equivalence.
    ///
    /// **Purpose:** Normalizes token identification for price feeds, treating
    /// WEGLD (wrapped EGLD) identically to native EGLD for pricing purposes.
    ///
    /// **How it works:**
    /// 1. **EGLD detection:** Returns cached EGLD ticker for native EGLD
    /// 2. **WEGLD normalization:** Converts WEGLD ticker to EGLD ticker
    /// 3. **Standard tokens:** Returns actual token ticker unchanged
    ///
    /// **Security considerations:**
    /// - **WEGLD equivalence:** Prevents arbitrage between EGLD/WEGLD pricing
    /// - **Ticker consistency:** Ensures consistent price feed lookups
    /// - **Cache usage:** Leverages cached ticker for efficiency
    ///
    /// **Returns:** Normalized token ticker for price feed queries
    fn get_token_ticker(
        &self,
        token_id: &EgldOrEsdtTokenIdentifier,
        cache: &mut Cache<Self>,
    ) -> ManagedBuffer {
        if token_id.is_egld() || token_id.clone().into_name() == cache.egld_ticker {
            return cache.egld_ticker.clone();
        }
        let result = unsafe { token_id.as_esdt_option().unwrap_unchecked().ticker() };
        if result == ManagedBuffer::new_from_bytes(WEGLD_TICKER) {
            cache.egld_ticker.clone()
        } else {
            result
        }
    }

    /// Calculates LP token price using the sophisticated Arda LP pricing formula.
    ///
    /// **Purpose:** Implements the mathematically sound Arda LP pricing formula
    /// that provides fair valuation for LP tokens based on underlying asset reserves
    /// and their current market prices.
    ///
    /// **How it works:**
    /// 1. **Constant product:** Calculates K = Reserve_A × Reserve_B
    /// 2. **Price ratios:** Computes Price_B/Price_A and Price_A/Price_B
    /// 3. **Modified reserves:** Applies sqrt transformation with price ratios
    /// 4. **Asset valuation:** Values modified reserves at current prices
    /// 5. **LP price:** Divides total value by LP token supply
    ///
    /// **Security considerations:**
    /// - **Reserve manipulation:** Formula resists reserve-based attacks
    /// - **Price consistency:** Requires accurate underlying asset prices
    /// - **Mathematical soundness:** Arda formula prevents common LP pricing exploits
    /// - **Precision safety:** Maintains WAD precision throughout calculations
    ///
    /// **Mathematical formula (Arda LP):**
    /// ```
    /// K = Reserve_A × Reserve_B
    /// X' = sqrt(K × Price_B/Price_A)
    /// Y' = sqrt(K × Price_A/Price_B)
    /// LP_Value = X' × Price_A + Y' × Price_B
    /// LP_Price = LP_Value ÷ Total_Supply
    /// ```
    ///
    /// **Why Arda formula:**
    /// - Prevents sandwich attacks on LP pricing
    /// - Accounts for impermanent loss in valuation
    /// - Provides fair value regardless of pool balance manipulation
    ///
    /// **Returns:** LP token price in EGLD per LP token (WAD precision)
    fn get_lp_price(
        &self,
        configs: &OracleProvider<Self::Api>,
        reserve_first: &ManagedDecimal<Self::Api, NumDecimals>, // Amount of Token A (scaled by WAD)
        reserve_second: &ManagedDecimal<Self::Api, NumDecimals>, // Amount of Token B (scaled by WAD)
        total_supply: &ManagedDecimal<Self::Api, NumDecimals>, // Amount of LP token (scaled by LP decimals)
        first_token_egld_price: &ManagedDecimal<Self::Api, NumDecimals>, // Price A (EGLD/UnitA, scaled WAD)
        second_token_egld_price: &ManagedDecimal<Self::Api, NumDecimals>, // Price B (EGLD/UnitB, scaled WAD)
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        if configs.exchange_source != ExchangeSource::XExchange {
            sc_panic!(ERROR_INVALID_EXCHANGE_SOURCE);
        }

        // PRECISION: All inputs assumed to be in WAD precision (10^18) as validated by caller
        // This ensures mathematical consistency throughout the Arda LP pricing calculation
        let price_a = first_token_egld_price;
        let price_b = second_token_egld_price;

        // STEP 1: Calculate constant product K = Reserve_A × Reserve_B
        // Mathematical foundation of the Arda LP pricing formula
        // Result precision: WAD (maintains 18 decimal precision)
        let constant_product = self.mul_half_up(reserve_first, reserve_second, WAD_PRECISION);

        // STEP 2: Calculate price ratios for geometric mean calculation
        // These ratios enable the sqrt transformations that make Arda formula manipulation-resistant
        let price_ratio_x = self.div_half_up(price_b, price_a, WAD_PRECISION); // Price_B / Price_A (unitless, WAD)
        let price_ratio_y = self.div_half_up(price_a, price_b, WAD_PRECISION); // Price_A / Price_B (unitless, WAD)

        // STEP 3: Prepare values for square root operations in Arda formula
        // These represent K × price_ratio terms used in the geometric mean calculation
        // Mathematical basis: sqrt(Reserve_A × Reserve_B × Price_ratio)
        let inner_x = self.mul_half_up(&constant_product, &price_ratio_x, WAD_PRECISION);
        let inner_y = self.mul_half_up(&constant_product, &price_ratio_y, WAD_PRECISION);

        // STEP 4a: Calculate modified reserve X' using geometric mean approach
        // X' = sqrt(K × Price_B/Price_A) where K is the constant product
        // Step 4a.1: Extract square root from WAD-precision value
        let sqrt_raw_x = inner_x.into_raw_units().sqrt(); // sqrt(K × ratio * 10^18) = result * 10^9

        // Step 4a.2: Convert sqrt result to ManagedDecimal with half-WAD precision (9 decimals)
        // Square root of WAD value naturally has 9 decimals: sqrt(10^18) = 10^9
        let sqrt_decimal_temp_x = self.to_decimal(sqrt_raw_x, WAD_HALF_PRECISION);

        // Step 4a.3: Create scaling factor to restore full WAD precision
        // Need to multiply by 10^9 to get from 9 decimals back to 18 decimals
        let ten_pow_9 = BigUint::from(10u64).pow(WAD_HALF_PRECISION as u32);
        let sqrt_wad_factor = self.to_decimal(ten_pow_9, WAD_HALF_PRECISION);

        // Step 4a.4: Scale back to full WAD precision for consistent arithmetic
        // Mathematical requirement: maintain 18-decimal precision throughout calculation
        let x_prime = self.mul_half_up(&sqrt_decimal_temp_x, &sqrt_wad_factor, WAD_PRECISION); // X' in WAD precision

        // STEP 4b: Calculate modified reserve Y' using same geometric mean approach
        // Y' = sqrt(K × Price_A/Price_B) - symmetric calculation to X'
        let sqrt_raw_y = inner_y.into_raw_units().sqrt();
        let sqrt_decimal_temp_y = self.to_decimal(sqrt_raw_y, WAD_HALF_PRECISION);
        // Step 4b.1-4: Apply same sqrt and scaling operations as X' calculation
        // Reuse scaling factor for efficiency and consistency
        let y_prime = self.mul_half_up(&sqrt_decimal_temp_y, &sqrt_wad_factor, WAD_PRECISION); // Y' in WAD precision

        // STEP 5: Calculate total LP value by pricing modified reserves
        // Value_A = X' × Price_A (modified reserve A valued at current price)
        let value_a = self.mul_half_up(&x_prime, price_a, WAD_PRECISION);
        // Value_B = Y' × Price_B (modified reserve B valued at current price)
        let value_b = self.mul_half_up(&y_prime, price_b, WAD_PRECISION);

        let lp_total_value_egld = value_a + value_b; // Total LP value in EGLD (WAD precision)

        // STEP 6: Calculate final LP token price
        // Convert total_supply to WAD precision for consistent division
        let total_supply_wad = self.rescale_half_up(total_supply, WAD_PRECISION);
        // Final calculation: LP_Price = Total_Value ÷ Total_Supply
        // Result: Price per LP token in EGLD (WAD precision)
        self.rescale_half_up(
            &self.div_half_up(&lp_total_value_egld, &total_supply_wad, WAD_PRECISION),
            WAD_PRECISION,
        )
    }

    /// Fetches and validates price feed from the off-chain price aggregator.
    ///
    /// **Purpose:** Retrieves USD-denominated price data from the aggregator oracle
    /// with comprehensive validation of feed freshness and system status.
    ///
    /// **How it works:**
    /// 1. **System validation:** Checks aggregator is configured and operational
    /// 2. **Pause status:** Ensures aggregator is not paused
    /// 3. **Feed existence:** Validates token pair has available price data
    /// 4. **Staleness check:** Rejects feeds older than maximum age
    /// 5. **Feed creation:** Constructs validated price feed object
    ///
    /// **Security considerations:**
    /// - **Staleness protection:** Prevents using outdated price data
    /// - **Pause mechanism:** Respects emergency pause functionality
    /// - **Feed validation:** Ensures price data exists for token pair
    /// - **System health:** Checks aggregator configuration and status
    ///
    /// **Staleness requirements:**
    /// - Maximum age varies by token (typically 300-900 seconds)
    /// - Critical tokens may have stricter freshness requirements
    /// - Emergency situations may require immediate price updates
    ///
    /// **Returns:** PriceFeed with validated timestamp, price, and metadata
    fn get_aggregator_price_feed(
        &self,
        from_ticker: ManagedBuffer,
        price_aggregator_sc: &ManagedAddress,
        max_seconds_stale: u64,
    ) -> PriceFeed<Self::Api> {
        require!(
            !price_aggregator_sc.is_zero(),
            ERROR_PRICE_AGGREGATOR_NOT_SET
        );
        require!(
            !self
                .price_aggregator_paused_state(price_aggregator_sc.clone())
                .get(),
            PAUSED_ERROR
        );

        let token_pair = TokenPair {
            from: from_ticker,
            to: ManagedBuffer::new_from_bytes(USD_TICKER),
        };
        let round_values = self.rounds(
            price_aggregator_sc.clone(),
            token_pair.from.clone(),
            token_pair.to.clone(),
        );
        require!(!round_values.is_empty(), TOKEN_PAIR_NOT_FOUND_ERROR);

        let feed = self.make_price_feed(token_pair, round_values.get());

        require!(
            self.blockchain().get_block_timestamp() - feed.timestamp < max_seconds_stale,
            ERROR_PRICE_FEED_STALE
        );

        feed
    }

    /// Constructs a standardized price feed object from aggregator data.
    ///
    /// **Purpose:** Creates a uniform price feed structure from raw aggregator
    /// response data for consistent handling throughout the system.
    ///
    /// **How it works:**
    /// - Maps token pair information to feed structure
    /// - Preserves timestamp and round metadata
    /// - Maintains price precision from aggregator
    ///
    /// **Security considerations:**
    /// - Preserves all validation metadata
    /// - Maintains traceability through round IDs
    /// - No data transformation that could introduce errors
    ///
    /// **Returns:** Structured PriceFeed with complete metadata

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

    /// Fetches current pool reserves and total LP supply from xExchange pair contract.
    ///
    /// **Purpose:** Retrieves real-time liquidity pool state required for LP token
    /// pricing calculations using the Arda formula.
    ///
    /// **How it works:**
    /// - Queries xExchange pair contract for current state
    /// - Returns atomic snapshot of reserves and supply
    /// - Used immediately in LP pricing to prevent manipulation
    ///
    /// **Security considerations:**
    /// - **Atomic snapshot:** Single call prevents reserve/supply manipulation
    /// - **Real-time data:** Uses current block state for accurate pricing
    /// - **Contract trust:** Relies on xExchange pair contract integrity
    ///
    /// **Returns:** (Reserve_Token0, Reserve_Token1, Total_LP_Supply)
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
    }
}
