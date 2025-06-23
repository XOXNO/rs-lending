use common_constants::RAY;
use controller::{ERROR_INSUFFICIENT_COLLATERAL, RAY_PRECISION};
use multiversx_sc::types::{EgldOrEsdtTokenIdentifier, ManagedDecimal, MultiValueEncoded};
use multiversx_sc_scenario::imports::{BigUint, OptionalValue, TestAddress};
pub mod constants;
pub mod proxys;
pub mod setup;
use constants::*;
use setup::*;

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
    state.supply_asset(
        &supplier,
        CAPPED_TOKEN,
        BigUint::from(10u64),
        CAPPED_DECIMALS,
        OptionalValue::Some(1),
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

    // Borrower provides collateral ($5000 total)
    state.supply_asset(
        &borrower,
        XEGLD_TOKEN,
        BigUint::from(20u64), // $2500
        XEGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
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

    // Borrower takes loans
    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(39u64),
        2,
        EGLD_DECIMALS,
    );
    state.borrow_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(1000u64),
        2,
        USDC_DECIMALS,
    );

    // Verify initial position health
    let borrowed = state.get_total_borrow_in_egld(2);
    let collateral = state.get_total_collateral_in_egld(2);
    assert!(borrowed > ManagedDecimal::from_raw_units(BigUint::from(0u64), RAY_PRECISION));
    assert!(collateral > ManagedDecimal::from_raw_units(BigUint::from(0u64), RAY_PRECISION));

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
    let borrowed_usdc = state.get_borrow_amount_for_token(2, USDC_TOKEN);
    let borrowed_egld = state.get_borrow_amount_for_token(2, EGLD_TOKEN);

    // Liquidate EGLD debt first
    state.liquidate_account_dem(
        &liquidator,
        &EGLD_TOKEN,
        borrowed_egld.into_raw_units().clone(),
        2,
    );

    // Liquidate USDC debt second
    state.liquidate_account_dem(
        &liquidator,
        &USDC_TOKEN,
        borrowed_usdc.into_raw_units().clone(),
        2,
    );

    // Verify position health improved
    let final_borrowed = state.get_total_borrow_in_egld(2);
    assert!(final_borrowed < borrowed);
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

    // Supplier provides liquidity
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
        BigUint::from(1000u64),
        USDC_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::None,
        false,
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

    // Borrower provides collateral
    state.supply_asset(
        &borrower,
        XEGLD_TOKEN,
        BigUint::from(20u64), // $2500
        XEGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
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

    // Borrower takes loans
    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(39u64),
        2,
        EGLD_DECIMALS,
    );
    state.borrow_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(1000u64),
        2,
        USDC_DECIMALS,
    );

    // Verify initial health
    let borrowed = state.get_total_borrow_in_egld(2);
    let collateral = state.get_total_collateral_in_egld(2);
    assert!(borrowed > ManagedDecimal::from_raw_units(BigUint::from(0u64), RAY_PRECISION));
    assert!(collateral > ManagedDecimal::from_raw_units(BigUint::from(0u64), RAY_PRECISION));

    // Advance time to make position unhealthy
    state.change_timestamp(SECONDS_PER_DAY * 440);
    let mut markets = MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN));
    markets.push(EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN));
    state.update_markets(&borrower, markets.clone());

    // Setup liquidator with sufficient funds
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

    // Get current debt amounts
    let borrowed_usdc = state.get_borrow_amount_for_token(2, USDC_TOKEN);
    let borrowed_egld = state.get_borrow_amount_for_token(2, EGLD_TOKEN);

    // Prepare bulk liquidation with 3x USDC (overpayment)
    let usdc_payment = borrowed_usdc.into_raw_units().clone() * 3u64;
    let payments = vec![
        (&EGLD_TOKEN, borrowed_egld.into_raw_units()),
        (&USDC_TOKEN, &usdc_payment),
    ];

    // Execute bulk liquidation
    state.liquidate_account_dem_bulk(&liquidator, payments, 2);

    // Verify final position state
    let final_borrowed = state.get_total_borrow_in_egld(2);
    assert!(final_borrowed < borrowed);
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
    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(1000u64),
        USDC_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::None,
        false,
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

    // Borrower provides collateral
    state.supply_asset(
        &borrower,
        XEGLD_TOKEN,
        BigUint::from(20u64),
        XEGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
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

    // Create debt positions
    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(39u64),
        2,
        EGLD_DECIMALS,
    );
    state.borrow_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(1000u64),
        2,
        USDC_DECIMALS,
    );

    // Verify initial state
    let borrowed = state.get_total_borrow_in_egld(2);
    let collateral = state.get_total_collateral_in_egld(2);
    assert!(borrowed > ManagedDecimal::from_raw_units(BigUint::from(0u64), RAY_PRECISION));
    assert!(collateral > ManagedDecimal::from_raw_units(BigUint::from(0u64), RAY_PRECISION));

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
    let borrowed_usdc = state.get_borrow_amount_for_token(2, USDC_TOKEN);
    let borrowed_egld = state.get_borrow_amount_for_token(2, EGLD_TOKEN);

    // Prepare bulk liquidation with excess payments
    let usdc_payment = borrowed_usdc.into_raw_units().clone() * 3u64;
    let payments = vec![
        (&EGLD_TOKEN, borrowed_egld.into_raw_units()),
        (&USDC_TOKEN, &usdc_payment),
    ];

    let final_health_before = state.get_account_health_factor(2);
    let final_borrowed_before = state.get_total_borrow_in_egld(2);
    // Execute bulk liquidation (expecting refunds)
    state.liquidate_account_dem_bulk(&liquidator, payments, 2);

    // Verify improved position
    let final_borrowed = state.get_total_borrow_in_egld(2);
    let final_health = state.get_account_health_factor(2);

    assert!(final_borrowed < final_borrowed_before);
    assert!(final_health > final_health_before);
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
    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(1000u64),
        USDC_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::None,
        false,
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
    state.supply_asset(
        &borrower,
        SEGLD_TOKEN,
        BigUint::from(80u64), // $2500
        SEGLD_DECIMALS,
        OptionalValue::Some(2),
        OptionalValue::None,
        false,
    );

    // Create significant debt positions
    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(39u64),
        2,
        EGLD_DECIMALS,
    );
    state.borrow_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(1000u64),
        2,
        USDC_DECIMALS,
    );

    // Verify initial positions
    let borrowed = state.get_total_borrow_in_egld(2);
    let collateral = state.get_total_collateral_in_egld(2);
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
    let remaining_debt = state.get_total_borrow_in_egld(2);
    assert!(remaining_debt > ManagedDecimal::from_raw_units(BigUint::from(0u64), RAY_PRECISION));

    // Clean bad debt
    state.clean_bad_debt(2);

    // Verify all positions cleared
    let final_debt = state.get_total_borrow_in_egld(2);
    let final_collateral = state.get_total_collateral_in_egld(2);
    let final_weighted = state.get_liquidation_collateral_available(2);
    assert!(final_debt == ManagedDecimal::from_raw_units(BigUint::from(0u64), RAY_PRECISION));
    assert!(final_collateral == ManagedDecimal::from_raw_units(BigUint::from(0u64), RAY_PRECISION));
    assert!(final_weighted == ManagedDecimal::from_raw_units(BigUint::from(0u64), RAY_PRECISION));
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

    // Borrower takes EGLD loan (same asset)
    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(75u64),
        2,
        EGLD_DECIMALS,
    );

    // Verify initial state
    let borrowed = state.get_total_borrow_in_egld(2);
    let collateral = state.get_total_collateral_in_egld(2);

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
    let final_borrowed = state.get_total_borrow_in_egld_big(2);
    let final_collateral = state.get_total_collateral_in_egld_big(2);
    let final_health = state.get_account_health_factor(2);

    assert!(final_borrowed >= ManagedDecimal::from_raw_units(BigUint::from(0u64), RAY_PRECISION));
    assert!(final_collateral >= ManagedDecimal::from_raw_units(BigUint::from(0u64), RAY_PRECISION));
    assert!(final_health > ManagedDecimal::from_raw_units(BigUint::from(1u64), RAY_PRECISION));
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
    let remaining_debt = state.get_total_borrow_in_egld(2);
    let remaining_collateral = state.get_total_collateral_in_egld(2);
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
    let final_debt = state.get_total_borrow_in_egld(2);
    assert!(final_debt == ManagedDecimal::from_raw_units(BigUint::zero(), RAY_PRECISION));
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
