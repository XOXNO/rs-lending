use controller::{
    ERROR_ASSET_NOT_SUPPORTED_AS_COLLATERAL, ERROR_DEBT_CEILING_REACHED,
    ERROR_EMODE_CATEGORY_NOT_FOUND, ERROR_MIX_ISOLATED_COLLATERAL, WAD_PRECISION,
};
use multiversx_sc::types::{EgldOrEsdtTokenIdentifier, ManagedDecimal, MultiValueEncoded};
use multiversx_sc_scenario::imports::{BigUint, OptionalValue, TestAddress};
pub mod constants;
pub mod proxys;
pub mod setup;
use constants::*;
use setup::*;

/// Tests that isolated assets cannot be used with E-Mode.
/// 
/// Covers:
/// - Controller::supply validation for isolated assets
/// - E-Mode incompatibility with isolated assets
/// - ERROR_EMODE_CATEGORY_NOT_FOUND error condition
#[test]
fn isolated_supply_with_emode_incompatible_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_accounts(&mut state, supplier, borrower);
    
    // First supply a regular asset with E-Mode
    state.supply_asset(
        &supplier,
        XEGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::Some(1),
        false,
    );

    // Attempt to supply isolated asset with E-Mode (should fail)
    state.supply_asset_error(
        &supplier,
        ISOLATED_TOKEN,
        BigUint::from(100u64),
        ISOLATED_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::None,
        false,
        ERROR_EMODE_CATEGORY_NOT_FOUND,
    );
}

/// Tests that isolated collateral cannot be mixed with other collateral types.
/// 
/// Covers:
/// - Controller::supply validation for collateral mixing
/// - Isolated asset exclusivity requirement
/// - ERROR_MIX_ISOLATED_COLLATERAL error condition
#[test]
fn isolated_mix_with_regular_collateral_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_accounts(&mut state, supplier, borrower);
    
    // First supply isolated asset
    state.supply_asset(
        &supplier,
        ISOLATED_TOKEN,
        BigUint::from(100u64),
        ISOLATED_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Attempt to supply non-isolated asset (should fail)
    state.supply_asset_error(
        &supplier,
        XEGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::None,
        false,
        ERROR_MIX_ISOLATED_COLLATERAL,
    );
}

/// Tests borrowing against isolated collateral with debt ceiling tracking.
/// 
/// Covers:
/// - Controller::borrow with isolated collateral
/// - Debt ceiling tracking for isolated assets
/// - Controller::repay updating debt ceiling
/// - Full repayment clearing debt ceiling usage
#[test]
fn isolated_borrow_within_debt_ceiling_success() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_accounts(&mut state, supplier, borrower);

    // Supply liquidity for borrowing
    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(1000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Borrower supplies isolated asset as collateral
    state.supply_asset(
        &borrower,
        ISOLATED_TOKEN,
        BigUint::from(100u64), // $500 collateral value
        ISOLATED_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Borrow against isolated collateral
    state.borrow_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(100u64), // $100 borrow
        2,
        USDC_DECIMALS,
    );
    
    // Verify debt ceiling usage is tracked
    let debt_usage = state.get_used_isolated_asset_debt_usd(&ISOLATED_TOKEN);
    assert!(debt_usage > ManagedDecimal::from_raw_units(BigUint::zero(), ISOLATED_DECIMALS));

    // Repay more than owed (overpayment)
    state.repay_asset(
        &borrower,
        &USDC_TOKEN,
        BigUint::from(1000u64), // $1000 repayment
        2,
        USDC_DECIMALS,
    );
    
    // Verify debt ceiling usage is cleared
    let final_debt_usage = state.get_used_isolated_asset_debt_usd(&ISOLATED_TOKEN);
    assert!(final_debt_usage == ManagedDecimal::from_raw_units(BigUint::zero(), ISOLATED_DECIMALS));
}

/// Tests borrowing against isolated collateral hitting debt ceiling limit.
/// 
/// Covers:
/// - Controller::borrow debt ceiling validation
/// - Isolated asset debt ceiling enforcement
/// - ERROR_DEBT_CEILING_REACHED error condition
#[test]
fn isolated_borrow_exceeds_debt_ceiling_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_accounts(&mut state, supplier, borrower);

    // Supply liquidity for borrowing
    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(1005u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Borrower supplies large isolated asset position
    state.supply_asset(
        &borrower,
        ISOLATED_TOKEN,
        BigUint::from(1000u64), // $5000 collateral value
        ISOLATED_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Attempt to borrow beyond debt ceiling ($1000 limit)
    state.borrow_asset_error(
        &borrower,
        USDC_TOKEN,
        BigUint::from(1001u64), // $1001 borrow exceeds ceiling
        2,
        USDC_DECIMALS,
        ERROR_DEBT_CEILING_REACHED,
    );
}

/// Tests debt ceiling tracking with interest accrual on isolated collateral.
/// 
/// Covers:
/// - Controller::updateIndexes impact on isolated debt tracking
/// - Interest accrual vs principal tracking for debt ceiling
/// - Partial repayment with interest consideration
#[test]
fn isolated_debt_ceiling_with_interest_accrual() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_accounts(&mut state, supplier, borrower);

    // Supply liquidity
    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(1000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Supply isolated collateral
    state.supply_asset(
        &borrower,
        ISOLATED_TOKEN,
        BigUint::from(100u64), // $500 collateral
        ISOLATED_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Borrow against isolated collateral
    state.borrow_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(100u64), // $100 borrow
        2,
        USDC_DECIMALS,
    );

    // Verify initial debt ceiling usage
    let initial_debt_usage = state.get_used_isolated_asset_debt_usd(&ISOLATED_TOKEN);
    assert!(initial_debt_usage > ManagedDecimal::from_raw_units(BigUint::zero(), ISOLATED_DECIMALS));
    
    // Advance time for interest accrual
    state.change_timestamp(SECONDS_PER_DAY);

    // Update market indexes
    let mut markets = MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN));
    state.update_markets(&borrower, markets.clone());
    
    // Partial repayment (less than total with interest)
    state.repay_asset(
        &borrower,
        &USDC_TOKEN,
        BigUint::from(90u64), // $90 partial repayment
        2,
        USDC_DECIMALS,
    );
    
    // Verify debt ceiling usage still exists (interest not fully covered)
    let final_debt_usage = state.get_used_isolated_asset_debt_usd(&ISOLATED_TOKEN);
    assert!(final_debt_usage > ManagedDecimal::from_raw_units(BigUint::zero(), WAD_PRECISION));
}

/// Tests liquidation impact on isolated asset debt ceiling.
/// 
/// Covers:
/// - Controller::liquidate with isolated collateral
/// - Debt ceiling reduction through liquidation
/// - Controller::cleanBadDebt clearing debt ceiling
/// - Bad debt scenario with isolated assets
#[test]
fn isolated_liquidation_reduces_debt_ceiling() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");
    let liquidator = TestAddress::new("liquidator");
    
    setup_accounts(&mut state, supplier, borrower);
    state.world.account(liquidator).nonce(1).esdt_balance(
        USDC_TOKEN,
        BigUint::from(20000u64) * BigUint::from(10u64).pow(USDC_DECIMALS as u32),
    );

    // Supply liquidity
    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(1000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Supply isolated collateral
    state.supply_asset(
        &borrower,
        ISOLATED_TOKEN,
        BigUint::from(200u64), // $1000 collateral
        ISOLATED_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Borrow close to liquidation threshold
    state.borrow_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(700u64), // $700 borrow
        2,
        USDC_DECIMALS,
    );
    
    // Record initial debt ceiling usage
    let initial_debt_usage = state.get_used_isolated_asset_debt_usd(&ISOLATED_TOKEN);
    assert!(initial_debt_usage > ManagedDecimal::from_raw_units(BigUint::zero(), ISOLATED_DECIMALS));
    
    // Advance time significantly to make position unhealthy
    state.change_timestamp(SECONDS_PER_DAY * 1600);
    let mut markets = MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN));
    state.update_markets(&borrower, markets.clone());
    
    // Liquidate position
    state.liquidate_account(
        &liquidator,
        &USDC_TOKEN,
        BigUint::from(2300u64),
        2,
        USDC_DECIMALS,
    );
    
    // Verify debt ceiling reduced after liquidation
    let post_liquidation_usage = state.get_used_isolated_asset_debt_usd(&ISOLATED_TOKEN);
    assert!(post_liquidation_usage < initial_debt_usage);
    
    // Clean remaining bad debt
    state.clean_bad_debt(2);
    
    // Verify debt ceiling fully cleared
    let final_debt_usage = state.get_used_isolated_asset_debt_usd(&ISOLATED_TOKEN);
    assert_eq!(
        final_debt_usage,
        ManagedDecimal::from_raw_units(BigUint::zero(), EGLD_DECIMALS)
    );
}

/// Tests supply attempt with non-collateralizable asset in E-Mode.
/// 
/// Covers:
/// - Controller::supply validation for asset support
/// - E-Mode respecting collateralization settings
/// - ERROR_ASSET_NOT_SUPPORTED_AS_COLLATERAL error condition
#[test]
fn isolated_non_collateralizable_asset_with_emode_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_accounts(&mut state, supplier, borrower);

    // Attempt to supply non-collateralizable asset with E-Mode
    state.supply_asset_error(
        &borrower,
        SEGLD_TOKEN,
        BigUint::from(100u64),
        SEGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::Some(1),
        false,
        ERROR_ASSET_NOT_SUPPORTED_AS_COLLATERAL,
    );
}
