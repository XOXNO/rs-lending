use controller::ERROR_STRATEGY_FEE_EXCEEDS_AMOUNT;
use multiversx_sc::types::{EgldOrEsdtTokenIdentifier, ManagedDecimal};
use multiversx_sc_scenario::imports::{
    BigUint, ExpectMessage, OptionalValue, ScenarioTxRun, TestAddress,
};

pub mod constants;
pub mod proxys;
pub mod setup;
use constants::*;
use setup::*;

fn make_borrow_position(
    asset: EgldOrEsdtTokenIdentifier<multiversx_sc_scenario::api::StaticApi>,
    account_nonce: u64,
) -> common_structs::AccountPosition<multiversx_sc_scenario::api::StaticApi> {
    common_structs::AccountPosition::new(
        common_structs::AccountPositionType::Borrow,
        asset,
        ManagedDecimal::from_raw_units(BigUint::zero(), RAY_PRECISION),
        account_nonce,
        ManagedDecimal::from_raw_units(BigUint::zero(), BPS_PRECISION),
        ManagedDecimal::from_raw_units(BigUint::zero(), BPS_PRECISION),
        ManagedDecimal::from_raw_units(BigUint::zero(), BPS_PRECISION),
        ManagedDecimal::from_raw_units(BigUint::zero(), BPS_PRECISION),
    )
}

#[test]
fn pool_create_strategy_invalid_asset_error() {
    let mut state = LendingPoolTestState::new();

    // Use EGLD pool but pass a USDC borrow position => ERROR_INVALID_ASSET
    let egld_pool = state.pool_address(EgldOrEsdtTokenIdentifier::esdt(
        EGLD_TOKEN.to_token_identifier(),
    ));

    let wrong_position = make_borrow_position(
        EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN.to_token_identifier()),
        1,
    );

    let amount = ManagedDecimal::from_raw_units(BigUint::from(1u64), EGLD_DECIMALS);
    let fee = ManagedDecimal::from_raw_units(BigUint::zero(), EGLD_DECIMALS);
    let price = ManagedDecimal::from_raw_units(BigUint::from(WAD), WAD_PRECISION);

    let _ = state
        .world
        .tx()
        // Only-owner of pool is the controller; call from controller address
        .from(state.lending_sc.clone())
        .to(egld_pool)
        .typed(proxys::proxy_liquidity_pool::LiquidityPoolProxy)
        .create_strategy(wrong_position, amount, fee, price)
        .returns(ExpectMessage(
            core::str::from_utf8(common_errors::ERROR_INVALID_ASSET).unwrap(),
        ))
        .run();
}

#[test]
fn pool_create_strategy_insufficient_liquidity_error() {
    let mut state = LendingPoolTestState::new();

    // EGLD pool has zero reserves initially; attempt strategy borrow > 0
    let egld_pool = state.pool_address(EgldOrEsdtTokenIdentifier::esdt(
        EGLD_TOKEN.to_token_identifier(),
    ));

    let position = make_borrow_position(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        1,
    );
    let amount = ManagedDecimal::from_raw_units(BigUint::from(10u64), EGLD_DECIMALS);
    let fee = ManagedDecimal::from_raw_units(BigUint::zero(), EGLD_DECIMALS);
    let price = ManagedDecimal::from_raw_units(BigUint::from(WAD), WAD_PRECISION);

    let _ = state
        .world
        .tx()
        .from(state.lending_sc.clone())
        .to(egld_pool)
        .typed(proxys::proxy_liquidity_pool::LiquidityPoolProxy)
        .create_strategy(position, amount, fee, price)
        .returns(ExpectMessage(
            core::str::from_utf8(common_errors::ERROR_INSUFFICIENT_LIQUIDITY).unwrap(),
        ))
        .run();
}

#[test]
fn pool_create_strategy_fee_exceeds_amount_error() {
    let mut state = LendingPoolTestState::new();

    let egld_pool = state.pool_address(EgldOrEsdtTokenIdentifier::esdt(
        EGLD_TOKEN.to_token_identifier(),
    ));

    // Ensure pool has enough reserves so we hit the fee check, not liquidity
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");
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

    let position = make_borrow_position(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        1,
    );

    // amount < fee => ERROR_STRATEGY_FEE_EXCEEDS_AMOUNT
    let amount = ManagedDecimal::from_raw_units(BigUint::from(1u64), EGLD_DECIMALS);
    let fee = ManagedDecimal::from_raw_units(BigUint::from(2u64), EGLD_DECIMALS);
    let price = ManagedDecimal::from_raw_units(BigUint::from(WAD), WAD_PRECISION);

    let _ = state
        .world
        .tx()
        .from(state.lending_sc.clone())
        .to(egld_pool)
        .typed(proxys::proxy_liquidity_pool::LiquidityPoolProxy)
        .create_strategy(position, amount, fee, price)
        .returns(ExpectMessage(
            core::str::from_utf8(ERROR_STRATEGY_FEE_EXCEEDS_AMOUNT).unwrap(),
        ))
        .run();
}

#[test]
fn pool_claim_revenue_partial_burn_and_invariants() {
    let mut state = LendingPoolTestState::new();

    // Setup and provide large EGLD liquidity
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");
    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(1_000u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Owner (controller) triggers pool.createStrategy with amount equal to reserves
    let egld_pool = state.pool_address(EgldOrEsdtTokenIdentifier::esdt(
        EGLD_TOKEN.to_token_identifier(),
    ));
    let borrow_position = make_borrow_position(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        1,
    );
    // Borrow full reserves; leave small fee as on-chain reserves
    let amount =
        ManagedDecimal::from_raw_units(BigUint::from(1_000u64) * BigUint::from(WAD), EGLD_DECIMALS);
    let fee =
        ManagedDecimal::from_raw_units(BigUint::from(1u64) * BigUint::from(WAD), EGLD_DECIMALS);
    let price = ManagedDecimal::from_raw_units(BigUint::from(WAD), WAD_PRECISION);

    state
        .world
        .tx()
        .from(state.lending_sc.clone())
        .to(egld_pool.clone())
        .typed(proxys::proxy_liquidity_pool::LiquidityPoolProxy)
        .create_strategy(borrow_position, amount, fee.clone(), price.clone())
        .run();

    // Advance time and update indexes to accrue protocol revenue (interest fee share)
    state.change_timestamp(MS_PER_YEAR * 5);
    state
        .world
        .tx()
        .from(state.lending_sc.clone())
        .to(egld_pool.clone())
        .typed(proxys::proxy_liquidity_pool::LiquidityPoolProxy)
        .update_indexes(price.clone())
        .run();

    // Pre-claim snapshots
    let pre_rev = state
        .world
        .query()
        .to(egld_pool.clone())
        .typed(proxys::proxy_liquidity_pool::LiquidityPoolProxy)
        .protocol_revenue()
        .returns(multiversx_sc::proxy_imports::ReturnsResult)
        .run();
    let pre_reserves = state
        .world
        .query()
        .to(egld_pool.clone())
        .typed(proxys::proxy_liquidity_pool::LiquidityPoolProxy)
        .reserves()
        .returns(multiversx_sc::proxy_imports::ReturnsResult)
        .run();
    let pre_supplied = state
        .world
        .query()
        .to(egld_pool.clone())
        .typed(proxys::proxy_liquidity_pool::LiquidityPoolProxy)
        .supplied_amount()
        .returns(multiversx_sc::proxy_imports::ReturnsResult)
        .run();
    let pre_borrowed = state
        .world
        .query()
        .to(egld_pool.clone())
        .typed(proxys::proxy_liquidity_pool::LiquidityPoolProxy)
        .borrowed_amount()
        .returns(multiversx_sc::proxy_imports::ReturnsResult)
        .run();

    // Ensure partial-claim condition holds (revenue > reserves)
    assert!(pre_rev.into_raw_units() > pre_reserves.into_raw_units());

    // Claim revenue from pool directly (owner = controller)
    let payment = state
        .world
        .tx()
        .from(state.lending_sc.clone())
        .to(egld_pool.clone())
        .typed(proxys::proxy_liquidity_pool::LiquidityPoolProxy)
        .claim_revenue(price)
        .returns(multiversx_sc::proxy_imports::ReturnsResult)
        .run();

    // Post-claim snapshots
    let post_rev = state
        .world
        .query()
        .to(egld_pool.clone())
        .typed(proxys::proxy_liquidity_pool::LiquidityPoolProxy)
        .protocol_revenue()
        .returns(multiversx_sc::proxy_imports::ReturnsResult)
        .run();
    let post_reserves = state
        .world
        .query()
        .to(egld_pool.clone())
        .typed(proxys::proxy_liquidity_pool::LiquidityPoolProxy)
        .reserves()
        .returns(multiversx_sc::proxy_imports::ReturnsResult)
        .run();
    let post_supplied = state
        .world
        .query()
        .to(egld_pool)
        .typed(proxys::proxy_liquidity_pool::LiquidityPoolProxy)
        .supplied_amount()
        .returns(multiversx_sc::proxy_imports::ReturnsResult)
        .run();

    // Invariants: partial claim behavior and solvency
    let claimed = payment.amount;
    // Claimed equals pre-claim reserves
    assert_eq!(claimed, pre_reserves.into_raw_units().clone());
    // Reserves reduced by claimed amount (to zero in our construction)
    assert_eq!(post_reserves.into_raw_units().clone(), BigUint::zero());
    // Revenue decreased but remains positive (partial burn)
    assert!(post_rev.into_raw_units().clone() < pre_rev.into_raw_units().clone());
    assert!(post_rev.into_raw_units().clone() > BigUint::zero());
    // Total supplied decreased and by at least the claimed amount
    let supplied_delta =
        pre_supplied.into_raw_units().clone() - post_supplied.into_raw_units().clone();
    assert!(supplied_delta >= claimed.clone());
    // Borrowed unchanged by revenue claim
    let pool_addr = state.pool_address(EgldOrEsdtTokenIdentifier::esdt(
        EGLD_TOKEN.to_token_identifier(),
    ));
    let post_borrowed = state
        .world
        .query()
        .to(pool_addr)
        .typed(proxys::proxy_liquidity_pool::LiquidityPoolProxy)
        .borrowed_amount()
        .returns(multiversx_sc::proxy_imports::ReturnsResult)
        .run();
    assert_eq!(
        post_borrowed.into_raw_units(),
        pre_borrowed.into_raw_units()
    );
    // Solvency: contract never transferred more than available reserves
    // (already enforced by claimed == pre_reserves and post_reserves == 0)
}
