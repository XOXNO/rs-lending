use controller::ERROR_NO_POOL_FOUND;
use multiversx_sc::types::EgldOrEsdtTokenIdentifier;
use multiversx_sc_scenario::imports::{
    BigUint, ExpectMessage, MultiValueEncoded, OptionalValue, ScenarioTxRun, TestAddress,
    TestTokenIdentifier,
};
use multiversx_sc_scenario::ScenarioTxWhitebox;

pub mod constants;
pub mod proxys;
pub mod setup;
use constants::*;
use controller::storage::Storage;
use setup::*;

#[test]
fn router_upgrade_liquidity_pool_params_success() {
    let mut state = LendingPoolTestState::new();
    // Use existing EGLD market
    let pool_addr = state.egld_market.clone();

    // Query current parameters
    let old_params = state
        .world
        .query()
        .to(pool_addr.clone())
        .typed(proxys::proxy_liquidity_pool::LiquidityPoolProxy)
        .parameters()
        .returns(multiversx_sc::proxy_imports::ReturnsResult)
        .run();

    // Apply an upgrade with modified slopes and reserve factor
    state.upgrade_liquidity_pool_params(
        &EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        BigUint::from(9_000_000u64), // max_borrow_rate (RAY)
        BigUint::from(100_000u64),   // base_borrow_rate (RAY)
        BigUint::from(400_000u64),   // slope1 (RAY)
        BigUint::from(700_000u64),   // slope2 (RAY)
        BigUint::from(900_000u64),   // slope3 (RAY)
        BigUint::from(4_000_000u64), // mid_utilization (RAY)
        BigUint::from(8_000_000u64), // optimal_utilization (RAY)
        BigUint::from(1_500u64),     // reserve_factor (BPS)
    );

    let new_params = state
        .world
        .query()
        .to(pool_addr)
        .typed(proxys::proxy_liquidity_pool::LiquidityPoolProxy)
        .parameters()
        .returns(multiversx_sc::proxy_imports::ReturnsResult)
        .run();

    assert_eq!(
        new_params.reserve_factor_bps.into_raw_units().clone(),
        BigUint::from(1_500u64),
        "reserve factor must update to requested value",
    );
    assert_ne!(
        new_params.max_borrow_rate_ray.into_raw_units().clone(),
        old_params.max_borrow_rate_ray.into_raw_units().clone(),
        "max borrow rate should change after upgrade",
    );
    assert_ne!(
        new_params.base_borrow_rate_ray.into_raw_units().clone(),
        old_params.base_borrow_rate_ray.into_raw_units().clone(),
    );
    assert_ne!(
        new_params.slope1_ray.into_raw_units().clone(),
        old_params.slope1_ray.into_raw_units().clone(),
    );
    assert_ne!(
        new_params.slope2_ray.into_raw_units().clone(),
        old_params.slope2_ray.into_raw_units().clone(),
    );
    assert_ne!(
        new_params.slope3_ray.into_raw_units().clone(),
        old_params.slope3_ray.into_raw_units().clone(),
    );
    assert_ne!(
        new_params.mid_utilization_ray.into_raw_units().clone(),
        old_params.mid_utilization_ray.into_raw_units().clone(),
    );
    assert_ne!(
        new_params.optimal_utilization_ray.into_raw_units().clone(),
        old_params.optimal_utilization_ray.into_raw_units().clone(),
    );
    assert_ne!(
        new_params.reserve_factor_bps.into_raw_units().clone(),
        old_params.reserve_factor_bps.into_raw_units().clone(),
        "reserve factor must differ from previous value",
    );
}

#[test]
fn router_upgrade_liquidity_pool_params_no_pool_error() {
    let mut state = LendingPoolTestState::new();
    // Non-existing token id
    let bogus = TestTokenIdentifier::new("NOPOOL-123456");

    let egld_before = state
        .world
        .query()
        .to(state.egld_market.clone())
        .typed(proxys::proxy_liquidity_pool::LiquidityPoolProxy)
        .parameters()
        .returns(multiversx_sc::proxy_imports::ReturnsResult)
        .run();

    let _ = state
        .world
        .tx()
        .from(OWNER_ADDRESS)
        .to(state.lending_sc.clone())
        .typed(proxys::proxy_lending_pool::ControllerProxy)
        .upgrade_liquidity_pool_params(
            EgldOrEsdtTokenIdentifier::esdt(bogus.to_token_identifier()),
            BigUint::from(1u64),
            BigUint::from(1u64),
            BigUint::from(1u64),
            BigUint::from(1u64),
            BigUint::from(1u64),
            BigUint::from(1u64),
            BigUint::from(1u64),
            BigUint::from(1u64),
        )
        .returns(multiversx_sc_scenario::imports::ExpectMessage(
            core::str::from_utf8(ERROR_NO_POOL_FOUND).unwrap(),
        ))
        .run();

    let egld_after = state
        .world
        .query()
        .to(state.egld_market.clone())
        .typed(proxys::proxy_liquidity_pool::LiquidityPoolProxy)
        .parameters()
        .returns(multiversx_sc::proxy_imports::ReturnsResult)
        .run();
    assert_eq!(
        egld_after.max_borrow_rate_ray.into_raw_units().clone(),
        egld_before.max_borrow_rate_ray.into_raw_units().clone(),
        "failed upgrade should leave pool params unchanged",
    );
    assert_eq!(
        egld_after.reserve_factor_bps.into_raw_units().clone(),
        egld_before.reserve_factor_bps.into_raw_units().clone(),
    );
}

#[test]
fn router_claim_revenue_runs_successfully() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    // Setup accounts and do a small borrow/repay cycle to accrue some revenue
    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    let pre_reserves = state
        .market_reserves(state.egld_market.clone())
        .into_raw_units()
        .clone();

    // Advance time to accrue some interest and call claim revenue
    state.change_timestamp(10_000);
    state.claim_revenue(EGLD_TOKEN);

    let post_reserves = state
        .market_reserves(state.egld_market.clone())
        .into_raw_units()
        .clone();
    assert_eq!(post_reserves, pre_reserves);
}

#[test]
fn router_claim_revenue_no_accumulator_error() {
    let mut state = LendingPoolTestState::new();

    // seed some reserves so we can confirm they remain after failure
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");
    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    state.supply_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(5_000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(20u64),
        2,
        EGLD_DECIMALS,
    );
    state.change_timestamp(SECONDS_PER_DAY);
    let mut markets = MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN));
    state.update_markets(&borrower, markets.clone());
    let outstanding = state
        .borrow_amount_for_token(2, EGLD_TOKEN)
        .into_raw_units()
        .clone();
    state.repay_asset_deno(
        &borrower,
        &EGLD_TOKEN,
        outstanding.clone() + BigUint::from(1_000u64),
        2,
    );
    let reserves_before = state
        .market_reserves(state.egld_market.clone())
        .into_raw_units()
        .clone();
    assert!(reserves_before > BigUint::zero());

    // Clear accumulator address to trigger ERROR_NO_ACCUMULATOR_FOUND
    state
        .world
        .tx()
        .from(OWNER_ADDRESS)
        .to(state.lending_sc.clone())
        .whitebox(controller::contract_obj, |sc| {
            sc.accumulator_address().clear();
        });

    // Try to claim revenue for EGLD; expect ERROR_NO_ACCUMULATOR_FOUND
    let mut array = multiversx_sc::types::MultiValueEncoded::new();
    array.push(EgldOrEsdtTokenIdentifier::esdt(
        EGLD_TOKEN.to_token_identifier(),
    ));

    let _ = state
        .world
        .tx()
        .from(OWNER_ADDRESS)
        .to(state.lending_sc.clone())
        .typed(proxys::proxy_lending_pool::ControllerProxy)
        .claim_revenue(array)
        .returns(ExpectMessage(
            core::str::from_utf8(controller::ERROR_NO_ACCUMULATOR_FOUND).unwrap(),
        ))
        .run();

    let reserves_after = state
        .market_reserves(state.egld_market.clone())
        .into_raw_units()
        .clone();
    assert_eq!(reserves_after, reserves_before);
}

#[test]
fn router_upgrade_liquidity_pool_mid_usage_keeps_state_and_rates() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    // Prepare accounts and provide initial liquidity and usage
    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Supplier provides EGLD liquidity
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(1_000u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Borrower supplies USDC as collateral and borrows EGLD to create utilization
    state.supply_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(10_000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Borrow some EGLD against collateral (borrower account NFT nonce = 2)
    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(100u64),
        2,
        EGLD_DECIMALS,
    );

    // Snapshot pool state and rates before upgrade
    let pool_addr = state.pool_address(EgldOrEsdtTokenIdentifier::esdt(
        EGLD_TOKEN.to_token_identifier(),
    ));
    let pre_supplied = state
        .world
        .query()
        .to(pool_addr.clone())
        .typed(proxys::proxy_liquidity_pool::LiquidityPoolProxy)
        .supplied_amount()
        .returns(multiversx_sc::proxy_imports::ReturnsResult)
        .run();
    let pre_borrowed = state
        .world
        .query()
        .to(pool_addr.clone())
        .typed(proxys::proxy_liquidity_pool::LiquidityPoolProxy)
        .borrowed_amount()
        .returns(multiversx_sc::proxy_imports::ReturnsResult)
        .run();
    let pre_borrow_rate = state
        .world
        .query()
        .to(pool_addr.clone())
        .typed(proxys::proxy_liquidity_pool::LiquidityPoolProxy)
        .borrow_rate()
        .returns(multiversx_sc::proxy_imports::ReturnsResult)
        .run();
    let pre_deposit_rate = state
        .world
        .query()
        .to(pool_addr.clone())
        .typed(proxys::proxy_liquidity_pool::LiquidityPoolProxy)
        .deposit_rate()
        .returns(multiversx_sc::proxy_imports::ReturnsResult)
        .run();
    let pre_borrow_index = state
        .world
        .query()
        .to(pool_addr.clone())
        .typed(proxys::proxy_liquidity_pool::LiquidityPoolProxy)
        .borrow_index()
        .returns(multiversx_sc::proxy_imports::ReturnsResult)
        .run();
    let pre_supply_index = state
        .world
        .query()
        .to(pool_addr.clone())
        .typed(proxys::proxy_liquidity_pool::LiquidityPoolProxy)
        .supply_index()
        .returns(multiversx_sc::proxy_imports::ReturnsResult)
        .run();
    let pre_reserves = state
        .world
        .query()
        .to(pool_addr.clone())
        .typed(proxys::proxy_liquidity_pool::LiquidityPoolProxy)
        .reserves()
        .returns(multiversx_sc::proxy_imports::ReturnsResult)
        .run();

    // Perform controller-driven code upgrade of the liquidity pool
    let _ = state
        .world
        .tx()
        .from(OWNER_ADDRESS)
        .to(state.lending_sc.clone())
        .typed(proxys::proxy_lending_pool::ControllerProxy)
        .upgrade_liquidity_pool(EgldOrEsdtTokenIdentifier::esdt(
            EGLD_TOKEN.to_token_identifier(),
        ))
        .returns(multiversx_sc::proxy_imports::ReturnsResult)
        .run();

    // Re-query state and rates after upgrade
    let post_supplied = state
        .world
        .query()
        .to(pool_addr.clone())
        .typed(proxys::proxy_liquidity_pool::LiquidityPoolProxy)
        .supplied_amount()
        .returns(multiversx_sc::proxy_imports::ReturnsResult)
        .run();
    let post_borrowed = state
        .world
        .query()
        .to(pool_addr.clone())
        .typed(proxys::proxy_liquidity_pool::LiquidityPoolProxy)
        .borrowed_amount()
        .returns(multiversx_sc::proxy_imports::ReturnsResult)
        .run();
    let post_borrow_rate = state
        .world
        .query()
        .to(pool_addr.clone())
        .typed(proxys::proxy_liquidity_pool::LiquidityPoolProxy)
        .borrow_rate()
        .returns(multiversx_sc::proxy_imports::ReturnsResult)
        .run();
    let post_deposit_rate = state
        .world
        .query()
        .to(pool_addr.clone())
        .typed(proxys::proxy_liquidity_pool::LiquidityPoolProxy)
        .deposit_rate()
        .returns(multiversx_sc::proxy_imports::ReturnsResult)
        .run();
    let post_borrow_index = state
        .world
        .query()
        .to(pool_addr.clone())
        .typed(proxys::proxy_liquidity_pool::LiquidityPoolProxy)
        .borrow_index()
        .returns(multiversx_sc::proxy_imports::ReturnsResult)
        .run();
    let post_supply_index = state
        .world
        .query()
        .to(pool_addr.clone())
        .typed(proxys::proxy_liquidity_pool::LiquidityPoolProxy)
        .supply_index()
        .returns(multiversx_sc::proxy_imports::ReturnsResult)
        .run();
    let post_reserves = state
        .world
        .query()
        .to(pool_addr.clone())
        .typed(proxys::proxy_liquidity_pool::LiquidityPoolProxy)
        .reserves()
        .returns(multiversx_sc::proxy_imports::ReturnsResult)
        .run();

    // Validate state and rate invariants across code upgrade
    assert_eq!(
        pre_supplied, post_supplied,
        "supplied amount changed after upgrade"
    );
    assert_eq!(
        pre_borrowed, post_borrowed,
        "borrowed amount changed after upgrade"
    );
    assert_eq!(
        pre_borrow_rate, post_borrow_rate,
        "borrow rate changed after upgrade"
    );
    assert_eq!(
        pre_deposit_rate, post_deposit_rate,
        "deposit rate changed after upgrade"
    );
    assert_eq!(
        pre_borrow_index, post_borrow_index,
        "borrow index changed after upgrade"
    );
    assert_eq!(
        pre_supply_index, post_supply_index,
        "supply index changed after upgrade"
    );
    assert_eq!(
        pre_reserves, post_reserves,
        "reserves changed after upgrade"
    );
}

#[test]
fn router_create_liquidity_pool_asset_already_supported_error() {
    let mut state = LendingPoolTestState::new();

    // Attempt to create a second pool for EGLD which is already set up in state
    let cfg = state.asset_config(EgldOrEsdtTokenIdentifier::esdt(
        EGLD_TOKEN.to_token_identifier(),
    ));
    state.create_liquidity_pool_error(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        BigUint::from(1u64),
        BigUint::from(1u64),
        BigUint::from(1u64),
        BigUint::from(1u64),
        BigUint::from(1u64),
        BigUint::from(1u64),
        BigUint::from(1u64),
        BigUint::from(1u64),
        cfg.loan_to_value_bps.into_raw_units().clone(),
        cfg.liquidation_threshold_bps.into_raw_units().clone(),
        cfg.liquidation_bonus_bps.into_raw_units().clone(),
        cfg.liquidation_fees_bps.into_raw_units().clone(),
        cfg.is_collateralizable,
        cfg.is_borrowable,
        cfg.is_isolated_asset,
        cfg.isolation_debt_ceiling_usd_wad.into_raw_units().clone(),
        cfg.flashloan_fee_bps.into_raw_units().clone(),
        cfg.is_siloed_borrowing,
        cfg.is_flashloanable,
        cfg.isolation_borrow_enabled,
        EGLD_DECIMALS,
        cfg.borrow_cap_wad.unwrap_or_default(),
        cfg.supply_cap_wad.unwrap_or_default(),
        controller::ERROR_ASSET_ALREADY_SUPPORTED,
    );
}

#[test]
fn router_create_liquidity_pool_invalid_ticker_error() {
    let mut state = LendingPoolTestState::new();
    // Use an invalid identifier
    let bogus = TestTokenIdentifier::new("INVALID\u{0}");

    state.create_liquidity_pool_error(
        EgldOrEsdtTokenIdentifier::esdt(bogus.to_token_identifier()),
        BigUint::from(1u64),
        BigUint::from(1u64),
        BigUint::from(1u64),
        BigUint::from(1u64),
        BigUint::from(1u64),
        BigUint::from(1u64),
        BigUint::from(1u64),
        BigUint::from(1u64),
        BigUint::from(1u64),
        BigUint::from(2u64),
        BigUint::from(1u64),
        BigUint::from(1u64),
        true,
        true,
        false,
        BigUint::zero(),
        BigUint::from(10u64),
        false,
        true,
        true,
        EGLD_DECIMALS,
        BigUint::zero(),
        BigUint::zero(),
        controller::ERROR_INVALID_TICKER,
    );
}

#[test]
fn router_create_liquidity_pool_invalid_liquidation_threshold_error() {
    let mut state = LendingPoolTestState::new();
    // liquidation_threshold <= ltv should error
    // Use WEGLD which has an oracle configured in setup but no pool yet
    state.create_liquidity_pool_error(
        EgldOrEsdtTokenIdentifier::esdt(WEGLD_TOKEN.to_token_identifier()),
        BigUint::from(R_MAX),
        BigUint::from(R_BASE),
        BigUint::from(R_SLOPE1),
        BigUint::from(R_SLOPE2),
        BigUint::from(R_SLOPE3),
        BigUint::from(U_MID),
        BigUint::from(U_OPTIMAL),
        BigUint::from(RESERVE_FACTOR),
        BigUint::from(5_000u64), // ltv
        BigUint::from(5_000u64), // liquidation_threshold == ltv
        BigUint::from(1_000u64),
        BigUint::from(500u64),
        true,
        true,
        false,
        BigUint::zero(),
        BigUint::from(10u64),
        false,
        true,
        true,
        EGLD_DECIMALS,
        BigUint::zero(),
        BigUint::zero(),
        controller::ERROR_INVALID_LIQUIDATION_THRESHOLD,
    );
}
