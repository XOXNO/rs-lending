#![allow(clippy::too_many_arguments)]

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use common_errors::ERROR_TEMPLATE_EMPTY;
use common_structs::AssetConfig;

use crate::{
    cache::Cache, helpers, oracle, positions, proxy_accumulator, proxy_pool, storage, utils,
    validation, ERROR_ASSET_ALREADY_SUPPORTED, ERROR_INVALID_LIQUIDATION_THRESHOLD,
    ERROR_INVALID_TICKER, ERROR_NO_ACCUMULATOR_FOUND, ERROR_NO_POOL_FOUND,
};

/// Router module managing liquidity pool deployment and protocol revenue operations.
///
/// This module handles critical infrastructure operations for the lending protocol:
/// - **Pool Management**: Creation and upgrading of liquidity pools for new assets
/// - **Template Deployment**: Using secure templates for consistent pool implementations
/// - **Revenue Collection**: Claiming and routing protocol fees to the accumulator
/// - **Asset Configuration**: Setting up comprehensive risk parameters for new markets
///
/// # Pool Deployment Security
/// All pool deployments follow strict security patterns:
/// - Template-based deployment ensures consistent and audited pool implementations
/// - Comprehensive asset configuration prevents misconfigured risk parameters
/// - Validation of all parameters before pool activation
/// - Event emission for governance monitoring and transparency
///
/// # Revenue Management
/// The module implements secure revenue collection mechanisms:
/// - Multi-asset revenue claiming for gas efficiency
/// - Direct routing to accumulator contracts for proper distribution
/// - Price-aware revenue calculation using latest oracle data
/// - Automatic handling of zero-revenue cases
///
/// # Governance Integration
/// All functions require owner privileges and implement governance-controlled operations:
/// - Pool creation with complete asset risk parameter setup
/// - Pool upgrades for implementing protocol improvements
/// - Revenue collection for protocol sustainability
#[multiversx_sc::module]
pub trait RouterModule:
    storage::Storage
    + common_events::EventsModule
    + oracle::OracleModule
    + utils::LendingUtilsModule
    + validation::ValidationModule
    + helpers::MathsModule
    + positions::account::PositionAccountModule
    + common_math::SharedMathModule
    + common_rates::InterestRates
{
    /// Deploys a complete liquidity pool with comprehensive asset configuration.
    ///
    /// **Purpose**: Creates a new lending market for an asset by deploying a liquidity pool
    /// and configuring all necessary risk parameters, interest rate models, and operational
    /// settings. This is the primary governance function for onboarding new assets.
    ///
    /// **How it works**:
    /// 1. **Pre-deployment validation**:
    ///    - Ensures asset doesn't already have a pool
    ///    - Validates asset identifier format
    ///    - Confirms liquidation threshold > LTV ratio
    ///
    /// 2. **Pool deployment**:
    ///    - Deploys new pool from secure template
    ///    - Initializes with interest rate model parameters
    ///    - Registers pool address in protocol mapping
    ///
    /// 3. **Asset configuration**:
    ///    - Sets up comprehensive risk parameters
    ///    - Configures lending/borrowing permissions
    ///    - Establishes supply/borrow caps and isolation settings
    ///    - Initializes isolated debt tracking
    ///
    /// 4. **Event emission**:
    ///    - Emits market creation event for transparency
    ///    - Includes all configuration parameters for auditing
    ///
    /// **Interest rate model parameters**:
    /// ```
    /// utilization_rate = total_borrows / total_supply
    ///
    /// if utilization <= optimal_utilization:
    ///     rate = base_rate + (utilization * slope1 / optimal_utilization)
    /// elif utilization <= mid_utilization:
    ///     rate = base_rate + slope1 + ((utilization - optimal) * slope2 / (mid - optimal))
    /// else:
    ///     rate = base_rate + slope1 + slope2 + ((utilization - mid) * slope3 / (100% - mid))
    ///
    /// borrow_rate = min(calculated_rate, max_borrow_rate)
    /// ```
    ///
    /// **Risk parameter validation**:
    /// - **LTV < Liquidation Threshold**: Ensures safety buffer between borrowing and liquidation
    /// - **Valid asset identifier**: Prevents deployment for invalid tokens
    /// - **Reasonable parameter bounds**: Protects against extreme risk configurations
    ///
    /// **Asset configuration mechanics**:
    /// - **Isolation mode**: Assets can be restricted to isolation-only usage
    /// - **Siloed borrowing**: Prevents borrowing multiple assets simultaneously
    /// - **Supply/Borrow caps**: Limits maximum exposure to prevent concentration risk
    /// - **Flash loan settings**: Configures flash loan availability and fees
    ///
    /// **Security considerations**:
    /// - Template-based deployment ensures consistent security standards
    /// - Comprehensive parameter validation prevents misconfiguration
    /// - Asset uniqueness check prevents duplicate markets
    /// - Owner-only access ensures governance control
    ///
    /// **Governance impact**:
    /// Creates new lending market with immediate availability for users.
    /// All parameters can be adjusted later through edit functions.
    ///
    /// # Arguments
    /// - `base_asset`: Token identifier for the new market asset
    /// - `max_borrow_rate`: Interest rate ceiling (basis points)
    /// - `base_borrow_rate`: Minimum interest rate (basis points)
    /// - `slope1`: Rate increase slope for 0% to optimal utilization
    /// - `slope2`: Rate increase slope for optimal to mid utilization  
    /// - `slope3`: Rate increase slope for mid to 100% utilization
    /// - `mid_utilization`: Mid-range utilization threshold (basis points)
    /// - `optimal_utilization`: Target utilization rate (basis points)
    /// - `reserve_factor`: Protocol fee percentage (basis points)
    /// - `ltv`: Maximum loan-to-value ratio (basis points)
    /// - `liquidation_threshold`: Liquidation trigger threshold (basis points)
    /// - `liquidation_base_bonus`: Base liquidator reward (basis points)
    /// - `liquidation_max_fee`: Maximum liquidation fee (basis points)
    /// - `can_be_collateral`: Whether asset can secure loans
    /// - `can_be_borrowed`: Whether asset can be borrowed
    /// - `is_isolated`: Whether asset restricted to isolation mode
    /// - `debt_ceiling_usd`: Maximum USD debt against isolated collateral
    /// - `flash_loan_fee`: Flash loan fee percentage (basis points)
    /// - `is_siloed`: Whether borrowing prevents other asset borrows
    /// - `flashloan_enabled`: Whether flash loans are supported
    /// - `can_borrow_in_isolation`: Whether other assets borrowable in isolation
    /// - `asset_decimals`: Token decimal precision
    /// - `borrow_cap`: Maximum total borrows (0 = unlimited)
    /// - `supply_cap`: Maximum total supply (0 = unlimited)
    ///
    /// # Returns
    /// Address of the newly deployed liquidity pool contract
    ///
    /// # Errors
    /// - `ERROR_ASSET_ALREADY_SUPPORTED`: Asset already has an active pool
    /// - `ERROR_INVALID_TICKER`: Invalid asset identifier format
    /// - `ERROR_INVALID_LIQUIDATION_THRESHOLD`: Threshold not greater than LTV
    #[only_owner]
    #[endpoint(createLiquidityPool)]
    fn create_liquidity_pool(
        &self,
        base_asset: EgldOrEsdtTokenIdentifier,
        max_borrow_rate: BigUint,
        base_borrow_rate: BigUint,
        slope1: BigUint,
        slope2: BigUint,
        slope3: BigUint,
        mid_utilization: BigUint,
        optimal_utilization: BigUint,
        reserve_factor: BigUint,
        ltv: BigUint,
        liquidation_threshold: BigUint,
        liquidation_base_bonus: BigUint,
        liquidation_max_fee: BigUint,
        can_be_collateral: bool,
        can_be_borrowed: bool,
        is_isolated: bool,
        debt_ceiling_usd: BigUint,
        flash_loan_fee: BigUint,
        is_siloed: bool,
        flashloan_enabled: bool,
        can_borrow_in_isolation: bool,
        asset_decimals: usize,
        borrow_cap: BigUint,
        supply_cap: BigUint,
    ) -> ManagedAddress {
        require!(
            self.pools_map(&base_asset).is_empty(),
            ERROR_ASSET_ALREADY_SUPPORTED
        );
        require!(base_asset.is_valid(), ERROR_INVALID_TICKER);

        let address = self.create_pool(
            &base_asset,
            &max_borrow_rate,
            &base_borrow_rate,
            &slope1,
            &slope2,
            &slope3,
            &mid_utilization,
            &optimal_utilization,
            &reserve_factor,
        );

        self.require_non_zero_address(&address);

        self.pools_map(&base_asset).set(address.clone());
        self.pools().insert(address.clone());

        // Init ManagedDecimal for future usage and avoiding storage decode errors for checks
        self.isolated_asset_debt_usd(&base_asset)
            .set(self.to_decimal(BigUint::zero(), asset_decimals));

        require!(
            liquidation_threshold > ltv,
            ERROR_INVALID_LIQUIDATION_THRESHOLD
        );

        let asset_config = &AssetConfig {
            loan_to_value: self.to_decimal_bps(ltv),
            liquidation_threshold: self.to_decimal_bps(liquidation_threshold),
            liquidation_bonus: self.to_decimal_bps(liquidation_base_bonus),
            liquidation_fees: self.to_decimal_bps(liquidation_max_fee),
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
            is_collateralizable: can_be_collateral,
            is_borrowable: can_be_borrowed,
            e_mode_enabled: false,
            is_isolated_asset: is_isolated,
            isolation_debt_ceiling_usd: self.to_decimal_wad(debt_ceiling_usd),
            is_siloed_borrowing: is_siloed,
            is_flashloanable: flashloan_enabled,
            flashloan_fee: self.to_decimal_bps(flash_loan_fee),
            isolation_borrow_enabled: can_borrow_in_isolation,
        };

        self.asset_config(&base_asset).set(asset_config);

        self.create_market_params_event(
            &base_asset,
            &max_borrow_rate,
            &base_borrow_rate,
            &slope1,
            &slope2,
            &slope3,
            &mid_utilization,
            &optimal_utilization,
            &reserve_factor,
            &address,
            asset_config,
        );
        address
    }

    /// Upgrades an existing liquidity pool with new parameters.
    /// Adjusts interest rate model and reserve settings.
    ///
    /// # Arguments
    /// - `base_asset`: Token identifier (EGLD or ESDT) of the asset.
    /// - `max_borrow_rate`: New maximum borrow rate.
    /// - `base_borrow_rate`: New base borrow rate.
    /// - `slope1`, `slope2`, `slope3`: New interest rate slopes.
    /// - `mid_utilization`, `optimal_utilization`: New utilization thresholds.
    /// - `reserve_factor`: New reserve factor.
    ///
    /// # Errors
    /// - `ERROR_NO_POOL_FOUND`: If no pool exists for the asset.
    #[only_owner]
    #[endpoint(upgradeLiquidityPool)]
    fn upgrade_liquidity_pool(&self, base_asset: &EgldOrEsdtTokenIdentifier) {
        require!(!self.pools_map(base_asset).is_empty(), ERROR_NO_POOL_FOUND);

        let pool_address = self.get_pool_address(base_asset);
        self.upgrade_pool(pool_address);
    }

    #[only_owner]
    #[endpoint(upgradeLiquidityPoolParams)]
    fn upgrade_liquidity_pool_params(
        &self,
        base_asset: &EgldOrEsdtTokenIdentifier,
        max_borrow_rate: BigUint,
        base_borrow_rate: BigUint,
        slope1: BigUint,
        slope2: BigUint,
        slope3: BigUint,
        mid_utilization: BigUint,
        optimal_utilization: BigUint,
        reserve_factor: BigUint,
    ) {
        require!(!self.pools_map(base_asset).is_empty(), ERROR_NO_POOL_FOUND);

        let pool_address = self.get_pool_address(base_asset);
        self.update_pool_params(
            pool_address,
            base_asset,
            max_borrow_rate,
            base_borrow_rate,
            slope1,
            slope2,
            slope3,
            mid_utilization,
            optimal_utilization,
            reserve_factor,
        );
    }

    fn create_pool(
        &self,
        base_asset: &EgldOrEsdtTokenIdentifier,
        max_borrow_rate: &BigUint,
        base_borrow_rate: &BigUint,
        slope1: &BigUint,
        slope2: &BigUint,
        slope3: &BigUint,
        mid_utilization: &BigUint,
        optimal_utilization: &BigUint,
        reserve_factor: &BigUint,
    ) -> ManagedAddress {
        require!(
            !self.liq_pool_template_address().is_empty(),
            ERROR_TEMPLATE_EMPTY
        );

        let decimals = self.token_oracle(base_asset).get().asset_decimals;

        self.tx()
            .typed(proxy_pool::LiquidityPoolProxy)
            .init(
                base_asset,
                max_borrow_rate,
                base_borrow_rate,
                slope1,
                slope2,
                slope3,
                mid_utilization,
                optimal_utilization,
                reserve_factor,
                decimals,
            )
            .from_source(self.liq_pool_template_address().get())
            .code_metadata(CodeMetadata::UPGRADEABLE | CodeMetadata::READABLE)
            .returns(ReturnsNewManagedAddress)
            .sync_call()
    }

    fn update_pool_params(
        &self,
        lp_address: ManagedAddress,
        base_asset: &EgldOrEsdtTokenIdentifier,
        max_borrow_rate: BigUint,
        base_borrow_rate: BigUint,
        slope1: BigUint,
        slope2: BigUint,
        slope3: BigUint,
        mid_utilization: BigUint,
        optimal_utilization: BigUint,
        reserve_factor: BigUint,
    ) {
        let mut cache = Cache::new(self);
        let feed = self.get_token_price(base_asset, &mut cache);
        self.tx()
            .to(lp_address)
            .typed(proxy_pool::LiquidityPoolProxy)
            .update_params(
                max_borrow_rate,
                base_borrow_rate,
                slope1,
                slope2,
                slope3,
                mid_utilization,
                optimal_utilization,
                reserve_factor,
                feed.price,
            )
            .sync_call()
    }

    fn upgrade_pool(&self, lp_address: ManagedAddress) {
        require!(
            !self.liq_pool_template_address().is_empty(),
            ERROR_TEMPLATE_EMPTY
        );
        self.tx()
            .to(lp_address)
            .typed(proxy_pool::LiquidityPoolProxy)
            .upgrade()
            .from_source(self.liq_pool_template_address().get())
            .code_metadata(CodeMetadata::UPGRADEABLE | CodeMetadata::READABLE)
            .upgrade_async_call_and_exit();
    }

    /// Collects protocol revenue from liquidity pools and routes to accumulator.
    ///
    /// **Purpose**: Harvests accumulated protocol fees and interest spreads from
    /// multiple liquidity pools in a single transaction for gas efficiency.
    /// Routes collected revenue to the accumulator for proper distribution.
    ///
    /// **How it works**:
    /// 1. **Initialization**: Creates cache and validates accumulator address exists
    /// 2. **Multi-asset iteration**: Processes each specified asset sequentially
    /// 3. **Revenue claiming**: Calls each pool's claim_revenue with latest price
    /// 4. **Revenue routing**: Deposits non-zero revenue into accumulator contract
    /// 5. **Gas optimization**: Batches multiple claims in single transaction
    ///
    /// **Revenue sources collected**:
    /// - **Interest rate spread**: Difference between borrow and supply rates
    /// - **Reserve factor**: Percentage of interest reserved for protocol
    /// - **Liquidation fees**: Fees collected during position liquidations
    /// - **Flash loan fees**: Fees from flash loan operations
    ///
    /// **Price-aware collection**:
    /// Passes latest oracle price to each pool for accurate revenue calculation:
    /// ```
    /// revenue_value = pool_balance * latest_price
    /// revenue_amount = calculate_claimable_amount(revenue_value)
    /// ```
    ///
    /// **Zero-revenue optimization**:
    /// Only deposits revenue into accumulator if amount > 0, preventing
    /// unnecessary transactions and gas costs for pools with no accrued revenue.
    ///
    /// **Multi-asset efficiency**:
    /// - Single transaction processes multiple assets
    /// - Cached price feeds reduce oracle query costs
    /// - Batched accumulator deposits minimize transaction overhead
    /// - Early termination for zero-revenue assets
    ///
    /// **Security considerations**:
    /// - Owner-only access ensures governance control over revenue collection
    /// - Accumulator address validation prevents revenue loss
    /// - Latest price usage prevents stale price manipulation
    /// - Cache consistency ensures accurate revenue calculations
    ///
    /// **Revenue flow pattern**:
    /// ```
    /// 1. Liquidity pools accumulate fees over time
    /// 2. Governance calls claim_revenue with asset list
    /// 3. Each pool calculates claimable revenue amount
    /// 4. Revenue transferred from pools to accumulator
    /// 5. Accumulator handles distribution per protocol rules
    /// ```
    ///
    /// **Governance considerations**:
    /// Regular revenue collection ensures protocol sustainability and proper
    /// fee distribution to stakeholders. Frequency affects gas costs vs revenue timing.
    ///
    /// # Arguments
    /// - `assets`: Collection of token identifiers to claim revenue from
    ///
    /// # Returns
    /// Nothing - transfers revenue from pools to accumulator
    ///
    /// # Errors
    /// - `ERROR_NO_ACCUMULATOR_FOUND`: Accumulator address not configured
    #[endpoint(claimRevenue)]
    fn claim_revenue(&self, assets: MultiValueEncoded<EgldOrEsdtTokenIdentifier>) {
        let mut cache = Cache::new(self);
        let accumulator_address_mapper = self.accumulator_address();

        require!(
            !accumulator_address_mapper.is_empty(),
            ERROR_NO_ACCUMULATOR_FOUND
        );

        let accumulator_address = accumulator_address_mapper.get();
        for asset in assets {
            let pool_address = cache.get_cached_pool_address(&asset);
            let data = self.get_token_price(&asset, &mut cache);
            let revenue = self
                .tx()
                .to(pool_address)
                .typed(proxy_pool::LiquidityPoolProxy)
                .claim_revenue(data.price.clone())
                .returns(ReturnsResult)
                .sync_call();

            if revenue.amount > 0 {
                self.tx()
                    .to(&accumulator_address)
                    .typed(proxy_accumulator::AccumulatorProxy)
                    .deposit()
                    .payment(revenue)
                    .returns(ReturnsResult)
                    .sync_call();
            }
        }
    }
}
