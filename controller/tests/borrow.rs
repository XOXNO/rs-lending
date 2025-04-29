use controller::ERROR_BORROW_CAP;
use multiversx_sc::types::{
    EgldOrEsdtTokenIdentifier, EgldOrEsdtTokenPayment, ManagedDecimal, MultiValueEncoded,
};
use multiversx_sc_scenario::{
    api::StaticApi,
    imports::{BigUint, OptionalValue, TestAddress},
};
pub mod constants;
pub mod proxys;
pub mod setup;
use constants::*;

use setup::*;
#[test]
fn test_basic_supply_and_borrow() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    // Setup accounts
    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Test supply
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Test borrow
    state.supply_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(5000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(50u64),
        2,
        EGLD_DECIMALS,
    );

    // Verify amounts
    let borrowed = state.get_borrow_amount_for_token(2, EGLD_TOKEN);
    let collateral = state.get_collateral_amount_for_token(2, USDC_TOKEN);

    assert!(borrowed > ManagedDecimal::from_raw_units(BigUint::zero(), EGLD_DECIMALS));
    assert!(collateral > ManagedDecimal::from_raw_units(BigUint::zero(), USDC_DECIMALS));
}

#[test]
fn test_basic_borrow_capped_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    // Setup accounts
    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);
    // Test supply
    state.supply_asset(
        &supplier,
        CAPPED_TOKEN,
        BigUint::from(150u64),
        CAPPED_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    // Test Borrow
    state.borrow_asset(
        &supplier,
        CAPPED_TOKEN,
        BigUint::from(1u64),
        1,
        CAPPED_DECIMALS,
    );
    // Test Borrow
    state.borrow_asset_error(
        &supplier,
        CAPPED_TOKEN,
        BigUint::from(100u64),
        1,
        CAPPED_DECIMALS,
        ERROR_BORROW_CAP,
    );
}

#[test]
fn test_bulk_borrow_all_new_positions() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    // Setup accounts
    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Test supply
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
        &borrower,
        EGLD_TOKEN,
        BigUint::from(1000u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        true,
    );

    let mut assets: MultiValueEncoded<StaticApi, EgldOrEsdtTokenPayment<StaticApi>> =
        MultiValueEncoded::new();

    let egld = EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        0,
        BigUint::from(50u64) * BigUint::from(10u64.pow(EGLD_DECIMALS as u32)),
    );
    assets.push(egld.clone());
    let usdc = EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN.to_token_identifier()),
        0,
        BigUint::from(500u64) * BigUint::from(10u64.pow(USDC_DECIMALS as u32)),
    );
    assets.push(usdc.clone());
    state.borrow_assets(2, &borrower, assets);

    let total_usdc_borrowed = state.get_borrow_amount_for_token(2, USDC_TOKEN);
    let total_egld_borrowed = state.get_borrow_amount_for_token(2, EGLD_TOKEN);
    assert_eq!(total_usdc_borrowed.into_raw_units().clone(), usdc.amount);
    assert_eq!(total_egld_borrowed.into_raw_units().clone(), egld.amount);
}

#[test]
fn test_bulk_borrow_existing_positions() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    // Setup accounts
    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Test supply
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
        &borrower,
        EGLD_TOKEN,
        BigUint::from(1000u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        true,
    );

    state.borrow_asset(&borrower, USDC_TOKEN, BigUint::from(1u64), 2, USDC_DECIMALS);
    state.borrow_asset(&borrower, EGLD_TOKEN, BigUint::from(1u64), 2, EGLD_DECIMALS);

    let mut assets: MultiValueEncoded<StaticApi, EgldOrEsdtTokenPayment<StaticApi>> =
        MultiValueEncoded::new();

    let egld = EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        0,
        BigUint::from(50u64) * BigUint::from(10u64.pow(EGLD_DECIMALS as u32)),
    );
    assets.push(egld.clone());
    let usdc = EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN.to_token_identifier()),
        0,
        BigUint::from(500u64) * BigUint::from(10u64.pow(USDC_DECIMALS as u32)),
    );
    assets.push(usdc.clone());
    state.borrow_assets(2, &borrower, assets);

    let total_usdc_borrowed = state.get_borrow_amount_for_token(2, USDC_TOKEN);
    let total_egld_borrowed = state.get_borrow_amount_for_token(2, EGLD_TOKEN);
    assert!(total_usdc_borrowed.into_raw_units().clone() > usdc.amount);
    assert!(total_egld_borrowed.into_raw_units().clone() > egld.amount);
}
