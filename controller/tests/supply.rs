use controller::{
    ERROR_ACCOUNT_NOT_IN_THE_MARKET, ERROR_BULK_SUPPLY_NOT_SUPPORTED, ERROR_INVALID_NUMBER_OF_ESDT_TRANSFERS, ERROR_MIX_ISOLATED_COLLATERAL, ERROR_SUPPLY_CAP
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

/// Tests that supplying with an inactive account nonce fails.
/// 
/// Covers:
/// - Controller::supply endpoint error path
/// - Account validation in positions::account::PositionAccountModule
/// - ERROR_ACCOUNT_NOT_IN_THE_MARKET error condition
#[test]
fn supply_with_inactive_account_nonce_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);
    
    // Attempt to supply with non-existent account nonce
    state.supply_asset_error(
        &supplier,
        CAPPED_TOKEN,
        BigUint::from(150u64),
        CAPPED_DECIMALS,
        OptionalValue::Some(1), // Non-existent account nonce
        OptionalValue::None,
        false, // is_vault = false
        ERROR_ACCOUNT_NOT_IN_THE_MARKET,
    );
}

/// Tests that supplying beyond the supply cap for an asset fails.
/// 
/// Covers:
/// - Controller::supply endpoint error path
/// - Supply cap validation in positions::supply::PositionDepositModule
/// - ERROR_SUPPLY_CAP error condition
#[test]
fn supply_exceeds_cap_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);
    
    // First supply within cap succeeds
    state.supply_asset(
        &supplier,
        CAPPED_TOKEN,
        BigUint::from(1u64),
        CAPPED_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false, // is_vault = false
    );
    
    // Second supply exceeds cap and fails
    state.supply_asset_error(
        &supplier,
        CAPPED_TOKEN,
        BigUint::from(150u64),
        CAPPED_DECIMALS,
        OptionalValue::Some(1), // Existing account
        OptionalValue::None,
        false, // is_vault = false
        ERROR_SUPPLY_CAP,
    );
}

/// Tests that calling supply endpoint without any payments fails.
/// 
/// Covers:
/// - Controller::supply endpoint validation
/// - Payment validation in validation::ValidationModule
/// - ERROR_INVALID_NUMBER_OF_ESDT_TRANSFERS error condition
#[test]
fn supply_without_payments_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);
    
    // Call supply without any ESDT transfers
    state.empty_supply_asset_error(
        &supplier,
        OptionalValue::None,
        false, // is_vault = false
        ERROR_INVALID_NUMBER_OF_ESDT_TRANSFERS,
    );
}

/// Tests that supplying with only account NFT but no assets fails.
/// 
/// Covers:
/// - Controller::supply endpoint validation
/// - Collateral validation in validate_supply_payment
/// - ERROR_INVALID_NUMBER_OF_ESDT_TRANSFERS error condition
#[test]
fn supply_account_nft_only_no_assets_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);
    
    // Create initial position
    state.supply_asset(
        &supplier,
        CAPPED_TOKEN,
        BigUint::from(1u64),
        CAPPED_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false, // is_vault = false
    );
    
    // Try to supply with only account NFT, no collateral
    state.supply_empty_asset_error(
        &supplier,
        OptionalValue::Some(1), // Account NFT
        OptionalValue::None,
        false, // is_vault = false
        ERROR_INVALID_NUMBER_OF_ESDT_TRANSFERS,
    );
}

/// Tests that bulk supply with isolated asset as first token fails.
/// 
/// Covers:
/// - Controller::supply endpoint with isolated assets
/// - Isolated asset validation in supply flow
/// - ERROR_BULK_SUPPLY_NOT_SUPPORTED error condition
#[test]
fn supply_bulk_isolated_asset_first_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    // Prepare bulk supply with isolated asset first
    let mut assets = ManagedVec::<StaticApi, EsdtTokenPayment<StaticApi>>::new();
    assets.push(EsdtTokenPayment::new(
        ISOLATED_TOKEN.to_token_identifier(),
        0,
        BigUint::from(10u64).mul(BigUint::from(10u64).pow(ISOLATED_DECIMALS as u32)),
    ));
    assets.push(EsdtTokenPayment::new(
        EGLD_TOKEN.to_token_identifier(),
        0,
        BigUint::from(10u64).mul(BigUint::from(10u64).pow(EGLD_DECIMALS as u32)),
    ));
    
    setup_accounts(&mut state, supplier, borrower);
    
    // Bulk supply with isolated asset should fail
    state.supply_bulk_error(
        &supplier,
        OptionalValue::None,
        OptionalValue::None,
        false, // is_vault = false
        assets,
        ERROR_BULK_SUPPLY_NOT_SUPPORTED,
    );
}

/// Tests that mixing isolated assets with regular assets in supply fails.
/// 
/// Covers:
/// - Controller::supply endpoint validation for isolated assets
/// - Mixed collateral validation
/// - ERROR_MIX_ISOLATED_COLLATERAL error condition
#[test]
fn supply_mix_isolated_with_regular_assets_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    // Prepare bulk supply with regular asset first, then isolated
    let mut assets = ManagedVec::<StaticApi, EsdtTokenPayment<StaticApi>>::new();
    assets.push(EsdtTokenPayment::new(
        EGLD_TOKEN.to_token_identifier(),
        0,
        BigUint::from(10u64).mul(BigUint::from(10u64).pow(EGLD_DECIMALS as u32)),
    ));
    assets.push(EsdtTokenPayment::new(
        ISOLATED_TOKEN.to_token_identifier(),
        0,
        BigUint::from(10u64).mul(BigUint::from(10u64).pow(ISOLATED_DECIMALS as u32)),
    ));
    
    setup_accounts(&mut state, supplier, borrower);
    
    // Mixing isolated with regular assets should fail
    state.supply_bulk_error(
        &supplier,
        OptionalValue::None,
        OptionalValue::None,
        false, // is_vault = false
        assets,
        ERROR_MIX_ISOLATED_COLLATERAL,
    );
}

/// Tests that bulk supply exceeding cap for duplicated asset fails.
/// 
/// Covers:
/// - Controller::supply endpoint with bulk assets
/// - Supply cap validation for multiple payments of same asset
/// - ERROR_SUPPLY_CAP error condition
#[test]
fn supply_bulk_same_asset_exceeds_cap_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    // Prepare bulk supply with same capped asset twice
    let mut assets = ManagedVec::<StaticApi, EsdtTokenPayment<StaticApi>>::new();
    assets.push(EsdtTokenPayment::new(
        CAPPED_TOKEN.to_token_identifier(),
        0,
        BigUint::from(100u64).mul(BigUint::from(10u64).pow(CAPPED_DECIMALS as u32)),
    ));
    assets.push(EsdtTokenPayment::new(
        CAPPED_TOKEN.to_token_identifier(),
        0,
        BigUint::from(51u64).mul(BigUint::from(10u64).pow(CAPPED_DECIMALS as u32)),
    ));
    
    setup_accounts(&mut state, supplier, borrower);
    
    // Total supply exceeds cap and should fail
    state.supply_bulk_error(
        &supplier,
        OptionalValue::None,
        OptionalValue::None,
        false, // is_vault = false
        assets,
        ERROR_SUPPLY_CAP,
    );
}
