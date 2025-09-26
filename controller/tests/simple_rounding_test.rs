use multiversx_sc::types::BigUint;
use multiversx_sc_scenario::imports::{OptionalValue, TestAddress};

pub mod constants;
pub mod proxys;
pub mod setup;
use constants::*;
use setup::*;

/// Validates that RAY-based math keeps rounding drift negligible even when
/// positions operate on dust-scale amounts and repeated same-block cycles.
#[test]
fn rounding_precision_stays_stable_for_dust_flows() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    state.supply_asset_den(
        &supplier,
        USDC_TOKEN,
        BigUint::from(10_000_000_000u64),
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    state.assert_collateral_raw_eq(
        1,
        &USDC_TOKEN,
        scaled_amount(10_000, USDC_DECIMALS),
        "Supplier deposit should be tracked in raw units",
    );

    state.supply_asset_den(
        &borrower,
        USDC_TOKEN,
        BigUint::from(100_000_000u64),
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    state.assert_collateral_raw_eq(
        2,
        &USDC_TOKEN,
        scaled_amount(100, USDC_DECIMALS),
        "Borrower collateral should be recorded exactly",
    );

    let borrower_nonce = 2;

    state.borrow_asset_den(&borrower, USDC_TOKEN, BigUint::from(1u64), borrower_nonce);
    let initial_borrow = state.borrow_amount_for_token(borrower_nonce, USDC_TOKEN);

    state.change_timestamp(SECONDS_PER_YEAR);
    state.supply_asset_den(
        &borrower,
        USDC_TOKEN,
        BigUint::from(1u64),
        OptionalValue::Some(borrower_nonce),
        OptionalValue::None,
        false,
    );

    let borrow_after_year = state.borrow_amount_for_token(borrower_nonce, USDC_TOKEN);
    let interest_wei = borrow_after_year.into_raw_units() - initial_borrow.into_raw_units();

    state.assert_borrow_raw_eq(
        borrower_nonce,
        &USDC_TOKEN,
        BigUint::from(1u64),
        "Initial dust borrow must equal 1 wei",
    );
    state.assert_borrow_raw_within(
        borrower_nonce,
        &USDC_TOKEN,
        BigUint::from(1u64),
        BigUint::from(1u64),
        "Dust borrow should accrue at most 1 wei of interest over a year",
    );
    assert!(interest_wei <= BigUint::from(1u64));

    state.repay_asset(
        &borrower,
        &USDC_TOKEN,
        borrow_after_year.into_raw_units().clone(),
        borrower_nonce,
        USDC_DECIMALS,
    );

    let mut total_interest_paid = BigUint::zero();
    for cycle_number in 0..100 {
        state.change_timestamp(SECONDS_PER_YEAR + (cycle_number + 1) * 3600);
        state.borrow_asset_den(
            &borrower,
            USDC_TOKEN,
            BigUint::from(1_000_000u64),
            borrower_nonce,
        );
        let debt = state.borrow_amount_for_token(borrower_nonce, USDC_TOKEN);
        state.repay_asset_deno(
            &borrower,
            &USDC_TOKEN,
            debt.into_raw_units().clone(),
            borrower_nonce,
        );
        let interest = debt.into_raw_units().clone() - BigUint::from(1_000_000u64);
        total_interest_paid += interest;
    }
    assert!(
        total_interest_paid <= BigUint::from(10u64),
        "Same-block borrow/repay cycles should accrue at most 10 wei of interest",
    );

    let _final_collateral = state.collateral_amount_for_token(borrower_nonce, USDC_TOKEN);
    let _supplier_balance = state.collateral_amount_for_token(1, USDC_TOKEN);
    let reserves = state.market_reserves(state.usdc_market.clone());
    let revenue = state.market_revenue(state.usdc_market.clone());

    state.assert_collateral_raw_within(
        borrower_nonce,
        &USDC_TOKEN,
        scaled_amount(100, USDC_DECIMALS),
        BigUint::from(20u64),
        "Borrower collateral should remain within 20 wei of the starting balance",
    );
    state.assert_collateral_raw_within(
        1,
        &USDC_TOKEN,
        scaled_amount(10_000, USDC_DECIMALS),
        BigUint::from(2_000u64),
        "Supplier balance should only earn minimal interest",
    );
    assert!(revenue.into_raw_units() <= &BigUint::from(1_000u64));
    assert!(reserves.into_raw_units() >= &scaled_amount(10_000, USDC_DECIMALS));
    state.assert_no_borrow_entry(borrower_nonce, &USDC_TOKEN);
    state.assert_no_borrow_entry(borrower_nonce, &EGLD_TOKEN);
}
