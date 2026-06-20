use controller::ERROR_ASSET_NOT_BORROWABLE_IN_SILOED;
use multiversx_sc::types::ManagedDecimal;
use multiversx_sc_scenario::imports::{BigUint, OptionalValue, TestAddress};
pub mod constants;
pub mod proxys;
pub mod setup;
use constants::*;
use setup::*;

/// Tests successful borrowing of siloed asset as only debt position.
///
/// Covers:
/// - Controller::borrow with siloed asset
/// - Siloed asset allowed as sole borrowing position
/// - Normal borrowing flow for siloed assets
#[test]
fn siloed_borrow_as_only_debt_success() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_accounts(&mut state, supplier, borrower);

    // Borrower supplies regular collateral
    state.supply_asset(
        &borrower,
        SupplyParams {
            token_id: EGLD_TOKEN,
            amount: BigUint::from(100u64),
            asset_decimals: EGLD_DECIMALS,
            account_nonce: OptionalValue::None,
            e_mode_category: OptionalValue::None,
        },
    );

    // Supplier provides siloed token liquidity
    state.supply_asset(
        &supplier,
        SupplyParams {
            token_id: SILOED_TOKEN,
            amount: BigUint::from(100u64),
            asset_decimals: SILOED_DECIMALS,
            account_nonce: OptionalValue::None,
            e_mode_category: OptionalValue::None,
        },
    );

    // Borrower takes siloed asset loan (allowed as only debt)
    state.borrow_asset(
        &borrower,
        SILOED_TOKEN,
        BigUint::from(1u64),
        1,
        SILOED_DECIMALS,
    );

    // Verify siloed debt position exists
    let borrow_amount = state.borrow_amount_for_token(1, SILOED_TOKEN);
    assert!(borrow_amount > ManagedDecimal::from_raw_units(BigUint::zero(), SILOED_DECIMALS));
}

/// Tests that siloed asset cannot be borrowed when other debts exist.
///
/// Covers:
/// - Controller::borrow siloed asset validation
/// - Existing debt check for siloed borrowing
/// - ERROR_ASSET_NOT_BORROWABLE_IN_SILOED error condition
/// - Multiple debt position scenarios
#[test]
fn siloed_borrow_with_existing_debts_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_accounts(&mut state, supplier, borrower);

    // Borrower supplies collateral
    state.supply_asset(
        &borrower,
        SupplyParams {
            token_id: EGLD_TOKEN,
            amount: BigUint::from(100u64),
            asset_decimals: EGLD_DECIMALS,
            account_nonce: OptionalValue::None,
            e_mode_category: OptionalValue::None,
        },
    );

    // Supplier provides liquidity for multiple assets
    state.supply_asset(
        &supplier,
        SupplyParams {
            token_id: SILOED_TOKEN,
            amount: BigUint::from(100u64),
            asset_decimals: SILOED_DECIMALS,
            account_nonce: OptionalValue::None,
            e_mode_category: OptionalValue::None,
        },
    );

    state.supply_asset(
        &supplier,
        SupplyParams {
            token_id: USDC_TOKEN,
            amount: BigUint::from(100u64),
            asset_decimals: USDC_DECIMALS,
            account_nonce: OptionalValue::Some(2),
            e_mode_category: OptionalValue::None,
        },
    );

    state.supply_asset(
        &supplier,
        SupplyParams {
            token_id: USDC_TOKEN,
            amount: BigUint::from(100u64),
            asset_decimals: USDC_DECIMALS,
            account_nonce: OptionalValue::None,
            e_mode_category: OptionalValue::None,
        },
    );

    // Borrower takes regular loan first
    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(10u64),
        1,
        EGLD_DECIMALS,
    );

    // Attempt to borrow siloed asset (should fail - already has debt)
    state.borrow_asset_error(
        &borrower,
        SILOED_TOKEN,
        BigUint::from(1u64),
        1,
        SILOED_DECIMALS,
        ERROR_ASSET_NOT_BORROWABLE_IN_SILOED,
    );

    // Add another debt position
    state.borrow_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(10u64),
        1,
        USDC_DECIMALS,
    );

    // Verify siloed borrowing still blocked with multiple debts
    state.borrow_asset_error(
        &borrower,
        SILOED_TOKEN,
        BigUint::from(1u64),
        1,
        SILOED_DECIMALS,
        ERROR_ASSET_NOT_BORROWABLE_IN_SILOED,
    );
}

/// Tests that regular assets cannot be borrowed after siloed debt.
///
/// Covers:
/// - Controller::borrow validation with existing siloed debt
/// - Siloed debt exclusivity enforcement
/// - ERROR_ASSET_NOT_BORROWABLE_IN_SILOED for regular assets
/// - Reverse scenario of siloed borrowing restrictions
#[test]
fn siloed_prevents_additional_borrows_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_accounts(&mut state, supplier, borrower);

    // Borrower supplies significant collateral
    state.supply_asset(
        &borrower,
        SupplyParams {
            token_id: USDC_TOKEN,
            amount: BigUint::from(1000u64),
            asset_decimals: USDC_DECIMALS,
            account_nonce: OptionalValue::None,
            e_mode_category: OptionalValue::None,
        },
    );

    // Supplier provides liquidity for multiple assets
    state.supply_asset(
        &supplier,
        SupplyParams {
            token_id: EGLD_TOKEN,
            amount: BigUint::from(100u64),
            asset_decimals: EGLD_DECIMALS,
            account_nonce: OptionalValue::None,
            e_mode_category: OptionalValue::None,
        },
    );

    state.supply_asset(
        &supplier,
        SupplyParams {
            token_id: SILOED_TOKEN,
            amount: BigUint::from(1000u64),
            asset_decimals: SILOED_DECIMALS,
            account_nonce: OptionalValue::Some(2),
            e_mode_category: OptionalValue::None,
        },
    );

    // Borrower takes siloed loan first
    state.borrow_asset(
        &borrower,
        SILOED_TOKEN,
        BigUint::from(100u64),
        1,
        SILOED_DECIMALS,
    );

    // Attempt to borrow regular asset (should fail - has siloed debt)
    state.borrow_asset_error(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(1u64),
        1,
        EGLD_DECIMALS,
        ERROR_ASSET_NOT_BORROWABLE_IN_SILOED,
    );
}

// Siloed Tests End
