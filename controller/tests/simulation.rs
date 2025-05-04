use multiversx_sc::types::{EgldOrEsdtTokenIdentifier, MultiValueEncoded};
use multiversx_sc_scenario::imports::{BigUint, OptionalValue, TestAddress};
pub mod constants;
pub mod proxys;
pub mod setup;
use constants::*;
use setup::*;

#[test]
fn test_leave_dust_in_market() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let supplier2 = TestAddress::new("supplier2");
    let borrower = TestAddress::new("borrower");
    let borrower2 = TestAddress::new("borrower2");

    // Setup initial state
    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);
    setup_accounts(&mut state, supplier2, borrower2);

    // Initial supply and borrow
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(100000u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.supply_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(100000u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(50000u64),
        2,
        EGLD_DECIMALS,
    );

    // Initial supply and borrow
    state.supply_asset(
        &supplier2,
        EGLD_TOKEN,
        BigUint::from(100000u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.supply_asset(
        &borrower2,
        EGLD_TOKEN,
        BigUint::from(100000u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    // Borrow from second supplier
    state.borrow_asset(
        &borrower2,
        EGLD_TOKEN,
        BigUint::from(50000u64),
        4,
        EGLD_DECIMALS,
    );

    let utilization_ratio = state.get_market_utilization(state.egld_market.clone());
    let borrow_rate = state.get_market_borrow_rate(state.egld_market.clone());
    let supply_rate = state.get_market_supply_rate(state.egld_market.clone());
    println!("utilization_ratio: {:?}", utilization_ratio);
    println!("borrow_rate: {:?}", borrow_rate);
    println!("supply_rate: {:?}", supply_rate);

    // Record initial amounts
    let initial_supply_borrower = state.get_collateral_amount_for_token(2, EGLD_TOKEN);
    let initial_supply_supplier = state.get_collateral_amount_for_token(1, EGLD_TOKEN);
    let initial_borrow_borrower = state.get_borrow_amount_for_token(2, EGLD_TOKEN);

    println!("initial_supply_borrower: {:?}", initial_supply_borrower);
    println!("initial_supply_supplier: {:?}", initial_supply_supplier);
    println!("initial_borrow_borrower: {:?}", initial_borrow_borrower);
    //     utilization_ratio: ManagedDecimal { handle: -3224, number: "0.250000000000000000000000000" }
    // borrow_rate: ManagedDecimal { handle: -3228, number: "0.000000000926286965681297056" }
    // supply_rate: ManagedDecimal { handle: -3232, number: "0.000000000173678806065243198" }
    // initial_supply_borrower: ManagedDecimal { handle: -3241, number: "100000.000000000000000000" }
    // initial_supply_supplier: ManagedDecimal { handle: -3247, number: "100000.000000000000000000" }
    // initial_borrow_borrower: ManagedDecimal { handle: -3253, number: "50000.000000000000000000" }
    // final_supply_borrower: ManagedDecimal { handle: -8441681, number: "100001.500677031522870139" }
    // final_supply_supplier: ManagedDecimal { handle: -8441687, number: "100001.500677031522870139" }
    // final_borrow_borrower: ManagedDecimal { handle: -8441693, number: "50004.001805417394320372" }
    // protocol_revenue: ManagedDecimal { handle: -8441743, number: "1.000451354348572890" }
    // reserves: ManagedDecimal { handle: -8441748, number: "1.000451354348580094" }
    // dust: ManagedDecimal { handle: -8441748, number: "0.000000000000007204" }
    //     utilization_ratio: ManagedDecimal { handle: -3224, number: "0.250000000000000000000000000" }
    // borrow_rate: ManagedDecimal { handle: -3228, number: "0.000000000926286965681297056" }
    // supply_rate: ManagedDecimal { handle: -3232, number: "0.000000000173678806065243198" }
    // initial_supply_borrower: ManagedDecimal { handle: -3241, number: "100000.000000000000000000" }
    // initial_supply_supplier: ManagedDecimal { handle: -3247, number: "100000.000000000000000000" }
    // initial_borrow_borrower: ManagedDecimal { handle: -3253, number: "50000.000000000000000000" }
    // final_supply_borrower: ManagedDecimal { handle: -8672065, number: "100001.500677031522862979" }
    // final_supply_supplier: ManagedDecimal { handle: -8672071, number: "100001.500677031522862979" }
    // final_borrow_borrower: ManagedDecimal { handle: -8672077, number: "50004.001805417394313162" }
    // protocol_revenue: ManagedDecimal { handle: -8672127, number: "1.000451354348572890" }
    // reserves: ManagedDecimal { handle: -8672132, number: "1.000451354348587204" }
    // dust: ManagedDecimal { handle: -8672132, number: "0.000000000000014314" }
    //     utilization_ratio: ManagedDecimal { handle: -3399, number: "0.250000000000000000000000000" }
    // borrow_rate: ManagedDecimal { handle: -3403, number: "0.000000000926286965681297056" }
    // supply_rate: ManagedDecimal { handle: -3407, number: "0.000000000173678806065243198" }
    // initial_supply_borrower: ManagedDecimal { handle: -3416, number: "100000.000000000000000000" }
    // initial_supply_supplier: ManagedDecimal { handle: -3422, number: "100000.000000000000000000" }
    // initial_borrow_borrower: ManagedDecimal { handle: -3428, number: "50000.000000000000000000" }
    // final_supply_supplier: ManagedDecimal { handle: -8902640, number: "100001.500677031522862979" }
    // final_supply_borrower: ManagedDecimal { handle: -8902646, number: "100001.500677031522862979" }
    // final_borrow_borrower: ManagedDecimal { handle: -8902652, number: "50004.001805417394313162" }
    // final_supply_supplier2: ManagedDecimal { handle: -8902658, number: "100001.500677031522862979" }
    // final_supply_borrower2: ManagedDecimal { handle: -8902664, number: "100001.500677031522862979" }
    // final_borrow_borrower2: ManagedDecimal { handle: -8902670, number: "50004.001805417394313162" }
    // protocol_revenue: ManagedDecimal { handle: -8902766, number: "2.000902708697152982" }
    // reserves: ManagedDecimal { handle: -8902771, number: "2.000902708697174408" }
    // dust: ManagedDecimal { handle: -8902771, number: "0.000000000000021426" }
    let mut markets = MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN));
    // Simulate hourly updates for 2 years
    for day in 1..=365 {
        state.change_timestamp(day * SECONDS_PER_DAY);
        state.update_markets(&OWNER_ADDRESS, markets.clone());
        state.update_account_positions(&supplier, 1);
        state.update_account_positions(&borrower, 2);
        state.update_account_positions(&supplier2, 3);
        state.update_account_positions(&borrower2, 4);
    }

    let final_supply_supplier = state.get_collateral_amount_for_token(1, EGLD_TOKEN);
    let final_supply_borrower = state.get_collateral_amount_for_token(2, EGLD_TOKEN);
    let final_borrow_borrower = state.get_borrow_amount_for_token(2, EGLD_TOKEN);
    let final_supply_supplier2 = state.get_collateral_amount_for_token(3, EGLD_TOKEN);
    let final_supply_borrower2 = state.get_collateral_amount_for_token(4, EGLD_TOKEN);
    let final_borrow_borrower2 = state.get_borrow_amount_for_token(4, EGLD_TOKEN);
    println!("final_supply_supplier: {:?}", final_supply_supplier);
    println!("final_supply_borrower: {:?}", final_supply_borrower);
    println!("final_borrow_borrower: {:?}", final_borrow_borrower);
    println!("final_supply_supplier2: {:?}", final_supply_supplier2);
    println!("final_supply_borrower2: {:?}", final_supply_borrower2);
    println!("final_borrow_borrower2: {:?}", final_borrow_borrower2);

    state.repay_asset_deno(
        &borrower,
        &EGLD_TOKEN,
        final_borrow_borrower.into_raw_units().clone(),
        2,
    );

    state.withdraw_asset_den(
        &supplier,
        EGLD_TOKEN,
        final_supply_supplier.into_raw_units().clone(),
        1,
    );

    state.withdraw_asset_den(
        &borrower,
        EGLD_TOKEN,
        final_supply_borrower.into_raw_units().clone(),
        2,
    );

    state.repay_asset_deno(
        &borrower2,
        &EGLD_TOKEN,
        final_borrow_borrower2.into_raw_units().clone(),
        4,
    );

    state.withdraw_asset_den(
        &supplier2,
        EGLD_TOKEN,
        final_supply_supplier2.into_raw_units().clone(),
        3,
    );

    state.withdraw_asset_den(
        &borrower2,
        EGLD_TOKEN,
        final_supply_borrower2.into_raw_units().clone(),
        4,
    );

    let protocol_revenue = state.get_market_revenue(state.egld_market.clone());
    println!("protocol_revenue: {:?}", protocol_revenue);
    let reserves = state.get_market_reserves(state.egld_market.clone());
    println!("reserves: {:?}", reserves);

    let diff = reserves - protocol_revenue;
    println!("dust: {:?}", diff);
}
