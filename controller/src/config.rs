multiversx_sc::imports!();

use crate::helpers;
use crate::oracle;
use crate::storage;
use crate::utils;
use common_errors::*;
pub use common_events::*;
pub use common_proxies::*;

/// Configuration module for the MultiversX lending protocol controller.
///
/// This module handles critical protocol governance functions including:
/// - Oracle configuration and price feed management
/// - Asset configuration with risk parameters (LTV, liquidation thresholds, fees)
/// - E-mode category management for optimized asset usage
/// - Protocol service address management (aggregator, accumulator, etc.)
///
/// # Security Considerations
/// All functions in this module are restricted to the contract owner and affect
/// core protocol parameters. Changes must be carefully validated to maintain
/// protocol safety and prevent economic exploits.
///
/// # Governance Aspects
/// This module implements governance-controlled configuration that determines:
/// - Risk parameters for lending and borrowing
/// - Oracle sources and tolerance settings for price feeds
/// - Efficiency mode categories for correlated assets
/// - Protocol fee collection and revenue distribution
#[multiversx_sc::module]
pub trait ConfigModule:
    storage::Storage
    + utils::LendingUtilsModule
    + common_events::EventsModule
    + oracle::OracleModule
    + helpers::MathsModule
    + common_math::SharedMathModule
    + common_rates::InterestRates
{
    /// Registers a new NFT token for tracking account positions in the lending protocol.
    ///
    /// **Purpose**: Creates a dynamic NFT collection used to represent user positions
    /// as transferable tokens. Each NFT contains position data including deposits,
    /// borrows, and risk parameters.
    ///
    /// **How it works**:
    /// 1. Issues a new ESDT token with DynamicNFT properties
    /// 2. Sets all necessary roles for minting/burning position NFTs
    /// 3. Enables position tracking and transferability
    ///
    /// **Security checks**:
    /// - Only contract owner can register account tokens
    /// - Requires EGLD payment for token issuance (protocol fee)
    /// - Validates token name and ticker parameters
    ///
    /// **Governance considerations**:
    /// This is a one-time setup function that establishes the position tracking
    /// mechanism. Once set, users can mint position NFTs representing their
    /// lending/borrowing activities.
    ///
    /// # Arguments
    /// - `token_name`: Human-readable name for the position NFT collection
    /// - `ticker`: Short ticker symbol for the NFT collection
    ///
    /// # Returns
    /// Nothing - sets up the account token for future position minting
    ///
    /// # Payment Required
    /// - EGLD payment for ESDT token issuance (amount determined by protocol)
    #[only_owner]
    #[payable("EGLD")]
    #[endpoint(registerAccountToken)]
    fn register_account_token(&self, token_name: ManagedBuffer, ticker: ManagedBuffer) {
        let payment_amount = self.call_value().egld();
        self.account().issue_and_set_all_roles(
            EsdtTokenType::DynamicNFT,
            payment_amount.clone_value(),
            token_name,
            ticker,
            1,
            None,
        );
    }

    /// Configures the oracle for a token’s price feed.
    /// Sets up pricing method, source, and tolerances.
    ///
    /// # Arguments
    /// - `market_token`: Token identifier (EGLD or ESDT).
    /// - `decimals`: Decimal precision for the price.
    /// - `contract_address`: Address of the oracle contract.
    /// - `pricing_method`: Method for price determination (e.g., Safe, Aggregator).
    /// - `token_type`: Oracle type (e.g., Normal, Derived).
    /// - `source`: Exchange source (e.g., XExchange).
    /// - `first_tolerance`, `last_tolerance`: Tolerance values for price fluctuations.
    ///
    /// # Errors
    /// - `ERROR_ORACLE_TOKEN_NOT_FOUND`: If oracle already exists for the token.
    #[only_owner]
    #[endpoint(setTokenOracle)]
    fn set_token_oracle(
        &self,
        market_token: &EgldOrEsdtTokenIdentifier,
        decimals: usize,
        contract_address: &ManagedAddress,
        pricing_method: PricingMethod,
        token_type: OracleType,
        source: ExchangeSource,
        first_tolerance: BigUint,
        last_tolerance: BigUint,
        max_price_stale_seconds: u64,
        one_dex_pair_id: OptionalValue<usize>,
    ) {
        let mapper = self.token_oracle(market_token);

        require!(mapper.is_empty(), ERROR_ORACLE_TOKEN_EXISTING);
        let one_dex_id = match one_dex_pair_id {
            OptionalValue::Some(id) => id,
            OptionalValue::None => 0,
        };
        let first_token_id = match source {
            ExchangeSource::LXOXNO => {
                let token_id = self
                    .tx()
                    .to(contract_address)
                    .typed(proxy_lxoxno::RsLiquidXoxnoProxy)
                    .main_token()
                    .returns(ReturnsResult)
                    .sync_call_readonly();
                EgldOrEsdtTokenIdentifier::esdt(token_id)
            },
            ExchangeSource::Onedex => {
                require!(one_dex_id > 0, ERROR_INVALID_ONEDEX_PAIR_ID);
                let token_id = self
                    .tx()
                    .to(contract_address)
                    .typed(proxy_onedex::OneDexProxy)
                    .pair_first_token_id(one_dex_id)
                    .returns(ReturnsResult)
                    .sync_call_readonly();
                EgldOrEsdtTokenIdentifier::esdt(token_id)
            },
            ExchangeSource::XExchange => {
                let token_id = self
                    .tx()
                    .to(contract_address)
                    .typed(proxy_xexchange_pair::PairProxy)
                    .first_token_id()
                    .returns(ReturnsResult)
                    .sync_call_readonly();
                EgldOrEsdtTokenIdentifier::esdt(token_id)
            },
            ExchangeSource::XEGLD => EgldOrEsdtTokenIdentifier::egld(),
            ExchangeSource::LEGLD => EgldOrEsdtTokenIdentifier::egld(),
            _ => {
                panic!("Invalid exchange source")
            },
        };

        let second_token_id = match source {
            ExchangeSource::XExchange => {
                let token_id = self
                    .tx()
                    .to(contract_address)
                    .typed(proxy_xexchange_pair::PairProxy)
                    .second_token_id()
                    .returns(ReturnsResult)
                    .sync_call_readonly();
                EgldOrEsdtTokenIdentifier::esdt(token_id)
            },
            ExchangeSource::Onedex => {
                let token_id = self
                    .tx()
                    .to(contract_address)
                    .typed(proxy_onedex::OneDexProxy)
                    .pair_second_token_id(one_dex_id)
                    .returns(ReturnsResult)
                    .sync_call_readonly();
                EgldOrEsdtTokenIdentifier::esdt(token_id)
            },
            ExchangeSource::XEGLD => first_token_id.clone(),
            ExchangeSource::LEGLD => first_token_id.clone(),
            ExchangeSource::LXOXNO => first_token_id.clone(),
            _ => {
                panic!("Invalid exchange source")
            },
        };

        let tolerance = self.validate_and_calculate_tolerances(&first_tolerance, &last_tolerance);

        let oracle = OracleProvider {
            base_token_id: first_token_id,
            quote_token_id: second_token_id,
            oracle_contract_address: contract_address.clone(),
            oracle_type: token_type,
            exchange_source: source,
            asset_decimals: decimals,
            pricing_method,
            tolerance,
            onedex_pair_id: one_dex_id,
            max_price_stale_seconds,
        };

        mapper.set(&oracle);
    }

    /// Updates the tolerance settings for a token’s oracle.
    /// Adjusts acceptable price deviation ranges.
    ///
    /// # Arguments
    /// - `market_token`: Token identifier (EGLD or ESDT).
    /// - `first_tolerance`, `last_tolerance`: New tolerance values.
    ///
    /// # Errors
    /// - `ERROR_ORACLE_TOKEN_NOT_FOUND`: If no oracle exists for the token.
    #[only_owner]
    #[endpoint(editTokenOracleTolerance)]
    fn edit_token_oracle_tolerance(
        &self,
        market_token: &EgldOrEsdtTokenIdentifier,
        first_tolerance: BigUint,
        last_tolerance: BigUint,
    ) {
        require!(
            !self.token_oracle(market_token).is_empty(),
            ERROR_ORACLE_TOKEN_NOT_FOUND
        );

        let tolerance = self.validate_and_calculate_tolerances(&first_tolerance, &last_tolerance);
        self.token_oracle(market_token).update(|oracle| {
            oracle.tolerance = tolerance;
        });
    }

    /// Sets the price aggregator contract address.
    /// Configures the source for aggregated price data.
    ///
    /// # Arguments
    /// - `aggregator`: Address of the price aggregator contract.
    ///
    /// # Errors
    /// - `ERROR_INVALID_AGGREGATOR`: If address is zero or not a smart contract.
    #[only_owner]
    #[endpoint(setAggregator)]
    fn set_aggregator(&self, aggregator: ManagedAddress) {
        require!(!aggregator.is_zero(), ERROR_INVALID_AGGREGATOR);

        require!(
            self.blockchain().is_smart_contract(&aggregator),
            ERROR_INVALID_AGGREGATOR
        );
        self.price_aggregator_address().set(&aggregator);
    }

    /// Sets the Swap Router contract address.
    /// Configures the source for Swap Router price data.
    ///
    /// # Arguments
    /// - `address`: Address of the Swap Router contract.
    ///
    /// # Errors
    /// - `ERROR_INVALID_AGGREGATOR`: If address is zero or not a smart contract.
    #[only_owner]
    #[endpoint(setSwapRouter)]
    fn set_swap_router(&self, address: ManagedAddress) {
        require!(!address.is_zero(), ERROR_INVALID_AGGREGATOR);

        require!(
            self.blockchain().is_smart_contract(&address),
            ERROR_INVALID_AGGREGATOR
        );
        self.swap_router().set(&address);
    }

    /// Sets the accumulator contract address.
    /// Configures where protocol revenue is collected.
    ///
    /// # Arguments
    /// - `accumulator`: Address of the accumulator contract.
    ///
    /// # Errors
    /// - `ERROR_INVALID_AGGREGATOR`: If address is zero or not a smart contract.
    #[only_owner]
    #[endpoint(setAccumulator)]
    fn set_accumulator(&self, accumulator: ManagedAddress) {
        require!(!accumulator.is_zero(), ERROR_INVALID_AGGREGATOR);

        require!(
            self.blockchain().is_smart_contract(&accumulator),
            ERROR_INVALID_AGGREGATOR
        );
        self.accumulator_address().set(&accumulator);
    }

    /// Sets the safe price view contract address.
    /// Configures the source for safe price data in liquidation checks.
    ///
    /// # Arguments
    /// - `safe_view_address`: Address of the safe price view contract.
    ///
    /// # Errors
    /// - `ERROR_INVALID_AGGREGATOR`: If address is zero or not a smart contract.
    #[only_owner]
    #[endpoint(setSafePriceView)]
    fn set_safe_price_view(&self, safe_view_address: ManagedAddress) {
        require!(!safe_view_address.is_zero(), ERROR_INVALID_AGGREGATOR);

        require!(
            self.blockchain().is_smart_contract(&safe_view_address),
            ERROR_INVALID_AGGREGATOR
        );
        self.safe_price_view().set(&safe_view_address);
    }

    /// Sets the template address for liquidity pools.
    /// Used for deploying new pools with a standard template.
    ///
    /// # Arguments
    /// - `address`: Address of the liquidity pool template contract.
    ///
    /// # Errors
    /// - `ERROR_INVALID_LIQUIDITY_POOL_TEMPLATE`: If address is zero or not a smart contract.
    #[only_owner]
    #[endpoint(setLiquidityPoolTemplate)]
    fn set_liquidity_pool_template(&self, address: ManagedAddress) {
        require!(!address.is_zero(), ERROR_INVALID_LIQUIDITY_POOL_TEMPLATE);
        require!(
            self.blockchain().is_smart_contract(&address),
            ERROR_INVALID_LIQUIDITY_POOL_TEMPLATE
        );

        self.liq_pool_template_address().set(&address);
    }

    /// Creates a new efficiency mode (e-mode) category with optimized risk parameters.
    ///
    /// **Purpose**: E-mode categories allow users to achieve higher capital efficiency
    /// when using correlated assets (e.g., different stablecoins or ETH derivatives).
    /// Categories group assets with similar risk profiles for optimized lending terms.
    ///
    /// **How it works**:
    /// 1. Increments the global e-mode category counter
    /// 2. Converts risk parameters from basis points to decimal representation
    /// 3. Creates EModeCategory struct with new ID and parameters
    /// 4. Stores the category and emits configuration event
    ///
    /// **Risk parameter conversion**:
    /// All parameters are converted using `to_decimal_bps()` formula:
    /// ```
    /// decimal_value = bps_value / 10000
    /// ```
    /// This converts basis points (1 bps = 0.01%) to decimal representation.
    ///
    /// **E-mode benefits**:
    /// - Higher LTV ratios for correlated assets
    /// - Lower liquidation thresholds (safer for protocol)
    /// - Optimized capital utilization for users
    /// - Reduced risk through asset correlation
    ///
    /// **Security considerations**:
    /// - Only owner can create e-mode categories (governance control)
    /// - Risk parameters must be carefully calibrated for asset correlations
    /// - Categories cannot be deleted, only deprecated
    /// - Assets must be explicitly added to categories after creation
    ///
    /// **Governance impact**:
    /// E-mode categories affect user capital efficiency and protocol risk.
    /// Parameters should reflect actual asset correlations and market conditions.
    ///
    /// # Arguments
    /// - `ltv`: Loan-to-value ratio in basis points (e.g., 8000 = 80%)
    /// - `liquidation_threshold`: Liquidation threshold in basis points (e.g., 8500 = 85%)
    /// - `liquidation_bonus`: Liquidation bonus in basis points (e.g., 500 = 5%)
    ///
    /// # Returns
    /// Nothing - creates new e-mode category with auto-assigned ID
    ///
    /// # Mathematical formulas
    /// - **LTV**: Maximum borrowing capacity = collateral_value * ltv
    /// - **Liquidation threshold**: Position becomes liquidatable when health_factor < 1
    ///   where health_factor = (collateral * threshold) / debt
    /// - **Liquidation bonus**: Additional reward for liquidators = liquidated_amount * bonus
    #[only_owner]
    #[endpoint(addEModeCategory)]
    fn add_e_mode_category(
        &self,
        ltv: BigUint,
        liquidation_threshold: BigUint,
        liquidation_bonus: BigUint,
    ) {
        let map = self.last_e_mode_category_id();

        let last_id = map.get();
        let category = EModeCategory {
            category_id: last_id + 1,
            loan_to_value: self.to_decimal_bps(ltv),
            liquidation_threshold: self.to_decimal_bps(liquidation_threshold),
            liquidation_bonus: self.to_decimal_bps(liquidation_bonus),
            is_deprecated: false,
        };

        map.set(category.category_id);

        self.update_e_mode_category_event(&category);
        self.e_mode_category()
            .insert(category.category_id, category);
    }

    /// Edits an existing e-mode category’s parameters.
    /// Updates risk settings for the category.
    ///
    /// # Arguments
    /// - `category`: The updated `EModeCategory` struct.
    ///
    /// # Errors
    /// - `ERROR_EMODE_CATEGORY_NOT_FOUND`: If the category ID does not exist.
    #[only_owner]
    #[endpoint(editEModeCategory)]
    fn edit_e_mode_category(&self, category: EModeCategory<Self::Api>) {
        let mut map = self.e_mode_category();
        require!(
            map.contains_key(&category.category_id),
            ERROR_EMODE_CATEGORY_NOT_FOUND
        );

        self.update_e_mode_category_event(&category);
        map.insert(category.category_id, category);
    }

    /// Removes an e-mode category by marking it as deprecated.
    /// Disables the category for new positions.
    ///
    /// # Arguments
    /// - `category_id`: ID of the e-mode category to remove.
    ///
    /// # Errors
    /// - `ERROR_EMODE_CATEGORY_NOT_FOUND`: If the category ID does not exist.
    #[only_owner]
    #[endpoint(removeEModeCategory)]
    fn remove_e_mode_category(&self, category_id: u8) {
        let mut map = self.e_mode_category();
        require!(
            map.contains_key(&category_id),
            ERROR_EMODE_CATEGORY_NOT_FOUND
        );

        let assets = self
            .e_mode_assets(category_id)
            .keys()
            .collect::<ManagedVec<_>>();

        for asset in &assets {
            self.remove_asset_from_e_mode_category(asset.clone_value(), category_id);
        }
        let mut old_info = unsafe { map.get(&category_id).unwrap_unchecked() };
        old_info.is_deprecated = true;

        self.update_e_mode_category_event(&old_info);

        map.insert(category_id, old_info);
    }

    /// Adds an asset to an e-mode category with usage flags.
    /// Configures collateral and borrowability in e-mode.
    ///
    /// # Arguments
    /// - `asset`: Token identifier (EGLD or ESDT).
    /// - `category_id`: E-mode category ID.
    /// - `can_be_collateral`: Flag for collateral usability.
    /// - `can_be_borrowed`: Flag for borrowability.
    ///
    /// # Errors
    /// - `ERROR_EMODE_CATEGORY_NOT_FOUND`: If the category ID does not exist.
    /// - `ERROR_ASSET_NOT_SUPPORTED`: If the asset has no pool.
    /// - `ERROR_ASSET_ALREADY_SUPPORTED_IN_EMODE`: If the asset is already in the category.
    #[only_owner]
    #[endpoint(addAssetToEModeCategory)]
    fn add_asset_to_e_mode_category(
        &self,
        asset: EgldOrEsdtTokenIdentifier,
        category_id: u8,
        can_be_collateral: bool,
        can_be_borrowed: bool,
    ) {
        require!(
            self.e_mode_category().contains_key(&category_id),
            ERROR_EMODE_CATEGORY_NOT_FOUND
        );
        require!(
            !self.pools_map(&asset).is_empty(),
            ERROR_ASSET_NOT_SUPPORTED
        );

        let mut e_mode_assets = self.e_mode_assets(category_id);
        require!(
            !e_mode_assets.contains_key(&asset),
            ERROR_ASSET_ALREADY_SUPPORTED_IN_EMODE
        );

        let mut asset_e_modes = self.asset_e_modes(&asset);
        require!(
            !asset_e_modes.contains(&category_id),
            ERROR_ASSET_ALREADY_SUPPORTED_IN_EMODE
        );

        let asset_map = self.asset_config(&asset);

        let mut asset_data = asset_map.get();

        if !asset_data.has_emode() {
            asset_data.e_mode_enabled = true;

            self.update_asset_config_event(&asset, &asset_data);
            asset_map.set(asset_data);
        }
        let e_mode_asset_config = EModeAssetConfig {
            is_collateralizable: can_be_collateral,
            is_borrowable: can_be_borrowed,
        };
        self.update_e_mode_asset_event(&asset, &e_mode_asset_config, category_id);
        asset_e_modes.insert(category_id);
        e_mode_assets.insert(asset, e_mode_asset_config);
    }

    /// Edits an asset’s configuration within an e-mode category.
    /// Updates usage flags for collateral or borrowing.
    ///
    /// # Arguments
    /// - `asset`: Token identifier (EGLD or ESDT).
    /// - `category_id`: E-mode category ID.
    /// - `config`: New `EModeAssetConfig` settings.
    ///
    /// # Errors
    /// - `ERROR_EMODE_CATEGORY_NOT_FOUND`: If the category ID does not exist.
    /// - `ERROR_ASSET_NOT_SUPPORTED_IN_EMODE`: If the asset is not in the category.
    #[only_owner]
    #[endpoint(editAssetInEModeCategory)]
    fn edit_asset_in_e_mode_category(
        &self,
        asset: EgldOrEsdtTokenIdentifier,
        category_id: u8,
        config: EModeAssetConfig,
    ) {
        let mut map = self.e_mode_assets(category_id);
        require!(!map.is_empty(), ERROR_EMODE_CATEGORY_NOT_FOUND);
        require!(map.contains_key(&asset), ERROR_ASSET_NOT_SUPPORTED_IN_EMODE);

        self.update_e_mode_asset_event(&asset, &config, category_id);
        map.insert(asset, config);
    }

    /// Removes an asset from an e-mode category.
    /// Disables the asset’s e-mode capabilities for the category.
    ///
    /// # Arguments
    /// - `asset`: Token identifier (EGLD or ESDT).
    /// - `category_id`: E-mode category ID.
    ///
    /// # Errors
    /// - `ERROR_EMODE_CATEGORY_NOT_FOUND`: If the category ID does not exist.
    /// - `ERROR_ASSET_NOT_SUPPORTED`: If the asset has no pool.
    /// - `ERROR_ASSET_NOT_SUPPORTED_IN_EMODE`: If the asset is not in the category.
    #[only_owner]
    #[endpoint(removeAssetFromEModeCategory)]
    fn remove_asset_from_e_mode_category(&self, asset: EgldOrEsdtTokenIdentifier, category_id: u8) {
        let mut e_mode_assets = self.e_mode_assets(category_id);
        require!(!e_mode_assets.is_empty(), ERROR_EMODE_CATEGORY_NOT_FOUND);
        require!(
            !self.pools_map(&asset).is_empty(),
            ERROR_ASSET_NOT_SUPPORTED
        );
        require!(
            e_mode_assets.contains_key(&asset),
            ERROR_ASSET_NOT_SUPPORTED_IN_EMODE
        );

        let config = e_mode_assets.remove(&asset);
        let mut asset_e_modes = self.asset_e_modes(&asset);
        asset_e_modes.swap_remove(&category_id);

        self.update_e_mode_asset_event(&asset, &unsafe { config.unwrap_unchecked() }, category_id);
        if asset_e_modes.is_empty() {
            let mut asset_data = self.asset_config(&asset).get();
            asset_data.e_mode_enabled = false;

            self.update_asset_config_event(&asset, &asset_data);
            self.asset_config(&asset).set(asset_data);
        }
    }

    /// Edits an asset’s configuration in the protocol.
    /// Updates risk parameters, usage flags, and caps.
    ///
    /// # Arguments
    /// - `asset`: Token identifier (EGLD or ESDT).
    /// - `loan_to_value`: New LTV in BPS.
    /// - `liquidation_threshold`: New liquidation threshold in BPS.
    /// - `liquidation_bonus`: New liquidation bonus in BPS.
    /// - `liquidation_fees`: New liquidation fees in BPS.
    /// - `is_isolated_asset`: Flag for isolated asset status.
    /// - `isolation_debt_ceiling_usd`: Debt ceiling for isolated assets in USD.
    /// - `is_siloed_borrowing`: Flag for siloed borrowing.
    /// - `is_flashloanable`: Flag for flash loan support.
    /// - `flashloan_fee`: Flash loan fee in BPS.
    /// - `is_collateralizable`: Flag for collateral usability.
    /// - `is_borrowable`: Flag for borrowability.
    /// - `isolation_borrow_enabled`: Flag for borrowing in isolation mode.
    /// - `borrow_cap`: New borrow cap (zero for no cap).
    /// - `supply_cap`: New supply cap (zero for no cap).
    ///
    /// # Errors
    /// - `ERROR_ASSET_NOT_SUPPORTED`: If the asset has no pool or config.
    /// - `ERROR_INVALID_LIQUIDATION_THRESHOLD`: If threshold is not greater than LTV.
    #[only_owner]
    #[endpoint(editAssetConfig)]
    fn edit_asset_config(
        &self,
        asset: EgldOrEsdtTokenIdentifier,
        loan_to_value: BigUint,
        liquidation_threshold: BigUint,
        liquidation_bonus: BigUint,
        liquidation_fees: BigUint,
        is_isolated_asset: bool,
        isolation_debt_ceiling_usd: BigUint,
        is_siloed_borrowing: bool,
        is_flashloanable: bool,
        flashloan_fee: BigUint,
        is_collateralizable: bool,
        is_borrowable: bool,
        isolation_borrow_enabled: bool,
        borrow_cap: BigUint,
        supply_cap: BigUint,
    ) {
        require!(
            !self.pools_map(&asset).is_empty(),
            ERROR_ASSET_NOT_SUPPORTED
        );

        let map = self.asset_config(&asset);
        require!(!map.is_empty(), ERROR_ASSET_NOT_SUPPORTED);

        require!(
            liquidation_threshold > loan_to_value,
            ERROR_INVALID_LIQUIDATION_THRESHOLD
        );

        let old_config = map.get();

        let new_config = &AssetConfig {
            loan_to_value: self.to_decimal_bps(loan_to_value),
            liquidation_threshold: self.to_decimal_bps(liquidation_threshold),
            liquidation_bonus: self.to_decimal_bps(liquidation_bonus),
            liquidation_fees: self.to_decimal_bps(liquidation_fees),
            e_mode_enabled: old_config.e_mode_enabled,
            is_isolated_asset,
            isolation_debt_ceiling_usd: self.to_decimal_wad(isolation_debt_ceiling_usd),
            is_siloed_borrowing,
            is_flashloanable,
            flashloan_fee: self.to_decimal_bps(flashloan_fee),
            is_collateralizable,
            is_borrowable,
            isolation_borrow_enabled,
            borrow_cap: if borrow_cap == BigUint::zero() {
                None
            } else {
                Some(borrow_cap)
            },
            supply_cap: if supply_cap == BigUint::zero() {
                None
            } else {
                Some(supply_cap)
            },
        };

        map.set(new_config);

        self.update_asset_config_event(&asset, new_config);
    }

    /// Sets the position limits for NFT accounts.
    /// Configures maximum number of borrow and supply positions per NFT.
    ///
    /// **Purpose**: Controls the maximum number of positions an NFT can hold to optimize
    /// gas costs during liquidations and prevent excessive complexity in position management.
    ///
    /// **Gas Optimization**: By limiting positions per NFT, liquidation operations remain
    /// within reasonable gas limits, preventing failed liquidations due to gas constraints.
    ///
    /// **Default Configuration**: 10 borrow positions + 10 supply positions = 20 total positions
    ///
    /// # Arguments
    /// - `max_borrow_positions`: Maximum number of borrow positions per NFT
    /// - `max_supply_positions`: Maximum number of supply positions per NFT  
    ///
    /// # Security
    /// - Only contract owner can modify position limits
    /// - Changes affect new positions only, existing positions remain valid
    /// - Limits are enforced in supply and borrow operations
    #[only_owner]
    #[endpoint(setPositionLimits)]
    fn set_position_limits(&self, max_borrow_positions: u8, max_supply_positions: u8) {
        let limits = PositionLimits {
            max_borrow_positions,
            max_supply_positions,
        };
        self.position_limits().set(limits);
    }
}
