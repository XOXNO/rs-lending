use controller::{
    ERROR_ACCOUNT_NOT_IN_THE_MARKET, ERROR_BULK_SUPPLY_NOT_SUPPORTED,
    ERROR_INVALID_NUMBER_OF_ESDT_TRANSFERS, ERROR_MIX_ISOLATED_COLLATERAL,
    ERROR_POSITION_LIMIT_EXCEEDED, ERROR_SUPPLY_CAP,
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

    assert_eq!(
        state.last_account_nonce(),
        0,
        "failed supply must not create new accounts",
    );
    assert!(
        state.accounts().into_iter().next().is_none(),
        "no account entries expected after rejected supply",
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
    let expected_collateral = scaled_amount(1, CAPPED_DECIMALS);
    state.assert_collateral_raw_eq(
        1,
        &CAPPED_TOKEN,
        expected_collateral.clone(),
        "initial supply should mint collateral",
    );

    let expected_capped_supply = scaled_amount(1, CAPPED_DECIMALS);
    state.assert_collateral_raw_eq(
        1,
        &CAPPED_TOKEN,
        expected_capped_supply.clone(),
        "initial capped supply should be tracked",
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

    state.assert_collateral_raw_eq(
        1,
        &CAPPED_TOKEN,
        expected_capped_supply,
        "failed capped supply must not mutate balance",
    );
    assert_eq!(
        state.last_account_nonce(),
        1,
        "no new accounts should be minted after cap violation",
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

    assert_eq!(
        state.last_account_nonce(),
        0,
        "empty supply call should not mint accounts",
    );
    assert!(
        state.accounts().into_iter().next().is_none(),
        "no account entries expected when deposit fails",
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

    let expected_collateral = scaled_amount(1, CAPPED_DECIMALS);
    state.assert_collateral_raw_eq(
        1,
        &CAPPED_TOKEN,
        expected_collateral.clone(),
        "initial supply should mint collateral",
    );

    // Try to supply with only account NFT, no collateral
    state.supply_empty_asset_error(
        &supplier,
        OptionalValue::Some(1), // Account NFT
        OptionalValue::None,
        false, // is_vault = false
        ERROR_INVALID_NUMBER_OF_ESDT_TRANSFERS,
    );

    state.assert_collateral_raw_eq(
        1,
        &CAPPED_TOKEN,
        expected_collateral,
        "attempt supplying only NFT must not change collateral",
    );
    assert_eq!(
        state.last_account_nonce(),
        1,
        "no additional accounts expected after invalid NFT-only deposit",
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

    assert_eq!(
        state.last_account_nonce(),
        0,
        "isolated asset bulk failure must not mint accounts",
    );
    assert!(
        state.accounts().into_iter().next().is_none(),
        "controller should not record accounts for rejected isolated bulk supply",
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

    assert_eq!(
        state.last_account_nonce(),
        0,
        "mixed isolated collateral should not mint accounts",
    );
    assert!(
        state.accounts().into_iter().next().is_none(),
        "mixed isolated collateral should leave accounts empty",
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

    assert_eq!(
        state.last_account_nonce(),
        0,
        "capped bulk failure must not mint accounts",
    );
    assert!(
        state.accounts().into_iter().next().is_none(),
        "capped bulk failure must not register accounts",
    );
}

/// Tests that supplying beyond the position limit for an NFT fails.
///
/// Covers:
/// - Controller::supply endpoint error path
/// - Position limits validation in validation::ValidationModule
/// - ERROR_POSITION_LIMIT_EXCEEDED error condition
#[test]
fn supply_exceeds_position_limit_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Set position limits to 2 supply positions max for testing
    state.set_position_limits(10, 2); // 10 borrow, 2 supply

    // Create account for supplier and supply first asset
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(10u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false, // is_vault = false
    );

    let expected_egld_collateral = scaled_amount(10, EGLD_DECIMALS);
    state.assert_collateral_raw_eq(
        1,
        &EGLD_TOKEN,
        expected_egld_collateral.clone(),
        "first supply should mint EGLD collateral",
    );

    let account_nonce = 1;

    // Supply second asset - should succeed (at limit)
    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(100u64),
        USDC_DECIMALS,
        OptionalValue::Some(account_nonce),
        OptionalValue::None,
        false, // is_vault = false
    );

    let expected_usdc_collateral = scaled_amount(100, USDC_DECIMALS);
    state.assert_collateral_raw_eq(
        account_nonce,
        &USDC_TOKEN,
        expected_usdc_collateral.clone(),
        "second supply should mint USDC collateral",
    );

    // Try to supply third asset - should fail due to position limit
    state.supply_asset_error(
        &supplier,
        CAPPED_TOKEN,
        BigUint::from(1u64),
        CAPPED_DECIMALS,
        OptionalValue::Some(account_nonce),
        OptionalValue::None,
        false, // is_vault = false
        ERROR_POSITION_LIMIT_EXCEEDED,
    );

    state.assert_collateral_raw_eq(
        account_nonce,
        &EGLD_TOKEN,
        expected_egld_collateral,
        "failed third supply must not change EGLD collateral",
    );
    state.assert_collateral_raw_eq(
        account_nonce,
        &USDC_TOKEN,
        expected_usdc_collateral,
        "failed third supply must not change USDC collateral",
    );
    assert_eq!(
        state.last_account_nonce(),
        account_nonce,
        "position limit breach should not mint new accounts",
    );
}

/// Tests that bulk supply exceeding position limits fails even when individual supplies would pass.
///
/// Covers:
/// - Controller::supply endpoint bulk validation
/// - Bulk position limits validation in validation::ValidationModule  
/// - ERROR_POSITION_LIMIT_EXCEEDED error condition for bulk operations
#[test]
fn supply_bulk_exceeds_position_limit_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Set position limits to 1 supply position max for testing (very restrictive)
    state.set_position_limits(10, 1); // 10 borrow, 1 supply

    // Prepare bulk supply with 2 assets when limit is 1
    // This should fail because we're trying to create 2 positions when limit is 1
    let mut assets = ManagedVec::<StaticApi, EsdtTokenPayment<StaticApi>>::new();
    assets.push(EsdtTokenPayment::new(
        EGLD_TOKEN.to_token_identifier(),
        0,
        BigUint::from(10u64) * BigUint::from(10u64.pow(EGLD_DECIMALS as u32)),
    ));
    assets.push(EsdtTokenPayment::new(
        USDC_TOKEN.to_token_identifier(),
        0,
        BigUint::from(100u64) * BigUint::from(10u64.pow(USDC_DECIMALS as u32)),
    ));

    // This bulk supply should fail because it would create 2 new positions
    // when the limit is 1
    state.supply_bulk_error(
        &supplier,
        OptionalValue::None,
        OptionalValue::None,
        false, // is_vault = false
        assets,
        ERROR_POSITION_LIMIT_EXCEEDED,
    );

    assert_eq!(
        state.last_account_nonce(),
        0,
        "bulk position limit breach must not mint accounts",
    );
    assert!(
        state.accounts().into_iter().next().is_none(),
        "bulk position limit breach must not register accounts",
    );
}
