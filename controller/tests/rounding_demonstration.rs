use multiversx_sc::types::BigUint;
use multiversx_sc_scenario::imports::{OptionalValue, TestAddress};

pub mod constants;
pub mod proxys;
pub mod setup;
use constants::*;
use setup::*;

/// Demonstrates that both dollar-based and raw-unit supply helpers lead to precise dust handling.
#[test]
fn dollar_and_raw_supply_share_consistent_rounding() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Seed the market with $10,000 liquidity using the dollar-based helper.
    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(10_000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    state.assert_collateral_raw_eq(
        1,
        &USDC_TOKEN,
        scaled_amount(10_000, USDC_DECIMALS),
        "Supplier liquidity should be tracked in raw units",
    );

    // Borrower supplies $100 as collateral.
    let borrower_nonce = 2_u64;
    state.supply_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(100u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    state.assert_collateral_raw_eq(
        borrower_nonce,
        &USDC_TOKEN,
        scaled_amount(100, USDC_DECIMALS),
        "Borrower collateral should equal $100",
    );

    // Borrow the smallest unit via the raw helper and ensure interest stays bounded.
    state.borrow_asset_den(&borrower, USDC_TOKEN, BigUint::from(1u64), borrower_nonce);
    state.assert_borrow_raw_eq(
        borrower_nonce,
        &USDC_TOKEN,
        BigUint::from(1u64),
        "Dust borrow should be tracked exactly",
    );

    state.change_timestamp(SECONDS_PER_YEAR);
    state.supply_asset_den(
        &borrower,
        USDC_TOKEN,
        BigUint::from(1u64),
        OptionalValue::Some(borrower_nonce),
        OptionalValue::None,
        false,
    );

    state.assert_borrow_raw_within(
        borrower_nonce,
        &USDC_TOKEN,
        BigUint::from(1u64),
        BigUint::from(1u64),
        "Dust borrow should accrue at most 1 wei of interest",
    );

    state.repay_asset_deno(&borrower, &USDC_TOKEN, BigUint::from(2u64), borrower_nonce);
    state.assert_no_borrow_entry(borrower_nonce, &USDC_TOKEN);
}

/// Shows that repeating decimal borrows stay within expected interest bounds over time.
#[test]
fn repeating_decimal_borrow_interest_within_bounds() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(1_000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    let borrower_nonce = 2_u64;
    state.supply_asset_den(
        &borrower,
        USDC_TOKEN,
        BigUint::from(4_000_000u64),
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    state.assert_collateral_raw_eq(
        borrower_nonce,
        &USDC_TOKEN,
        scaled_amount(4, USDC_DECIMALS),
        "Borrower supply should equal 4 USDC",
    );

    let borrow_units = BigUint::from(1_333_333u64);
    state.borrow_asset_den(&borrower, USDC_TOKEN, borrow_units.clone(), borrower_nonce);
    state.assert_borrow_raw_eq(
        borrower_nonce,
        &USDC_TOKEN,
        borrow_units.clone(),
        "Borrow with repeating decimals should be exact in raw units",
    );

    state.change_timestamp(SECONDS_PER_YEAR + SECONDS_PER_DAY);
    state.supply_asset_den(
        &borrower,
        USDC_TOKEN,
        BigUint::from(1u64),
        OptionalValue::Some(borrower_nonce),
        OptionalValue::None,
        false,
    );

    let debt_after = state.borrow_amount_for_token(borrower_nonce, USDC_TOKEN);
    state.assert_borrow_raw_within(
        borrower_nonce,
        &USDC_TOKEN,
        borrow_units.clone(),
        BigUint::from(80_000u64),
        "Interest on ~1.33 USDC over ~1 year should stay under 0.08 USDC",
    );

    let reserves = state.market_reserves(state.usdc_market.clone());
    let revenue = state.market_revenue(state.usdc_market.clone());
    assert!(reserves.into_raw_units() >= &BigUint::zero());
    assert!(revenue.into_raw_units() >= &BigUint::zero());

    let outstanding_raw = debt_after.into_raw_units().clone();
    state.repay_asset_deno(&borrower, &USDC_TOKEN, outstanding_raw, borrower_nonce);
    state.assert_no_borrow_entry(borrower_nonce, &USDC_TOKEN);
}
