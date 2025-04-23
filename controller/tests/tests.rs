use controller::{
    AccountAttributes, PositionMode, ERROR_HEALTH_FACTOR_WITHDRAW,
    ERROR_INVALID_LIQUIDATION_THRESHOLD,
};
use multiversx_sc::types::{
    EgldOrEsdtTokenIdentifier, ManagedDecimal, ManagedOption, MultiValueEncoded,
};
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
fn test_edge_case_math_rounding() {
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

    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(5000u64),
        USDC_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::None,
        false,
    );

    state.borrow_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(100u64),
        1,
        EGLD_DECIMALS,
    );

    // Verify amounts
    let borrowed = state.get_borrow_amount_for_token(1, EGLD_TOKEN);
    let collateral = state.get_collateral_amount_for_token(1, EGLD_TOKEN);
    let utilization = state.get_market_utilization(state.egld_market.clone());

    assert!(borrowed > ManagedDecimal::from_raw_units(BigUint::zero(), EGLD_DECIMALS));
    assert!(collateral > ManagedDecimal::from_raw_units(BigUint::zero(), EGLD_DECIMALS));

    println!("borrow_amount: {:?}", borrowed); //   100000000000000000000
    println!("supply_amount: {:?}", collateral); // 100000000000000000000
    println!("utilization: {:?}", utilization);

    state.world.current_block().block_timestamp(1111u64);
    state.update_account_positions(&supplier, 1);

    let borrowed = state.get_borrow_amount_for_token(1, EGLD_TOKEN);
    let collateral = state.get_collateral_amount_for_token(1, EGLD_TOKEN);
    let utilization = state.get_market_utilization(state.egld_market.clone());
    let revenue = state.get_market_revenue(state.egld_market.clone());
    let reserves = state.get_market_reserves(state.egld_market.clone());

    println!("reserves: {:?}", collateral.clone() + revenue.clone()); // 100019013258769247676
    println!("borrow_amount: {:?}", borrowed); // 100019013258769247676
    println!("supply_amount: {:?}", collateral); // 100013309281138473374
    println!("revenue_value: {:?}", revenue); //      5703977630774302
    println!("utilization: {:?}", utilization);
    assert_eq!(
        collateral + revenue,
        borrowed,
        "Collateral + revenue not equal with borrowed!"
    );
    assert_eq!(
        reserves,
        ManagedDecimal::from_raw_units(BigUint::zero(), EGLD_DECIMALS)
    );

    state.repay_asset_deno(&supplier, &EGLD_TOKEN, borrowed.into_raw_units().clone(), 1);

    // let borrowed = state.get_borrow_amount_for_token(1, EGLD_TOKEN);
    let collateral = state.get_collateral_amount_for_token(1, EGLD_TOKEN);
    let utilization = state.get_market_utilization(state.egld_market.clone());
    let revenue = state.get_market_revenue(state.egld_market.clone());

    println!("reserves: {:?}", collateral.clone() + revenue.clone()); // 100019013258769247676
                                                                      // println!("borrow_amount: {:?}", borrowed); // 100019013258769247676
    println!("supply_amount: {:?}", collateral); // 100013309281138473374
    println!("revenue_value: {:?}", revenue); //      5703977630774302
    println!("utilization: {:?}", utilization);
    // assert_eq!(
    //     collateral.clone() + revenue,
    //     reserves,
    //     "Collateral + revenue not equal with reserves after repayment!"
    // );

    state.withdraw_asset_den(
        &supplier,
        EGLD_TOKEN,
        collateral.into_raw_units().clone(),
        1,
    );
    // let borrowed = state.get_borrow_amount_for_token(1, EGLD_TOKEN);
    // let reserves = state.get_market_reserves(state.egld_market.clone());
    // let revenue = state.get_market_revenue(state.egld_market.clone());

    // println!("revenue_value: {:?}", revenue); //      5703977630774302
    // println!("borrow_amount: {:?}", borrowed); // 100019013258769247676
    // assert_eq!(
    //     borrowed,
    //     ManagedDecimal::from_raw_units(BigUint::zero(), EGLD_DECIMALS)
    // );
    // assert_eq!(reserves, revenue);
}

#[test]
fn test_edge_case_math_rounding_no_compound() {
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

    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(5000u64),
        USDC_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::None,
        false,
    );

    state.borrow_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(100u64),
        1,
        EGLD_DECIMALS,
    );

    // Verify amounts
    let borrowed = state.get_borrow_amount_for_token(1, EGLD_TOKEN);
    let collateral = state.get_collateral_amount_for_token(1, EGLD_TOKEN);
    let utilization = state.get_market_utilization(state.egld_market.clone());

    assert!(borrowed > ManagedDecimal::from_raw_units(BigUint::zero(), EGLD_DECIMALS));
    assert!(collateral > ManagedDecimal::from_raw_units(BigUint::zero(), EGLD_DECIMALS));

    println!("borrow_amount: {:?}", borrowed); //   100000000000000000000
    println!("supply_amount: {:?}", collateral); // 100000000000000000000
    println!("utilization: {:?}", utilization);

    state.world.current_block().block_timestamp(1111u64);
    // state.update_account_positions(&supplier, 1);

    let borrowed = state.get_borrow_amount_for_token(1, EGLD_TOKEN);
    let collateral = state.get_collateral_amount_for_token(1, EGLD_TOKEN);
    let utilization = state.get_market_utilization(state.egld_market.clone());
    let revenue = state.get_market_revenue(state.egld_market.clone());
    let reserves = state.get_market_reserves(state.egld_market.clone());

    println!("reserves: {:?}", collateral.clone() + revenue.clone()); // 100019013258769247676
    println!("borrow_amount: {:?}", borrowed); // 100019013258769247676
    println!("supply_amount: {:?}", collateral); // 100013309281138473374
    println!("revenue_value: {:?}", revenue); //      5703977630774302
    println!("utilization: {:?}", utilization);
    assert_eq!(
        reserves,
        ManagedDecimal::from_raw_units(BigUint::zero(), EGLD_DECIMALS)
    );

    state.repay_asset(
        &supplier,
        &EGLD_TOKEN,
        BigUint::from(105u64),
        1,
        EGLD_DECIMALS,
    );
    let custom_error_message = format!("Token not existing in the account {}", EGLD_TOKEN.as_str());

    state.get_borrow_amount_for_token_non_existing(1, EGLD_TOKEN, custom_error_message.as_bytes());
    let collateral = state.get_collateral_amount_for_token(1, EGLD_TOKEN);
    let utilization = state.get_market_utilization(state.egld_market.clone());
    let revenue = state.get_market_revenue(state.egld_market.clone());
    let reserves = state.get_market_reserves(state.egld_market.clone());

    println!("reserves: {:?}", reserves); // 100001056186524631708
    println!("borrow_amount: {:?}", borrowed); // 0
    println!("supply_amount: {:?}", collateral); // 100000000000000000000
    println!("revenue_value: {:?}", revenue); //      1056186524631708
    println!("utilization: {:?}", utilization);
    assert!(reserves > collateral + revenue, "Reserves are not enough");

    state.withdraw_asset(&supplier, EGLD_TOKEN, BigUint::from(1u64), 1, EGLD_DECIMALS);
    let reserves = state.get_market_reserves(state.egld_market.clone());
    let revenue = state.get_market_revenue(state.egld_market.clone());
    let collateral = state.get_collateral_amount_for_token(1, EGLD_TOKEN);

    println!("reserves: {:?}", reserves); //        99003495977396530955
    println!("supply_amount: {:?}", collateral); // 99000000000000000000
    println!("revenue_value: {:?}", revenue); //        1056186524631708
    assert!(reserves > collateral + revenue, "Reserves are not enough");
    state.update_account_positions(&supplier, 1);
    let reserves = state.get_market_reserves(state.egld_market.clone());
    let revenue = state.get_market_revenue(state.egld_market.clone());
    let collateral = state.get_collateral_amount_for_token(1, EGLD_TOKEN);
    println!("reserves: {:?}", reserves); //        99003495977396530955
    println!("supply_amount: {:?}", collateral); // 99002439790871899246
    println!("revenue_value: {:?}", revenue); //        1056186524631708
                                              // assert_eq!(borrowed, ManagedDecimal::from_raw_units(BigUint::zero(), 0usize));
    assert!(
        reserves > collateral.clone() + revenue,
        "Reserves are not enough"
    );
    state.withdraw_asset_den(
        &supplier,
        EGLD_TOKEN,
        collateral.into_raw_units().clone(),
        1,
    );
    let reserves = state.get_market_reserves(state.egld_market.clone());
    let revenue = state.get_market_revenue(state.egld_market.clone());
    println!("reserves: {:?}", reserves); //     1056186524631709
    println!("revenue_value: {:?}", revenue); // 1056186524631708

    assert!(reserves > revenue);
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

    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(50u64),
        2,
        EGLD_DECIMALS,
    );

    state.supply_asset(
        &OWNER_ADDRESS,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.world.current_block().block_timestamp(8000u64);
    state.update_borrows_with_debt(&borrower, 2);
    state.global_sync(&supplier, 1);
    state.global_sync(&OWNER_ADDRESS, 3);

    state
        .world
        .check_account(borrower)
        .esdt_nft_balance_and_attributes(
            ACCOUNT_TOKEN,
            2,
            BigUint::from(1u64),
            AccountAttributes::<StaticApi> {
                is_isolated_position: false,
                e_mode_category_id: 0,
                is_vault_position: false,
                mode: PositionMode::Normal,
                isolated_token: ManagedOption::none(),
            },
        );
    let borrow_amount_in_dollars = state.get_borrow_amount_for_token(2, EGLD_TOKEN);
    println!("borrow_amount_in_dollars: {:?}", borrow_amount_in_dollars);

    state.repay_asset_deno(
        &borrower,
        &EGLD_TOKEN,
        borrow_amount_in_dollars.into_raw_units().clone(),
        2,
    );
    let custom_error_message = format!("Token not existing in the account {}", EGLD_TOKEN.as_str());
    state.get_borrow_amount_for_token_non_existing(2, EGLD_TOKEN, custom_error_message.as_bytes());

    state.world.current_block().block_timestamp(1000000u64);
    state.update_borrows_with_debt(&borrower, 2);
    state.global_sync(&supplier, 1);
    state.global_sync(&supplier, 3);

    state.withdraw_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(5000u64),
        2,
        USDC_DECIMALS,
    );
    state
        .world
        .check_account(borrower)
        .esdt_nft_balance_and_attributes(
            ACCOUNT_TOKEN,
            2,
            BigUint::zero(),
            AccountAttributes::<StaticApi> {
                is_isolated_position: false,
                e_mode_category_id: 0,
                is_vault_position: false,
                mode: PositionMode::Normal,
                isolated_token: ManagedOption::none(),
            },
        );

    let total_collateral = state.get_total_collateral_in_egld(1);
    println!("total_collateral: {:?}", total_collateral);

    let supplied_collateral = state.get_collateral_amount_for_token(1, EGLD_TOKEN);
    println!("supplied_collateral: {:?}", supplied_collateral);

    state.withdraw_asset_den(
        &supplier,
        EGLD_TOKEN,
        supplied_collateral.into_raw_units().clone(),
        1,
    );
    let custom_error_message = format!("Token not existing in the account {}", EGLD_TOKEN.as_str());

    state.get_collateral_amount_for_token_non_existing(
        1,
        EGLD_TOKEN,
        custom_error_message.as_bytes(),
    );

    state.global_sync(&supplier, 3);
    let supplied_collateral = state.get_collateral_amount_for_token(3, EGLD_TOKEN);
    let reserves = state.get_market_reserves(state.egld_market.clone());
    let revenue = state.get_market_revenue(state.egld_market.clone());
    println!("supplied_collateral: {:?}", supplied_collateral.clone());
    println!("reserves           : {:?}", reserves);
    println!("revenue            : {:?}", revenue);
    println!(
        "dust            : {:?}",
        (reserves - supplied_collateral.clone() - revenue)
    );
    // state.claim_revenue(EGLD_TOKEN);
    // return;
    // 100.000056842393347411 // Actual Reserves
    // 100.000056842705420770 // Supplied
    // 100.000340855634615550 // Reserves
    //   0.000284013241268139 // Revenue
    state.withdraw_asset_den(
        &OWNER_ADDRESS,
        EGLD_TOKEN,
        supplied_collateral.into_raw_units().clone(),
        3,
    );
    state.get_collateral_amount_for_token_non_existing(
        3,
        EGLD_TOKEN,
        custom_error_message.as_bytes(),
    );

    let reserves = state.get_market_reserves(state.egld_market.clone());
    let revenue = state.get_market_revenue(state.egld_market.clone());
    assert!(reserves >= revenue);
    // state.claim_revenue(EGLD_TOKEN);
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
        BigUint::from(200u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    state.supply_asset(
        &borrower,
        XEGLD_TOKEN,
        BigUint::from(1500000u64),
        XEGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(160u64),
        2,
        EGLD_DECIMALS,
    );

    // Record initial amounts
    let initial_borrow = state.get_borrow_amount_for_token(2, EGLD_TOKEN);
    let initial_supply = state.get_collateral_amount_for_token(1, EGLD_TOKEN);
    let mut markets = MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN));
    let utilization = state.get_market_utilization(state.egld_market.clone());
    let borrow_rate = state.get_market_borrow_rate(state.egld_market.clone());
    println!("utilization: {:?}", utilization);
    println!("borrow_rate: {:?}", borrow_rate);
    // Simulate daily updates for a month
    // for day in 1..=SECONDS_PER_DAY {
    state
        .world
        .current_block()
        .block_timestamp(SECONDS_PER_YEAR);
    state.update_markets(&supplier, markets.clone());
    state.update_borrows_with_debt(&borrower, 2);
    state.global_sync(&supplier, 1);
    // }

    // Verify interest accrual
    let final_borrow = state.get_borrow_amount_for_token(2, EGLD_TOKEN);
    let final_supply = state.get_collateral_amount_for_token(1, EGLD_TOKEN);

    assert!(final_borrow > initial_borrow);
    assert!(final_supply > initial_supply);
    println!("borrow     principal: {:?}", initial_borrow);
    println!("borrow with interest: {:?}", final_borrow);
    state.repay_asset_deno(
        &borrower,
        &EGLD_TOKEN,
        final_borrow.into_raw_units().clone(),
        2,
    );

    let final_supply = state.get_collateral_amount_for_token(1, EGLD_TOKEN);
    let reserves = state.get_market_reserves(state.egld_market.clone());
    let revenue = state.get_market_revenue(state.egld_market.clone());
    println!("initial_supply: {:?}", initial_supply);
    println!("final_supply:   {:?}", final_supply);
    println!("reserves:       {:?}", reserves);
    println!("revenue:        {:?}", revenue);
    println!(
        "diff dust:    {:?}",
        reserves.into_signed() - final_supply.clone().into_signed() - revenue.into_signed()
    );
    state.withdraw_asset_den(
        &supplier,
        EGLD_TOKEN,
        final_supply.into_raw_units().clone(),
        1,
    );
    let reserves = state.get_market_reserves(state.egld_market.clone());
    let revenue = state.get_market_revenue(state.egld_market.clone());
    println!("reserves:       {:?}", reserves);
    println!("revenue:        {:?}", revenue);
    println!(
        "diff dust:    {:?}",
        reserves.into_signed() - revenue.into_signed()
    );
}

#[test]
fn test_interest_accrual_two_suppliers_at_different_times() {
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
        &borrower,
        USDC_TOKEN,
        BigUint::from(10000u64),
        USDC_DECIMALS,
        OptionalValue::Some(2),
        OptionalValue::None,
        false,
    );
    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(200u64),
        2,
        EGLD_DECIMALS,
    );
    let utilization_ratio = state.get_market_utilization(state.egld_market.clone());
    println!("utilization_ratio: {:?}", utilization_ratio);

    // Record initial amounts
    let initial_supply_borrower = state.get_collateral_amount_for_token(2, EGLD_TOKEN);
    let initial_supply_supplier = state.get_collateral_amount_for_token(1, EGLD_TOKEN);

    // Simulate hourly updates for 2 years
    for day in 1..=365 * 2 {
        state
            .world
            .current_block()
            .block_timestamp(day * SECONDS_PER_DAY);
    }
    state.global_sync(&borrower, 2);
    state.update_borrows_with_debt(&borrower, 2);
    state.global_sync(&supplier, 1);
    // Verify interest accrual
    let final_supply_borrower = state.get_collateral_amount_for_token(2, EGLD_TOKEN);
    let final_supply_supplier = state.get_collateral_amount_for_token(1, EGLD_TOKEN);
    let final_borrow_borrower = state.get_borrow_amount_for_token(2, EGLD_TOKEN);
    println!("final_borrow_borrower: {:?}", final_borrow_borrower);
    assert!(final_supply_borrower > initial_supply_borrower);
    assert!(final_supply_supplier > initial_supply_supplier);
    println!(
        "borrow_rate: {:?}",
        state.get_market_borrow_rate(state.egld_market.clone())
    );
    println!(
        "supply_rate: {:?}",
        state.get_market_supply_rate(state.egld_market.clone())
    );

    println!(
        "initial_supply_borrower: {:?} | final_supply_borrower: {:?}",
        initial_supply_borrower.clone(),
        final_supply_borrower.clone()
    );
    println!(
        "hex_initial_supply_borrower: {:?} | hex_final_supply_borrower: {:?}",
        initial_supply_borrower, final_supply_borrower
    );

    println!(
        "initial_supply_supplier: {:?} | final_supply_supplier: {:?}",
        initial_supply_supplier.clone(),
        final_supply_supplier.clone()
    );
    println!(
        "hex_initial_supply_supplier: {:?} | hex_final_supply_supplier: {:?}",
        initial_supply_supplier, final_supply_supplier
    );
}

#[test]
fn test_oracle_price_feed_lp() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_accounts(&mut state, supplier, borrower);

    let price = state.get_usd_price(LP_EGLD_TOKEN);
    println!("price: {:?}", price);
}

#[test]
fn test_update_asset_config_after_next_supply() {
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

    let initial_position = state.deposit_positions(1);
    let position_opt = initial_position.into_iter().find_map(|data| {
        let (token, position) = data.into_tuple();
        if token == EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()) {
            return Option::Some(position);
        } else {
            return Option::None;
        }
    });
    let position = position_opt.unwrap();
    let config = get_egld_config();

    state.edit_asset_config(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        &BigUint::from(5_000u64),
        &BigUint::from(5_500u64),
        &BigUint::from(555u64),
        &BigUint::from(600u64),
        config.config.is_isolated_asset,
        config.config.isolation_debt_ceiling_usd.into_raw_units(),
        config.config.is_siloed_borrowing,
        config.config.is_flashloanable,
        config.config.flashloan_fee.into_raw_units(),
        config.config.is_collateralizable,
        config.config.is_borrowable,
        config.config.isolation_borrow_enabled,
        &config.config.borrow_cap.unwrap_or(BigUint::from(0u64)),
        &config.config.supply_cap.unwrap_or(BigUint::from(0u64)),
        None,
    );

    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::None,
        false,
    );

    let initial_position = state.deposit_positions(1);
    let last_position_opt = initial_position.into_iter().find_map(|data| {
        let (token, position) = data.into_tuple();
        if token == EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()) {
            return Option::Some(position);
        } else {
            return Option::None;
        }
    });

    let last_position = last_position_opt.unwrap();
    assert!(position.loan_to_value != last_position.loan_to_value);
    assert!(position.liquidation_bonus != last_position.liquidation_bonus);
    assert!(position.liquidation_fees != last_position.liquidation_fees);
    assert!(position.liquidation_threshold == last_position.liquidation_threshold);
}

#[test]
fn test_update_asset_config_via_endpoint_only_safe_values() {
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

    let initial_position = state.deposit_positions(1);
    let position_opt = initial_position.into_iter().find_map(|data| {
        let (token, position) = data.into_tuple();
        if token == EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()) {
            return Option::Some(position);
        } else {
            return Option::None;
        }
    });
    let position = position_opt.unwrap();
    let config = get_egld_config();

    state.edit_asset_config(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        &BigUint::from(5_000u64),
        &BigUint::from(5_500u64),
        &BigUint::from(555u64),
        &BigUint::from(600u64),
        config.config.is_isolated_asset,
        config.config.isolation_debt_ceiling_usd.into_raw_units(),
        config.config.is_siloed_borrowing,
        config.config.is_flashloanable,
        config.config.flashloan_fee.into_raw_units(),
        config.config.is_collateralizable,
        config.config.is_borrowable,
        config.config.isolation_borrow_enabled,
        &config.config.borrow_cap.unwrap_or(BigUint::from(0u64)),
        &config.config.supply_cap.unwrap_or(BigUint::from(0u64)),
        None,
    );

    let mut nonces = MultiValueEncoded::new();
    nonces.push(1u64);
    state.update_account_threshold(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        false,
        nonces,
        None,
    );

    let initial_position = state.deposit_positions(1);
    let last_position_opt = initial_position.into_iter().find_map(|data| {
        let (token, position) = data.into_tuple();
        if token == EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()) {
            return Option::Some(position);
        } else {
            return Option::None;
        }
    });

    let last_position = last_position_opt.unwrap();
    assert!(position.loan_to_value != last_position.loan_to_value);
    assert!(position.liquidation_bonus != last_position.liquidation_bonus);
    assert!(position.liquidation_fees != last_position.liquidation_fees);
    assert!(position.liquidation_threshold == last_position.liquidation_threshold);
}

#[test]
fn test_update_asset_config_via_endpoint_only_rirsky_values() {
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

    let initial_position = state.deposit_positions(1);
    let position_opt = initial_position.into_iter().find_map(|data| {
        let (token, position) = data.into_tuple();
        if token == EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()) {
            return Option::Some(position);
        } else {
            return Option::None;
        }
    });
    let position = position_opt.unwrap();
    let config = get_egld_config();

    state.edit_asset_config(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        &BigUint::from(5_000u64),
        &BigUint::from(5_500u64),
        &BigUint::from(555u64),
        &BigUint::from(600u64),
        config.config.is_isolated_asset,
        config.config.isolation_debt_ceiling_usd.into_raw_units(),
        config.config.is_siloed_borrowing,
        config.config.is_flashloanable,
        config.config.flashloan_fee.into_raw_units(),
        config.config.is_collateralizable,
        config.config.is_borrowable,
        config.config.isolation_borrow_enabled,
        &config.config.borrow_cap.unwrap_or(BigUint::from(0u64)),
        &config.config.supply_cap.unwrap_or(BigUint::from(0u64)),
        None,
    );

    let mut nonces = MultiValueEncoded::new();
    nonces.push(1u64);
    state.update_account_threshold(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        true,
        nonces,
        None,
    );

    let initial_position = state.deposit_positions(1);
    let last_position_opt = initial_position.into_iter().find_map(|data| {
        let (token, position) = data.into_tuple();
        if token == EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()) {
            return Option::Some(position);
        } else {
            return Option::None;
        }
    });

    let last_position = last_position_opt.unwrap();
    assert!(position.loan_to_value == last_position.loan_to_value);
    assert!(position.liquidation_bonus == last_position.liquidation_bonus);
    assert!(position.liquidation_fees == last_position.liquidation_fees);
    assert!(position.liquidation_threshold != last_position.liquidation_threshold);
}

#[test]
fn test_update_asset_config_via_endpoint_only_rirsky_values_with_borrows_valid() {
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

    state.borrow_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(70u64),
        1,
        EGLD_DECIMALS,
    );

    let initial_position = state.deposit_positions(1);
    let position_opt = initial_position.into_iter().find_map(|data| {
        let (token, position) = data.into_tuple();
        if token == EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()) {
            return Option::Some(position);
        } else {
            return Option::None;
        }
    });
    let position = position_opt.unwrap();
    let config = get_egld_config();

    state.edit_asset_config(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        &BigUint::from(5_000u64),
        &BigUint::from(9_000u64),
        &BigUint::from(555u64),
        &BigUint::from(600u64),
        config.config.is_isolated_asset,
        config.config.isolation_debt_ceiling_usd.into_raw_units(),
        config.config.is_siloed_borrowing,
        config.config.is_flashloanable,
        config.config.flashloan_fee.into_raw_units(),
        config.config.is_collateralizable,
        config.config.is_borrowable,
        config.config.isolation_borrow_enabled,
        &config.config.borrow_cap.unwrap_or(BigUint::from(0u64)),
        &config.config.supply_cap.unwrap_or(BigUint::from(0u64)),
        None,
    );

    let mut nonces = MultiValueEncoded::new();
    nonces.push(1u64);
    state.update_account_threshold(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        true,
        nonces,
        None,
    );

    let initial_position = state.deposit_positions(1);
    let last_position_opt = initial_position.into_iter().find_map(|data| {
        let (token, position) = data.into_tuple();
        if token == EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()) {
            return Option::Some(position);
        } else {
            return Option::None;
        }
    });

    let last_position = last_position_opt.unwrap();
    assert!(position.loan_to_value == last_position.loan_to_value);
    assert!(position.liquidation_bonus == last_position.liquidation_bonus);
    assert!(position.liquidation_fees == last_position.liquidation_fees);
    assert!(position.liquidation_threshold != last_position.liquidation_threshold);
}

#[test]
fn test_update_asset_config_via_endpoint_only_rirsky_values_with_borrows_fail_health() {
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

    state.borrow_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(70u64),
        1,
        EGLD_DECIMALS,
    );

    let config = get_egld_config();

    state.edit_asset_config(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        &BigUint::from(5_000u64),
        &BigUint::from(5_500u64),
        &BigUint::from(555u64),
        &BigUint::from(600u64),
        config.config.is_isolated_asset,
        config.config.isolation_debt_ceiling_usd.into_raw_units(),
        config.config.is_siloed_borrowing,
        config.config.is_flashloanable,
        config.config.flashloan_fee.into_raw_units(),
        config.config.is_collateralizable,
        config.config.is_borrowable,
        config.config.isolation_borrow_enabled,
        &config.config.borrow_cap.unwrap_or(BigUint::from(0u64)),
        &config.config.supply_cap.unwrap_or(BigUint::from(0u64)),
        None,
    );

    let mut nonces = MultiValueEncoded::new();
    nonces.push(1u64);
    state.update_account_threshold(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        true,
        nonces,
        Some(ERROR_HEALTH_FACTOR_WITHDRAW),
    );
}

#[test]
fn test_update_asset_config_via_endpoint_throw_ltv_too_high() {
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

    let config = get_egld_config();

    state.edit_asset_config(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        &BigUint::from(5_000u64),
        &BigUint::from(4_000u64),
        &BigUint::from(555u64),
        &BigUint::from(600u64),
        config.config.is_isolated_asset,
        config.config.isolation_debt_ceiling_usd.into_raw_units(),
        config.config.is_siloed_borrowing,
        config.config.is_flashloanable,
        config.config.flashloan_fee.into_raw_units(),
        config.config.is_collateralizable,
        config.config.is_borrowable,
        config.config.isolation_borrow_enabled,
        &config.config.borrow_cap.unwrap_or(BigUint::from(0u64)),
        &config.config.supply_cap.unwrap_or(BigUint::from(0u64)),
        Some(ERROR_INVALID_LIQUIDATION_THRESHOLD),
    );
}
