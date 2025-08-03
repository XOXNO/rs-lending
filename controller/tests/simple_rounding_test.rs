use multiversx_sc::types::BigUint;
use multiversx_sc_scenario::imports::{OptionalValue, TestAddress};

pub mod constants;
pub mod proxys;
pub mod setup;
use constants::*;
use setup::*;

/// Demonstrates that rounding with RAY precision has negligible impact.
/// Shows that even with minimum amounts and repeated operations,
/// the protocol's mathematical precision prevents exploitation.
#[test]
fn test_rounding_behavior_with_ray_precision() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Supplier provides liquidity
    state.supply_asset_den(
        &supplier,
        USDC_TOKEN,
        BigUint::from(10_000_000_000u64), // $10,000 USDC (6 decimals)
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Borrower supplies collateral
    state.supply_asset_den(
        &borrower,
        USDC_TOKEN,
        BigUint::from(100_000_000u64), // $100 USDC
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    let borrower_nonce = 2;

    // Test 1: Borrow minimum amount (1 unit = $0.000001)
    println!("\n=== Test 1: Minimum Borrow ===");
    state.borrow_asset_den(
        &borrower,
        USDC_TOKEN,
        BigUint::from(1u64), // 0.000001 USDC
        borrower_nonce,
    );

    let initial_borrow = state.borrow_amount_for_token(borrower_nonce, USDC_TOKEN);
    println!("Initial borrow: {} USDC", initial_borrow);

    // Advance time by 1 year
    state.change_timestamp(SECONDS_PER_YEAR);

    // Trigger interest update
    state.supply_asset_den(
        &borrower,
        USDC_TOKEN,
        BigUint::from(1u64),
        OptionalValue::Some(borrower_nonce),
        OptionalValue::None,
        false,
    );

    let borrow_after_year = state.borrow_amount_for_token(borrower_nonce, USDC_TOKEN);
    println!("Borrow after 1 year: {} USDC", borrow_after_year);

    // Calculate interest on minimal amount
    let interest_wei = borrow_after_year.into_raw_units() - initial_borrow.into_raw_units();
    println!("Interest on $0.000001 over 1 year: {:?} wei", interest_wei);

    // Assert Test 1: Verify interest calculation on dust amount
    assert_eq!(
        initial_borrow.into_raw_units().clone(),
        BigUint::from(1u64),
        "Initial borrow should be exactly 1 unit"
    );
    // With seconds (not milliseconds), 1 unit * 1% APR = 0.01 units/year → rounds to 0
    assert!(
        borrow_after_year.into_raw_units() <= &BigUint::from(2u64),
        "After 1 year, 1 unit should have minimal or no interest"
    );
    assert!(
        interest_wei <= BigUint::from(1u64),
        "Interest on 1 unit should be 0 or 1 unit max"
    );

    // Repay
    state.repay_asset(
        &borrower,
        &USDC_TOKEN,
        borrow_after_year.into_raw_units().clone(),
        borrower_nonce,
        USDC_DECIMALS,
    );

    // Test 2: Multiple small operations
    println!("\n=== Test 2: 100 Small Borrow-Repay Cycles ===");
    let mut total_interest_paid = BigUint::zero();

    for cycle_number in 0..100 {
        // Advance time slightly
        state.change_timestamp(SECONDS_PER_YEAR + (cycle_number + 1) * 3600); // +1 hour each iteration

        // Borrow $1 USDC
        state.borrow_asset_den(
            &borrower,
            USDC_TOKEN,
            BigUint::from(1_000_000u64), // $1 USDC
            borrower_nonce,
        );

        let debt = state.borrow_amount_for_token(borrower_nonce, USDC_TOKEN);

        // Immediately repay
        state.repay_asset_deno(
            &borrower,
            &USDC_TOKEN,
            debt.into_raw_units().clone(),
            borrower_nonce,
        );

        // Track interest paid (debt - principal)
        let interest = debt.into_raw_units().clone() - BigUint::from(1_000_000u64);
        total_interest_paid += interest;
    }

    println!(
        "Total interest paid over 100 cycles: {:?} wei",
        total_interest_paid
    );
    println!(
        "Average interest per cycle: {:?} wei",
        if total_interest_paid.clone() > BigUint::zero() {
            total_interest_paid.clone() / 100u64
        } else {
            BigUint::zero()
        }
    );

    // Assert Test 2: Verify no interest accumulation in same-block operations
    assert_eq!(
        total_interest_paid.clone(),
        BigUint::zero(),
        "Total interest should be 0 for same-block borrow-repay cycles"
    );

    // Test 3: Check final state
    println!("\n=== Test 3: Final State Check ===");
    let final_collateral = state.collateral_amount_for_token(borrower_nonce, USDC_TOKEN);
    let supplier_balance = state.collateral_amount_for_token(1, USDC_TOKEN);

    println!("Borrower final collateral: {} USDC", final_collateral);
    println!("Supplier balance: {} USDC", supplier_balance);

    // Calculate protocol's total value
    let reserves = state.market_reserves(state.usdc_market.clone());
    let revenue = state.market_revenue(state.usdc_market.clone());

    println!("Protocol reserves: {} USDC", reserves);
    println!("Protocol revenue: {} USDC", revenue);

    // Assert Test 3: Verify final state consistency
    // Borrower should have slightly more than initial due to supply interest
    assert!(
        final_collateral.into_raw_units() >= &BigUint::from(100_000_000u64),
        "Borrower collateral should be at least initial amount"
    );
    assert!(
        final_collateral.into_raw_units() <= &BigUint::from(100_000_020u64),
        "Borrower collateral gain should be minimal (< 20 units)"
    );

    // Supplier should have gained interest
    assert!(
        supplier_balance.into_raw_units() >= &BigUint::from(10_000_000_000u64),
        "Supplier balance should be at least initial amount"
    );
    assert!(
        supplier_balance.into_raw_units() <= &BigUint::from(10_000_002_000u64),
        "Supplier interest gain should be reasonable (< 2000 units)"
    );

    // Protocol revenue might be zero with minimal interest
    assert!(
        revenue.into_raw_units() <= &BigUint::from(1_000u64),
        "Protocol revenue should be minimal (< 0.001 USDC)"
    );

    // Reserves should equal total deposits plus minimal interest
    let expected_reserves = BigUint::from(10_100_000_000u64);
    assert!(
        reserves.into_raw_units() >= &expected_reserves,
        "Reserves should include all deposits"
    );
    assert!(
        reserves.into_raw_units() <= &BigUint::from(10_100_001_000u64),
        "Reserves shouldn't have excessive interest"
    );

    // Key insight: With RAY precision (27 decimals) and USDC having 6 decimals,
    // there's a 21-decimal buffer that absorbs any rounding effects.
    // Even after 100 operations, rounding impact is negligible.
}
