use multiversx_sc::types::{EgldOrEsdtTokenIdentifier, MultiValueEncoded};
use multiversx_sc_scenario::imports::{BigUint, OptionalValue, TestAddress};
pub mod constants;
pub mod proxys;
pub mod setup;
use constants::*;

use setup::*;

/// Tests repaying a loan with interest, including overpayment handling.
///
/// Covers:
/// - Controller::repay endpoint functionality
/// - Interest accrual over time
/// - Full repayment clearing debt position
/// - Overpayment handling (extra amount beyond debt)
/// - Debt position removal after full repayment
#[test]
fn repay_full_debt_with_interest_and_overpayment_success() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Supplier provides liquidity
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    state.assert_collateral_raw_eq(
        1,
        &EGLD_TOKEN,
        scaled_amount(100, EGLD_DECIMALS),
        "supplier EGLD deposit should be tracked",
    );

    // Borrower supplies collateral
    state.supply_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(5000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    let initial_collateral_raw = scaled_amount(5000, USDC_DECIMALS);
    state.assert_collateral_raw_eq(
        2,
        &USDC_TOKEN,
        initial_collateral_raw.clone(),
        "borrower USDC collateral should be recorded",
    );

    // Borrower takes 50 EGLD loan
    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(50u64),
        2,
        EGLD_DECIMALS,
    );
    let initial_debt_raw = scaled_amount(50, EGLD_DECIMALS);
    state.assert_borrow_raw_eq(
        2,
        &EGLD_TOKEN,
        initial_debt_raw.clone(),
        "initial EGLD borrow should match requested amount",
    );

    // Advance 10 days to accumulate interest
    state.change_timestamp(SECONDS_PER_DAY * 10);

    // Update market indexes to reflect interest
    let mut markets = MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN));
    markets.push(EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN));
    state.update_markets(&borrower, markets.clone());

    let reserves_before = state
        .market_reserves(state.egld_market.clone())
        .into_raw_units()
        .clone();

    // Verify debt increased due to interest
    let debt_with_interest_raw = state
        .borrow_amount_for_token(2, EGLD_TOKEN)
        .into_raw_units()
        .clone();
    assert!(
        debt_with_interest_raw > initial_debt_raw,
        "interest accrual should increase outstanding debt",
    );

    // Repay 51 EGLD (initial 50 + extra to cover interest and overpay)
    state.repay_asset(
        &borrower,
        &EGLD_TOKEN,
        BigUint::from(51u64),
        2,
        EGLD_DECIMALS,
    );

    // Verify debt position was fully cleared and reserves captured the overpayment
    state.assert_no_borrow_entry(2, &EGLD_TOKEN);
    state.assert_total_borrow_raw_eq(2, BigUint::zero(), "Repayment should clear borrower debt");
    state.assert_collateral_raw_eq(
        2,
        &USDC_TOKEN,
        initial_collateral_raw,
        "repayment should not alter posted collateral",
    );

    let reserves_raw = state
        .market_reserves(state.egld_market.clone())
        .into_raw_units()
        .clone();
    assert!(reserves_raw >= reserves_before);
}
