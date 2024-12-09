use lending_pool::{errors::*, NftAccountAttributes, DECIMAL_PRECISION};
use multiversx_sc::types::ManagedDecimal;
use multiversx_sc_scenario::{
    api::StaticApi,
    imports::{BigUint, OptionalValue, TestAddress},
};

pub mod constants;
pub mod proxys;
pub mod setup;

use constants::*;
use setup::*;

// Basic Operations
#[test]
fn test_basic_supply_and_borrow() {
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

    assert!(borrowed > BigUint::zero());
    assert!(collateral > BigUint::zero());
}

#[test]
fn test_basic_supply_capped_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    // Setup accounts
    state.world.current_block().block_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);
    // Test supply
    state.supply_asset(
        &supplier,
        CAPPED_TOKEN,
        BigUint::from(1u64),
        CAPPED_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    // Test supply
    state.supply_asset_error(
        &supplier,
        CAPPED_TOKEN,
        BigUint::from(150u64),
        CAPPED_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::None,
        false,
        ERROR_SUPPLY_CAP,
    );
}

#[test]
fn test_basic_borrow_capped_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    // Setup accounts
    state.world.current_block().block_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);
    // Test supply
    state.supply_asset(
        &supplier,
        CAPPED_TOKEN,
        BigUint::from(150u64),
        CAPPED_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    // Test Borrow
    state.borrow_asset(
        &supplier,
        CAPPED_TOKEN,
        BigUint::from(1u64),
        1,
        CAPPED_DECIMALS,
    );
    // Test Borrow
    state.borrow_asset_error(
        &supplier,
        CAPPED_TOKEN,
        BigUint::from(100u64),
        1,
        CAPPED_DECIMALS,
        ERROR_BORROW_CAP,
    );
}

#[test]
fn test_complete_market_exit() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    // Setup accounts
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
    state.world.current_block().block_timestamp(6000u64);
    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(50u64),
        2,
        EGLD_DECIMALS,
    );
    // return;
    state.world.current_block().block_timestamp(8000u64);
    state.update_borrows_with_debt(&borrower, 2);
    state.update_interest_indexes(&supplier, 1);

    state
        .world
        .check_account(borrower)
        .esdt_nft_balance_and_attributes(
            ACCOUNT_TOKEN,
            2,
            BigUint::from(1u64),
            NftAccountAttributes {
                is_isolated: false,
                e_mode_category: 0,
                is_vault: false,
            },
        );
    let borrow_amount_in_dollars = state.get_borrow_amount_for_token(2, EGLD_TOKEN);
    println!("borrow_amount_in_dollars: {:?}", borrow_amount_in_dollars);

    state.repay_asset(
        &borrower,
        &EGLD_TOKEN,
        BigUint::from(57u64),
        2,
        EGLD_DECIMALS,
    );
    state.update_borrows_with_debt(&borrower, 2);
    state.update_interest_indexes(&supplier, 1);
    state.world.current_block().block_timestamp(9000u64);

    state.withdraw_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(5000u64),
        2,
        USDC_DECIMALS,
    );
    state.world.current_block().block_timestamp(10000u64);
    state.update_interest_indexes(&supplier, 1);

    state
        .world
        .check_account(borrower)
        .esdt_nft_balance_and_attributes(
            ACCOUNT_TOKEN,
            2,
            BigUint::zero(),
            NftAccountAttributes {
                is_isolated: false,
                e_mode_category: 0,
                is_vault: false,
            },
        );
    // let collateral_in_dollars = state.get_collateral_amount_for_token(1, EGLD_TOKEN);
    // println!("collateral_in_dollars: {:?}", collateral_in_dollars);

    state.withdraw_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(102u64),
        1,
        EGLD_DECIMALS,
    );
    return;
}

#[test]
fn test_interest_accrual() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    // Setup initial state
    state.world.current_block().block_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Initial supply and borrow
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    state.update_interest_indexes(&supplier, 1);
    state.supply_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(10000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    state.update_interest_indexes(&supplier, 1);

    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(50u64),
        2,
        EGLD_DECIMALS,
    );

    state.update_borrows_with_debt(&borrower, 2);
    let utilization_ratio = state.get_market_utilization(state.egld_market.clone());
    println!("utilization_ratio: {:?}", utilization_ratio);

    // Record initial amounts
    let initial_borrow = state.get_borrow_amount_for_token(2, EGLD_TOKEN);
    let initial_supply = state.get_collateral_amount_for_token(1, EGLD_TOKEN);

    // Simulate daily updates for a month
    for day in 1..=365 {
        state
            .world
            .current_block()
            .block_timestamp(SECONDS_PER_DAY * day);
        state.update_borrows_with_debt(&borrower, 2);
        state.update_interest_indexes(&supplier, 1);
    }

    // Verify interest accrual
    let final_borrow = state.get_borrow_amount_for_token(2, EGLD_TOKEN);
    let final_supply = state.get_collateral_amount_for_token(1, EGLD_TOKEN);
    println!("final_borrow: {:?}", final_borrow);
    println!("initial_borrow: {:?}", initial_borrow);
    println!("final_supply: {:?}", final_supply);
    println!("initial_supply: {:?}", initial_supply);
    assert!(final_borrow > initial_borrow);
    assert!(final_supply > initial_supply);
}

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

    assert!(borrowed > BigUint::zero());
    assert!(collateral > BigUint::zero());

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

    let borrowed_after_repay = state.get_borrow_amount_for_token(2, EGLD_TOKEN);

    assert!(borrowed_after_repay == BigUint::zero());
}

#[test]
fn test_withdrawal_with_interest() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    // Setup supplier account
    state.world.current_block().block_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Initial supply
    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(1000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Borrower supplies collateral and borrows
    state.supply_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(10u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.borrow_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(200u64),
        2,
        USDC_DECIMALS,
    );

    // Advance time to accumulate interest
    state
        .world
        .current_block()
        .block_timestamp(SECONDS_PER_DAY * 10); // 10 days

    // Update interest before withdrawal
    state.update_interest_indexes(&supplier, 1);

    // Get initial state
    let initial_collateral = state.get_collateral_amount_for_token(1, USDC_TOKEN);
    println!(
        "initial_collateral: {}",
        initial_collateral.to_u64().unwrap()
    );

    // Advance time to accumulate interest
    state
        .world
        .current_block()
        .block_timestamp(SECONDS_PER_DAY * 20); // 20 days

    // Withdraw partial amount
    state.withdraw_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(500u64),
        1,
        USDC_DECIMALS,
    );

    // Get initial state
    let final_collateral = state.get_collateral_amount_for_token(1, USDC_TOKEN);
    println!("final_collateral:   {}", final_collateral.to_u64().unwrap());
}
// Basic Operations End

// E-Mode Tests
#[test]
fn test_basic_supply_and_borrow_with_e_mode() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    // Setup accounts
    state.world.current_block().block_timestamp(0);
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
    state.supply_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::Some(1),
        false,
    );

    state.borrow_asset(
        &borrower,
        XEGLD_TOKEN,
        BigUint::from(50u64),
        2,
        EGLD_DECIMALS,
    );

    // Verify amounts
    let borrowed = state.get_borrow_amount_for_token(2, XEGLD_TOKEN);
    let collateral = state.get_collateral_amount_for_token(2, EGLD_TOKEN);

    assert!(borrowed > BigUint::zero());
    assert!(collateral > BigUint::zero());
}

#[test]
fn test_e_mode_category_not_found_at_supply_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_accounts(&mut state, supplier, borrower);
    // Test borrow
    state.supply_asset_error(
        &borrower,
        USDC_TOKEN,
        BigUint::from(100u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::Some(1),
        false,
        ERROR_EMODE_CATEGORY_NOT_FOUND,
    );
}

#[test]
fn test_e_mode_asset_not_supported_as_collateral_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_accounts(&mut state, supplier, borrower);

    // Test borrow
    state.supply_asset(
        &borrower,
        ISOLATED_TOKEN,
        BigUint::from(1000u64),
        ISOLATED_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.borrow_asset_error(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(100u64),
        1,
        EGLD_DECIMALS,
        ERROR_ASSET_NOT_BORROWABLE_IN_ISOLATION,
    );
}

#[test]
fn test_borrow_asset_not_supported_in_e_mode_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_accounts(&mut state, supplier, borrower);

    // Test borrow
    state.supply_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::Some(1),
        false,
    );

    state.borrow_asset_error(
        &borrower,
        USDC_TOKEN,
        BigUint::from(100u64),
        1,
        USDC_DECIMALS,
        ERROR_EMODE_CATEGORY_NOT_FOUND,
    );
}

#[test]
fn test_borrow_asset_not_borrowable_in_e_mode_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_accounts(&mut state, supplier, borrower);

    // Test borrow
    state.supply_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::Some(1),
        false,
    );

    state.borrow_asset_error(
        &borrower,
        LEGLD_TOKEN,
        BigUint::from(10u64),
        1,
        LEGLD_DECIMALS,
        ERROR_ASSET_NOT_BORROWABLE,
    );
}

// E-Mode Tests End

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
        ERROR_CANNOT_USE_EMODE_WITH_ISOLATED_ASSETS,
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

    let borrow_amount = state.get_used_isolated_asset_debt_usd(&ISOLATED_TOKEN);
    println!("borrow_amount: {:?}", borrow_amount);
    assert!(borrow_amount > BigUint::zero());

    state.repay_asset(
        &borrower,
        &USDC_TOKEN,
        BigUint::from(100u64), // $100 borrow
        2,
        USDC_DECIMALS,
    );
    let borrow_amount = state.get_used_isolated_asset_debt_usd(&ISOLATED_TOKEN);
    println!("borrow_amount: {:?}", borrow_amount);
    assert!(borrow_amount == BigUint::zero());
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

    assert!(borrow_amount > BigUint::zero());
    state.world.current_block().block_timestamp(SECONDS_PER_DAY);

    state.repay_asset(
        &borrower,
        &USDC_TOKEN,
        BigUint::from(100u64), // $100 borrow
        2,
        USDC_DECIMALS,
    );
    let borrow_amount = state.get_used_isolated_asset_debt_usd(&ISOLATED_TOKEN);

    // Higher due to interest that was paid and not counted as repaid principal asset global debt
    assert!(borrow_amount > BigUint::zero());
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
    let total_collateral = state.get_collateral_amount_for_token(2, ISOLATED_TOKEN);
    let total_debt = state.get_borrow_amount_for_token(2, USDC_TOKEN);
    println!("total_collateral: {:?}", total_collateral);
    println!("total_debt: {:?}", total_debt);
    let borrow_amount_first = state.get_used_isolated_asset_debt_usd(&ISOLATED_TOKEN);
    println!("borrow_amount: {:?}", borrow_amount_first);
    assert!(borrow_amount_first > BigUint::zero());
    state
        .world
        .current_block()
        .block_timestamp(SECONDS_PER_DAY * 700);
    state.update_borrows_with_debt(&borrower, 2);
    let health_factor = state.get_account_health_factor(2);
    println!("health_factor: {:?}", health_factor);
    state.liquidate_account(
        &liquidator,
        &ISOLATED_TOKEN,
        &USDC_TOKEN,
        BigUint::from(1500u64),
        2,
        USDC_DECIMALS,
    );
    let borrow_amount = state.get_used_isolated_asset_debt_usd(&ISOLATED_TOKEN);
    let total_collateral = state.get_collateral_amount_for_token(2, ISOLATED_TOKEN);
    let total_debt = state.get_borrow_amount_for_token(2, USDC_TOKEN);
    println!("total_collateral: {:?}", total_collateral);
    println!("total_debt: {:?}", total_debt);
    println!("borrow_amount: {:?}", borrow_amount);
    let health_factor = state.get_account_health_factor(2);
    println!("health_factor: {:?}", health_factor);
    // Higher due to interest that was paid and not counted as repaid principal asset global debt
    assert!(borrow_amount < borrow_amount_first);
    assert!(
        health_factor
            > ManagedDecimal::<StaticApi, usize>::from_raw_units(
                BigUint::from(BP),
                DECIMAL_PRECISION
            )
    );
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

    assert!(borrow_amount_first > BigUint::zero());
    state
        .world
        .current_block()
        .block_timestamp(SECONDS_PER_DAY * 500);

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

// Siloed Tests
#[test]
fn test_borrow_asset_as_siloed_normal_case() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_accounts(&mut state, supplier, borrower);

    // First supply a normal asset not siloed
    state.supply_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.supply_asset(
        &supplier,
        SILOED_TOKEN,
        BigUint::from(100u64),
        SILOED_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Then borrow the siloed asset
    state.borrow_asset(
        &borrower,
        SILOED_TOKEN,
        BigUint::from(1u64),
        1,
        SILOED_DECIMALS,
    );

    let borrow_amount = state.get_borrow_amount_for_token(1, SILOED_TOKEN);
    assert!(borrow_amount > BigUint::zero());
}

#[test]
fn test_borrow_asset_as_siloed_with_another_asset_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_accounts(&mut state, supplier, borrower);

    // Test borrow
    state.supply_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.supply_asset(
        &supplier,
        SILOED_TOKEN,
        BigUint::from(100u64),
        SILOED_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(100u64),
        USDC_DECIMALS,
        OptionalValue::Some(2),
        OptionalValue::None,
        false,
    );

    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(100u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(10u64),
        1,
        EGLD_DECIMALS,
    );

    state.borrow_asset_error(
        &borrower,
        SILOED_TOKEN,
        BigUint::from(1u64),
        1,
        SILOED_DECIMALS,
        ERROR_ASSET_NOT_BORROWABLE_IN_SILOED,
    );

    // Cover the error when there are more assets borrowed and early throw
    state.borrow_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(10u64),
        1,
        USDC_DECIMALS,
    );

    state.borrow_asset_error(
        &borrower,
        SILOED_TOKEN,
        BigUint::from(1u64),
        1,
        SILOED_DECIMALS,
        ERROR_ASSET_NOT_BORROWABLE_IN_SILOED,
    );
}

#[test]
fn test_borrow_asset_then_borrow_siloed_asset_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_accounts(&mut state, supplier, borrower);

    state.supply_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(1000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Test borrow
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.supply_asset(
        &supplier,
        SILOED_TOKEN,
        BigUint::from(1000u64),
        SILOED_DECIMALS,
        OptionalValue::Some(2),
        OptionalValue::None,
        false,
    );

    state.borrow_asset(
        &borrower,
        SILOED_TOKEN,
        BigUint::from(100u64),
        1,
        SILOED_DECIMALS,
    );

    state.borrow_asset_error(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(1u64),
        1,
        EGLD_DECIMALS,
        ERROR_ASSET_NOT_BORROWABLE_IN_SILOED,
    );
}

// Siloed Tests End

// Withdrawal Tests
#[test]
fn test_withdraw_auto_liquidation_protection_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    // Setup accounts
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

    state.withdraw_asset_error(
        &borrower,
        USDC_TOKEN,
        BigUint::from(5000u64),
        2,
        USDC_DECIMALS,
        ERROR_HEALTH_FACTOR_WITHDRAW,
    );
}

#[test]
fn test_withdraw_non_borrowed_asset_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    // Setup accounts
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

    let custom_error_message = format!(
        "Token {} is not available for this account",
        XEGLD_TOKEN.as_str()
    );
    // println!("custom_error_message: {}", custom_error_message);

    state.withdraw_asset_error(
        &borrower,
        XEGLD_TOKEN,
        BigUint::from(50u64),
        2,
        XEGLD_DECIMALS,
        custom_error_message.as_bytes(),
    );
}

// Withdrawal Tests End

// Liquidation Tests
#[test]
fn test_liquidation_and_left_bad_debt() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");
    let liquidator = TestAddress::new("liquidator");

    // Setup accounts including liquidator
    setup_accounts(&mut state, supplier, borrower);
    state.world.account(liquidator).nonce(1).esdt_balance(
        USDC_TOKEN,
        BigUint::from(20000u64) * BigUint::from(10u64).pow(USDC_DECIMALS as u32),
    );

    // Create risky position
    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(4000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Create risky position
    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(1000u64),
        USDC_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::None,
        false,
    );

    state.supply_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.borrow_asset(
        &borrower,
        USDC_TOKEN.clone(),
        BigUint::from(1000u64),
        2,
        USDC_DECIMALS,
    );

    state.borrow_asset(
        &borrower,
        USDC_TOKEN.clone(),
        BigUint::from(1000u64),
        2,
        USDC_DECIMALS,
    );

    state.world.current_block().block_timestamp(1);

    state.world.current_block().block_timestamp(600000000u64);
    state.update_borrows_with_debt(&borrower, 2);
    let health = state.get_account_health_factor(2);
    println!("health: {:?}", health);
    // Attempt liquidation
    state.liquidate_account(
        &liquidator,
        &EGLD_TOKEN,
        &USDC_TOKEN,
        BigUint::from(20000u64),
        2,
        USDC_DECIMALS,
    );

    let borrow_amount_in_dollars = state.get_total_borrow_in_dollars(2);
    let collateral_in_dollars = state.get_collateral_amount_for_token(2, EGLD_TOKEN);

    let health = state.get_account_health_factor(2);
    println!("health: {:?}", health);
    println!("collateral_in_dollars: {:?}", collateral_in_dollars);
    println!("borrow_amount_in_dollars: {:?}", borrow_amount_in_dollars);
    assert!(borrow_amount_in_dollars > 0);
    assert!(collateral_in_dollars == BigUint::from(0u64));

    // Repay the bad debt, usually the protocol will do this
    state.repay_asset(
        &liquidator,
        &USDC_TOKEN,
        BigUint::from(4000u64),
        2,
        USDC_DECIMALS,
    );
    let borrow_amount_in_dollars = state.get_total_borrow_in_dollars(2);
    println!("borrow_amount_in_dollars: {:?}", borrow_amount_in_dollars);
    assert!(borrow_amount_in_dollars == 0);
}

#[test]
fn test_borrow_not_enough_collateral_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_accounts(&mut state, supplier, borrower);

    state.supply_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(1000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.supply_asset(
        &supplier,
        SILOED_TOKEN,
        BigUint::from(1000u64),
        SILOED_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.borrow_asset_error(
        &borrower,
        SILOED_TOKEN,
        BigUint::from(600u64),
        1,
        SILOED_DECIMALS,
        ERROR_INSUFFICIENT_COLLATERAL,
    );
}

#[test]
fn test_liquidation_partial_payment() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");
    let liquidator = TestAddress::new("liquidator");
    // Setup accounts including liquidator
    setup_accounts(&mut state, supplier, borrower);
    state.world.account(liquidator).nonce(1).esdt_balance(
        USDC_TOKEN,
        BigUint::from(20000u64) * BigUint::from(10u64).pow(USDC_DECIMALS as u32),
    );

    // Create risky position
    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(5000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.supply_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.borrow_asset(
        &borrower,
        USDC_TOKEN.clone(),
        BigUint::from(2000u64),
        2,
        USDC_DECIMALS,
    );

    state.world.current_block().block_timestamp(1);

    let borrow_amount_in_dollars = state.get_borrow_amount_for_token(2, USDC_TOKEN);
    let collateral_in_dollars = state.get_collateral_amount_for_token(2, EGLD_TOKEN);

    println!(
        "borrow_amount_in_dollars: {}",
        borrow_amount_in_dollars.to_u64().unwrap()
    );
    println!("collateral_in_dollars: {:?}", collateral_in_dollars);

    state.world.current_block().block_timestamp(600000000u64);
    state.update_borrows_with_debt(&borrower, 2);
    println!("borrow_amount_in_dollars: {:?}", borrow_amount_in_dollars);

    // Attempt liquidation
    state.liquidate_account(
        &liquidator,
        &EGLD_TOKEN,
        &USDC_TOKEN,
        BigUint::from(800u64),
        2,
        USDC_DECIMALS,
    );

    let borrow_amount_in_dollars = state.get_borrow_amount_for_token(2, USDC_TOKEN);
    println!("borrow_amount_in_dollars: {:?}", borrow_amount_in_dollars);
    assert!(borrow_amount_in_dollars > BigUint::from(0u64));
}

// Liquidation Tests End

// Input Validation Tests
#[test]
fn test_supply_asset_payment_count_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_accounts(&mut state, supplier, borrower);
    // Supply non isolated asset
    state.supply_asset_error_payment_count(
        &supplier,
        XEGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
        ERROR_INVALID_NUMBER_OF_ESDT_TRANSFERS,
    );
}
// Input Validation Tests End

#[test]
fn test_interest_accrual_test() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    // Setup initial state
    setup_accounts(&mut state, supplier, borrower);

    // Initial supply and borrow
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(110u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.supply_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(10000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    state.borrow_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(10u64),
        1,
        EGLD_DECIMALS,
    );
    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(100u64),
        2,
        EGLD_DECIMALS,
    );

    // Record initial amounts
    let initial_borrow = state.get_borrow_amount_for_token(2, EGLD_TOKEN);
    let initial_supply = state.get_collateral_amount_for_token(1, EGLD_TOKEN);
    let capacity = state.get_market_utilization(state.egld_market.clone());
    println!("capacity: {:?}", capacity);
    // Simulate daily updates for a month
    // for day in 1..=182 {
    //     state
    //         .world
    //         .current_block()
    //         .block_timestamp(day * SECONDS_PER_DAY);
    //     state.update_borrows_with_debt(&supplier, 1);
    //     state.update_interest_indexes(&supplier, 1);
    // }
    state
        .world
        .current_block()
        .block_timestamp(SECONDS_PER_YEAR / 2);
    let borrow_rate = state.get_market_borrow_rate(state.egld_market.clone());
    let supply_rate = state.get_market_supply_rate(state.egld_market.clone());
    println!("borrow_rate: {:?}", borrow_rate);
    println!("supply_rate: {:?}", supply_rate);
    state.update_borrows_with_debt(&borrower, 2);

    state
        .world
        .current_block()
        .block_timestamp(SECONDS_PER_YEAR);
    // Verify interest accrual
    let final_borrow = state.get_borrow_amount_for_token(2, EGLD_TOKEN);
    let final_supply = state.get_collateral_amount_for_token(1, EGLD_TOKEN);
    println!("final_borrow: {:?}", final_borrow);
    println!("final_supply: {:?}", final_supply);

    state.update_borrows_with_debt(&borrower, 2);
    let final_borrow = state.get_borrow_amount_for_token(2, EGLD_TOKEN);
    println!("final_borrow: {:?}", final_borrow);
    // assert!(final_borrow > initial_borrow);
    // assert!(final_supply > initial_supply);
}
