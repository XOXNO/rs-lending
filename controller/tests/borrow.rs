use common_constants::RAY;
use controller::{ERROR_BORROW_CAP, ERROR_POSITION_LIMIT_EXCEEDED};
use multiversx_sc::types::{EgldOrEsdtTokenIdentifier, EgldOrEsdtTokenPayment, MultiValueEncoded};
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

    state.assert_borrow_raw_eq(
        2,
        &EGLD_TOKEN,
        scaled_amount(50, EGLD_DECIMALS),
        "borrowed EGLD amount should match request",
    );
    state.assert_collateral_raw_eq(
        2,
        &USDC_TOKEN,
        scaled_amount(5000, USDC_DECIMALS),
        "USDC collateral should equal supplied amount",
    );
    state.assert_health_factor_at_least(2, RAY);
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

    let expected_capped_debt = scaled_amount(1, CAPPED_DECIMALS);
    state.assert_borrow_raw_eq(
        1,
        &CAPPED_TOKEN,
        expected_capped_debt.clone(),
        "initial capped borrow should be tracked exactly",
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

    state.assert_borrow_raw_eq(
        1,
        &CAPPED_TOKEN,
        expected_capped_debt,
        "failed borrow must not mutate capped debt",
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

    state.assert_borrow_raw_eq(
        2,
        &USDC_TOKEN,
        usdc_borrow.amount.clone(),
        "USDC borrow recorded precisely",
    );
    state.assert_borrow_raw_eq(
        2,
        &EGLD_TOKEN,
        egld_borrow.amount.clone(),
        "EGLD borrow recorded precisely",
    );
    state.assert_health_factor_at_least(2, RAY);
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

    let existing_usdc_debt = state
        .borrow_amount_for_token(2, USDC_TOKEN)
        .into_raw_units()
        .clone();
    let existing_egld_debt = state
        .borrow_amount_for_token(2, EGLD_TOKEN)
        .into_raw_units()
        .clone();

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

    let expected_usdc_total = existing_usdc_debt.clone() + usdc_borrow.amount.clone();
    let expected_egld_total = existing_egld_debt.clone() + egld_borrow.amount.clone();

    state.assert_borrow_raw_eq(
        2,
        &USDC_TOKEN,
        expected_usdc_total,
        "existing USDC debt should accumulate bulk borrow",
    );
    state.assert_borrow_raw_eq(
        2,
        &EGLD_TOKEN,
        expected_egld_total,
        "existing EGLD debt should accumulate bulk borrow",
    );
    state.assert_health_factor_at_least(2, RAY);
}

/// Tests that borrowing beyond the position limit for an NFT fails.
///
/// Covers:
/// - Controller::borrow endpoint error path
/// - Position limits validation in validation::ValidationModule
/// - ERROR_POSITION_LIMIT_EXCEEDED error condition
#[test]
fn borrow_exceeds_position_limit_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Set position limits to 2 borrow positions max for testing
    state.set_position_limits(2, 10); // 2 borrow, 10 supply

    // Supplier provides liquidity to pools for borrowing (following successful test pattern)
    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(10000u64), // $10,000 worth
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false, // is_vault = false
    );

    state.supply_asset(
        &supplier,
        CAPPED_TOKEN,
        BigUint::from(100u64), // $200 worth (CAPPED_PRICE = $2)
        CAPPED_DECIMALS,
        OptionalValue::Some(1), // Existing account
        OptionalValue::None,
        false, // is_vault = false
    );

    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(100u64), // $4,000 worth (EGLD_PRICE = $40)
        EGLD_DECIMALS,
        OptionalValue::Some(1), // Existing account
        OptionalValue::None,
        false, // is_vault = false
    );

    // Borrower supplies collateral (following successful test pattern)
    state.supply_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(5000u64), // $5,000 collateral
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false, // is_vault = false
    );

    let account_nonce = 2; // borrower account

    // First borrow - should succeed (small amount)
    state.borrow_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(100u64), // $100 worth
        account_nonce,
        USDC_DECIMALS,
    );

    // Second borrow - should succeed (at limit)
    state.borrow_asset(
        &borrower,
        CAPPED_TOKEN,
        BigUint::from(10u64), // $20 worth
        account_nonce,
        CAPPED_DECIMALS,
    );

    // Try to borrow third asset - should fail due to position limit
    state.borrow_asset_error(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(1u64), // $40 worth
        account_nonce,
        EGLD_DECIMALS,
        ERROR_POSITION_LIMIT_EXCEEDED,
    );
}

/// Tests that bulk borrow exceeding position limits fails even when individual borrows would pass.
///
/// Covers:
/// - Controller::borrow endpoint bulk validation
/// - Bulk position limits validation in validation::ValidationModule
/// - ERROR_POSITION_LIMIT_EXCEEDED error condition for bulk operations
#[test]
fn borrow_bulk_exceeds_position_limit_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Set position limits to 1 borrow position max for testing (very restrictive)
    state.set_position_limits(1, 10); // 1 borrow, 10 supply

    // Supply liquidity for both assets (follow working test pattern exactly)
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

    // Borrower supplies collateral (follow working test pattern exactly)
    state.supply_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(1000u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false, // is_vault = false
    );

    // Prepare bulk borrow request for 2 assets when limit is 1
    // This should fail because we're trying to create 2 positions when limit is 1
    let mut assets: MultiValueEncoded<StaticApi, EgldOrEsdtTokenPayment<StaticApi>> =
        MultiValueEncoded::new();

    let egld_borrow = EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        0,
        BigUint::from(50u64) * BigUint::from(10u64.pow(EGLD_DECIMALS as u32)),
    );
    assets.push(egld_borrow);

    let usdc_borrow = EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN.to_token_identifier()),
        0,
        BigUint::from(500u64) * BigUint::from(10u64.pow(USDC_DECIMALS as u32)),
    );
    assets.push(usdc_borrow);

    let total_borrow_before = state.total_borrow_in_egld(2).into_raw_units().clone();

    // This bulk borrow should fail because it would create 2 new positions
    // when the limit is 1
    state.borrow_assets_error(2, &borrower, assets, ERROR_POSITION_LIMIT_EXCEEDED);

    let total_borrow_after = state.total_borrow_in_egld(2).into_raw_units().clone();
    assert_eq!(
        total_borrow_after, total_borrow_before,
        "bulk borrow failure must leave account debt unchanged",
    );
}
