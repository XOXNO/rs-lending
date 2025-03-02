use multiversx_sc::types::{ConstDecimals, ManagedDecimal};
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
fn test_liquidation() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    // Setup accounts
    state.world.current_block().block_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);
    // Total Supplied 5000$
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
        USDC_TOKEN,
        BigUint::from(1000u64),
        USDC_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::None,
        false,
    );

    state.supply_asset(
        &borrower,
        XEGLD_TOKEN,
        BigUint::from(20u64), // 2500$
        XEGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.supply_asset(
        &borrower,
        SEGLD_TOKEN,
        BigUint::from(80u64), // 2500$
        SEGLD_DECIMALS,
        OptionalValue::Some(2),
        OptionalValue::None,
        false,
    );

    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(39u64),
        2,
        EGLD_DECIMALS,
    );

    state.borrow_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(1000u64),
        2,
        USDC_DECIMALS,
    );

    // Verify amounts
    let borrowed = state.get_total_borrow_in_egld(2);
    let collateral = state.get_total_collateral_in_egld(2);
    let collateral_weighted = state.get_liquidation_collateral_available(2);
    let health_factor = state.get_account_health_factor(2);
    println!("Total EGLD Borrowed {:?}", borrowed);
    println!("Total EGLD Deposite {:?}", collateral);
    println!("Total EGLD Weighted {:?}", collateral_weighted);
    println!("Health Factor {:?}", health_factor);
    assert!(
        borrowed
            > ManagedDecimal::<StaticApi, ConstDecimals<EGLD_DECIMALS>>::from(BigUint::from(0u64))
    );
    assert!(
        collateral
            > ManagedDecimal::<StaticApi, ConstDecimals<EGLD_DECIMALS>>::from(BigUint::from(0u64))
    );
    state
        .world
        .current_block()
        .block_timestamp(SECONDS_PER_DAY * 500);
    state.update_account_positions(&borrower, 2);
    let health_factor = state.get_account_health_factor(2);
    println!("Health Factor {:?}", health_factor);

    let liquidator = TestAddress::new("liquidator");
    state.world.account(liquidator).nonce(1).esdt_balance(
        USDC_TOKEN,
        BigUint::from(10000u64) * BigUint::from(10u64).pow(USDC_DECIMALS as u32),
    );

    state.liquidate_account(
        &liquidator,
        &USDC_TOKEN,
        BigUint::from(10000u64),
        2,
        USDC_DECIMALS,
    );
    let borrowed = state.get_total_borrow_in_egld(2);
    let collateral = state.get_total_collateral_in_egld(2);
    let collateral_weighted = state.get_liquidation_collateral_available(2);
    let health_factor = state.get_account_health_factor(2);
    println!("Total EGLD Borrowed {:?}", borrowed);
    println!("Total EGLD Deposite {:?}", collateral);
    println!("Total EGLD Weighted {:?}", collateral_weighted);
    println!("Health Factor {:?}", health_factor);
}

#[test]
fn test_liquidation_bad_debt_multi_asset() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    // Setup accounts
    state.world.current_block().block_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);
    // Total Supplied 5000$
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
        USDC_TOKEN,
        BigUint::from(1000u64),
        USDC_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::None,
        false,
    );

    state.supply_asset(
        &borrower,
        XEGLD_TOKEN,
        BigUint::from(20u64), // 2500$
        XEGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.supply_asset(
        &borrower,
        SEGLD_TOKEN,
        BigUint::from(80u64), // 2500$
        SEGLD_DECIMALS,
        OptionalValue::Some(2),
        OptionalValue::None,
        false,
    );

    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(39u64),
        2,
        EGLD_DECIMALS,
    );

    state.borrow_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(1000u64),
        2,
        USDC_DECIMALS,
    );

    // Verify amounts
    let borrowed = state.get_total_borrow_in_egld(2);
    let collateral = state.get_total_collateral_in_egld(2);
    let collateral_weighted = state.get_liquidation_collateral_available(2);
    let health_factor = state.get_account_health_factor(2);
    println!("Total EGLD Borrowed {:?}", borrowed);
    println!("Total EGLD Deposite {:?}", collateral);
    println!("Total EGLD Weighted {:?}", collateral_weighted);
    println!("Health Factor {:?}", health_factor);
    assert!(
        borrowed
            > ManagedDecimal::<StaticApi, ConstDecimals<EGLD_DECIMALS>>::from(BigUint::from(0u64))
    );
    assert!(
        collateral
            > ManagedDecimal::<StaticApi, ConstDecimals<EGLD_DECIMALS>>::from(BigUint::from(0u64))
    );
    state
        .world
        .current_block()
        .block_timestamp(SECONDS_PER_DAY * 1000);
    state.update_account_positions(&borrower, 2);
    let borrowed = state.get_total_borrow_in_egld(2);
    let borrowed_egld = state.get_borrow_amount_for_token(2, EGLD_TOKEN);
    let collateral = state.get_total_collateral_in_egld(2);
    let collateral_weighted = state.get_liquidation_collateral_available(2);
    let health_factor = state.get_account_health_factor(2);
    // 46341019210806527860
    // 139 - 46 = 93 EGLD debt via USDC
    println!("Total EGLD TOKEN Borrowed {:?}", borrowed_egld);
    println!("Total EGLD Borrowed {:?}", borrowed);
    println!("Total EGLD Deposite {:?}", collateral);
    println!("Total EGLD Weighted {:?}", collateral_weighted);
    println!("Health Factor {:?}", health_factor);

    let liquidator = TestAddress::new("liquidator");
    state
        .world
        .account(liquidator)
        .nonce(1)
        .esdt_balance(
            USDC_TOKEN,
            BigUint::from(10000u64) * BigUint::from(10u64).pow(USDC_DECIMALS as u32),
        )
        .esdt_balance(
            EGLD_TOKEN,
            BigUint::from(1000u64) * BigUint::from(10u64).pow(EGLD_DECIMALS as u32),
        );
    state.liquidate_account(
        &liquidator,
        &EGLD_TOKEN,
        BigUint::from(50u64),
        2,
        EGLD_DECIMALS,
    );

    let borrowed = state.get_total_borrow_in_egld(2);
    let borrowed_usdc = state.get_borrow_amount_for_token(2, USDC_TOKEN);
    let collateral = state.get_total_collateral_in_egld(2);
    let collateral_weighted = state.get_liquidation_collateral_available(2);
    let health_factor = state.get_account_health_factor(2);
    println!("Total USDC Borrowed {:?}", borrowed_usdc);
    println!("Total EGLD Borrowed {:?}", borrowed);
    println!("Total EGLD Deposite {:?}", collateral);
    println!("Total EGLD Weighted {:?}", collateral_weighted);
    println!("Health Factor {:?}", health_factor);

    state.liquidate_account(
        &liquidator,
        &USDC_TOKEN,
        BigUint::from(10000u64),
        2,
        USDC_DECIMALS,
    );
    let borrowed = state.get_total_borrow_in_egld(2);
    let collateral = state.get_total_collateral_in_egld(2);
    let collateral_weighted = state.get_liquidation_collateral_available(2);
    let health_factor = state.get_account_health_factor(2);
    println!("Total EGLD Borrowed {:?}", borrowed);
    println!("Total EGLD Deposite {:?}", collateral);
    println!("Total EGLD Weighted {:?}", collateral_weighted);
    println!("Health Factor {:?}", health_factor);
}

#[test]
fn test_liquidation_single_position() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");
    let liquidator = TestAddress::new("liquidator");

    state.world.account(liquidator).nonce(1).esdt_balance(
        EGLD_TOKEN,
        BigUint::from(1000u64) * BigUint::from(10u64).pow(EGLD_DECIMALS as u32),
    );

    // Setup accounts
    state.world.current_block().block_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Total Supplied 5000$
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
        &borrower,
        EGLD_TOKEN,
        BigUint::from(100u64), // 2500$
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(75u64),
        2,
        EGLD_DECIMALS,
    );

    // Verify amounts
    let borrowed = state.get_total_borrow_in_egld(2);
    let collateral = state.get_total_collateral_in_egld(2);
    let collateral_weighted = state.get_liquidation_collateral_available(2);
    let utilization = state.get_market_utilization(state.egld_market.clone());
    let health_factor = state.get_account_health_factor(2);

    println!("Utilization {:?}", utilization);
    println!("Total EGLD Borrowed {:?}", borrowed);
    println!("Total EGLD Deposite {:?}", collateral);
    println!("Total EGLD Weighted {:?}", collateral_weighted);
    println!("Health Factor {:?}", health_factor);
    assert!(
        borrowed
            > ManagedDecimal::<StaticApi, ConstDecimals<EGLD_DECIMALS>>::from(BigUint::from(0u64))
    );
    assert!(
        collateral
            > ManagedDecimal::<StaticApi, ConstDecimals<EGLD_DECIMALS>>::from(BigUint::from(0u64))
    );

    state
        .world
        .current_block()
        .block_timestamp(SECONDS_PER_YEAR + SECONDS_PER_DAY * 1500);
    state.update_account_positions(&borrower, 2);
    let borrowed_egld = state.get_borrow_amount_for_token(2, EGLD_TOKEN);
    let borrowed = state.get_total_borrow_in_egld(2);
    let collateral = state.get_total_collateral_in_egld(2);
    let collateral_weighted = state.get_liquidation_collateral_available(2);
    let health_factor = state.get_account_health_factor(2);
    println!("Total EGLD Token Borrowed {:?}", borrowed_egld); // 70000000000000000000
    println!("Total EGLD Borrowed {:?}", borrowed);
    println!("Total EGLD Deposite {:?}", collateral);
    println!("Total EGLD Weighted {:?}", collateral_weighted);
    println!("Health Factor {:?}", health_factor);

    state.liquidate_account(
        &liquidator,
        &EGLD_TOKEN,
        BigUint::from(105u64),
        2,
        EGLD_DECIMALS,
    );
    let borrowed = state.get_total_borrow_in_egld_big(2);
    let collateral = state.get_total_collateral_in_egld_big(2);
    let collateral_weighted = state.get_liquidation_collateral_available(2);
    let health_factor = state.get_account_health_factor(2);

    println!("Total EGLD Borrowed {:?}", borrowed);
    println!("Total EGLD Deposite {:?}", collateral);
    println!("Total EGLD Weighted {:?}", collateral_weighted);
    println!("Health Factor {:?}", health_factor);
    assert!(
        borrowed
            >= ManagedDecimal::<StaticApi, ConstDecimals<EGLD_DECIMALS>>::from(BigUint::from(0u64))
    );
    assert!(
        collateral
            >= ManagedDecimal::<StaticApi, ConstDecimals<EGLD_DECIMALS>>::from(BigUint::from(0u64))
    );
    // state.liquidate_account(
    //     &liquidator,
    //     &EGLD_TOKEN,
    //     BigUint::from(40u64),
    //     2,
    //     EGLD_DECIMALS,
    // );
    // let borrowed = state.get_total_borrow_in_egld(2);
    // let collateral = state.get_total_collateral_in_egld(2);
    // let collateral_weighted = state.get_liquidation_collateral_available(2);
    // let health_factor = state.get_account_health_factor(2);
    // println!("Total EGLD Borrowed {:?}", borrowed);
    // println!("Total EGLD Deposite {:?}", collateral);
    // println!("Total EGLD Weighted {:?}", collateral_weighted);
    // println!("Health Factor {:?}", health_factor);
}
