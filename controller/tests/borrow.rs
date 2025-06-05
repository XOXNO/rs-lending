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

/// Tests the basic flow of supplying collateral and borrowing against it.
/// 
/// Covers:
/// - Controller::supply endpoint (normal single asset supply)
/// - Controller::borrow endpoint (single asset borrow)
/// - Verifies that borrowed amount and collateral are properly tracked
#[test]
fn borrow_single_asset_against_collateral_success() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Supplier provides liquidity to the pool
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false, // is_vault = false
    );

    // Borrower supplies collateral
    state.supply_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(5000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false, // is_vault = false
    );

    // Borrower takes out a loan against their collateral
    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(50u64),
        2, // account_nonce
        EGLD_DECIMALS,
    );

    // Verify the borrow and collateral positions are recorded
    let borrowed_amount = state.get_borrow_amount_for_token(2, EGLD_TOKEN);
    let collateral_amount = state.get_collateral_amount_for_token(2, USDC_TOKEN);

    assert!(borrowed_amount > ManagedDecimal::from_raw_units(BigUint::zero(), EGLD_DECIMALS));
    assert!(collateral_amount > ManagedDecimal::from_raw_units(BigUint::zero(), USDC_DECIMALS));
}

/// Tests that borrowing fails when the borrow cap for an asset is exceeded.
/// 
/// Covers:
/// - Controller::borrow endpoint error path
/// - Borrow cap validation in positions::borrow::PositionBorrowModule
/// - ERROR_BORROW_CAP error condition
#[test]
fn borrow_exceeds_cap_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);
    
    // Supply capped token to enable borrowing
    state.supply_asset(
        &supplier,
        CAPPED_TOKEN,
        BigUint::from(150u64),
        CAPPED_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false, // is_vault = false
    );
    
    // First borrow succeeds (within cap)
    state.borrow_asset(
        &supplier,
        CAPPED_TOKEN,
        BigUint::from(1u64),
        1, // account_nonce
        CAPPED_DECIMALS,
    );
    
    // Second borrow fails (exceeds cap)
    state.borrow_asset_error(
        &supplier,
        CAPPED_TOKEN,
        BigUint::from(100u64),
        1, // account_nonce
        CAPPED_DECIMALS,
        ERROR_BORROW_CAP,
    );
}

/// Tests bulk borrowing of multiple assets in a single transaction for new positions.
/// 
/// Covers:
/// - Controller::borrow endpoint with multiple assets
/// - Bulk borrow processing in positions::borrow::PositionBorrowModule
/// - Creating new borrow positions for multiple assets simultaneously
#[test]
fn borrow_bulk_new_positions_success() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Supply liquidity for both assets
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false, // is_vault = false
    );
    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(1000u64),
        USDC_DECIMALS,
        OptionalValue::Some(1), // Existing account
        OptionalValue::None,
        false, // is_vault = false
    );

    // Borrower supplies collateral
    state.supply_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(1000u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        true, // is_vault = true (though this parameter seems unused in test)
    );

    // Prepare bulk borrow request
    let mut assets: MultiValueEncoded<StaticApi, EgldOrEsdtTokenPayment<StaticApi>> =
        MultiValueEncoded::new();

    let egld_borrow = EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        0,
        BigUint::from(50u64) * BigUint::from(10u64.pow(EGLD_DECIMALS as u32)),
    );
    assets.push(egld_borrow.clone());
    
    let usdc_borrow = EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN.to_token_identifier()),
        0,
        BigUint::from(500u64) * BigUint::from(10u64.pow(USDC_DECIMALS as u32)),
    );
    assets.push(usdc_borrow.clone());
    
    // Execute bulk borrow
    state.borrow_assets(2, &borrower, assets);

    // Verify both positions were created with exact amounts
    let usdc_borrowed = state.get_borrow_amount_for_token(2, USDC_TOKEN);
    let egld_borrowed = state.get_borrow_amount_for_token(2, EGLD_TOKEN);
    assert_eq!(usdc_borrowed.into_raw_units().clone(), usdc_borrow.amount);
    assert_eq!(egld_borrowed.into_raw_units().clone(), egld_borrow.amount);
}

/// Tests bulk borrowing when the account already has existing borrow positions.
/// 
/// Covers:
/// - Controller::borrow endpoint with multiple assets on existing positions
/// - Updating existing borrow positions vs creating new ones
/// - Interest accrual on existing positions before new borrows
#[test]
fn borrow_bulk_existing_positions_success() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Supply liquidity for both assets
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false, // is_vault = false
    );
    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(1000u64),
        USDC_DECIMALS,
        OptionalValue::Some(1), // Existing account
        OptionalValue::None,
        false, // is_vault = false
    );

    // Borrower supplies collateral
    state.supply_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(1000u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        true, // is_vault = true (though this parameter seems unused in test)
    );

    // Create initial borrow positions
    state.borrow_asset(&borrower, USDC_TOKEN, BigUint::from(1u64), 2, USDC_DECIMALS);
    state.borrow_asset(&borrower, EGLD_TOKEN, BigUint::from(1u64), 2, EGLD_DECIMALS);

    // Prepare additional bulk borrow
    let mut assets: MultiValueEncoded<StaticApi, EgldOrEsdtTokenPayment<StaticApi>> =
        MultiValueEncoded::new();

    let egld_borrow = EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        0,
        BigUint::from(50u64) * BigUint::from(10u64.pow(EGLD_DECIMALS as u32)),
    );
    assets.push(egld_borrow.clone());
    
    let usdc_borrow = EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN.to_token_identifier()),
        0,
        BigUint::from(500u64) * BigUint::from(10u64.pow(USDC_DECIMALS as u32)),
    );
    assets.push(usdc_borrow.clone());
    
    // Execute bulk borrow on existing positions
    state.borrow_assets(2, &borrower, assets);

    // Verify positions were updated (amounts should be greater than new borrow due to existing debt)
    let usdc_total_borrowed = state.get_borrow_amount_for_token(2, USDC_TOKEN);
    let egld_total_borrowed = state.get_borrow_amount_for_token(2, EGLD_TOKEN);
    assert!(usdc_total_borrowed.into_raw_units().clone() > usdc_borrow.amount);
    assert!(egld_total_borrowed.into_raw_units().clone() > egld_borrow.amount);
}
