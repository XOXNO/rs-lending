use multiversx_sc::types::ManagedDecimal;
use multiversx_sc_scenario::imports::{BigUint, OptionalValue, TestAddress};
pub mod constants;
pub mod proxys;
pub mod setup;
use constants::*;

use setup::*;
#[test]
fn test_repay_debt_in_full_and_extra() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    // Setup accounts
    state.world.current_block().block_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Test supply
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Test borrow
    state.supply_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(5000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(50u64),
        2,
        EGLD_DECIMALS,
    );

    // Verify amounts
    let borrowed = state.get_borrow_amount_for_token(2, EGLD_TOKEN);
    let collateral = state.get_collateral_amount_for_token(2, USDC_TOKEN);

    assert!(borrowed > ManagedDecimal::from_raw_units(BigUint::zero(), EGLD_DECIMALS));
    assert!(collateral > ManagedDecimal::from_raw_units(BigUint::zero(), USDC_DECIMALS));

    state
        .world
        .current_block()
        .block_timestamp(SECONDS_PER_DAY * 10);
    state.update_borrows_with_debt(&borrower, 2);
    let borrowed_after_10_days = state.get_borrow_amount_for_token(2, EGLD_TOKEN);

    assert!(borrowed_after_10_days > borrowed);
    println!("borrowed_after_10_days: {:?}", borrowed_after_10_days);

    // Repay debt in full + extra
    state.repay_asset(
        &borrower,
        &EGLD_TOKEN,
        BigUint::from(51u64),
        2,
        EGLD_DECIMALS,
    );
    let custom_error_message = format!("Token not existing in the account {}", EGLD_TOKEN.as_str());

    state.get_borrow_amount_for_token_non_existing(2, EGLD_TOKEN, custom_error_message.as_bytes());
}
