use controller::{
    ERROR_ASSET_NOT_BORROWABLE, ERROR_ASSET_NOT_BORROWABLE_IN_ISOLATION,
    ERROR_EMODE_CATEGORY_NOT_FOUND,
};
use multiversx_sc::types::ManagedDecimal;
use multiversx_sc_scenario::imports::{BigUint, OptionalValue, TestAddress};
pub mod constants;
pub mod proxys;
pub mod setup;
use constants::*;
use setup::*;

/// Tests basic supply and borrow operations within E-Mode category.
/// 
/// Covers:
/// - Controller::supply with E-Mode category selection
/// - Controller::borrow within same E-Mode category
/// - E-Mode allowing higher capital efficiency
/// - Successful operations with compatible E-Mode assets
#[test]
fn emode_supply_and_borrow_same_category_success() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);
    
    // Supplier provides XEGLD liquidity with E-Mode category 1
    state.supply_asset(
        &supplier,
        XEGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::Some(1), // E-Mode category 1
        false,
    );

    // Borrower supplies EGLD as collateral with E-Mode category 1
    state.supply_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::Some(1), // E-Mode category 1
        false,
    );

    // Borrower takes XEGLD loan (compatible in same E-Mode)
    state.borrow_asset(
        &borrower,
        XEGLD_TOKEN,
        BigUint::from(50u64),
        2,
        EGLD_DECIMALS,
    );

    // Verify positions exist
    let borrowed = state.get_borrow_amount_for_token(2, XEGLD_TOKEN);
    let collateral = state.get_collateral_amount_for_token(2, EGLD_TOKEN);

    assert!(borrowed > ManagedDecimal::from_raw_units(BigUint::zero(), XEGLD_DECIMALS));
    assert!(collateral > ManagedDecimal::from_raw_units(BigUint::zero(), EGLD_DECIMALS));
}

/// Tests supply attempt with invalid E-Mode category.
/// 
/// Covers:
/// - Controller::supply E-Mode validation
/// - Asset compatibility with E-Mode categories
/// - ERROR_EMODE_CATEGORY_NOT_FOUND error condition
#[test]
fn emode_supply_incompatible_asset_category_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_accounts(&mut state, supplier, borrower);
    
    // Attempt to supply USDC with E-Mode category 1 (USDC not compatible)
    state.supply_asset_error(
        &borrower,
        USDC_TOKEN,
        BigUint::from(100u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::Some(1), // E-Mode category 1 doesn't support USDC
        false,
        ERROR_EMODE_CATEGORY_NOT_FOUND,
    );
}

/// Tests borrow restriction when collateral is isolated asset.
/// 
/// Covers:
/// - Controller::borrow with isolated collateral
/// - E-Mode interaction with isolated assets
/// - ERROR_ASSET_NOT_BORROWABLE_IN_ISOLATION error condition
/// - Isolation mode borrowing restrictions
#[test]
fn emode_borrow_with_isolated_collateral_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_accounts(&mut state, supplier, borrower);

    // Borrower supplies isolated asset as collateral
    state.supply_asset(
        &borrower,
        ISOLATED_TOKEN,
        BigUint::from(1000u64),
        ISOLATED_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Attempt to borrow EGLD (not allowed with isolated collateral)
    state.borrow_asset_error(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(100u64),
        1,
        EGLD_DECIMALS,
        ERROR_ASSET_NOT_BORROWABLE_IN_ISOLATION,
    );
}

/// Tests borrow attempt for asset not supported in active E-Mode category.
/// 
/// Covers:
/// - Controller::borrow E-Mode category validation
/// - Cross-category borrowing restriction
/// - ERROR_EMODE_CATEGORY_NOT_FOUND for incompatible assets
#[test]
fn emode_borrow_asset_outside_category_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_accounts(&mut state, supplier, borrower);

    // Borrower supplies EGLD with E-Mode category 1
    state.supply_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::Some(1), // E-Mode category 1
        false,
    );

    // Attempt to borrow USDC (not in E-Mode category 1)
    state.borrow_asset_error(
        &borrower,
        USDC_TOKEN,
        BigUint::from(100u64),
        1,
        USDC_DECIMALS,
        ERROR_EMODE_CATEGORY_NOT_FOUND,
    );
}

/// Tests borrow attempt for non-borrowable asset within E-Mode.
/// 
/// Covers:
/// - Controller::borrow with non-borrowable assets
/// - E-Mode respecting asset borrowability settings
/// - ERROR_ASSET_NOT_BORROWABLE error condition
#[test]
fn emode_borrow_non_borrowable_asset_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_accounts(&mut state, supplier, borrower);

    // Borrower supplies EGLD with E-Mode category 1
    state.supply_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::Some(1), // E-Mode category 1
        false,
    );

    // Attempt to borrow LEGLD (not borrowable)
    state.borrow_asset_error(
        &borrower,
        LEGLD_TOKEN,
        BigUint::from(10u64),
        1,
        LEGLD_DECIMALS,
        ERROR_ASSET_NOT_BORROWABLE,
    );
}
