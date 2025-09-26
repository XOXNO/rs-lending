use controller::ERROR_HEALTH_FACTOR_WITHDRAW;
use multiversx_sc::types::BigUint;
use multiversx_sc_scenario::imports::{OptionalValue, TestAddress};

pub mod constants;
pub mod proxys;
pub mod setup;
use constants::*;
use setup::*;

/// Validates that dust-sized borrows cannot accrue exploitable interest over time.
#[test]
fn dust_borrow_interest_stays_bounded() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Seed liquidity for the market.
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
        "Supplier deposit should be recorded exactly",
    );

    // Borrower posts dust collateral (0.01 USDC) and opens a position.
    let borrower_nonce = 2_u64;
    let dust_collateral_raw = BigUint::from(10_000u64);
    state.supply_asset_den(
        &borrower,
        USDC_TOKEN,
        dust_collateral_raw.clone(),
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    state.assert_collateral_raw_eq(
        borrower_nonce,
        &USDC_TOKEN,
        dust_collateral_raw.clone(),
        "Borrower collateral should equal supplied dust amount",
    );

    // Borrow a single wei of USDC (0.000001).
    state.borrow_asset_den(&borrower, USDC_TOKEN, BigUint::from(1u64), borrower_nonce);
    state.assert_borrow_raw_eq(
        borrower_nonce,
        &USDC_TOKEN,
        BigUint::from(1u64),
        "Dust borrow should be tracked precisely",
    );

    // Advance one year and trigger interest accrual with a tiny top-up.
    state.change_timestamp(SECONDS_PER_YEAR);
    state.supply_asset_den(
        &borrower,
        USDC_TOKEN,
        BigUint::from(1u64),
        OptionalValue::Some(borrower_nonce),
        OptionalValue::None,
        false,
    );

    let debt_after_year = state.borrow_amount_for_token(borrower_nonce, USDC_TOKEN);
    state.assert_borrow_raw_within(
        borrower_nonce,
        &USDC_TOKEN,
        BigUint::from(1u64),
        BigUint::from(1u64),
        "Dust borrow should accrue at most 1 wei of interest",
    );

    // Withdrawal should still be blocked until repayment.
    state.withdraw_asset_error(
        &borrower,
        USDC_TOKEN,
        dust_collateral_raw.clone(),
        borrower_nonce,
        USDC_DECIMALS,
        ERROR_HEALTH_FACTOR_WITHDRAW,
    );

    // Repay and close the position.
    state.repay_asset_deno(
        &borrower,
        &USDC_TOKEN,
        debt_after_year.into_raw_units().clone(),
        borrower_nonce,
    );
    let final_collateral = state.collateral_amount_for_token(borrower_nonce, USDC_TOKEN);
    state.assert_collateral_raw_within(
        borrower_nonce,
        &USDC_TOKEN,
        dust_collateral_raw,
        BigUint::from(20u64),
        "Collateral should remain near the initial dust amount",
    );
    state.withdraw_asset_den(
        &borrower,
        USDC_TOKEN,
        final_collateral.into_raw_units().clone(),
        borrower_nonce,
    );
    state.assert_no_collateral_entry(borrower_nonce, &USDC_TOKEN);
}

/// Confirms that rapid borrow/repay cycles cannot accumulate rounding profits.
#[test]
fn rapid_same_block_cycles_accumulate_no_interest() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let attacker = TestAddress::new("attacker");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, attacker);

    // Provide ample liquidity so the attacker can loop.
    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(10_000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Attacker posts collateral worth 100 USDC.
    let attacker_nonce = 2_u64;
    state.supply_asset(
        &attacker,
        USDC_TOKEN,
        BigUint::from(100u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    state.assert_collateral_raw_eq(
        attacker_nonce,
        &USDC_TOKEN,
        scaled_amount(100, USDC_DECIMALS),
        "Attacker collateral should be recorded",
    );

    let mut total_borrowed = BigUint::zero();
    let mut total_repaid = BigUint::zero();

    for cycle in 0..100 {
        state.change_timestamp(cycle as u64); // simulate sequential transactions
        let borrow_raw = BigUint::from(10_000u64); // 0.01 USDC

        state.borrow_asset_den(&attacker, USDC_TOKEN, borrow_raw.clone(), attacker_nonce);
        let debt = state.borrow_amount_for_token(attacker_nonce, USDC_TOKEN);
        state.repay_asset_deno(
            &attacker,
            &USDC_TOKEN,
            debt.into_raw_units().clone(),
            attacker_nonce,
        );

        total_borrowed += borrow_raw;
        total_repaid += debt.into_raw_units();
    }

    assert_eq!(
        total_borrowed,
        BigUint::from(1_000_000u64),
        "Borrowed principal should equal 100 * 0.01 USDC",
    );
    assert!(
        total_repaid <= BigUint::from(1_000_010u64),
        "Repayments should differ from principal by at most 10 wei of interest",
    );
    assert!(
        total_repaid >= BigUint::from(1_000_000u64),
        "Repayments should never be lower than the borrowed principal",
    );
}

/// Checks fractional amounts around precision boundaries behave within expected tolerances.
#[test]
fn fractional_interest_growth_respects_bounds() {
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
    let supply_amount_raw = BigUint::from(1_000_001u64); // 1.000001 USDC
    state.supply_asset_den(
        &borrower,
        USDC_TOKEN,
        supply_amount_raw.clone(),
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    state.assert_collateral_raw_eq(
        borrower_nonce,
        &USDC_TOKEN,
        supply_amount_raw.clone(),
        "Borrower supply should include the fractional component",
    );

    let borrow_amount_raw = BigUint::from(333_333u64);
    state.borrow_asset_den(
        &borrower,
        USDC_TOKEN,
        borrow_amount_raw.clone(),
        borrower_nonce,
    );
    state.assert_borrow_raw_eq(
        borrower_nonce,
        &USDC_TOKEN,
        borrow_amount_raw.clone(),
        "Initial borrow should equal the requested fractional amount",
    );

    state.change_timestamp(SECONDS_PER_YEAR / 12); // one month
    state.supply_asset_den(
        &borrower,
        USDC_TOKEN,
        BigUint::from(1u64),
        OptionalValue::Some(borrower_nonce),
        OptionalValue::None,
        false,
    );

    let collateral_after = state.collateral_amount_for_token(borrower_nonce, USDC_TOKEN);
    let debt_after = state.borrow_amount_for_token(borrower_nonce, USDC_TOKEN);

    state.assert_collateral_raw_within(
        borrower_nonce,
        &USDC_TOKEN,
        supply_amount_raw.clone(),
        BigUint::from(1_000u64),
        "Supply interest should grow collateral by less than 0.001 USDC",
    );
    state.assert_borrow_raw_within(
        borrower_nonce,
        &USDC_TOKEN,
        borrow_amount_raw.clone(),
        BigUint::from(700u64),
        "Debt interest over one month should stay below 700 wei (~0.0007 USDC)",
    );

    // Borrower should not be able to withdraw full collateral until debt is cleared.
    let outstanding_raw = debt_after.into_raw_units().clone();
    state.withdraw_asset_error(
        &borrower,
        USDC_TOKEN,
        supply_amount_raw.clone(),
        borrower_nonce,
        USDC_DECIMALS,
        ERROR_HEALTH_FACTOR_WITHDRAW,
    );

    // Repay and close the position to confirm balances reset cleanly.
    state.repay_asset_deno(&borrower, &USDC_TOKEN, outstanding_raw, borrower_nonce);
    state.withdraw_asset_den(
        &borrower,
        USDC_TOKEN,
        collateral_after.into_raw_units().clone(),
        borrower_nonce,
    );
    state.assert_no_collateral_entry(borrower_nonce, &USDC_TOKEN);
}
