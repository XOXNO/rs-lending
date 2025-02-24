use controller::{RAY_PRECISION, WAD_PRECISION};
use multiversx_sc::types::ManagedDecimal;
use multiversx_sc_scenario::imports::{BigUint, OptionalValue, TestAddress};
pub mod constants;
pub mod proxys;
pub mod setup;
use constants::*;
use setup::*;

// Basic Operations
#[test]
fn views_tests() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    // Setup accounts
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
        XEGLD_TOKEN,
        BigUint::from(100u64), // 2500$
        XEGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(45u64),
        2,
        EGLD_DECIMALS,
    );
    // Verify amounts
    let borrowed = state.get_total_borrow_in_egld(2);
    let collateral = state.get_total_collateral_in_egld(2);
    let collateral_weighted = state.get_liquidation_collateral_available(2);
    let health_factor = state.get_account_health_factor(2);
    let utilisation = state.get_market_utilization(state.egld_market.clone());
    let borrow_rate = state.get_market_borrow_rate(state.egld_market.clone());
    let deposit_rate = state.get_market_supply_rate(state.egld_market.clone());
    let total_capital = state.get_market_total_capital(state.egld_market.clone());
    let usd_price = state.get_usd_price(EGLD_TOKEN);
    let egld_price = state.get_egld_price(EGLD_TOKEN);
    println!("usd_price: {:?}", usd_price);
    println!("egld_price: {:?}", egld_price);
    println!("borrowed: {:?}", borrowed);
    println!("collateral: {:?}", collateral);
    println!("collateral_weighted: {:?}", collateral_weighted);
    println!("health_factor: {:?}", health_factor);
    println!("utilisation: {:?}", utilisation);
    println!("total_capital {:?}", total_capital);
    println!(
        "borrow_rate: {:?}",
        borrow_rate
            * ManagedDecimal::from_raw_units(BigUint::from(SECONDS_PER_YEAR), 0)
            * ManagedDecimal::from_raw_units(BigUint::from(100u64), 0)
    );
    println!(
        "deposit_rate: {:?}",
        deposit_rate
            * ManagedDecimal::from_raw_units(BigUint::from(SECONDS_PER_YEAR), 0)
            * ManagedDecimal::from_raw_units(BigUint::from(100u64), 0)
    );

    assert_eq!(
        utilisation,
        ManagedDecimal::from_raw_units(
            BigUint::from(450000000000000000000000000u128),
            RAY_PRECISION
        )
    );
    assert_eq!(
        total_capital,
        ManagedDecimal::from_raw_units(BigUint::from(100000000000000000000u128), WAD_PRECISION)
    );
    assert_eq!(
        usd_price,
        ManagedDecimal::from_raw_units(BigUint::from(40000000000000000000u128), WAD_PRECISION)
    );
    assert_eq!(
        egld_price,
        ManagedDecimal::from_raw_units(BigUint::from(1000000000000000000u128), WAD_PRECISION)
    );
}
