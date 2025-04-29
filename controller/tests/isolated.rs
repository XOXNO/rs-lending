use controller::{
    ERROR_ASSET_NOT_SUPPORTED_AS_COLLATERAL, ERROR_DEBT_CEILING_REACHED,
    ERROR_EMODE_CATEGORY_NOT_FOUND, ERROR_MIX_ISOLATED_COLLATERAL, WAD_PRECISION,
};
use multiversx_sc::types::ManagedDecimal;
use multiversx_sc_scenario::imports::{BigUint, OptionalValue, TestAddress};
pub mod constants;
pub mod proxys;
pub mod setup;
use constants::*;
use setup::*;

// Isolation Tests
#[test]
fn test_supply_isolated_asset_with_e_mode_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_accounts(&mut state, supplier, borrower);
    // Test supply
    state.supply_asset(
        &supplier,
        XEGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::Some(1),
        false,
    );

    // Test borrow
    state.supply_asset_error(
        &supplier,
        ISOLATED_TOKEN,
        BigUint::from(100u64),
        ISOLATED_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::None,
        false,
        ERROR_EMODE_CATEGORY_NOT_FOUND, // ERROR_CANNOT_USE_EMODE_WITH_ISOLATED_ASSETS,
    );
}

#[test]
fn test_mix_isolated_collateral_with_others_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_accounts(&mut state, supplier, borrower);
    state.supply_asset(
        &supplier,
        ISOLATED_TOKEN,
        BigUint::from(100u64),
        ISOLATED_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Supply non isolated asset
    state.supply_asset_error(
        &supplier,
        XEGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::None,
        false,
        ERROR_MIX_ISOLATED_COLLATERAL,
    );
}

#[test]
fn test_borrow_asset_as_isolated_debt_celling_case() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_accounts(&mut state, supplier, borrower);

    // First supply a normal asset not siloed
    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(1000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.supply_asset(
        &borrower,
        ISOLATED_TOKEN,
        BigUint::from(100u64), // $500 deposit
        ISOLATED_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Then borrow the siloed asset
    state.borrow_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(100u64), // $100 borrow
        2,
        USDC_DECIMALS,
    );
    let total_egld_borrow = state.get_borrow_amount_for_token(2, USDC_TOKEN);
    println!("total_Egld_borrow: {:?}", total_egld_borrow);
    let borrow_amount = state.get_used_isolated_asset_debt_usd(&ISOLATED_TOKEN);
    println!("borrow_amount: {:?}", borrow_amount);
    assert!(borrow_amount > ManagedDecimal::from_raw_units(BigUint::zero(), ISOLATED_DECIMALS));

    state.repay_asset(
        &borrower,
        &USDC_TOKEN,
        // Over repay
        BigUint::from(1000u64), // $1000 borrow
        2,
        USDC_DECIMALS,
    );
    let borrow_amount = state.get_used_isolated_asset_debt_usd(&ISOLATED_TOKEN);
    println!("borrow_amount: {:?}", borrow_amount);
    assert!(borrow_amount == ManagedDecimal::from_raw_units(BigUint::zero(), ISOLATED_DECIMALS));
}

#[test]
fn test_borrow_asset_as_isolated_debt_celling_case_error_limit_reached() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_accounts(&mut state, supplier, borrower);

    // First supply a normal asset not siloed
    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(1005u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.supply_asset(
        &borrower,
        ISOLATED_TOKEN,
        BigUint::from(1000u64), // $5000 deposit
        ISOLATED_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Then borrow the siloed asset
    state.borrow_asset_error(
        &borrower,
        USDC_TOKEN,
        BigUint::from(1001u64), // $1001 borrow
        2,
        USDC_DECIMALS,
        ERROR_DEBT_CEILING_REACHED,
    );
}

#[test]
fn test_borrow_asset_as_isolated_debt_celling_case_with_debt_interest() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_accounts(&mut state, supplier, borrower);

    // First supply a normal asset not siloed
    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(1000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.supply_asset(
        &borrower,
        ISOLATED_TOKEN,
        BigUint::from(100u64), // $500 deposit
        ISOLATED_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Then borrow the siloed asset
    state.borrow_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(100u64), // $100 borrow
        2,
        USDC_DECIMALS,
    );

    let borrow_amount = state.get_used_isolated_asset_debt_usd(&ISOLATED_TOKEN);

    println!("Borrow {:?}", borrow_amount);
    assert!(borrow_amount > ManagedDecimal::from_raw_units(BigUint::zero(), ISOLATED_DECIMALS));
    state.change_timestamp(SECONDS_PER_DAY);

    state.update_account_positions(&borrower, 2);
    let borrow_amount = state.get_borrow_amount_for_token(2, USDC_TOKEN);
    println!("Borrow {:?}", borrow_amount);
    state.repay_asset(
        &borrower,
        &USDC_TOKEN,
        BigUint::from(90u64), // $100 borrow
        2,
        USDC_DECIMALS,
    );
    let borrow_amount = state.get_used_isolated_asset_debt_usd(&ISOLATED_TOKEN);

    println!("Borrow {:?}", borrow_amount);
    // Higher due to interest that was paid and not counted as repaid principal asset global debt
    assert!(borrow_amount > ManagedDecimal::from_raw_units(BigUint::zero(), WAD_PRECISION));
}

#[test]
fn test_borrow_asset_as_isolated_debt_celling_liquidation_debt_paid() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");
    let liquidator = TestAddress::new("liquidator");
    setup_accounts(&mut state, supplier, borrower);
    state.world.account(liquidator).nonce(1).esdt_balance(
        USDC_TOKEN,
        BigUint::from(20000u64) * BigUint::from(10u64).pow(USDC_DECIMALS as u32),
    );

    // First supply a normal asset not siloed
    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(1000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.supply_asset(
        &borrower,
        ISOLATED_TOKEN,
        BigUint::from(200u64), // $1000 deposit
        ISOLATED_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Then borrow the siloed asset
    state.borrow_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(700u64), // $750 borrow
        2,
        USDC_DECIMALS,
    );
    let total_collateral = state.get_total_collateral_in_egld(2);
    let total_debt = state.get_borrow_amount_for_token(2, USDC_TOKEN);
    println!("total_collateral: {:?}", total_collateral);
    println!("total_debt: {:?}", total_debt);
    let borrow_amount_first = state.get_used_isolated_asset_debt_usd(&ISOLATED_TOKEN);
    println!("borrow_amount: {:?}", borrow_amount_first);
    assert!(
        borrow_amount_first > ManagedDecimal::from_raw_units(BigUint::zero(), ISOLATED_DECIMALS)
    );
    state.change_timestamp(SECONDS_PER_DAY * 1600);
    state.update_borrows_with_debt(&borrower, 2);
    let total_collateral = state.get_total_collateral_in_egld(2);
    let total_debt = state.get_borrow_amount_for_token(2, USDC_TOKEN);
    println!("total_collateral: {:?}", total_collateral);
    println!("total_debt: {:?}", total_debt);
    let health_factor = state.get_account_health_factor(2);
    println!("health_factor: {:?}", health_factor);
    state.liquidate_account(
        &liquidator,
        &USDC_TOKEN,
        BigUint::from(2300u64),
        2,
        USDC_DECIMALS,
    );
    let borrow_amount = state.get_used_isolated_asset_debt_usd(&ISOLATED_TOKEN);
    println!("borrow_amount: {:?}", borrow_amount);
    let total_debt = state.get_borrow_amount_for_token(2, USDC_TOKEN);
    println!("total_debt: {:?}", total_debt);
    let health_factor = state.get_account_health_factor(2);
    println!("health_factor: {:?}", health_factor);
    let total_collateral = state.get_total_collateral_in_egld(2);
    println!("total_collateral: {:?}", total_collateral);
    // // Higher due to interest that was paid and not counted as repaid principal asset global debt
    assert!(borrow_amount < borrow_amount_first);
    state.clean_bad_debt(2);
    let borrow_amount = state.get_used_isolated_asset_debt_usd(&ISOLATED_TOKEN);
    println!("borrow_amount: {:?}", borrow_amount);
    assert_eq!(
        borrow_amount,
        ManagedDecimal::from_raw_units(BigUint::zero(), EGLD_DECIMALS)
    )
}

#[test]
fn test_borrow_asset_as_isolated_debt_celling_under_repayment_only_interest() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_accounts(&mut state, supplier, borrower);

    // First supply a normal asset not siloed
    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(1000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.supply_asset(
        &borrower,
        ISOLATED_TOKEN,
        BigUint::from(100u64), // $500 deposit
        ISOLATED_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Then borrow the siloed asset
    state.borrow_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(100u64), // $100 borrow
        2,
        USDC_DECIMALS,
    );

    let borrow_amount_first = state.get_used_isolated_asset_debt_usd(&ISOLATED_TOKEN);

    assert!(
        borrow_amount_first > ManagedDecimal::from_raw_units(BigUint::zero(), ISOLATED_DECIMALS)
    );
    state.change_timestamp(SECONDS_PER_DAY * 500);

    state.repay_asset(
        &borrower,
        &USDC_TOKEN,
        BigUint::from(1u64), // $100 borrow
        2,
        USDC_DECIMALS,
    );
    let borrow_amount = state.get_used_isolated_asset_debt_usd(&ISOLATED_TOKEN);

    // No change due to interest that was paid and not counted as repaid principal asset global debt
    assert!(borrow_amount == borrow_amount_first);
}

#[test]
fn test_borrow_asset_not_supported_in_isolation_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_accounts(&mut state, supplier, borrower);

    // Test borrow
    state.supply_asset_error(
        &borrower,
        SEGLD_TOKEN,
        BigUint::from(100u64),
        SEGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::Some(1),
        false,
        ERROR_ASSET_NOT_SUPPORTED_AS_COLLATERAL,
    );
}
// Isolation Tests End
