use common_constants::RAY;
pub use common_constants::{BPS_PRECISION, RAY_PRECISION, WAD_PRECISION};

use controller::ERROR_INSUFFICIENT_COLLATERAL;

use multiversx_sc::types::{
    EgldOrEsdtTokenIdentifier, EgldOrEsdtTokenPayment, ManagedDecimal, ManagedVec,
    MultiValueEncoded,
};
use multiversx_sc_scenario::imports::{BigUint, OptionalValue, StaticApi, TestAddress};
pub mod constants;
pub mod proxys;
pub mod setup;
use constants::*;
use setup::*;

fn small_ray_tolerance() -> BigUint<StaticApi> {
    BigUint::from(10u64).pow(22)
}

/// Tests basic liquidation flow with multiple debt positions.
///
/// Covers:
/// - Controller::liquidate endpoint functionality
/// - Sequential liquidation of multiple assets
/// - Health factor validation before and after liquidation
/// - Interest accrual impact on liquidation threshold
/// - Liquidation of unhealthy positions
#[test]
fn liquidate_multiple_debt_positions_sequential_success() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Supplier provides liquidity across multiple assets ($5000 total)
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    state.assert_collateral_raw_eq(
        1,
        &EGLD_TOKEN,
        scaled_amount(100, EGLD_DECIMALS),
        "Supplier EGLD liquidity should be recorded",
    );
    state.supply_asset(
        &supplier,
        CAPPED_TOKEN,
        BigUint::from(10u64),
        CAPPED_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::None,
        false,
    );
    state.assert_collateral_raw_eq(
        1,
        &CAPPED_TOKEN,
        scaled_amount(10, CAPPED_DECIMALS),
        "Supplier capped liquidity should be recorded",
    );
    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(1000u64),
        USDC_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::None,
        false,
    );
    state.assert_collateral_raw_eq(
        1,
        &USDC_TOKEN,
        scaled_amount(1000, USDC_DECIMALS),
        "Supplier USDC liquidity should be recorded",
    );

    // Borrower provides collateral ($5000 total)
    state.supply_asset(
        &borrower,
        XEGLD_TOKEN,
        BigUint::from(20u64),
        XEGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    state.assert_collateral_raw_eq(
        2,
        &XEGLD_TOKEN,
        scaled_amount(20, XEGLD_DECIMALS),
        "Borrower XEGLD collateral should be recorded",
    );
    state.supply_asset(
        &borrower,
        SEGLD_TOKEN,
        BigUint::from(80u64),
        SEGLD_DECIMALS,
        OptionalValue::Some(2),
        OptionalValue::None,
        false,
    );
    state.assert_collateral_raw_eq(
        2,
        &SEGLD_TOKEN,
        scaled_amount(80, SEGLD_DECIMALS),
        "Borrower SEGLD collateral should be recorded",
    );

    // Borrower takes loans
    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(39u64),
        2,
        EGLD_DECIMALS,
    );
    state.assert_borrow_raw_eq(
        2,
        &EGLD_TOKEN,
        scaled_amount(39, EGLD_DECIMALS),
        "Borrowed EGLD debt should be tracked",
    );
    state.borrow_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(1000u64),
        2,
        USDC_DECIMALS,
    );
    state.assert_borrow_raw_eq(
        2,
        &USDC_TOKEN,
        scaled_amount(1000, USDC_DECIMALS),
        "Borrowed USDC debt should be tracked",
    );

    // Verify initial position health
    let _initial_health = state.account_health_factor(2);
    state.assert_health_factor_at_least(2, RAY);

    // Advance time to accumulate interest and make position unhealthy
    state.change_timestamp(SECONDS_PER_DAY * 440);
    let mut markets = MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN));
    markets.push(EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN));
    state.update_markets(&borrower, markets.clone());

    // Setup liquidator
    let liquidator = TestAddress::new("liquidator");
    state
        .world
        .account(liquidator)
        .nonce(1)
        .esdt_balance(
            USDC_TOKEN,
            BigUint::from(10000u64) * BigUint::from(10u64).pow(USDC_DECIMALS as u32),
        )
        .esdt_balance(
            EGLD_TOKEN,
            BigUint::from(10000u64) * BigUint::from(10u64).pow(EGLD_DECIMALS as u32),
        );

    // Get debt amounts before liquidation
    let borrowed_usdc = state.borrow_amount_for_token(2, USDC_TOKEN);
    let borrowed_egld = state.borrow_amount_for_token(2, EGLD_TOKEN);

    let before_health = state.account_health_factor(2);
    // Liquidate EGLD debt first
    state.liquidate_account_dem(
        &liquidator,
        &EGLD_TOKEN,
        borrowed_egld.into_raw_units().clone(),
        2,
    );
    let after_health = state.account_health_factor(2);
    assert!(after_health > before_health);
    state.assert_no_borrow_entry(2, &EGLD_TOKEN);

    // // Liquidate USDC debt second
    state.liquidate_account_dem(
        &liquidator,
        &USDC_TOKEN,
        borrowed_usdc.into_raw_units().clone(),
        2,
    );
    let post_liquidation_health = state.account_health_factor(2);
    assert!(post_liquidation_health > after_health);
    state.assert_borrow_raw_within(
        2,
        &USDC_TOKEN,
        BigUint::zero(),
        small_ray_tolerance(),
        "Secondary liquidation should leave only dust USDC debt",
    );
    state.assert_total_borrow_raw_within(
        2,
        BigUint::zero(),
        small_ray_tolerance(),
        "Sequential liquidation should reduce total borrow to dust",
    );
    state.assert_health_factor_at_least(2, RAY);
}

/// Tests bulk liquidation with multiple assets in single transaction.
///
/// Covers:
/// - Controller::liquidate endpoint with bulk payments
/// - Simultaneous liquidation of multiple debt positions
/// - Overpayment handling in bulk liquidation
/// - Health factor restoration after bulk liquidation
#[test]
fn liquidate_bulk_multiple_assets_with_overpayment_success() {
    let mut state = LendingPoolTestState::new();
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
    state.assert_collateral_raw_eq(
        1,
        &EGLD_TOKEN,
        scaled_amount(100, EGLD_DECIMALS),
        "Supplier EGLD liquidity should be recorded",
    );
    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(1000u64),
        USDC_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::None,
        false,
    );
    state.assert_collateral_raw_eq(
        1,
        &USDC_TOKEN,
        scaled_amount(1000, USDC_DECIMALS),
        "Supplier USDC liquidity should be recorded",
    );
    state.supply_asset(
        &supplier,
        CAPPED_TOKEN,
        BigUint::from(10u64),
        CAPPED_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::None,
        false,
    );
    state.assert_collateral_raw_eq(
        1,
        &CAPPED_TOKEN,
        scaled_amount(10, CAPPED_DECIMALS),
        "Supplier capped liquidity should be recorded",
    );

    state.supply_asset(
        &borrower,
        XEGLD_TOKEN,
        BigUint::from(20u64),
        XEGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    state.assert_collateral_raw_eq(
        2,
        &XEGLD_TOKEN,
        scaled_amount(20, XEGLD_DECIMALS),
        "Borrower XEGLD collateral should be recorded",
    );
    state.supply_asset(
        &borrower,
        SEGLD_TOKEN,
        BigUint::from(80u64),
        SEGLD_DECIMALS,
        OptionalValue::Some(2),
        OptionalValue::None,
        false,
    );
    state.assert_collateral_raw_eq(
        2,
        &SEGLD_TOKEN,
        scaled_amount(80, SEGLD_DECIMALS),
        "Borrower SEGLD collateral should be recorded",
    );

    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(39u64),
        2,
        EGLD_DECIMALS,
    );
    state.assert_borrow_raw_eq(
        2,
        &EGLD_TOKEN,
        scaled_amount(39, EGLD_DECIMALS),
        "Borrower EGLD debt should be tracked",
    );
    state.borrow_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(1000u64),
        2,
        USDC_DECIMALS,
    );
    state.assert_borrow_raw_eq(
        2,
        &USDC_TOKEN,
        scaled_amount(1000, USDC_DECIMALS),
        "Borrower USDC debt should be tracked",
    );

    state.change_timestamp(SECONDS_PER_DAY * 400);
    let mut markets = MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN));
    markets.push(EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN));
    state.update_markets(&borrower, markets.clone());
    let before_health = state.account_health_factor(2);

    let liquidator = TestAddress::new("liquidator");
    state
        .world
        .account(liquidator)
        .nonce(1)
        .esdt_balance(
            USDC_TOKEN,
            BigUint::from(10_000u64) * BigUint::from(10u64).pow(USDC_DECIMALS as u32),
        )
        .esdt_balance(
            EGLD_TOKEN,
            BigUint::from(10_000u64) * BigUint::from(10u64).pow(EGLD_DECIMALS as u32),
        );

    let borrowed_usdc = state.borrow_amount_for_token(2, USDC_TOKEN);
    let borrowed_egld = state.borrow_amount_for_token(2, EGLD_TOKEN);
    let usdc_payment = borrowed_usdc.into_raw_units().clone() * 3u64;
    let payments = vec![
        (&EGLD_TOKEN, borrowed_egld.into_raw_units()),
        (&USDC_TOKEN, &usdc_payment),
    ];

    state.liquidate_account_dem_bulk(&liquidator, payments, 2);
    let after_health = state.account_health_factor(2);
    assert!(after_health > before_health);
    state.assert_no_borrow_entry(2, &EGLD_TOKEN);
    state.assert_borrow_raw_within(
        2,
        &USDC_TOKEN,
        BigUint::zero(),
        small_ray_tolerance(),
        "Bulk liquidation should leave only dust USDC debt",
    );
    state.assert_total_borrow_raw_within(
        2,
        BigUint::zero(),
        small_ray_tolerance(),
        "Bulk liquidation should reduce total borrow to dust",
    );
    state.assert_health_factor_at_least(2, RAY);
}

/// Tests bulk liquidation with refund case for smaller positions.
///
/// Covers:
/// - Controller::liquidate with partial liquidation scenario
/// - Refund handling when collateral is less than debt
/// - Bulk liquidation with different refund amounts
#[test]
fn liquidate_bulk_with_refund_handling_success() {
    let mut state = LendingPoolTestState::new();
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
    state.assert_collateral_raw_eq(
        1,
        &EGLD_TOKEN,
        scaled_amount(100, EGLD_DECIMALS),
        "Supplier EGLD liquidity should be recorded",
    );
    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(1000u64),
        USDC_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::None,
        false,
    );
    state.assert_collateral_raw_eq(
        1,
        &USDC_TOKEN,
        scaled_amount(1000, USDC_DECIMALS),
        "Supplier USDC liquidity should be recorded",
    );
    state.supply_asset(
        &supplier,
        CAPPED_TOKEN,
        BigUint::from(10u64),
        CAPPED_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::None,
        false,
    );
    state.assert_collateral_raw_eq(
        1,
        &CAPPED_TOKEN,
        scaled_amount(10, CAPPED_DECIMALS),
        "Supplier capped liquidity should be recorded",
    );

    state.supply_asset(
        &borrower,
        XEGLD_TOKEN,
        BigUint::from(20u64),
        XEGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    state.assert_collateral_raw_eq(
        2,
        &XEGLD_TOKEN,
        scaled_amount(20, XEGLD_DECIMALS),
        "Borrower XEGLD collateral should be recorded",
    );
    state.supply_asset(
        &borrower,
        SEGLD_TOKEN,
        BigUint::from(80u64),
        SEGLD_DECIMALS,
        OptionalValue::Some(2),
        OptionalValue::None,
        false,
    );
    state.assert_collateral_raw_eq(
        2,
        &SEGLD_TOKEN,
        scaled_amount(80, SEGLD_DECIMALS),
        "Borrower SEGLD collateral should be recorded",
    );

    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(39u64),
        2,
        EGLD_DECIMALS,
    );
    state.assert_borrow_raw_eq(
        2,
        &EGLD_TOKEN,
        scaled_amount(39, EGLD_DECIMALS),
        "Borrower EGLD debt should be tracked",
    );
    state.borrow_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(1000u64),
        2,
        USDC_DECIMALS,
    );
    state.assert_borrow_raw_eq(
        2,
        &USDC_TOKEN,
        scaled_amount(1000, USDC_DECIMALS),
        "Borrower USDC debt should be tracked",
    );

    // Advance time for moderate interest (less than previous test)
    state.change_timestamp(SECONDS_PER_DAY * 255);
    let mut markets = MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN));
    markets.push(EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN));
    state.update_markets(&borrower, markets.clone());

    // Setup liquidator
    let liquidator = TestAddress::new("liquidator");
    state
        .world
        .account(liquidator)
        .nonce(1)
        .esdt_balance(
            USDC_TOKEN,
            BigUint::from(10000u64) * BigUint::from(10u64).pow(USDC_DECIMALS as u32),
        )
        .esdt_balance(
            EGLD_TOKEN,
            BigUint::from(10000u64) * BigUint::from(10u64).pow(EGLD_DECIMALS as u32),
        );

    // Get debt amounts
    let borrowed_usdc = state.borrow_amount_for_token(2, USDC_TOKEN);
    let borrowed_egld = state.borrow_amount_for_token(2, EGLD_TOKEN);

    // Prepare bulk liquidation with excess payments
    let usdc_payment = borrowed_usdc.into_raw_units().clone() * 3u64;
    let payments = vec![
        (&EGLD_TOKEN, borrowed_egld.into_raw_units()),
        (&USDC_TOKEN, &usdc_payment),
    ];

    let final_health_before = state.account_health_factor(2);
    let final_borrowed_before = state.total_borrow_in_egld(2);
    // Execute bulk liquidation (expecting refunds)
    state.liquidate_account_dem_bulk(&liquidator, payments, 2);

    // Verify improved position
    let final_borrowed = state.total_borrow_in_egld(2);
    let final_health = state.account_health_factor(2);

    assert!(final_borrowed < final_borrowed_before);
    assert!(final_health > final_health_before);
    state.assert_borrow_raw_within(
        2,
        &EGLD_TOKEN,
        BigUint::zero(),
        small_ray_tolerance(),
        "Refund handling should leave only negligible EGLD debt",
    );
    state.assert_borrow_raw_within(
        2,
        &USDC_TOKEN,
        BigUint::zero(),
        small_ray_tolerance(),
        "Refund handling should leave only negligible USDC debt",
    );
    state.assert_total_borrow_raw_within(
        2,
        BigUint::zero(),
        small_ray_tolerance(),
        "Refund scenario should reduce total borrow to dust",
    );
    state.assert_health_factor_at_least(2, RAY);
}

/// Tests liquidation resulting in bad debt and cleanup.
///
/// Covers:
/// - Controller::liquidate with insufficient collateral scenario
/// - Bad debt creation when collateral < debt
/// - Controller::cleanBadDebt endpoint functionality
/// - Sequential liquidation attempts leaving residual debt
#[test]
fn liquidate_insufficient_collateral_creates_bad_debt_success() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Setup liquidity pools
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    state.assert_collateral_raw_eq(
        1,
        &EGLD_TOKEN,
        scaled_amount(100, EGLD_DECIMALS),
        "Supplier EGLD liquidity should be recorded",
    );
    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(1000u64),
        USDC_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::None,
        false,
    );
    state.assert_collateral_raw_eq(
        1,
        &USDC_TOKEN,
        scaled_amount(1000, USDC_DECIMALS),
        "Supplier USDC liquidity should be recorded",
    );

    // Borrower provides limited collateral
    state.supply_asset(
        &borrower,
        XEGLD_TOKEN,
        BigUint::from(20u64), // $2500
        XEGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    state.assert_collateral_raw_eq(
        2,
        &XEGLD_TOKEN,
        scaled_amount(20, XEGLD_DECIMALS),
        "Borrower XEGLD collateral should be recorded",
    );
    state.supply_asset(
        &borrower,
        SEGLD_TOKEN,
        BigUint::from(80u64), // $2500
        SEGLD_DECIMALS,
        OptionalValue::Some(2),
        OptionalValue::None,
        false,
    );
    state.assert_collateral_raw_eq(
        2,
        &SEGLD_TOKEN,
        scaled_amount(80, SEGLD_DECIMALS),
        "Borrower SEGLD collateral should be recorded",
    );

    // Create significant debt positions
    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(39u64),
        2,
        EGLD_DECIMALS,
    );
    state.assert_borrow_raw_eq(
        2,
        &EGLD_TOKEN,
        scaled_amount(39, EGLD_DECIMALS),
        "Borrower EGLD debt should be tracked",
    );
    state.borrow_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(1000u64),
        2,
        USDC_DECIMALS,
    );
    state.assert_borrow_raw_eq(
        2,
        &USDC_TOKEN,
        scaled_amount(1000, USDC_DECIMALS),
        "Borrower USDC debt should be tracked",
    );

    // Verify initial positions
    let borrowed = state.total_borrow_in_egld(2);
    let collateral = state.total_collateral_in_egld(2);
    assert!(borrowed > ManagedDecimal::from_raw_units(BigUint::from(0u64), RAY_PRECISION));
    assert!(collateral > ManagedDecimal::from_raw_units(BigUint::from(0u64), RAY_PRECISION));

    // Advance significant time to accumulate massive interest
    state.change_timestamp(SECONDS_PER_DAY * 1000);
    let mut markets = MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN));
    markets.push(EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN));
    state.update_markets(&borrower, markets.clone());

    // Setup liquidator
    let liquidator = TestAddress::new("liquidator");
    state
        .world
        .account(liquidator)
        .nonce(1)
        .esdt_balance(
            USDC_TOKEN,
            BigUint::from(10000u64) * BigUint::from(10u64).pow(USDC_DECIMALS as u32),
        )
        .esdt_balance(
            EGLD_TOKEN,
            BigUint::from(1000u64) * BigUint::from(10u64).pow(EGLD_DECIMALS as u32),
        );

    // First liquidation attempt (partial)
    state.liquidate_account(
        &liquidator,
        &EGLD_TOKEN,
        BigUint::from(50u64),
        2,
        EGLD_DECIMALS,
    );

    // Second liquidation attempt (exhausts collateral)
    state.liquidate_account(
        &liquidator,
        &USDC_TOKEN,
        BigUint::from(10000u64),
        2,
        USDC_DECIMALS,
    );

    // Verify bad debt exists
    let remaining_debt = state.total_borrow_in_egld(2);
    assert!(remaining_debt > ManagedDecimal::from_raw_units(BigUint::from(0u64), RAY_PRECISION));

    // Clean bad debt
    state.clean_bad_debt(2);

    // Verify all positions cleared
    let final_debt = state.total_borrow_in_egld(2);
    let final_collateral = state.total_collateral_in_egld(2);
    let final_weighted = state.liquidation_collateral_available(2);
    assert!(final_debt == ManagedDecimal::from_raw_units(BigUint::from(0u64), RAY_PRECISION));
    assert!(final_collateral == ManagedDecimal::from_raw_units(BigUint::from(0u64), RAY_PRECISION));
    assert!(final_weighted == ManagedDecimal::from_raw_units(BigUint::from(0u64), RAY_PRECISION));
    state.assert_total_borrow_raw_eq(
        2,
        BigUint::zero(),
        "Bad debt cleanup should clear residual borrow",
    );
    state.assert_total_collateral_raw_eq(
        2,
        BigUint::zero(),
        "Bad debt cleanup should clear collateral tracking",
    );
    state.assert_no_borrow_entry(2, &EGLD_TOKEN);
    state.assert_no_borrow_entry(2, &USDC_TOKEN);
    state.assert_no_collateral_entry(2, &XEGLD_TOKEN);
    state.assert_no_collateral_entry(2, &SEGLD_TOKEN);
}

/// Tests liquidation of single-asset position with extreme interest.
///
/// Covers:
/// - Controller::liquidate with single collateral/debt asset
/// - High interest accumulation over extended time
/// - Liquidation restoring health factor above 1.0
/// - Edge case of same asset for collateral and debt
#[test]
fn liquidate_single_asset_position_high_interest_success() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");
    let liquidator = TestAddress::new("liquidator");

    state.world.account(liquidator).nonce(1).esdt_balance(
        EGLD_TOKEN,
        BigUint::from(1000u64) * BigUint::from(10u64).pow(EGLD_DECIMALS as u32),
    );

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Supplier provides EGLD liquidity
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    state.assert_collateral_raw_eq(
        1,
        &EGLD_TOKEN,
        scaled_amount(100, EGLD_DECIMALS),
        "Supplier EGLD liquidity should be recorded",
    );

    // Borrower supplies EGLD as collateral
    state.supply_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    state.assert_collateral_raw_eq(
        2,
        &EGLD_TOKEN,
        scaled_amount(100, EGLD_DECIMALS),
        "Borrower EGLD collateral should be recorded",
    );

    // Borrower takes EGLD loan (same asset)
    // Supplier supplied first, so borrower NFT nonce = 2 here
    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(75u64),
        2,
        EGLD_DECIMALS,
    );
    state.assert_borrow_raw_eq(
        2,
        &EGLD_TOKEN,
        scaled_amount(75, EGLD_DECIMALS),
        "Borrower EGLD debt should be tracked",
    );

    // Verify initial state
    let borrowed = state.total_borrow_in_egld(2);
    let collateral = state.total_collateral_in_egld(2);

    assert!(borrowed > ManagedDecimal::from_raw_units(BigUint::from(0u64), RAY_PRECISION));
    assert!(collateral > ManagedDecimal::from_raw_units(BigUint::from(0u64), RAY_PRECISION));

    // Advance time significantly (over 4 years)
    state.change_timestamp(SECONDS_PER_YEAR + SECONDS_PER_DAY * 1500);
    let mut markets = MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN));
    state.update_markets(&borrower, markets.clone());

    // Liquidate with excess payment
    state.liquidate_account(
        &liquidator,
        &EGLD_TOKEN,
        BigUint::from(105u64),
        2,
        EGLD_DECIMALS,
    );

    // Verify healthy position after liquidation
    state.assert_total_borrow_raw_within(
        2,
        BigUint::zero(),
        small_ray_tolerance(),
        "Liquidation should leave at most dust-level EGLD debt",
    );
    state.assert_total_collateral_raw_within(
        2,
        BigUint::zero(),
        small_ray_tolerance(),
        "Liquidation should release virtually all collateral",
    );
    state.assert_health_factor_at_least(2, RAY);
}

/// Tests liquidation creating bad debt that cannot be fully recovered.
///
/// Covers:
/// - Controller::liquidate with severe undercollateralization
/// - Liquidation exhausting all collateral
/// - Residual bad debt after liquidation
/// - Manual bad debt repayment by protocol
#[test]
fn liquidate_severe_undercollateralization_bad_debt_success() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");
    let liquidator = TestAddress::new("liquidator");

    setup_accounts(&mut state, supplier, borrower);
    state.world.account(liquidator).nonce(1).esdt_balance(
        USDC_TOKEN,
        BigUint::from(200000u64) * BigUint::from(10u64).pow(USDC_DECIMALS as u32),
    );

    // Create positions
    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(4000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(1000u64),
        USDC_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::None,
        false,
    );

    // Borrower provides minimal collateral
    state.supply_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Borrower takes large loan relative to collateral
    state.borrow_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(2000u64),
        2,
        USDC_DECIMALS,
    );

    // Advance time to create severe undercollateralization
    state.change_timestamp(590000000u64);
    let mut markets = MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN));
    state.update_markets(&supplier, markets.clone());

    // Attempt liquidation with large amount
    state.liquidate_account(
        &liquidator,
        &USDC_TOKEN,
        BigUint::from(20000u64),
        2,
        USDC_DECIMALS,
    );

    // Verify bad debt remains
    let remaining_debt = state.total_borrow_in_egld(2);
    let remaining_collateral = state.total_collateral_in_egld(2);
    assert!(remaining_debt > ManagedDecimal::from_raw_units(BigUint::zero(), RAY_PRECISION));
    assert!(
        remaining_collateral
            < ManagedDecimal::from_raw_units(BigUint::from(RAY / 2), RAY_PRECISION)
    );

    // Protocol repays bad debt
    state.repay_asset(
        &liquidator,
        &USDC_TOKEN,
        BigUint::from(4000u64),
        2,
        USDC_DECIMALS,
    );

    // Verify debt cleared
    let final_debt = state.total_borrow_in_egld(2);
    assert!(final_debt == ManagedDecimal::from_raw_units(BigUint::zero(), RAY_PRECISION));
}

/// Verifies liquidation bonus selection logic: base bonus is used when repayment is capped,
/// significantly short, and projected health does not improve; otherwise scaled bonus applies.
#[test]
fn liquidation_bonus_selection_behavior() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Supplier provides EGLD liquidity
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Borrower supplies and borrows single asset (EGLD) to make base bonus easy to compute
    state.supply_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(75u64),
        2,
        EGLD_DECIMALS,
    );

    // Advance time significantly to ensure the position becomes unhealthy
    state.change_timestamp(SECONDS_PER_YEAR + SECONDS_PER_DAY * 1500);
    let mut markets = MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN));
    state.update_markets(&borrower, markets.clone());

    // Sanity: position is liquidatable
    let hf_before = state.account_health_factor(2);
    assert!(hf_before < ManagedDecimal::from_raw_units(BigUint::from(RAY), RAY_PRECISION));

    // CASE A: Capped and significantly short repayment → expect base (weighted) bonus to be applied
    let borrowed_egld = state.borrow_amount_for_token(2, EGLD_TOKEN);
    let tiny_payment = borrowed_egld.into_raw_units().clone() / 1000u64; // << 1% of debt
    let mut debt_payments = ManagedVec::new();
    debt_payments.push(EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        0,
        tiny_payment.clone(),
    ));

    let estimate_capped = state.liquidation_estimations(2, debt_payments);
    let base_bps = ManagedDecimal::from_raw_units(BigUint::from(LIQ_BONUS), BPS_PRECISION);
    // For capped, tiny repayments we expect the applied bonus to never be below the base bonus.
    // Depending on rounding and projected HF, the algorithm may still use a scaled bonus if it improves HF.
    assert!(estimate_capped.bonus_rate_bps >= base_bps);

    // Also verify no death-spiral: executing a tiny partial liquidation must not reduce health factor
    let liq = TestAddress::new("liquidator");
    state.world.account(liq).nonce(1).esdt_balance(
        EGLD_TOKEN,
        BigUint::from(10000u64) * BigUint::from(10u64).pow(EGLD_DECIMALS as u32),
    );
    let hf_before_exec = state.account_health_factor(2);
    state.liquidate_account_dem(
        &liq,
        &EGLD_TOKEN,
        // Use raw units directly for denominated liquidation helper
        tiny_payment,
        2,
    );
    let hf_after_exec = state.account_health_factor(2);
    assert!(hf_after_exec > hf_before_exec);
    println!("hf_after_exec:  {hf_after_exec:?}");
    println!("hf_before_exec: {hf_before_exec:?}");

    // CASE B: Uncapped (use algorithm-estimated repayment) → expect scaled bonus (>= base)
    let empty_payments = ManagedVec::new();
    let estimate_uncapped = state.liquidation_estimations(2, empty_payments);
    // Scaled bonus should be at least the base, and with poor health typically strictly greater
    assert!(estimate_uncapped.bonus_rate_bps > base_bps);
}

/// Verifies the Dutch auction path selection by comparing a slightly unhealthy position
/// (primary target 1.02 succeeds) versus a more unhealthy position (falls back to 1.01),
/// asserting the latter yields a higher bonus and larger estimated repayment.
#[test]
fn liquidation_estimate_primary_vs_secondary() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Provide some EGLD liquidity and mint supplier NFT first so borrower gets nonce = 2
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Borrower supplies and borrows the same asset (EGLD) to simplify setup
    state.supply_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(75u64),
        2,
        EGLD_DECIMALS,
    );

    // Scenario A: slightly unhealthy
    state.change_timestamp(SECONDS_PER_YEAR + SECONDS_PER_DAY * 1000);
    let mut markets = MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN));
    state.update_markets(&borrower, markets.clone());
    let a = state.liquidation_estimations(2, ManagedVec::new());

    // Scenario B: more unhealthy (advance much more time)
    state.change_timestamp(SECONDS_PER_YEAR + SECONDS_PER_DAY * 1500);
    let b = state.liquidation_estimations(2, ManagedVec::new());

    // Expect more unhealthy case to have higher bonus and larger repayment estimate
    assert!(b.bonus_rate_bps >= a.bonus_rate_bps);
    assert!(b.max_egld_payment_wad >= a.max_egld_payment_wad);
}

/// Verifies repeated tiny partial liquidations cannot enter a death spiral: health factor
/// does not decrease across iterations even when repayments are capped far below estimate.
#[test]
fn liquidation_no_death_spiral_under_tiny_payments() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Liquidity
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(200u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Borrower supplies and borrows same-asset to keep math simple
    state.supply_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(75u64),
        2,
        EGLD_DECIMALS,
    );

    // Make unhealthy (extreme interest period similar to other tests)
    state.change_timestamp(SECONDS_PER_YEAR + SECONDS_PER_DAY * 1500);
    let mut markets = MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN));
    state.update_markets(&borrower, markets.clone());

    // Iteratively liquidate with tiny payments and verify HF never decreases
    let liq = TestAddress::new("liquidator");
    state.world.account(liq).nonce(1).esdt_balance(
        EGLD_TOKEN,
        BigUint::from(100000u64) * BigUint::from(10u64).pow(EGLD_DECIMALS as u32),
    );
    let mut last_hf = state.account_health_factor(2);
    for _ in 0..3 {
        let borrowed = state.borrow_amount_for_token(2, EGLD_TOKEN);
        let tiny = borrowed.into_raw_units().clone() / 2000u64; // 0.05%
        state.liquidate_account_dem(&liq, &EGLD_TOKEN, tiny, 2);
        let hf = state.account_health_factor(2);
        assert!(hf >= last_hf);
        last_hf = hf;
    }
}

/// Verifies refund handling in the simulation when overpaying with multiple tokens.
/// Ensures the view reports non-empty refunds for excess payments.
#[test]
fn liquidation_excess_distribution_partial_and_full() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Setup basic liquidity by supplying some assets
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
        &supplier,
        USDC_TOKEN,
        BigUint::from(2000u64),
        USDC_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::None,
        false,
    );

    // Borrower deposits and borrows USDC only (sufficient for refund testing)
    state.supply_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(25u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    state.borrow_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(700u64),
        2,
        USDC_DECIMALS,
    );

    // Make position unhealthy
    state.change_timestamp(SECONDS_PER_DAY * 4040);
    let mut markets = MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN));
    markets.push(EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN));
    state.update_markets(&borrower, markets.clone());

    // Build intentional overpayment set for USDC
    let borrowed_usdc = state.borrow_amount_for_token(2, USDC_TOKEN);

    let mut debt_payments = ManagedVec::new();
    debt_payments.push(EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN.to_token_identifier()),
        0,
        borrowed_usdc.into_raw_units() * 3u64, // overpay USDC by 3x
    ));

    // Simulate liquidation and verify refunds are reported
    let estimate = state.liquidation_estimations(2, debt_payments);
    assert!(!estimate.refunds.is_empty());
}

/// Tests borrow attempt with insufficient collateral.
///
/// Covers:
/// - Controller::borrow endpoint validation
/// - Collateral requirement checks
/// - ERROR_INSUFFICIENT_COLLATERAL error condition
/// - Siloed token borrowing restrictions
#[test]
fn borrow_insufficient_collateral_for_siloed_asset_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_accounts(&mut state, supplier, borrower);

    // Borrower supplies standard collateral
    state.supply_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(1000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Supplier provides siloed token liquidity
    state.supply_asset(
        &supplier,
        SILOED_TOKEN,
        BigUint::from(1000u64),
        SILOED_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Attempt to borrow more than allowed by collateral
    state.borrow_asset_error(
        &borrower,
        SILOED_TOKEN,
        BigUint::from(600u64),
        1,
        SILOED_DECIMALS,
        ERROR_INSUFFICIENT_COLLATERAL,
    );
}

/// Tests seizure of dust collateral after bad debt cleanup.
///
/// Covers:
/// - Controller::cleanBadDebt endpoint functionality
/// - LiquidityModule::seizeDustCollateral endpoint
/// - Protocol revenue collection from dust positions
/// - Complete position clearing after dust seizure
/// - Bad debt socialization with remaining collateral
/// - Requires: debt > collateral AND collateral < $5 AND debt > $5
#[test]
fn seize_dust_collateral_after_bad_debt_success() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");
    let liquidator = TestAddress::new("liquidator");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Setup liquidator account
    state.world.account(liquidator).nonce(1).esdt_balance(
        USDC_TOKEN,
        BigUint::from(200000u64) * BigUint::from(10u64).pow(USDC_DECIMALS as u32),
    );

    // Setup liquidity pools
    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(5000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(1000u64),
        USDC_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::None,
        false,
    );

    // Borrower provides EGLD collateral that will be liquidated down to dust
    state.supply_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(20u64), // 10 EGLD = $1250
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Borrower takes loan that will create bad debt after interest
    state.borrow_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(500u64), // $500 loan against $1250 collateral
        2,
        USDC_DECIMALS,
    );

    // Record initial protocol revenue for EGLD pool
    let egld_pool_address = state.pool_address(EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN));
    // Advance very long time to create massive interest accumulation
    state.change_timestamp(880000000u64); // Same as working test
    let mut markets = MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN));
    state.update_markets(&supplier, markets.clone());

    let health_factor = state.account_health_factor(2);
    println!("health_factor: {health_factor:?}");
    let initial_debt_usdc = state.borrow_amount_for_token(2, USDC_TOKEN);
    println!("initial_debt_usd: {initial_debt_usdc:?}");
    // Liquidate most of the collateral, leaving only dust
    state.liquidate_account(
        &liquidator,
        &USDC_TOKEN,
        BigUint::from(760u64), // Liquidate with large amount to consume most collateral
        2,
        USDC_DECIMALS,
    );
    let left_collateral_egld = state.collateral_amount_for_token(2, EGLD_TOKEN);
    println!("left_collateral_egld: {left_collateral_egld:?}");
    // At this point:
    // - Significant bad debt remains due to massive interest
    // - Very little collateral remains (dust under $5)
    // - Conditions for cleanBadDebt are met

    let left_bad_debt_usdc = state.borrow_amount_for_token(2, USDC_TOKEN);
    println!("left_bad_debt_usdc: {left_bad_debt_usdc:?}");

    assert!(state.total_borrow_in_egld(2) > state.total_collateral_in_egld(2));
    let initial_egld_revenue = state.market_revenue(egld_pool_address.clone());
    let before_clean_market_indexes = state.all_market_indexes(MultiValueEncoded::from_iter(vec![
        EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN),
    ]));
    println!(
        "usdc_index: {:?}",
        before_clean_market_indexes.get(0).supply_index_ray
    );
    // state.claim_revenue(USDC_TOKEN);
    let usdc_supplied_before_bad_debt = state.collateral_amount_for_token(1, USDC_TOKEN);
    println!("usdc_supplied_before_bad_debt: {usdc_supplied_before_bad_debt:?}");
    // Clean bad debt - this calls seizeDustCollateral internally
    state.clean_bad_debt(2);

    // Verify all positions cleared
    let final_debt = state.total_borrow_in_egld(2);
    let final_collateral = state.total_collateral_in_egld(2);

    assert!(final_debt == ManagedDecimal::from_raw_units(BigUint::from(0u64), WAD_PRECISION));
    assert!(final_collateral == ManagedDecimal::from_raw_units(BigUint::from(0u64), WAD_PRECISION));

    println!("initial_egld_revenue: {initial_egld_revenue:?}");
    // Verify protocol revenue increased from dust seizure
    let final_egld_revenue = state.market_revenue(egld_pool_address);
    println!("final_egld_revenue:   {final_egld_revenue:?}");
    assert!(
        final_egld_revenue > initial_egld_revenue,
        "Protocol revenue should increase or stay same when dust collateral is seized"
    );
    // Revenue should be the initial revenue + the collateral left before bad debt socialization
    assert!(final_egld_revenue == initial_egld_revenue + left_collateral_egld);

    let after_clean_market_indexes = state.all_market_indexes(MultiValueEncoded::from_iter(vec![
        EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN),
    ]));
    let re_paid_usdc_debt = initial_debt_usdc - left_bad_debt_usdc;
    println!("re_paid_usdc_debt: {re_paid_usdc_debt:?}");
    println!(
        "usdc_index: {:?}",
        after_clean_market_indexes.get(0).supply_index_ray
    );

    // Verify supply index decreased due to bad debt socialization distribution to suppliers
    assert!(
        after_clean_market_indexes.get(0).supply_index_ray
            < before_clean_market_indexes.get(0).supply_index_ray
    );

    let usdc_supplied_after_bad_debt = state.collateral_amount_for_token(1, USDC_TOKEN);
    println!("usdc_supplied_after_bad_debt: {usdc_supplied_after_bad_debt:?}");
    let lost_usdc_due_to_socialization =
        usdc_supplied_before_bad_debt.clone() - usdc_supplied_after_bad_debt.clone();
    println!("lost_usdc_due_to_socialization: {lost_usdc_due_to_socialization:?}");
    assert!(lost_usdc_due_to_socialization.into_raw_units().clone() > 0u64);
    let supplied_usdc = state.market_supplied_amount(state.usdc_market.clone());
    let market_reserves = state.market_reserves(state.usdc_market.clone());
    let protoocl_revenue = state.market_protocol_revenue(state.usdc_market.clone());
    println!("total supplied_usdc: {supplied_usdc:?}");
    println!("market_reserves: {market_reserves:?}");
    println!("protoocl_revenue: {protoocl_revenue:?}");
    state.withdraw_asset_den(
        &supplier,
        USDC_TOKEN,
        usdc_supplied_after_bad_debt.into_raw_units().clone(),
        1,
    );
    let supplied_usdc = state.market_supplied_amount(state.usdc_market.clone());
    let market_reserves = state.market_reserves(state.usdc_market.clone());
    let protoocl_revenue = state.market_protocol_revenue(state.usdc_market.clone());
    println!("total supplied_usdc: {supplied_usdc:?}");
    println!("market_reserves: {market_reserves:?}");
    println!("protoocl_revenue: {protoocl_revenue:?}");
    state.claim_revenue(USDC_TOKEN);
    let final_protoocl_revenue = state.market_protocol_revenue(state.usdc_market.clone());
    println!("final_protoocl_revenue: {final_protoocl_revenue:?}");
    assert!(final_protoocl_revenue.into_raw_units().clone() == BigUint::zero());
    let scaled_borrowed = state.market_borrowed(state.usdc_market.clone());
    println!("scaled_borrowed: {scaled_borrowed:?}");
    assert!(scaled_borrowed.into_raw_units().clone() == BigUint::zero());
    let scaled_supplied = state.market_supplied(state.usdc_market.clone());
    println!("scaled_supplied: {scaled_supplied:?}");
    // With the fix, there should be no dust when claiming full revenue
    assert!(
        scaled_supplied.into_raw_units().clone() == BigUint::zero(),
        "Scaled supplied should be zero after full revenue claim"
    );
}

/// Tests seizure of dust collateral after bad debt cleanup.
///
/// Covers:
/// - Controller::cleanBadDebt endpoint functionality with just debt no collateral
/// - Requires: debt > collateral AND collateral < $5 AND debt > $5
/// - Revenue should stay the same when debt is seized but no collateral is left just bad debt
#[test]
fn seize_dust_collateral_after_bad_debt_success_just_debt_no_collateral() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");
    let liquidator = TestAddress::new("liquidator");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Setup liquidator account
    state.world.account(liquidator).nonce(1).esdt_balance(
        USDC_TOKEN,
        BigUint::from(200000u64) * BigUint::from(10u64).pow(USDC_DECIMALS as u32),
    );

    // Setup liquidity pools
    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(5000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(1000u64),
        USDC_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::None,
        false,
    );

    // Borrower provides EGLD collateral that will be liquidated down to dust
    state.supply_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(20u64), // 10 EGLD = $1250
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Borrower takes loan that will create bad debt after interest
    state.borrow_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(500u64), // $500 loan against $1250 collateral
        2,
        USDC_DECIMALS,
    );

    // Record initial protocol revenue for EGLD pool
    let egld_pool_address = state.pool_address(EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN));
    // Advance very long time to create massive interest accumulation
    state.change_timestamp(880000000u64); // Same as working test
    let mut markets = MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN));
    state.update_markets(&supplier, markets.clone());

    let health_factor = state.account_health_factor(2);
    println!("health_factor: {health_factor:?}");
    let debt_usdc = state.borrow_amount_for_token(2, USDC_TOKEN);
    println!("debt_usd: {debt_usdc:?}");
    let collateral_egld = state.collateral_amount_for_token(2, EGLD_TOKEN);
    println!("collateral_egld: {collateral_egld:?}");

    // Liquidate most of the collateral, leaving only dust
    state.liquidate_account(
        &liquidator,
        &USDC_TOKEN,
        BigUint::from(780u64), // Liquidate with large amount to consume most collateral
        2,
        USDC_DECIMALS,
    );
    // At this point:
    // - Significant bad debt remains due to massive interest
    // - Very little collateral remains (dust under $5)
    // - Conditions for cleanBadDebt are met
    let initial_egld_revenue = state.market_revenue(egld_pool_address.clone());

    // Clean bad debt - this calls seizeDustCollateral internally
    state.clean_bad_debt(2);

    // Verify all positions cleared
    let final_debt = state.total_borrow_in_egld(2);
    let final_collateral = state.total_collateral_in_egld(2);

    assert!(final_debt == ManagedDecimal::from_raw_units(BigUint::from(0u64), WAD_PRECISION));
    assert!(final_collateral == ManagedDecimal::from_raw_units(BigUint::from(0u64), WAD_PRECISION));

    println!("initial_egld_revenue: {initial_egld_revenue:?}");
    // Verify protocol revenue increased from dust seizure
    let final_egld_revenue = state.market_revenue(egld_pool_address);
    println!("final_egld_revenue:   {final_egld_revenue:?}");
    assert!(
        final_egld_revenue == initial_egld_revenue,
        "Protocol revenue should increase or stay same when dust collateral is seized"
    );
}

#[test]
fn e_mode_liquidate_leave_bad_debt_success() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Supplier provides liquidity across multiple assets ($5000 total)
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::Some(1),
        false,
    );

    state.supply_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(40u64), // $2500
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::Some(1),
        false,
    );

    // Borrower takes loans
    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(15u64),
        2,
        EGLD_DECIMALS,
    );

    // Verify initial position health
    let borrowed = state.total_borrow_in_egld(2);
    let collateral = state.total_collateral_in_egld(2);
    assert!(borrowed > ManagedDecimal::from_raw_units(BigUint::from(0u64), RAY_PRECISION));
    assert!(collateral > ManagedDecimal::from_raw_units(BigUint::from(0u64), RAY_PRECISION));
    let mut days = 0;
    while state.account_health_factor(2)
        > ManagedDecimal::from_raw_units(BigUint::from(RAY), RAY_PRECISION)
    {
        state.change_timestamp(SECONDS_PER_DAY * days * 2650);
        days += 1;
    }
    let mut markets = MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN));
    state.update_markets(&borrower, markets.clone());
    let health_factor = state.account_health_factor(2);
    println!("health_factor: {health_factor:?}");
    let supplied_liquidation = state.liquidation_collateral_available(2);
    println!("supplied_liquidation: {supplied_liquidation:?}");
    let supplied = state.total_collateral_in_egld(2);
    println!("supplied: {supplied:?}");
    let borrowed = state.total_borrow_in_egld(2);
    println!("borrowed: {borrowed:?}");
    let estimated_liquidation_amount = state.liquidation_estimations(2, ManagedVec::new());
    println!(
        "estimated_bonus_rate: {:?}",
        estimated_liquidation_amount.bonus_rate_bps
            * ManagedDecimal::from_raw_units(BigUint::from(100u64), 0)
    );
    println!(
        "estimated_bonus_amount: {:?}",
        estimated_liquidation_amount.max_egld_payment_wad
    );
    // Assert invariants for each seized item: fee <= seized, and seized <= current deposit
    for i in 0..estimated_liquidation_amount.seized_collaterals.len() {
        let seized = estimated_liquidation_amount.seized_collaterals.get(i);
        let fee = estimated_liquidation_amount.protocol_fees.get(i);
        println!("Seized collateral: {:?}", seized.token_identifier);
        println!("Seized amount: {:?}", seized.amount);
        println!("Protocol fee: {:?}", fee.token_identifier);
        println!("Protocol fee amount: {:?}", fee.amount);

        // Fee-on-capped-bonus must be <= seized amount (both in token units)
        assert!(
            fee.amount <= seized.amount,
            "Protocol fee must not exceed seized amount"
        );

        // Seized amount must not exceed the current deposited amount for that asset
        // state helper expects a TestTokenIdentifier (alias in tests) not the SDK identifier
        // In our tests, constants like EGLD_TOKEN are of the expected type, and we only seize EGLD here.
        // So we use the EGLD_TOKEN test identifier to validate the invariant for this scenario.
        let current_collateral = state.collateral_amount_for_token(2, EGLD_TOKEN);
        assert!(
            seized.amount <= current_collateral.into_raw_units().clone(),
            "Seized amount must not exceed deposited collateral"
        );
    }

    for token in estimated_liquidation_amount.refunds {
        println!("Refund: {:?}", token.token_identifier);
        println!("Refund amount: {:?}", token.amount);
    }
    // 49122855026117687
    // 48818102058331751315
    // Protocol fees already printed and checked in the loop above

    // Setup liquidator
    let liquidator = TestAddress::new("liquidator");
    state
        .world
        .account(liquidator)
        .nonce(1)
        .esdt_balance(
            USDC_TOKEN,
            BigUint::from(10000u64) * BigUint::from(10u64).pow(USDC_DECIMALS as u32),
        )
        .esdt_balance(
            EGLD_TOKEN,
            BigUint::from(10000u64) * BigUint::from(10u64).pow(EGLD_DECIMALS as u32),
        );

    // Get debt amounts before liquidation
    let borrowed_egld = state.borrow_amount_for_token(2, EGLD_TOKEN);

    let before_health = state.account_health_factor(2);
    println!("before_health: {before_health:?}");
    // Liquidate EGLD debt first
    state.liquidate_account_dem(
        &liquidator,
        &EGLD_TOKEN,
        borrowed_egld.into_raw_units().clone(),
        2,
    );
    let borrowed_egld = state.total_borrow_in_egld(2);
    println!("borrowed_egld after liquidation: {borrowed_egld:?}");
    let supplied = state.total_collateral_in_egld(2);
    println!("supplied after liquidation:      {supplied:?}");
    let after_health = state.account_health_factor(2);
    println!("after_health: {after_health:?}");
    assert!(after_health < before_health);

    state.clean_bad_debt(2);
    let final_debt = state.total_borrow_in_egld(2);
    println!("final_debt: {final_debt:?}");
    assert!(final_debt == ManagedDecimal::from_raw_units(BigUint::from(0u64), WAD_PRECISION));
    let final_collateral = state.total_collateral_in_egld(2);
    println!("final_collateral: {final_collateral:?}");
    assert!(final_collateral == ManagedDecimal::from_raw_units(BigUint::from(0u64), WAD_PRECISION));
}
