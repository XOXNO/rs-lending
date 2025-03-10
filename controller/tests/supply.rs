use controller::{
    ERROR_BULK_SUPPLY_NOT_SUPPORTED, ERROR_INVALID_NUMBER_OF_ESDT_TRANSFERS,
    ERROR_MIX_ISOLATED_COLLATERAL, ERROR_SUPPLY_CAP,
};
use multiversx_sc::types::{EsdtTokenPayment, ManagedVec};
use multiversx_sc_scenario::{
    api::StaticApi,
    imports::{BigUint, OptionalValue, TestAddress},
};
pub mod constants;
pub mod proxys;
pub mod setup;
use constants::*;
use setup::*;
use std::ops::Mul;

#[test]
fn test_basic_supply_capped_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    // Setup accounts
    state.world.current_block().block_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);
    // Test supply
    state.supply_asset(
        &supplier,
        CAPPED_TOKEN,
        BigUint::from(1u64),
        CAPPED_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    // Test supply
    state.supply_asset_error(
        &supplier,
        CAPPED_TOKEN,
        BigUint::from(150u64),
        CAPPED_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::None,
        false,
        ERROR_SUPPLY_CAP,
    );
}

#[test]
fn test_empty_supply_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    // Setup accounts
    state.world.current_block().block_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);
    // Test supply
    state.empty_supply_asset_error(
        &supplier,
        OptionalValue::None,
        false,
        ERROR_INVALID_NUMBER_OF_ESDT_TRANSFERS,
    );
}

#[test]
fn test_basic_supply_no_assets_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    // Setup accounts
    state.world.current_block().block_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);
    // Test supply
    state.supply_asset(
        &supplier,
        CAPPED_TOKEN,
        BigUint::from(1u64),
        CAPPED_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    // Test supply
    state.supply_empty_asset_error(
        &supplier,
        OptionalValue::Some(1),
        OptionalValue::None,
        false,
        ERROR_INVALID_NUMBER_OF_ESDT_TRANSFERS,
    );
}

#[test]
fn test_bulk_supply_with_isolated_first_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    let mut vec = ManagedVec::<StaticApi, EsdtTokenPayment<StaticApi>>::new();
    vec.push(EsdtTokenPayment::new(
        ISOLATED_TOKEN.to_token_identifier(),
        0,
        BigUint::from(10u64).mul(BigUint::from(10u64).pow(ISOLATED_DECIMALS as u32)),
    ));
    vec.push(EsdtTokenPayment::new(
        EGLD_TOKEN.to_token_identifier(),
        0,
        BigUint::from(10u64).mul(BigUint::from(10u64).pow(EGLD_DECIMALS as u32)),
    ));
    setup_accounts(&mut state, supplier, borrower);
    // Test supply
    state.supply_bulk_error(
        &supplier,
        OptionalValue::None,
        OptionalValue::None,
        false,
        vec,
        ERROR_BULK_SUPPLY_NOT_SUPPORTED,
    );
}

#[test]
fn test_bulk_supply_with_isolated_last_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    let mut vec = ManagedVec::<StaticApi, EsdtTokenPayment<StaticApi>>::new();
    vec.push(EsdtTokenPayment::new(
        EGLD_TOKEN.to_token_identifier(),
        0,
        BigUint::from(10u64).mul(BigUint::from(10u64).pow(EGLD_DECIMALS as u32)),
    ));
    vec.push(EsdtTokenPayment::new(
        ISOLATED_TOKEN.to_token_identifier(),
        0,
        BigUint::from(10u64).mul(BigUint::from(10u64).pow(ISOLATED_DECIMALS as u32)),
    ));
    setup_accounts(&mut state, supplier, borrower);
    // Test supply
    state.supply_bulk_error(
        &supplier,
        OptionalValue::None,
        OptionalValue::None,
        false,
        vec,
        ERROR_MIX_ISOLATED_COLLATERAL,
    );
}

#[test]
fn test_bulk_supply_capped_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    let mut vec = ManagedVec::<StaticApi, EsdtTokenPayment<StaticApi>>::new();
    vec.push(EsdtTokenPayment::new(
        CAPPED_TOKEN.to_token_identifier(),
        0,
        BigUint::from(100u64).mul(BigUint::from(10u64).pow(CAPPED_DECIMALS as u32)),
    ));
    vec.push(EsdtTokenPayment::new(
        CAPPED_TOKEN.to_token_identifier(),
        0,
        BigUint::from(51u64).mul(BigUint::from(10u64).pow(CAPPED_DECIMALS as u32)),
    ));
    setup_accounts(&mut state, supplier, borrower);
    // Test supply
    state.supply_bulk_error(
        &supplier,
        OptionalValue::None,
        OptionalValue::None,
        false,
        vec,
        ERROR_SUPPLY_CAP,
    );
}
