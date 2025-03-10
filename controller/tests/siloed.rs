use controller::ERROR_ASSET_NOT_BORROWABLE_IN_SILOED;
use multiversx_sc::types::ManagedDecimal;
use multiversx_sc_scenario::imports::{BigUint, OptionalValue, TestAddress};
pub mod constants;
pub mod proxys;
pub mod setup;
use constants::*;
use setup::*;

#[test]
fn test_borrow_asset_as_siloed_normal_case() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_accounts(&mut state, supplier, borrower);

    // First supply a normal asset not siloed
    state.supply_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.supply_asset(
        &supplier,
        SILOED_TOKEN,
        BigUint::from(100u64),
        SILOED_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Then borrow the siloed asset
    state.borrow_asset(
        &borrower,
        SILOED_TOKEN,
        BigUint::from(1u64),
        1,
        SILOED_DECIMALS,
    );

    let borrow_amount = state.get_borrow_amount_for_token(1, SILOED_TOKEN);
    assert!(borrow_amount > ManagedDecimal::from_raw_units(BigUint::zero(), SILOED_DECIMALS));
}

#[test]
fn test_borrow_asset_as_siloed_with_another_asset_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_accounts(&mut state, supplier, borrower);

    // Test borrow
    state.supply_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.supply_asset(
        &supplier,
        SILOED_TOKEN,
        BigUint::from(100u64),
        SILOED_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(100u64),
        USDC_DECIMALS,
        OptionalValue::Some(2),
        OptionalValue::None,
        false,
    );

    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(100u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(10u64),
        1,
        EGLD_DECIMALS,
    );

    state.borrow_asset_error(
        &borrower,
        SILOED_TOKEN,
        BigUint::from(1u64),
        1,
        SILOED_DECIMALS,
        ERROR_ASSET_NOT_BORROWABLE_IN_SILOED,
    );

    // Cover the error when there are more assets borrowed and early throw
    state.borrow_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(10u64),
        1,
        USDC_DECIMALS,
    );

    state.borrow_asset_error(
        &borrower,
        SILOED_TOKEN,
        BigUint::from(1u64),
        1,
        SILOED_DECIMALS,
        ERROR_ASSET_NOT_BORROWABLE_IN_SILOED,
    );
}

#[test]
fn test_borrow_asset_then_borrow_siloed_asset_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_accounts(&mut state, supplier, borrower);

    state.supply_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(1000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Test borrow
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
        SILOED_TOKEN,
        BigUint::from(1000u64),
        SILOED_DECIMALS,
        OptionalValue::Some(2),
        OptionalValue::None,
        false,
    );

    state.borrow_asset(
        &borrower,
        SILOED_TOKEN,
        BigUint::from(100u64),
        1,
        SILOED_DECIMALS,
    );

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
