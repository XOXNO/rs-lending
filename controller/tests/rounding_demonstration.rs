use multiversx_sc::types::BigUint;
use multiversx_sc_scenario::imports::{OptionalValue, TestAddress};

pub mod constants;
pub mod proxys;
pub mod setup;
use constants::*;
use setup::*;

/// Demonstrates the difference between supply_asset (dollar amounts) and supply_asset_den (raw units).
/// Shows that rounding with RAY precision has negligible impact even with small amounts.
#[test]
fn test_rounding_with_small_amounts() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    println!("\n=== Setting up liquidity pool ===");

    // Supplier provides $10,000 USDC liquidity using supply_asset (dollar notation)
    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(10_000u64), // This means $10,000
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Borrower supplies $100 USDC as collateral using supply_asset (dollar notation)
    state.supply_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(100u64), // This means $100
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    let borrower_nonce = 2;

    println!("\n=== Testing minimum borrow amount ===");

    // Borrow 1 unit (0.000001 USDC) using supply_asset_den
    state.borrow_asset_den(
        &borrower,
        USDC_TOKEN,
        BigUint::from(1u64), // 1 unit = 0.000001 USDC
        borrower_nonce,
    );

    // Check initial borrow
    let initial_borrow = state.borrow_amount_for_token(borrower_nonce, USDC_TOKEN);
    println!("Initial borrow (1 unit): {}", initial_borrow);

    // Advance time by 1 year to accrue interest
    state.change_timestamp(SECONDS_PER_YEAR);

    // Trigger interest accrual with a tiny supply to update indexes
    state.supply_asset_den(
        &borrower,
        USDC_TOKEN,
        BigUint::from(1u64), // Add 1 unit to trigger update
        OptionalValue::Some(borrower_nonce),
        OptionalValue::None,
        false,
    );

    // Check borrow after interest
    let borrow_after_interest = state.borrow_amount_for_token(borrower_nonce, USDC_TOKEN);
    println!("Borrow after 1 year: {}", borrow_after_interest);

    // Calculate interest accrued
    let interest_units = borrow_after_interest.into_raw_units() - initial_borrow.into_raw_units();
    println!(
        "Interest accrued on 0.000001 USDC over 1 year: {:?} units",
        interest_units
    );

    // Assert: Verify minimal borrow behavior
    assert_eq!(
        initial_borrow.into_raw_units().clone(),
        BigUint::from(1u64),
        "Initial borrow should be exactly 1 unit"
    );
    // With correct time units (seconds), interest on 1 unit is minimal
    assert!(
        borrow_after_interest.into_raw_units() <= &BigUint::from(2u64),
        "1 unit borrow should have minimal interest after 1 year"
    );
    assert!(
        interest_units <= BigUint::from(1u64),
        "Interest on 1 unit should be 0 or 1 unit max"
    );

    // Repay the full debt
    state.repay_asset_deno(
        &borrower,
        &USDC_TOKEN,
        borrow_after_interest.into_raw_units().clone(),
        borrower_nonce,
    );

    println!("\n=== Testing larger borrow with rounding ===");

    // Borrow $1.333333 (which has repeating decimals)
    // In units: 1.333333 * 1_000_000 = 1_333_333 units
    state.borrow_asset_den(
        &borrower,
        USDC_TOKEN,
        BigUint::from(1_333_333u64), // $1.333333
        borrower_nonce,
    );

    let borrow_with_decimals = state.borrow_amount_for_token(borrower_nonce, USDC_TOKEN);
    println!("Borrow amount with decimals: {}", borrow_with_decimals);

    // Assert: Verify exact decimal handling
    assert_eq!(
        borrow_with_decimals.into_raw_units().clone(),
        BigUint::from(1_333_333u64),
        "Borrow with decimals should be exactly 1,333,333 units"
    );

    // Advance time for interest
    state.change_timestamp(SECONDS_PER_YEAR + SECONDS_PER_DAY); // 1 year + 1 day

    // Trigger update
    state.supply_asset_den(
        &borrower,
        USDC_TOKEN,
        BigUint::from(1u64),
        OptionalValue::Some(borrower_nonce),
        OptionalValue::None,
        false,
    );

    let final_borrow = state.borrow_amount_for_token(borrower_nonce, USDC_TOKEN);
    println!("Final borrow after interest: {}", final_borrow);

    // Assert: Verify interest calculation on decimal amounts
    assert!(
        final_borrow.into_raw_units().clone() > BigUint::from(1_333_333u64),
        "Borrow should have accrued interest"
    );
    assert!(
        final_borrow.into_raw_units().clone() <= BigUint::from(1_400_000u64),
        "Interest should be reasonable (< 5% for ~1 year)"
    );

    // Check protocol state
    let reserves = state.market_reserves(state.usdc_market.clone());
    let revenue = state.market_revenue(state.usdc_market.clone());

    println!("\n=== Protocol State ===");
    println!("Reserves: {}", reserves);
    println!("Revenue: {}", revenue);

    // Key insight: Even with minimum amounts and complex decimals,
    // the RAY precision (27 decimals) ensures accurate calculations
    // with no exploitable rounding errors.
}
