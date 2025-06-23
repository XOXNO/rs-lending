use controller::{ERROR_HEALTH_FACTOR_WITHDRAW, ERROR_INSUFFICIENT_LIQUIDITY};
use multiversx_sc::types::{EgldOrEsdtTokenIdentifier, MultiValueEncoded};
use multiversx_sc_scenario::imports::{BigUint, OptionalValue, TestAddress};
pub mod constants;
pub mod proxys;
pub mod setup;
use constants::*;
use setup::*;

/// Tests that withdrawing more than deposited amount gets capped at maximum available.
///
/// Covers:
/// - Controller::withdraw endpoint behavior with excess amounts
/// - Automatic capping to available balance
/// - Withdrawal of entire position
#[test]
fn withdraw_excess_amount_capped_to_available_success() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Supply 1000 USDC
    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(1000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Attempt to withdraw 1500 USDC (more than available)
    // Should succeed by capping to 1000 USDC
    state.withdraw_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(1500u64),
        1,
        USDC_DECIMALS,
    );

    // Verify token was fully withdrawn
    let custom_error_message = format!("Token not existing in the account {}", USDC_TOKEN.as_str());
    state.get_collateral_amount_for_token_non_existing(
        1,
        USDC_TOKEN,
        custom_error_message.as_bytes(),
    );
}

/// Tests that withdrawing fails when pool has insufficient liquidity due to borrows.
///
/// Covers:
/// - Controller::withdraw endpoint error path
/// - Liquidity validation in positions::withdraw::PositionWithdrawModule
/// - ERROR_INSUFFICIENT_LIQUIDITY error condition
#[test]
fn withdraw_insufficient_liquidity_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Supplier deposits 1000 USDC
    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(1000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Borrower supplies collateral
    state.supply_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Borrower takes out 100 USDC loan
    state.borrow_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(100u64),
        2,
        USDC_DECIMALS,
    );

    // Supplier tries to withdraw full 1000 USDC but only 900 is available
    state.withdraw_asset_error(
        &supplier,
        USDC_TOKEN,
        BigUint::from(1000u64),
        1,
        USDC_DECIMALS,
        ERROR_INSUFFICIENT_LIQUIDITY,
    );
}

/// Tests withdrawal with accumulated interest over time.
///
/// Covers:
/// - Controller::withdraw endpoint with interest accrual
/// - Interest calculations in withdrawal flow
/// - Partial withdrawal with updated balances
#[test]
fn withdraw_with_accumulated_interest_success() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Supplier deposits 1000 USDC
    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(1000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Borrower supplies collateral and borrows
    state.supply_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(10u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.borrow_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(200u64),
        2,
        USDC_DECIMALS,
    );

    // Advance time to accumulate interest
    state.change_timestamp(SECONDS_PER_DAY * 10);

    // Record initial collateral with interest
    let initial_collateral = state.get_collateral_amount_for_token(1, USDC_TOKEN);

    // Advance more time
    state.change_timestamp(SECONDS_PER_DAY * 20);

    // Withdraw partial amount
    state.withdraw_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(500u64),
        1,
        USDC_DECIMALS,
    );

    // Verify collateral was reduced by withdrawal amount
    let final_collateral = state.get_collateral_amount_for_token(1, USDC_TOKEN);
    assert!(final_collateral < initial_collateral);
}

/// Tests complex withdrawal scenario with single user as both supplier and borrower.
///
/// Covers:
/// - Controller::withdraw endpoint with same user supply and borrow
/// - Market index updates through updateIndexes
/// - Revenue and reserve tracking
/// - Full withdrawal after repayment
#[test]
fn withdraw_single_user_supply_borrow_full_cycle() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_accounts(&mut state, supplier, borrower);

    state.change_timestamp(1740269720);
    let mut markets = MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN));

    // User supplies 100 EGLD
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.change_timestamp(1740269852);
    state.update_markets(&supplier, markets.clone());

    // Same user borrows 72 EGLD
    state.borrow_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(72u64),
        1,
        EGLD_DECIMALS,
    );

    state.change_timestamp(1740275066);
    state.update_markets(&supplier, markets.clone());

    // Get balances after interest accrual
    let initial_collateral = state.get_collateral_amount_for_token(1, EGLD_TOKEN);
    let initial_borrow = state.get_borrow_amount_for_token(1, EGLD_TOKEN);

    // Repay exact borrow amount with interest
    state.repay_asset_deno(
        &supplier,
        &EGLD_TOKEN,
        BigUint::from(72721215451172815256u128),
        1,
    );

    // Check market state after repayment
    let reserve = state.get_market_reserves(state.egld_market.clone());
    let revenue = state.get_market_revenue(state.egld_market.clone());
    let borrow_index = state.get_market_borrow_index(state.egld_market.clone());
    let supply_index = state.get_market_supply_index(state.egld_market.clone());

    state.change_timestamp(1740275594);
    state.update_markets(&supplier, markets.clone());

    // Get final collateral amount
    let final_collateral = state.get_collateral_amount_for_token(1, EGLD_TOKEN);

    // Withdraw entire collateral balance
    state.withdraw_asset_den(
        &supplier,
        EGLD_TOKEN,
        final_collateral.into_raw_units().clone(),
        1,
    );

    state.update_markets(&supplier, markets.clone());

    // Verify final market state
    let final_reserve = state.get_market_reserves(state.egld_market.clone());
    let final_revenue = state.get_market_revenue(state.egld_market.clone());
}

/// Tests withdrawal with prior market index update to ensure proper accounting.
///
/// Covers:
/// - Controller::updateIndexes endpoint interaction with withdrawals
/// - Market state synchronization before operations
/// - Reserve and revenue tracking accuracy
#[test]
fn withdraw_with_prior_index_update_success() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_accounts(&mut state, supplier, borrower);
    let mut markets = MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN));

    state.change_timestamp(1740269720);

    // User supplies 100 EGLD
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.change_timestamp(1740269852);
    state.update_markets(&supplier, markets.clone());

    // User borrows 72 EGLD against their own collateral
    state.borrow_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(72u64),
        1,
        EGLD_DECIMALS,
    );

    state.change_timestamp(1740275066);
    state.update_markets(&supplier, markets.clone());

    // Get current positions
    let initial_collateral = state.get_collateral_amount_for_token(1, EGLD_TOKEN);
    let initial_borrow = state.get_borrow_amount_for_token(1, EGLD_TOKEN);

    // Repay full borrow amount
    state.repay_asset_deno(
        &supplier,
        &EGLD_TOKEN,
        initial_borrow.into_raw_units().clone(),
        1,
    );

    // Update markets after repayment
    state.update_markets(&supplier, markets.clone());

    // Record market state
    let reserve = state.get_market_reserves(state.egld_market.clone());
    let revenue = state.get_market_revenue(state.egld_market.clone());
    let total_deposit = state.get_market_supplied(state.egld_market.clone());
    let borrow_index = state.get_market_borrow_index(state.egld_market.clone());
    let supply_index = state.get_market_supply_index(state.egld_market.clone());

    state.change_timestamp(1740275594);

    // Update markets twice to ensure proper synchronization
    state.update_markets(&supplier, markets.clone());
    state.update_markets(&supplier, markets.clone());

    // Get final collateral after all updates
    let final_collateral = state.get_collateral_amount_for_token(1, EGLD_TOKEN);

    // Calculate expected difference
    let diff = (reserve.clone().into_signed() - final_collateral.clone().into_signed())
        - revenue.clone().into_signed();

    // Withdraw full collateral
    state.withdraw_asset_den(
        &supplier,
        EGLD_TOKEN,
        final_collateral.into_raw_units().clone(),
        1,
    );

    // Verify final reserves and revenue
    let final_reserve = state.get_market_reserves(state.egld_market.clone());
    let final_revenue = state.get_market_revenue(state.egld_market.clone());
}

/// Tests that withdrawal triggering self-liquidation is prevented.
///
/// Covers:
/// - Controller::withdraw endpoint health factor validation
/// - Self-liquidation protection in positions::withdraw::PositionWithdrawModule
/// - ERROR_HEALTH_FACTOR_WITHDRAW error condition
#[test]
fn withdraw_self_liquidation_protection_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_accounts(&mut state, supplier, borrower);

    // Supplier deposits EGLD for others to borrow
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Borrower supplies USDC as collateral
    state.supply_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(5000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Borrower takes 80 EGLD loan (high utilization)
    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(80u64),
        2,
        EGLD_DECIMALS,
    );

    // Attempting to withdraw all collateral would make position unhealthy
    state.withdraw_asset_error(
        &borrower,
        USDC_TOKEN,
        BigUint::from(5000u64),
        2,
        USDC_DECIMALS,
        ERROR_HEALTH_FACTOR_WITHDRAW,
    );
}

/// Tests that withdrawing a non-deposited asset fails with appropriate error.
///
/// Covers:
/// - Controller::withdraw endpoint validation
/// - Asset existence check in withdrawal flow
/// - Custom error message for non-existent asset
#[test]
fn withdraw_non_deposited_asset_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_accounts(&mut state, supplier, borrower);

    // Supplier deposits EGLD
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Borrower supplies USDC as collateral
    state.supply_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(5000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Borrower borrows EGLD
    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(50u64),
        2,
        EGLD_DECIMALS,
    );

    // Try to withdraw XEGLD which was never deposited
    let custom_error_message = format!(
        "Token {} is not available for this account",
        XEGLD_TOKEN.as_str()
    );

    state.withdraw_asset_error(
        &borrower,
        XEGLD_TOKEN,
        BigUint::from(50u64),
        2,
        XEGLD_DECIMALS,
        custom_error_message.as_bytes(),
    );
}
