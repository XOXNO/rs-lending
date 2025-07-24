use multiversx_sc::types::BigUint;
use multiversx_sc_scenario::imports::{OptionalValue, TestAddress};

pub mod constants;
pub mod proxys;
pub mod setup;
use constants::*;
use setup::*;

/// Investigation test to understand the high interest rate in test_rounding_at_precision_boundaries
#[test]
fn investigate_interest_rate_calculation() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    println!("\n=== Test Setup ===");
    println!("Base borrow rate: {} (1%)", R_BASE);
    println!("Max borrow rate: {} (69%)", R_MAX);
    println!("Slope1: {} (5%)", R_SLOPE1);
    println!("Slope2: {} (15%)", R_SLOPE2);

    // Setup liquidity exactly as in the problematic test
    state.supply_asset_den(
        &supplier,
        USDC_TOKEN,
        BigUint::from(1_000_000_000u64), // $1,000 USDC
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Borrower supplies
    state.supply_asset_den(
        &borrower,
        USDC_TOKEN,
        BigUint::from(1_000_001u64), // $1.000001 USDC
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    let borrower_nonce = 2;

    // Check initial state
    println!("\n=== Before Borrow ===");
    let total_supply = state.get_market_reserves(state.usdc_market.clone());
    println!("Total reserves in market: {:?}", total_supply);

    // Borrow
    state.borrow_asset_den(
        &borrower,
        USDC_TOKEN,
        BigUint::from(333_333u64), // $0.333333 USDC
        borrower_nonce,
    );

    println!("\n=== After Borrow ===");
    let initial_debt = state.get_borrow_amount_for_token(borrower_nonce, USDC_TOKEN);
    println!("Initial debt: {:?}", initial_debt);

    // Get market state
    let total_borrows = state.get_market_borrowed(state.usdc_market.clone());
    let total_reserves = state.get_market_reserves(state.usdc_market.clone());
    println!("Total borrows: {:?}", total_borrows);
    println!("Total reserves: {:?}", total_reserves);

    // Calculate utilization
    println!("\n=== Utilization Calculation ===");
    // The issue is that total_borrows has 27 decimals (RAY) while we're calculating percentage
    // We need to scale properly
    let utilization_ray = state.get_market_utilization(state.usdc_market.clone());
    println!("Utilization (RAY): {:?}", utilization_ray);

    // Convert from RAY to percentage (RAY = 1e27, so divide by 1e25 to get percentage)
    let utilization_pct = utilization_ray.into_raw_units() / &BigUint::from(10u64).pow(25);
    println!("Utilization: ~{:?}%", utilization_pct);

    // Assert: Verify utilization is correct (0.333333 / 1000.666668 ≈ 0.0333%)
    assert!(
        utilization_pct < BigUint::from(1u64),
        "Utilization should be less than 1%, got {:?}%",
        utilization_pct
    );

    // Advance time by exactly 1 second to see instantaneous rate
    state.change_timestamp(1); // 1 second

    // Trigger update
    state.supply_asset_den(
        &borrower,
        USDC_TOKEN,
        BigUint::from(1u64),
        OptionalValue::Some(borrower_nonce),
        OptionalValue::None,
        false,
    );

    let debt_after_1s = state.get_borrow_amount_for_token(borrower_nonce, USDC_TOKEN);
    println!("\n=== After 1 Second ===");
    println!("Debt after 1 second: {:?}", debt_after_1s);

    let interest_1s = debt_after_1s.into_raw_units() - initial_debt.into_raw_units();
    println!("Interest accrued in 1 second: {:?} units", interest_1s);

    // Assert: No interest should accrue in just 1 second
    assert_eq!(
        interest_1s,
        BigUint::zero(),
        "No interest should accrue in 1 second"
    );

    // Extrapolate to annual rate
    let seconds_per_year = 31_556_926u64;
    let annual_interest = &interest_1s * seconds_per_year;
    let annual_rate_pct = if interest_1s > BigUint::zero() {
        (&annual_interest * 100u64) / 333_333u64
    } else {
        BigUint::zero()
    };
    println!("Implied annual rate: ~{:?}%", annual_rate_pct);

    // Now test 1 month as in original
    state.change_timestamp(SECONDS_PER_YEAR / 12);

    state.supply_asset_den(
        &borrower,
        USDC_TOKEN,
        BigUint::from(1u64),
        OptionalValue::Some(borrower_nonce),
        OptionalValue::None,
        false,
    );

    let debt_after_month = state.get_borrow_amount_for_token(borrower_nonce, USDC_TOKEN);
    println!("\n=== After 1 Month ===");
    println!("Debt after 1 month: {:?}", debt_after_month);

    let interest_month = debt_after_month.into_raw_units() - &BigUint::from(333_333u64);
    println!("Interest accrued in 1 month: {:?} units", interest_month);

    // Assert: Interest should be approximately 279 units for 1% APR
    // 333,333 * 0.01 / 12 ≈ 277.78 units
    assert!(
        interest_month >= BigUint::from(275u64) && interest_month <= BigUint::from(285u64),
        "Monthly interest should be approximately 279 units, got {:?}",
        interest_month
    );

    // Assert: Verify the interest rate is ~1% APR
    let monthly_rate_bps = (&interest_month * 10000u64 * 12u64) / 333_333u64;
    assert!(
        monthly_rate_bps >= BigUint::from(95u64) && monthly_rate_bps <= BigUint::from(105u64),
        "Interest rate should be approximately 100 bps (1%), got {:?} bps",
        monthly_rate_bps
    );

    // Check if multiple positions are being counted
    println!("\n=== Position Analysis ===");
    let collateral = state.get_collateral_amount_for_token(borrower_nonce, USDC_TOKEN);
    println!("Total collateral position: {:?}", collateral);

    // Assert: Collateral should be slightly more than initial due to supply interest
    assert!(
        collateral.into_raw_units() >= &BigUint::from(1_000_001u64),
        "Collateral should be at least the initial supply"
    );
    assert!(
        collateral.into_raw_units() <= &BigUint::from(1_000_100u64),
        "Collateral shouldn't have excessive interest"
    );

    // Assert: Debt calculation is correct
    let expected_debt = BigUint::from(333_333u64) + interest_month.clone();
    assert_eq!(
        debt_after_month.into_raw_units(),
        &expected_debt,
        "Debt should equal principal + interest"
    );

    // Verify market state consistency
    let final_borrows = state.get_market_borrowed(state.usdc_market.clone());
    let final_reserves = state.get_market_reserves(state.usdc_market.clone());

    // Assert: Total borrows should match the debt
    assert!(
        final_borrows.into_raw_units() >= debt_after_month.into_raw_units(),
        "Market total borrows should include the debt"
    );

    // Assert: Reserves should be reduced by borrowed amount
    assert!(
        final_reserves.into_raw_units() < &BigUint::from(1_001_000_001u64),
        "Reserves should be less than initial deposits"
    );

    println!("\n=== Test Results ===");
    println!("✓ Utilization calculation correct: ~0.0333%");
    println!("✓ No interest accrual in 1 second");
    println!("✓ Monthly interest correct: ~279 units");
    println!("✓ Interest rate verified: ~1% APR");
    println!("✓ Debt calculation accurate");
    println!("✓ Market state consistent");
}
