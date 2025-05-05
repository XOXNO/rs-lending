use controller::{ERROR_HEALTH_FACTOR_WITHDRAW, ERROR_INSUFFICIENT_LIQUIDITY};
use multiversx_sc::types::{EgldOrEsdtTokenIdentifier, MultiValueEncoded};
use multiversx_sc_scenario::imports::{BigUint, OptionalValue, TestAddress};
pub mod constants;
pub mod proxys;
pub mod setup;
use constants::*;
use setup::*;

#[test]
fn test_withdrawal_higher_amount() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    // Setup supplier account
    state.change_timestamp(0);
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

    // Withdraw too much than existing, should cap it at maximum
    state.withdraw_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(1500u64),
        1,
        USDC_DECIMALS,
    );

    let custom_error_message = format!("Token not existing in the account {}", USDC_TOKEN.as_str());
    state.get_collateral_amount_for_token_non_existing(
        1,
        USDC_TOKEN,
        custom_error_message.as_bytes(),
    );
}

#[test]
fn test_withdrawal_with_low_reserves() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    // Setup supplier account
    state.change_timestamp(0);
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
        USDC_TOKEN,
        BigUint::from(100u64),
        2,
        USDC_DECIMALS,
    );

    // Withdraw too much than existing, should cap it at maximum
    state.withdraw_asset_error(
        &supplier,
        USDC_TOKEN,
        BigUint::from(1000u64),
        1,
        USDC_DECIMALS,
        ERROR_INSUFFICIENT_LIQUIDITY,
    );
}

#[test]
fn test_withdrawal_with_interest() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    // Setup supplier account
    state.change_timestamp(0);
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
    state.change_timestamp(SECONDS_PER_DAY * 10); // 10 days

    // Update interest before withdrawal
    // state.global_sync(&supplier, 1);

    // Get initial state
    let initial_collateral = state.get_collateral_amount_for_token(1, USDC_TOKEN);
    println!("initial_collateral: {}", initial_collateral);

    // Advance time to accumulate interest
    state.change_timestamp(SECONDS_PER_DAY * 20); // 20 days

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
    println!("final_collateral:   {}", final_collateral);
}

#[test]
fn test_withdrawal_with_interest_one_user() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    // Setup supplier account
    setup_accounts(&mut state, supplier, borrower);

    state.change_timestamp(1740269720);
    // Initial supply
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.change_timestamp(1740269852);

    state.borrow_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(72u64),
        1,
        EGLD_DECIMALS,
    );

    state.change_timestamp(1740275066);
    // Update interest before withdrawal

    // Get initial state
    let initial_collateral = state.get_collateral_amount_for_token(1, EGLD_TOKEN);
    let initial_borrow = state.get_borrow_amount_for_token(1, EGLD_TOKEN);
    println!("initial_collateral: {}", initial_collateral);
    println!("initial_borrow: {}", initial_borrow);

    state.repay_asset_deno(
        &supplier,
        &EGLD_TOKEN,
        BigUint::from(72721215451172815256u128),
        1,
    );
    let reserve = state.get_market_reserves(state.egld_market.clone());
    let revenue = state.get_market_revenue(state.egld_market.clone());
    let borrow_index = state.get_market_borrow_index(state.egld_market.clone());
    let supply_index = state.get_market_supply_index(state.egld_market.clone());
    println!("reserve: {}", reserve);
    println!("revenue: {}", revenue);
    println!("borrow_index: {}", borrow_index);
    println!("supply_index: {}", supply_index);
    state.change_timestamp(1740275594);
    let borrow_index = state.get_market_borrow_index(state.egld_market.clone());
    let supply_index = state.get_market_supply_index(state.egld_market.clone());
    println!("reserve: {}", reserve);
    println!("revenue: {}", revenue);
    println!("borrow_index: {}", borrow_index);
    println!("supply_index: {}", supply_index);
    // state.global_sync(&supplier, 1);
    // Get initial state
    let final_collateral = state.get_collateral_amount_for_token(1, EGLD_TOKEN);
    println!("collate: {}", final_collateral);
    println!("reserve: {}", reserve);
    println!("revenue: {}", revenue);

    // Withdraw partial amount
    state.withdraw_asset_den(
        &supplier,
        EGLD_TOKEN,
        final_collateral.into_raw_units().clone(),
        1,
    );
    let reserve = state.get_market_reserves(state.egld_market.clone());
    let revenue = state.get_market_revenue(state.egld_market.clone());
    println!("reserve: {}", reserve);
    println!("revenue: {}", revenue);
    let diff =  reserve - revenue.clone();
    println!("diff:    {}", diff);
}

#[test]
fn test_withdrawal_with_interest_one_user_prior_update() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    // Setup supplier account
    setup_accounts(&mut state, supplier, borrower);
    let mut markets = MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN));

    state.change_timestamp(1740269720);
    // Initial supply
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.change_timestamp(1740269852);
    state.update_markets(&supplier, markets.clone());
    state.borrow_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(72u64),
        1,
        EGLD_DECIMALS,
    );

    state.change_timestamp(1740275066);
    state.update_markets(&supplier, markets.clone());
    // Update interest before withdrawal

    // Get initial state
    let initial_collateral = state.get_collateral_amount_for_token(1, EGLD_TOKEN);
    let initial_borrow = state.get_borrow_amount_for_token(1, EGLD_TOKEN);
    println!("initial_collateral: {}", initial_collateral);
    println!("initial_borrow: {}", initial_borrow);

    state.repay_asset_deno(
        &supplier,
        &EGLD_TOKEN,
        initial_borrow.into_raw_units().clone(),
        1,
    );
    state.update_markets(&supplier, markets.clone());
    let reserve = state.get_market_reserves(state.egld_market.clone());
    let revenue = state.get_market_revenue(state.egld_market.clone());
    let borrow_index = state.get_market_borrow_index(state.egld_market.clone());
    let supply_index = state.get_market_supply_index(state.egld_market.clone());
    println!("reserve: {}", reserve);
    println!("revenue: {}", revenue);
    let total_deposit = state.get_market_total_deposit(state.egld_market.clone());
    println!("total_deposit: {}", total_deposit);
    println!("borrow_index: {}", borrow_index);
    println!("supply_index: {}", supply_index);
    state.change_timestamp(1740275594);
    state.update_markets(&supplier, markets.clone());
    let reserve = state.get_market_reserves(state.egld_market.clone());
    let revenue = state.get_market_revenue(state.egld_market.clone());
    let borrow_index = state.get_market_borrow_index(state.egld_market.clone());
    let supply_index = state.get_market_supply_index(state.egld_market.clone());
    println!("reserve: {}", reserve);
    println!("revenue: {}", revenue);
    println!("borrow_index: {}", borrow_index);
    println!("supply_index: {}", supply_index);
    state.update_markets(&supplier, markets.clone());
    // Get initial state
    let final_collateral = state.get_collateral_amount_for_token(1, EGLD_TOKEN);
    println!("final_collateral:   {}", final_collateral);
    let diff = (reserve - final_collateral.clone())- revenue.clone() ;
    println!("diff: {}", diff);
    println!("revenue: {}", revenue);
    // Withdraw partial amount
    state.withdraw_asset_den(
        &supplier,
        EGLD_TOKEN,
        final_collateral.into_raw_units().clone(),
        1,
    );
    let reserve = state.get_market_reserves(state.egld_market.clone());
    let revenue = state.get_market_revenue(state.egld_market.clone());
    println!("reserve: {}", reserve);
    println!("revenue: {}", revenue);
}

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
        BigUint::from(80u64),
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
