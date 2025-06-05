use multiversx_sc::types::{EgldOrEsdtTokenIdentifier, ManagedDecimal, MultiValueEncoded};
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

    // Borrower takes 50 EGLD loan
    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(50u64),
        2,
        EGLD_DECIMALS,
    );

    // Verify initial debt and collateral positions
    let initial_debt = state.get_borrow_amount_for_token(2, EGLD_TOKEN);
    let collateral = state.get_collateral_amount_for_token(2, USDC_TOKEN);

    assert!(initial_debt > ManagedDecimal::from_raw_units(BigUint::zero(), EGLD_DECIMALS));
    assert!(collateral > ManagedDecimal::from_raw_units(BigUint::zero(), USDC_DECIMALS));

    // Advance 10 days to accumulate interest
    state.change_timestamp(SECONDS_PER_DAY * 10);
    
    // Update market indexes to reflect interest
    let mut markets = MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN));
    markets.push(EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN));
    state.update_markets(&borrower, markets.clone());
    
    // Verify debt increased due to interest
    let debt_with_interest = state.get_borrow_amount_for_token(2, EGLD_TOKEN);
    assert!(debt_with_interest > initial_debt);

    // Repay 51 EGLD (initial 50 + extra to cover interest and overpay)
    state.repay_asset(
        &borrower,
        &EGLD_TOKEN,
        BigUint::from(51u64),
        2,
        EGLD_DECIMALS,
    );
    
    // Verify debt position was fully cleared
    let custom_error_message = format!("Token not existing in the account {}", EGLD_TOKEN.as_str());
    state.get_borrow_amount_for_token_non_existing(2, EGLD_TOKEN, custom_error_message.as_bytes());
}
