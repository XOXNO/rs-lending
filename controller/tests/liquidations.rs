use common_constants::RAY;
pub use common_constants::{BPS_PRECISION, RAY_PRECISION, WAD_PRECISION};

use controller::ERROR_INSUFFICIENT_COLLATERAL;

use multiversx_sc::types::{EgldOrEsdtTokenIdentifier, ManagedDecimal, ManagedVec, MultiValueEncoded};
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
    let borrowed = state.total_borrow_in_egld(2);
    let collateral = state.total_collateral_in_egld(2);
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
    let borrowed_usdc = state.borrow_amount_for_token(2, USDC_TOKEN);
    let borrowed_egld = state.borrow_amount_for_token(2, EGLD_TOKEN);

    let before_health = state.account_health_factor(2);
    println!("before_health: {:?}", before_health);
    // Liquidate EGLD debt first
    state.liquidate_account_dem(
        &liquidator,
        &EGLD_TOKEN,
        borrowed_egld.into_raw_units().clone(),
        2,
    );
    let after_health = state.account_health_factor(2);
    println!("after_health: {:?}", after_health);
    assert!(after_health > before_health);

    // // Liquidate USDC debt second
    state.liquidate_account_dem(
        &liquidator,
        &USDC_TOKEN,
        borrowed_usdc.into_raw_units().clone(),
        2,
    );
    let after_health = state.account_health_factor(2);
    println!("after_health: {:?}", after_health);
    assert!(after_health > before_health);
    // Verify position health improved
    let final_borrowed = state.total_borrow_in_egld(2);
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
    let borrowed = state.total_borrow_in_egld(2);
    let collateral = state.total_collateral_in_egld(2);
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
    let borrowed_usdc = state.borrow_amount_for_token(2, USDC_TOKEN);
    let borrowed_egld = state.borrow_amount_for_token(2, EGLD_TOKEN);

    // Prepare bulk liquidation with 3x USDC (overpayment)
    let usdc_payment = borrowed_usdc.into_raw_units().clone() * 3u64;
    let payments = vec![
        (&EGLD_TOKEN, borrowed_egld.into_raw_units()),
        (&USDC_TOKEN, &usdc_payment),
    ];

    // Execute bulk liquidation
    state.liquidate_account_dem_bulk(&liquidator, payments, 2);
    let after_health = state.account_health_factor(2);
    println!("after_health: {:?}", after_health);
    // Verify final position state
    let final_borrowed = state.total_borrow_in_egld(2);
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
    let borrowed = state.total_borrow_in_egld(2);
    let collateral = state.total_collateral_in_egld(2);
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
    let final_borrowed = state.total_borrow_in_egld_big(2);
    let final_collateral = state.total_collateral_in_egld_big(2);
    let final_health = state.account_health_factor(2);
    println!("final_borrowed: {:?}", final_borrowed);
    println!("final_collateral: {:?}", final_collateral);
    println!("final_health: {:?}", final_health);
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
    println!("health_factor: {:?}", health_factor);
    let initial_debt_usdc = state.borrow_amount_for_token(2, USDC_TOKEN);
    println!("initial_debt_usd: {:?}", initial_debt_usdc);
    // Liquidate most of the collateral, leaving only dust
    state.liquidate_account(
        &liquidator,
        &USDC_TOKEN,
        BigUint::from(730u64), // Liquidate with large amount to consume most collateral
        2,
        USDC_DECIMALS,
    );
    let left_collateral_egld = state.collateral_amount_for_token(2, EGLD_TOKEN);
    println!("left_collateral_egld: {:?}", left_collateral_egld);
    // At this point:
    // - Significant bad debt remains due to massive interest
    // - Very little collateral remains (dust under $5)
    // - Conditions for cleanBadDebt are met

    let left_bad_debt_usdc = state.borrow_amount_for_token(2, USDC_TOKEN);
    println!("left_bad_debt_usdc: {:?}", left_bad_debt_usdc);

    assert!(state.total_borrow_in_egld(2) > state.total_collateral_in_egld(2));
    let initial_egld_revenue = state.market_revenue(egld_pool_address.clone());
    let before_clean_market_indexes =
        state.all_market_indexes(MultiValueEncoded::from_iter(vec![
            EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN),
        ]));
    println!(
        "usdc_index: {:?}",
        before_clean_market_indexes.get(0).supply_index_ray
    );
    // state.claim_revenue(USDC_TOKEN);
    let usdc_supplied_before_bad_debt = state.collateral_amount_for_token(1, USDC_TOKEN);
    println!(
        "usdc_supplied_before_bad_debt: {:?}",
        usdc_supplied_before_bad_debt
    );
    // Clean bad debt - this calls seizeDustCollateral internally
    state.clean_bad_debt(2);

    // Verify all positions cleared
    let final_debt = state.total_borrow_in_egld(2);
    let final_collateral = state.total_collateral_in_egld(2);

    assert!(final_debt == ManagedDecimal::from_raw_units(BigUint::from(0u64), WAD_PRECISION));
    assert!(final_collateral == ManagedDecimal::from_raw_units(BigUint::from(0u64), WAD_PRECISION));

    println!("initial_egld_revenue: {:?}", initial_egld_revenue);
    // Verify protocol revenue increased from dust seizure
    let final_egld_revenue = state.market_revenue(egld_pool_address);
    println!("final_egld_revenue:   {:?}", final_egld_revenue);
    assert!(
        final_egld_revenue > initial_egld_revenue,
        "Protocol revenue should increase or stay same when dust collateral is seized"
    );
    // Revenue should be the initial revenue + the collateral left before bad debt socialization
    assert!(final_egld_revenue == initial_egld_revenue + left_collateral_egld);

    let after_clean_market_indexes =
        state.all_market_indexes(MultiValueEncoded::from_iter(vec![
            EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN),
        ]));
    let re_paid_usdc_debt = initial_debt_usdc - left_bad_debt_usdc;
    println!("re_paid_usdc_debt: {:?}", re_paid_usdc_debt);
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
    println!(
        "usdc_supplied_after_bad_debt: {:?}",
        usdc_supplied_after_bad_debt
    );
    let lost_usdc_due_to_socialization =
        usdc_supplied_before_bad_debt.clone() - usdc_supplied_after_bad_debt.clone();
    println!(
        "lost_usdc_due_to_socialization: {:?}",
        lost_usdc_due_to_socialization
    );
    assert!(lost_usdc_due_to_socialization.into_raw_units().clone() > BigUint::from(0u64));
    let supplied_usdc = state.market_supplied_amount(state.usdc_market.clone());
    let market_reserves = state.market_reserves(state.usdc_market.clone());
    let protoocl_revenue = state.market_protocol_revenue(state.usdc_market.clone());
    println!("total supplied_usdc: {:?}", supplied_usdc);
    println!("market_reserves: {:?}", market_reserves);
    println!("protoocl_revenue: {:?}", protoocl_revenue);
    state.withdraw_asset_den(
        &supplier,
        USDC_TOKEN,
        usdc_supplied_after_bad_debt.into_raw_units().clone(),
        1,
    );
    let supplied_usdc = state.market_supplied_amount(state.usdc_market.clone());
    let market_reserves = state.market_reserves(state.usdc_market.clone());
    let protoocl_revenue = state.market_protocol_revenue(state.usdc_market.clone());
    println!("total supplied_usdc: {:?}", supplied_usdc);
    println!("market_reserves: {:?}", market_reserves);
    println!("protoocl_revenue: {:?}", protoocl_revenue);
    state.claim_revenue(USDC_TOKEN);
    let final_protoocl_revenue = state.market_protocol_revenue(state.usdc_market.clone());
    println!("final_protoocl_revenue: {:?}", final_protoocl_revenue);
    assert!(final_protoocl_revenue.into_raw_units().clone() == BigUint::zero());
    let scaled_borrowed = state.market_borrowed(state.usdc_market.clone());
    println!("scaled_borrowed: {:?}", scaled_borrowed);
    assert!(scaled_borrowed.into_raw_units().clone() == BigUint::zero());
    let scaled_supplied = state.market_supplied(state.usdc_market.clone());
    println!("scaled_supplied: {:?}", scaled_supplied);
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
    println!("health_factor: {:?}", health_factor);
    let debt_usdc = state.borrow_amount_for_token(2, USDC_TOKEN);
    println!("debt_usd: {:?}", debt_usdc);
    let collateral_egld = state.collateral_amount_for_token(2, EGLD_TOKEN);
    println!("collateral_egld: {:?}", collateral_egld);

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

    println!("initial_egld_revenue: {:?}", initial_egld_revenue);
    // Verify protocol revenue increased from dust seizure
    let final_egld_revenue = state.market_revenue(egld_pool_address);
    println!("final_egld_revenue:   {:?}", final_egld_revenue);
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
    println!("health_factor: {:?}", health_factor);
    let supplied_liquidation = state.liquidation_collateral_available(2);
    println!("supplied_liquidation: {:?}", supplied_liquidation);
    let supplied = state.total_collateral_in_egld(2);
    println!("supplied: {:?}", supplied);
    let borrowed = state.total_borrow_in_egld(2);
    println!("borrowed: {:?}", borrowed);
    let estimated_liquidation_amount = state.liquidation_estimations(2, ManagedVec::new());
    println!("estimated_bonus_rate: {:?}", estimated_liquidation_amount.bonus_rate_bps * ManagedDecimal::from_raw_units(BigUint::from(100u64), 0));
    println!("estimated_bonus_amount: {:?}", estimated_liquidation_amount.max_egld_payment_wad);
    for token in estimated_liquidation_amount.seized_collaterals {
        println!("Seized collateral: {:?}", token.token_identifier);
        println!("Seized amount: {:?}", token.amount);
    }

    for token in estimated_liquidation_amount.refunds {
        println!("Refund: {:?}", token.token_identifier);
        println!("Refund amount: {:?}", token.amount);
    }

    for token in estimated_liquidation_amount.protocol_fees {
        println!("Protocol fee: {:?}", token.token_identifier);
        println!("Protocol fee amount: {:?}", token.amount);
    }

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
    println!("before_health: {:?}", before_health);
    // Liquidate EGLD debt first
    state.liquidate_account_dem(
        &liquidator,
        &EGLD_TOKEN,
        borrowed_egld.into_raw_units().clone(),
        2,
    );
    let borrowed_egld = state.total_borrow_in_egld(2);
    println!("borrowed_egld after liquidation: {:?}", borrowed_egld);
    let supplied = state.total_collateral_in_egld(2);
    println!("supplied after liquidation:      {:?}", supplied);
    let after_health = state.account_health_factor(2);
    println!("after_health: {:?}", after_health);
    assert!(after_health < before_health);

    state.clean_bad_debt(2);
    let final_debt = state.total_borrow_in_egld(2);
    println!("final_debt: {:?}", final_debt);
    assert!(final_debt == ManagedDecimal::from_raw_units(BigUint::from(0u64), WAD_PRECISION));
    let final_collateral = state.total_collateral_in_egld(2);
    println!("final_collateral: {:?}", final_collateral);
    assert!(final_collateral == ManagedDecimal::from_raw_units(BigUint::from(0u64), WAD_PRECISION));
}
