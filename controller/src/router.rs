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

#[multiversx_sc::module]
pub trait RouterModule:
    storage::Storage
    + common_events::EventsModule
    + oracle::OracleModule
    + utils::LendingUtilsModule
    + validation::ValidationModule
    + helpers::math::MathsModule
    + positions::account::PositionAccountModule
    + common_math::SharedMathModule
{
    /// Creates a new liquidity pool for an asset with specified parameters.
    /// Initializes the pool and configures lending/borrowing settings.
    ///
    /// # Arguments
    /// - `base_asset`: Token identifier (EGLD or ESDT) of the asset.
    /// - `max_borrow_rate`: Maximum borrow rate.
    /// - `base_borrow_rate`: Base borrow rate.
    /// - `slope1`, `slope2`, `slope3`: Interest rate slopes for utilization levels.
    /// - `mid_utilization`, `optimal_utilization`: Utilization thresholds for rate calculations.
    /// - `reserve_factor`: Fraction of interest reserved for the protocol.
    /// - `ltv`: Loan-to-value ratio in BPS.
    /// - `liquidation_threshold`: Liquidation threshold in BPS.
    /// - `liquidation_base_bonus`: Base liquidation bonus in BPS.
    /// - `liquidation_max_fee`: Maximum liquidation fee in BPS.
    /// - `can_be_collateral`: Flag for collateral usability.
    /// - `can_be_borrowed`: Flag for borrowability.
    /// - `is_isolated`: Flag for isolated asset status.
    /// - `debt_ceiling_usd`: Debt ceiling in USD for isolated assets.
    /// - `flash_loan_fee`: Flash loan fee in BPS.
    /// - `is_siloed`: Flag for siloed borrowing.
    /// - `flashloan_enabled`: Flag for flash loan support.
    /// - `can_borrow_in_isolation`: Flag for borrowing in isolation mode.
    /// - `asset_decimals`: Number of decimals for the asset.
    /// - `borrow_cap`: Optional borrow cap (`None` if unspecified).
    /// - `supply_cap`: Optional supply cap (`None` if unspecified).
    ///
    /// # Returns
    /// - `ManagedAddress`: Address of the newly created liquidity pool.
    ///
    /// # Errors
    /// - `ERROR_ASSET_ALREADY_SUPPORTED`: If the asset already has a pool.
    /// - `ERROR_INVALID_TICKER`: If the asset identifier is invalid.
    /// - `ERROR_INVALID_LIQUIDATION_THRESHOLD`: If threshold is invalid.
    #[allow_multiple_var_args]
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
        borrow_cap: OptionalValue<BigUint>,
        supply_cap: OptionalValue<BigUint>,
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
        self.pools_allowed().insert(address.clone());

        // Init ManagedDecimal for future usage and avoiding storage decode errors for checks
        self.vault_supplied_amount(&base_asset)
            .set(ManagedDecimal::from_raw_units(
                BigUint::zero(),
                asset_decimals,
            ));
        self.isolated_asset_debt_usd(&base_asset)
            .set(ManagedDecimal::from_raw_units(
                BigUint::zero(),
                asset_decimals,
            ));

        require!(
            &liquidation_threshold > &ltv,
            ERROR_INVALID_LIQUIDATION_THRESHOLD
        );

        let asset_config = &AssetConfig {
            loan_to_value: self.to_decimal_bps(ltv),
            liquidation_threshold: self.to_decimal_bps(liquidation_threshold),
            liquidation_bonus: self.to_decimal_bps(liquidation_base_bonus),
            liquidation_fees: self.to_decimal_bps(liquidation_max_fee),
            borrow_cap: borrow_cap.into_option(),
            supply_cap: supply_cap.into_option(),
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
    fn upgrade_liquidity_pool(
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
        self.upgrade_pool(
            pool_address,
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

        let decimals = self.token_oracle(base_asset).get().price_decimals;

        let new_address = self
            .tx()
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
            .sync_call();

        new_address
    }

    fn upgrade_pool(
        &self,
        lp_address: ManagedAddress,
        max_borrow_rate: BigUint,
        base_borrow_rate: BigUint,
        slope1: BigUint,
        slope2: BigUint,
        slope3: BigUint,
        mid_utilization: BigUint,
        optimal_utilization: BigUint,
        reserve_factor: BigUint,
    ) {
        require!(
            !self.liq_pool_template_address().is_empty(),
            ERROR_TEMPLATE_EMPTY
        );
        self.tx()
            .to(lp_address)
            .typed(proxy_pool::LiquidityPoolProxy)
            .upgrade(
                max_borrow_rate,
                base_borrow_rate,
                slope1,
                slope2,
                slope3,
                mid_utilization,
                optimal_utilization,
                reserve_factor,
            )
            .from_source(self.liq_pool_template_address().get())
            .code_metadata(CodeMetadata::UPGRADEABLE | CodeMetadata::READABLE)
            .upgrade_async_call_and_exit();
    }

    /// Claims revenue from multiple liquidity pools and deposits it into the accumulator.
    /// Collects protocol revenue from interest and fees.
    ///
    /// # Arguments
    /// - `assets`: List of token identifiers (EGLD or ESDT) to claim revenue from.
    ///
    /// # Errors
    /// - `ERROR_NO_ACCUMULATOR_FOUND`: If no accumulator address is set.
    #[only_owner]
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
            let pool_address = self.get_pool_address(&asset);
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
