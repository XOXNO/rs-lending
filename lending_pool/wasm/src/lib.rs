// Code generated by the multiversx-sc build system. DO NOT EDIT.

////////////////////////////////////////////////////
////////////////// AUTO-GENERATED //////////////////
////////////////////////////////////////////////////

// Init:                                 1
// Upgrade:                              1
// Endpoints:                           61
// Async Callback:                       1
// Total number of exported functions:  64

#![no_std]

multiversx_sc_wasm_adapter::allocator!();
multiversx_sc_wasm_adapter::panic_handler!();

multiversx_sc_wasm_adapter::endpoints! {
    lending_pool
    (
        init => init
        upgrade => upgrade
        supply => supply
        withdraw => withdraw
        borrow => borrow
        repay => repay
        liquidate => liquidate
        flashLoan => flash_loan
        updateAccountPositions => update_account_positions
        disableVault => disable_vault
        enableVault => enable_vault
        updatePositionThreshold => update_position_threshold
        updateIndexes => update_indexes
        createLiquidityPool => create_liquidity_pool
        upgradeLiquidityPool => upgrade_liquidity_pool
        claimRevenue => claim_revenue
        registerAccountToken => register_account_token
        setTokenOracle => set_token_oracle
        editTokenOracleTolerance => edit_token_oracle_tolerance
        setAggregator => set_aggregator
        setAccumulator => set_accumulator
        setSafePriceView => set_safe_price_view
        setLiquidityPoolTemplate => set_liquidity_pool_template
        addEModeCategory => add_e_mode_category
        editEModeCategory => edit_e_mode_category
        removeEModeCategory => remove_e_mode_category
        addAssetToEModeCategory => add_asset_to_e_mode_category
        editAssetInEModeCategory => edit_asset_in_e_mode_category
        removeAssetFromEModeCategory => remove_asset_from_e_mode_category
        editAssetConfig => edit_asset_config
        getPoolAllowed => pools_allowed
        getAccountToken => account_token
        getAccountPositions => account_positions
        getAccountAttributes => account_attributes
        getDepositPositions => deposit_positions
        getBorrowPositions => borrow_positions
        getLiqPoolTemplateAddress => liq_pool_template_address
        getAccumulatorAddress => accumulator_address
        getPoolsMap => pools_map
        getPriceAggregatorAddress => price_aggregator_address
        getSafePriceView => safe_price_view
        getAssetConfig => asset_config
        lastEModeCategoryId => last_e_mode_category_id
        getEModes => e_mode_category
        getAssetEModes => asset_e_modes
        getEModesAssets => e_mode_assets
        getIsolatedAssetDebtUsd => isolated_asset_debt_usd
        getVaultSuppliedAmount => vault_supplied_amount
        getTokenOracle => token_oracle
        getLastTokenPrice => last_token_price
        getPoolAddress => get_pool_address
        getAllMarkets => get_all_markets
        canBeLiquidated => can_be_liquidated
        getHealthFactor => get_health_factor
        getCollateralAmountForToken => get_collateral_amount_for_token
        getBorrowAmountForToken => get_borrow_amount_for_token
        getTotalBorrowInEgld => get_total_borrow_in_egld
        getTotalCollateralInEgld => get_total_collateral_in_egld
        getLiquidationCollateralAvailable => get_liquidation_collateral_available
        getLtvCollateralInEgld => get_ltv_collateral_in_egld
        getTokenPriceData => get_token_price_data_view
        getTokenPriceUSD => get_usd_price
        getTokenPriceEGLD => get_egld_price
    )
}

multiversx_sc_wasm_adapter::async_callback! { lending_pool }
