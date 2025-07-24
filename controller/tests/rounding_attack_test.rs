use controller::ERROR_HEALTH_FACTOR_WITHDRAW;
use multiversx_sc::types::BigUint;
use multiversx_sc_scenario::imports::{OptionalValue, TestAddress};

pub mod constants;
pub mod proxys;
pub mod setup;
use constants::*;
use setup::*;

/// Tests potential rounding exploitation with minimum amounts.
///
/// Attempts to exploit symmetric half-up rounding by:
/// 1. Supplying very small amounts of collateral
/// 2. Borrowing minimum amounts repeatedly
/// 3. Waiting for interest to accrue
/// 4. Checking if attacker can extract value through rounding errors
#[test]
fn test_rounding_attack_with_minimum_amounts() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let attacker = TestAddress::new("attacker");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, attacker);

    // Supplier provides liquidity to enable borrowing
    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(1_000_000u64), // $1,000,000 USDC (with 6 decimals)
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Attacker supplies minimal collateral
    // 0.01 USDC = 10,000 units (with 6 decimals)
    let min_supply = BigUint::from(10_000u64);
    state.supply_asset_den(
        &attacker,
        USDC_TOKEN,
        min_supply.clone(),
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    let attacker_account_nonce = 2;

    // Get initial collateral balance
    let initial_collateral =
        state.get_collateral_amount_for_token(attacker_account_nonce, USDC_TOKEN);
    println!("Initial collateral: {:?}", initial_collateral);

    // Try to borrow the smallest possible amount (1 unit = 0.000001 USDC)
    // With only 0.01 USDC collateral and 75% LTV, max borrow is ~0.0075 USDC
    // But 1 unit = 0.000001 USDC should work
    state.borrow_asset_den(
        &attacker,
        USDC_TOKEN,
        BigUint::from(1u64), // 0.000001 USDC
        attacker_account_nonce,
    );

    // Record initial borrow amount
    let initial_borrow = state.get_borrow_amount_for_token(attacker_account_nonce, USDC_TOKEN);
    println!("Initial borrow amount: {:?}", initial_borrow);

    // Fast forward time to accrue interest
    // 1 year = 31_556_926 seconds
    let one_year_s = SECONDS_PER_YEAR;
    state.change_timestamp(one_year_s);

    // Force interest accrual by performing a small operation
    state.supply_asset_den(
        &attacker,
        USDC_TOKEN,
        BigUint::from(1u64), // Tiny supply to trigger update
        OptionalValue::Some(attacker_account_nonce),
        OptionalValue::None,
        false,
    );

    // Check borrow amount after interest
    let borrow_after_interest =
        state.get_borrow_amount_for_token(attacker_account_nonce, USDC_TOKEN);
    println!("Borrow amount after 1 year: {:?}", borrow_after_interest);

    // Calculate interest accrued
    let interest_raw = borrow_after_interest.into_raw_units() - initial_borrow.into_raw_units();
    println!("Interest accrued (raw units): {:?}", interest_raw);

    // Try to withdraw all collateral (should fail due to outstanding debt)
    let total_collateral =
        state.get_collateral_amount_for_token(attacker_account_nonce, USDC_TOKEN);
    state.withdraw_asset_error(
        &attacker,
        USDC_TOKEN,
        total_collateral.into_raw_units().clone(),
        attacker_account_nonce,
        USDC_DECIMALS,
        ERROR_HEALTH_FACTOR_WITHDRAW,
    );

    // Repay the debt
    state.repay_asset_deno(
        &attacker,
        &USDC_TOKEN,
        borrow_after_interest.into_raw_units().clone(),
        attacker_account_nonce,
    );

    // Now withdraw all collateral
    let final_collateral =
        state.get_collateral_amount_for_token(attacker_account_nonce, USDC_TOKEN);
    state.withdraw_asset_den(
        &attacker,
        USDC_TOKEN,
        final_collateral.into_raw_units().clone(),
        attacker_account_nonce,
    );

    // Analysis: Check if attacker gained or lost value
    // Initial: supplied 0.01 USDC (10,000 units)
    // Borrowed: 0.000001 USDC (1 unit)
    // Paid back: 0.001477+ USDC (~1477 units)

    // Assertions
    assert_eq!(
        initial_collateral.into_raw_units().clone(),
        BigUint::from(10_000u64),
        "Initial collateral should be 10,000 units"
    );

    assert_eq!(
        initial_borrow.into_raw_units().clone(),
        BigUint::from(1u64),
        "Initial borrow should be 1 unit"
    );

    // With correct time units, interest on 1 unit might be 0 or very small
    // 1 unit * 1% APR = 0.01 units per year, which rounds to 0
    assert!(
        interest_raw <= BigUint::from(1u64),
        "Interest on 1 unit should be 0 or 1 unit max, got {:?}",
        interest_raw
    );

    // Final collateral should be slightly more than initial due to supply interest
    assert!(
        final_collateral.into_raw_units() >= &BigUint::from(10_000u64),
        "Final collateral should be at least initial amount"
    );
}

/// Tests multiple rapid small transactions to accumulate rounding errors
#[test]
fn test_rapid_small_transaction_attack() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let attacker = TestAddress::new("attacker");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, attacker);

    // Setup liquidity
    state.supply_asset_den(
        &supplier,
        USDC_TOKEN,
        BigUint::from(10_000_000_000u64), // $10,000 USDC (10,000 * 1e6)
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Attacker supplies collateral
    let collateral_amount = BigUint::from(100_000_000u64); // $100 USDC
    state.supply_asset_den(
        &attacker,
        USDC_TOKEN,
        collateral_amount.clone(),
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    let attacker_account_nonce = 2;
    let mut total_borrowed = BigUint::zero();
    let mut total_repaid = BigUint::zero();

    // Perform 100 small borrow-repay cycles
    for i in 0..100 {
        // Small time advancement to simulate realistic transaction spacing
        state.change_timestamp(i); // 1 second between transactions

        // Borrow 0.01 USDC
        let borrow_amount = BigUint::from(10_000u64); // 0.01 USDC
        state.borrow_asset_den(
            &attacker,
            USDC_TOKEN,
            borrow_amount.clone(),
            attacker_account_nonce,
        );
        total_borrowed += &borrow_amount;

        // Immediately repay
        let debt = state.get_borrow_amount_for_token(attacker_account_nonce, USDC_TOKEN);
        state.repay_asset_deno(
            &attacker,
            &USDC_TOKEN,
            debt.into_raw_units().clone(),
            attacker_account_nonce,
        );
        total_repaid += debt.into_raw_units();
    }

    let net_cost = &total_repaid - &total_borrowed;

    println!("Total borrowed: {:?}", total_borrowed);
    println!("Total repaid: {:?}", total_repaid);
    println!("Net cost: {:?}", net_cost);

    // The net cost represents interest paid + any rounding effects
    // With RAY precision (27 decimals) and USDC having only 6 decimals,
    // any rounding errors are absorbed in the 21-decimal buffer

    // Assertions
    assert_eq!(
        total_borrowed,
        BigUint::from(1_000_000u64), // 100 * 10,000
        "Total borrowed should be 1,000,000 units"
    );

    assert_eq!(
        total_repaid,
        BigUint::from(1_000_000u64),
        "Total repaid should equal total borrowed (no interest in rapid cycles)"
    );

    assert_eq!(
        net_cost,
        BigUint::zero(),
        "Net cost should be zero for rapid borrow-repay cycles"
    );
}

/// Tests edge case with exact threshold amounts
#[test]
fn test_rounding_at_precision_boundaries() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Setup liquidity
    state.supply_asset_den(
        &supplier,
        USDC_TOKEN,
        BigUint::from(1_000_000_000u64), // $1,000 USDC
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Supply amount that might trigger rounding: 1.000001 USDC
    let supply_amount = BigUint::from(1_000_001u64);
    state.supply_asset_den(
        &borrower,
        USDC_TOKEN,
        supply_amount.clone(),
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    let borrower_account_nonce = 2;

    // Borrow an amount that creates complex division: 0.333333 USDC
    let borrow_amount = BigUint::from(333_333u64);
    state.borrow_asset_den(&borrower, USDC_TOKEN, borrow_amount, borrower_account_nonce);

    let utilisation = state.get_market_utilization(state.usdc_market.clone());
    println!("Utilisation: {:?}", utilisation);

    // Check initial borrow
    let initial_debt = state.get_borrow_amount_for_token(borrower_account_nonce, USDC_TOKEN);
    println!("Initial debt after borrow: {:?}", initial_debt);

    // Also check total deposits before interest accrual
    let initial_collateral =
        state.get_collateral_amount_for_token(borrower_account_nonce, USDC_TOKEN);
    println!("Initial collateral: {:?}", initial_collateral);

    // Debug: Check what happens after the tiny supply
    println!("Time advanced: {} seconds (1 month)", SECONDS_PER_YEAR / 12);

    // Advance time for interest calculation
    state.change_timestamp(SECONDS_PER_YEAR / 12); // 1 month in seconds

    // Trigger interest update
    println!("Before tiny supply - checking positions...");
    let debt_before_supply = state.get_borrow_amount_for_token(borrower_account_nonce, USDC_TOKEN);
    let collateral_before_supply =
        state.get_collateral_amount_for_token(borrower_account_nonce, USDC_TOKEN);
    println!("Debt before supply: {:?}", debt_before_supply);
    println!("Collateral before supply: {:?}", collateral_before_supply);

    state.supply_asset_den(
        &borrower,
        USDC_TOKEN,
        BigUint::from(1u64),
        OptionalValue::Some(borrower_account_nonce),
        OptionalValue::None,
        false,
    );

    println!("After tiny supply - positions updated");

    // Get updated positions
    let collateral = state.get_collateral_amount_for_token(borrower_account_nonce, USDC_TOKEN);
    let debt = state.get_borrow_amount_for_token(borrower_account_nonce, USDC_TOKEN);

    println!("Collateral after 1 month: {:?}", collateral);
    println!("Debt after 1 month: {:?}", debt);

    // Debug: Check if this is showing multiple positions
    println!(
        "Expected debt growth (1 month @ ~3% APR): ~{}",
        333_333u64 + (333_333u64 * 3 / 100 / 12)
    );

    // Debug: Check the actual interest calculation
    let interest_accrued = debt.into_raw_units() - &BigUint::from(333_333u64);
    println!("Interest accrued: {:?} units", interest_accrued);
    let interest_rate = (interest_accrued.clone() * 100u64 * 12u64) / 333_333u64;
    println!("Interest rate implied: ~{:?}% annualized", interest_rate);

    // Even with complex fractions and time-based calculations,
    // the RAY precision ensures rounding errors are negligible

    // Assertions
    assert_eq!(
        supply_amount,
        BigUint::from(1_000_001u64),
        "Initial supply should be 1,000,001 units"
    );

    // Collateral should have gained some interest (supply APY)
    assert!(
        collateral.into_raw_units() > &BigUint::from(1_000_001u64),
        "Collateral should have gained interest"
    );
    assert!(
        collateral.into_raw_units() < &BigUint::from(1_001_000u64),
        "Collateral interest should be reasonable (< 0.1% per month)"
    );

    // Debt should be original borrow + ~1% annual interest for 1 month
    // Expected: 333,333 * (1 + 0.01/12) ≈ 333,611 units
    assert!(
        debt.into_raw_units() > &BigUint::from(333_333u64),
        "Debt should have accrued interest"
    );
    assert!(
        debt.into_raw_units() < &BigUint::from(334_000u64),
        "Debt interest should be ~1% annualized (< 0.1% per month)"
    );

    // Verify the interest rate is reasonable
    assert_eq!(
        interest_rate,
        BigUint::from(1u64),
        "Interest rate should be ~1% annualized at low utilization"
    );
}
