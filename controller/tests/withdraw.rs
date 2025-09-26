use common_constants::RAY;
use controller::{ERROR_HEALTH_FACTOR_WITHDRAW, ERROR_INSUFFICIENT_LIQUIDITY};
use multiversx_sc::types::{EgldOrEsdtTokenIdentifier, MultiValueEncoded};
use multiversx_sc_scenario::imports::{BigUint, OptionalValue, StaticApi, TestAddress};
pub mod constants;
pub mod proxys;
pub mod setup;
use constants::*;
use setup::*;

fn ray_dust_tolerance() -> BigUint<StaticApi> {
    BigUint::from(10u64).pow(22)
}

fn usdc_tolerance_raw(raw_units: u64) -> BigUint<StaticApi> {
    BigUint::from(raw_units)
}

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
    let supplied_raw = scaled_amount(1000, USDC_DECIMALS);
    state.assert_collateral_raw_eq(
        1,
        &USDC_TOKEN,
        supplied_raw.clone(),
        "initial USDC deposit should be tracked",
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
    state.assert_no_collateral_entry(1, &USDC_TOKEN);
    state.assert_total_collateral_raw_eq(
        1,
        BigUint::zero(),
        "Supplier total collateral should be zero after capped withdrawal",
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
    let expected_supply_raw = scaled_amount(1000, USDC_DECIMALS);
    state.assert_collateral_raw_eq(
        1,
        &USDC_TOKEN,
        expected_supply_raw.clone(),
        "supplier USDC collateral should be recorded",
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
    let expected_borrow_raw = scaled_amount(100, USDC_DECIMALS);
    state.assert_borrow_raw_eq(
        2,
        &USDC_TOKEN,
        expected_borrow_raw.clone(),
        "borrowed USDC debt should equal request",
    );

    let total_collateral_before = state.total_collateral_in_egld(1).into_raw_units().clone();
    let total_borrow_before = state.total_borrow_in_egld(2).into_raw_units().clone();

    // Supplier tries to withdraw full 1000 USDC but only 900 is available
    state.withdraw_asset_error(
        &supplier,
        USDC_TOKEN,
        BigUint::from(1000u64),
        1,
        USDC_DECIMALS,
        ERROR_INSUFFICIENT_LIQUIDITY,
    );

    // Collateral and debts must remain unchanged after failed withdrawal
    state.assert_collateral_raw_eq(
        1,
        &USDC_TOKEN,
        expected_supply_raw.clone(),
        "failed withdrawal must not reduce supplier collateral",
    );
    state.assert_borrow_raw_eq(
        2,
        &USDC_TOKEN,
        expected_borrow_raw.clone(),
        "failed withdrawal must not mutate borrower debt",
    );
    let total_collateral_after = state.total_collateral_in_egld(1).into_raw_units().clone();
    let total_borrow_after = state.total_borrow_in_egld(2).into_raw_units().clone();
    let coll_diff = if total_collateral_after >= total_collateral_before {
        total_collateral_after.clone() - total_collateral_before.clone()
    } else {
        total_collateral_before.clone() - total_collateral_after.clone()
    };
    assert!(
        coll_diff <= ray_dust_tolerance(),
        "Total collateral should remain unchanged after failed withdrawal",
    );
    let borrow_diff = if total_borrow_after >= total_borrow_before {
        total_borrow_after.clone() - total_borrow_before.clone()
    } else {
        total_borrow_before.clone() - total_borrow_after.clone()
    };
    assert!(
        borrow_diff <= ray_dust_tolerance(),
        "Total borrow should remain unchanged after failed withdrawal",
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
    let initial_collateral = state.collateral_amount_for_token(1, USDC_TOKEN);
    let initial_raw = initial_collateral.into_raw_units().clone();

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
    let final_collateral = state.collateral_amount_for_token(1, USDC_TOKEN);
    let final_raw = final_collateral.into_raw_units().clone();
    let withdrawn_raw = scaled_amount(500, USDC_DECIMALS);
    let tolerance_raw = usdc_tolerance_raw(200_000); // 0.2 USDC tolerance due to interest rounding
    assert!(
        final_raw < initial_raw,
        "Collateral should decrease after withdrawal"
    );
    let consumed_raw = initial_raw.clone() - final_raw.clone();
    let diff = if consumed_raw >= withdrawn_raw {
        consumed_raw.clone() - withdrawn_raw.clone()
    } else {
        withdrawn_raw.clone() - consumed_raw.clone()
    };
    assert!(
        diff <= tolerance_raw,
        "Withdrawn amount should match requested value within tolerance",
    );
    assert!(
        final_raw > BigUint::zero(),
        "partial withdraw should leave collateral"
    );
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
    let supplied_egld = scaled_amount(100, EGLD_DECIMALS);
    state.assert_collateral_raw_eq(
        1,
        &EGLD_TOKEN,
        supplied_egld.clone(),
        "initial EGLD supply should be tracked",
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

    let borrow_after_updates = state
        .borrow_amount_for_token(1, EGLD_TOKEN)
        .into_raw_units()
        .clone();
    assert!(
        borrow_after_updates >= scaled_amount(72, EGLD_DECIMALS),
        "borrow position should reflect at least the borrowed principal",
    );

    // Repay exact borrow amount with interest
    state.repay_asset_deno(
        &supplier,
        &EGLD_TOKEN,
        BigUint::from(72721215451172815256u128),
        1,
    );

    state.change_timestamp(1740275594);
    state.update_markets(&supplier, markets.clone());

    // Get final collateral amount
    let final_collateral = state.collateral_amount_for_token(1, EGLD_TOKEN);

    // Withdraw entire collateral balance
    state.withdraw_asset_den(
        &supplier,
        EGLD_TOKEN,
        final_collateral.into_raw_units().clone(),
        1,
    );

    state.update_markets(&supplier, markets.clone());

    state.assert_no_borrow_entry(1, &EGLD_TOKEN);
    state.assert_no_collateral_entry(1, &EGLD_TOKEN);
    state.assert_total_borrow_raw_eq(
        1,
        BigUint::zero(),
        "Borrow position should be cleared after repayment",
    );
    state.assert_total_collateral_raw_eq(
        1,
        BigUint::zero(),
        "Collateral should be withdrawn entirely after full exit",
    );
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
    let initial_supply_raw = scaled_amount(100, EGLD_DECIMALS);
    state.assert_collateral_raw_eq(
        1,
        &EGLD_TOKEN,
        initial_supply_raw.clone(),
        "initial EGLD supply should be tracked",
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
    let initial_borrow = state.borrow_amount_for_token(1, EGLD_TOKEN);
    assert!(
        initial_borrow.into_raw_units() > &BigUint::zero(),
        "borrow should accrue prior to repayment",
    );

    // Repay full borrow amount
    state.repay_asset_deno(
        &supplier,
        &EGLD_TOKEN,
        initial_borrow.into_raw_units().clone(),
        1,
    );

    // Update markets after repayment
    state.update_markets(&supplier, markets.clone());

    state.change_timestamp(1740275594);

    // Update markets twice to ensure proper synchronization
    state.update_markets(&supplier, markets.clone());
    state.update_markets(&supplier, markets.clone());

    // Get final collateral after all updates
    let final_collateral = state.collateral_amount_for_token(1, EGLD_TOKEN);

    // Withdraw full collateral
    state.withdraw_asset_den(
        &supplier,
        EGLD_TOKEN,
        final_collateral.into_raw_units().clone(),
        1,
    );

    state.assert_no_borrow_entry(1, &EGLD_TOKEN);
    state.assert_no_collateral_entry(1, &EGLD_TOKEN);
    state.assert_total_borrow_raw_eq(
        1,
        BigUint::zero(),
        "Borrow position should be cleared after repayment",
    );
    state.assert_total_collateral_raw_eq(
        1,
        BigUint::zero(),
        "Collateral should be withdrawn entirely after synchronization",
    );
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
    let collateral_before = scaled_amount(5000, USDC_DECIMALS);
    state.assert_collateral_raw_eq(
        2,
        &USDC_TOKEN,
        collateral_before.clone(),
        "borrower collateral should be tracked",
    );
    let debt_before = scaled_amount(80, EGLD_DECIMALS);
    state.assert_borrow_raw_eq(
        2,
        &EGLD_TOKEN,
        debt_before.clone(),
        "borrowed EGLD should match request",
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

    state.assert_collateral_raw_eq(
        2,
        &USDC_TOKEN,
        collateral_before,
        "failed withdrawal must not reduce collateral",
    );
    state.assert_borrow_raw_eq(
        2,
        &EGLD_TOKEN,
        debt_before.clone(),
        "failed withdrawal must not alter borrow",
    );
    state.assert_health_factor_at_least(2, RAY);
    state.assert_total_borrow_raw_eq(
        2,
        debt_before,
        "Borrower debt should remain constant after health-factor revert",
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
    let egld_debt_raw = scaled_amount(50, EGLD_DECIMALS);
    state.assert_borrow_raw_eq(
        2,
        &EGLD_TOKEN,
        egld_debt_raw.clone(),
        "borrowed EGLD should equal request",
    );
    let usdc_collateral_raw = scaled_amount(5000, USDC_DECIMALS);
    state.assert_collateral_raw_eq(
        2,
        &USDC_TOKEN,
        usdc_collateral_raw.clone(),
        "borrower USDC collateral should be tracked",
    );

    let total_collateral_before = state.total_collateral_in_egld(2).into_raw_units().clone();
    let total_borrow_before = state.total_borrow_in_egld(2).into_raw_units().clone();

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

    state.assert_borrow_raw_eq(
        2,
        &EGLD_TOKEN,
        egld_debt_raw.clone(),
        "failed withdrawal on non-deposited asset must not alter debt",
    );
    state.assert_collateral_raw_eq(
        2,
        &USDC_TOKEN,
        usdc_collateral_raw.clone(),
        "failed withdrawal on non-deposited asset must not alter collateral",
    );
    let total_collateral_after = state.total_collateral_in_egld(2).into_raw_units().clone();
    let total_borrow_after = state.total_borrow_in_egld(2).into_raw_units().clone();
    let coll_diff = if total_collateral_after >= total_collateral_before {
        total_collateral_after.clone() - total_collateral_before.clone()
    } else {
        total_collateral_before.clone() - total_collateral_after.clone()
    };
    assert!(
        coll_diff <= ray_dust_tolerance(),
        "Total collateral should remain unchanged when withdrawing non-existent asset",
    );
    let borrow_diff = if total_borrow_after >= total_borrow_before {
        total_borrow_after.clone() - total_borrow_before.clone()
    } else {
        total_borrow_before.clone() - total_borrow_after.clone()
    };
    assert!(
        borrow_diff <= ray_dust_tolerance(),
        "Total borrow should remain unchanged when withdrawing non-existent asset",
    );
}
