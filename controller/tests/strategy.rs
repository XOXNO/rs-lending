use controller::PositionMode;
use multiversx_sc::types::{
    EgldOrEsdtTokenIdentifier, EgldOrEsdtTokenPayment, ManagedArgBuffer, ManagedVec,
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

#[test]
fn multiply_strategy_success_payment_as_collateral_flow() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Supplier provides XEGLD liquidity with E-Mode category 1
    state.supply_asset(
        &supplier,
        XEGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::Some(1), // E-Mode category 1
        false,
    );
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::Some(1), // E-Mode category 1
        false,
    );

    // Borrower supplies EGLD as collateral with E-Mode category 1
    let mut steps = ManagedArgBuffer::<StaticApi>::new();
    steps.push_arg(EgldOrEsdtTokenIdentifier::<StaticApi>::from(
        XEGLD_TOKEN.as_bytes(),
    ));
    steps.push_arg(BigUint::<StaticApi>::from(100u64) * BigUint::from(WAD));

    let mut payments = ManagedVec::<StaticApi, EgldOrEsdtTokenPayment<StaticApi>>::new();
    payments.push(EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::from(XEGLD_TOKEN.as_bytes()),
        0,
        BigUint::from(20u64) * BigUint::from(WAD),
    ));
    let wanted_debt = BigUint::from(100u64) * BigUint::from(WAD);
    state.multiply(
        &borrower,
        1,
        &EgldOrEsdtTokenIdentifier::from(XEGLD_TOKEN.as_bytes()),
        wanted_debt.clone(),
        &EgldOrEsdtTokenIdentifier::from(EGLD_TOKEN.as_bytes()),
        PositionMode::Multiply,
        steps,
        OptionalValue::None,
        payments,
    );

    let total_debt = state.get_borrow_amount_for_token(2, EGLD_TOKEN);
    assert_eq!(total_debt.into_raw_units().clone(), wanted_debt);
    let market_revenue = state.get_market_revenue(state.egld_market.clone());
    assert_eq!(
        market_revenue.into_raw_units().clone(),
        BigUint::from(WAD) / 2u64
    );
}

#[test]
fn multiply_strategy_success_payment_as_debt_flow() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Supplier provides XEGLD liquidity with E-Mode category 1
    state.supply_asset(
        &supplier,
        XEGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::Some(1), // E-Mode category 1
        false,
    );
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::Some(1), // E-Mode category 1
        false,
    );

    // Borrower supplies EGLD as collateral with E-Mode category 1
    let mut steps = ManagedArgBuffer::<StaticApi>::new();
    steps.push_arg(EgldOrEsdtTokenIdentifier::<StaticApi>::from(
        XEGLD_TOKEN.as_bytes(),
    ));
    steps.push_arg(BigUint::<StaticApi>::from(100u64) * BigUint::from(WAD));

    let mut payments = ManagedVec::<StaticApi, EgldOrEsdtTokenPayment<StaticApi>>::new();
    payments.push(EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::from(EGLD_TOKEN.as_bytes()),
        0,
        BigUint::from(20u64) * BigUint::from(WAD),
    ));
    let wanted_debt = BigUint::from(80u64) * BigUint::from(WAD);
    state.multiply(
        &borrower,
        1,
        &EgldOrEsdtTokenIdentifier::from(XEGLD_TOKEN.as_bytes()),
        wanted_debt.clone(),
        &EgldOrEsdtTokenIdentifier::from(EGLD_TOKEN.as_bytes()),
        PositionMode::Multiply,
        steps,
        OptionalValue::None,
        payments,
    );

    let total_debt = state.get_borrow_amount_for_token(2, EGLD_TOKEN);
    assert_eq!(total_debt.into_raw_units().clone(), wanted_debt);
}

#[test]
fn multiply_strategy_success_payment_as_random_token_flow() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Supplier provides XEGLD liquidity with E-Mode category 1
    state.supply_asset(
        &supplier,
        XEGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::Some(1), // E-Mode category 1
        false,
    );
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::Some(1), // E-Mode category 1
        false,
    );

    // Borrower supplies EGLD as collateral with E-Mode category 1
    let mut steps = ManagedArgBuffer::<StaticApi>::new();
    steps.push_arg(EgldOrEsdtTokenIdentifier::<StaticApi>::from(
        XEGLD_TOKEN.as_bytes(),
    ));
    steps.push_arg(BigUint::<StaticApi>::from(100u64) * BigUint::from(WAD));

    let mut steps_last = ManagedArgBuffer::<StaticApi>::new();
    steps_last.push_arg(EgldOrEsdtTokenIdentifier::<StaticApi>::from(
        XEGLD_TOKEN.as_bytes(),
    ));
    steps_last.push_arg(BigUint::<StaticApi>::from(20u64) * BigUint::from(WAD));

    let mut payments = ManagedVec::<StaticApi, EgldOrEsdtTokenPayment<StaticApi>>::new();
    payments.push(EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::from(XOXNO_TOKEN.as_bytes()),
        0,
        BigUint::from(20u64) * BigUint::from(WAD),
    ));
    let wanted_debt = BigUint::from(80u64) * BigUint::from(WAD);
    state.multiply(
        &borrower,
        1,
        &EgldOrEsdtTokenIdentifier::from(XEGLD_TOKEN.as_bytes()),
        wanted_debt.clone(),
        &EgldOrEsdtTokenIdentifier::from(EGLD_TOKEN.as_bytes()),
        PositionMode::Multiply,
        steps,
        OptionalValue::Some(steps_last),
        payments,
    );

    let total_debt = state.get_borrow_amount_for_token(2, EGLD_TOKEN);
    assert_eq!(total_debt.into_raw_units().clone(), wanted_debt);
}

#[test]
fn multiply_strategy_success_payment_as_collateral_flow_increase_leverage() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Supplier provides XEGLD liquidity with E-Mode category 1
    state.supply_asset(
        &supplier,
        XEGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::Some(1), // E-Mode category 1
        false,
    );
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(200u64),
        EGLD_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::Some(1), // E-Mode category 1
        false,
    );

    // Borrower supplies EGLD as collateral with E-Mode category 1
    let mut steps = ManagedArgBuffer::<StaticApi>::new();
    steps.push_arg(EgldOrEsdtTokenIdentifier::<StaticApi>::from(
        XEGLD_TOKEN.as_bytes(),
    ));
    steps.push_arg(BigUint::<StaticApi>::from(100u64) * BigUint::from(WAD));

    let mut payments = ManagedVec::<StaticApi, EgldOrEsdtTokenPayment<StaticApi>>::new();
    payments.push(EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::from(XEGLD_TOKEN.as_bytes()),
        0,
        BigUint::from(20u64) * BigUint::from(WAD),
    ));
    let wanted_debt = BigUint::from(100u64) * BigUint::from(WAD);
    state.multiply(
        &borrower,
        1,
        &EgldOrEsdtTokenIdentifier::from(XEGLD_TOKEN.as_bytes()),
        wanted_debt.clone(),
        &EgldOrEsdtTokenIdentifier::from(EGLD_TOKEN.as_bytes()),
        PositionMode::Multiply,
        steps.clone(),
        OptionalValue::None,
        payments,
    );

    let total_debt = state.get_borrow_amount_for_token(2, EGLD_TOKEN);
    assert_eq!(total_debt.into_raw_units().clone(), wanted_debt);
    let market_revenue = state.get_market_revenue(state.egld_market.clone());
    assert_eq!(
        market_revenue.into_raw_units().clone(),
        BigUint::from(WAD) / 2u64
    );

    let mut nft_payment = ManagedVec::<StaticApi, EgldOrEsdtTokenPayment<StaticApi>>::new();
    nft_payment.push(EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::from(ACCOUNT_TOKEN.as_bytes()),
        2,
        BigUint::from(1u64),
    ));
    state.multiply(
        &borrower,
        1,
        &EgldOrEsdtTokenIdentifier::from(XEGLD_TOKEN.as_bytes()),
        wanted_debt.clone(),
        &EgldOrEsdtTokenIdentifier::from(EGLD_TOKEN.as_bytes()),
        PositionMode::Multiply,
        steps,
        OptionalValue::None,
        nft_payment,
    );
}

#[test]
fn swap_debt() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Supplier provides XEGLD liquidity with E-Mode category 1
    state.supply_asset(
        &supplier,
        XEGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::Some(1), // E-Mode category 1
        false,
    );
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(200u64),
        EGLD_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::Some(1), // E-Mode category 1
        false,
    );

    // Borrower supplies EGLD as collateral with E-Mode category 1
    let mut steps = ManagedArgBuffer::<StaticApi>::new();
    steps.push_arg(EgldOrEsdtTokenIdentifier::<StaticApi>::from(
        XEGLD_TOKEN.as_bytes(),
    ));
    steps.push_arg(BigUint::<StaticApi>::from(100u64) * BigUint::from(WAD));

    let mut payments = ManagedVec::<StaticApi, EgldOrEsdtTokenPayment<StaticApi>>::new();
    payments.push(EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::from(XEGLD_TOKEN.as_bytes()),
        0,
        BigUint::from(20u64) * BigUint::from(WAD),
    ));
    let wanted_debt = BigUint::from(100u64) * BigUint::from(WAD);
    state.multiply(
        &borrower,
        1,
        &EgldOrEsdtTokenIdentifier::from(XEGLD_TOKEN.as_bytes()),
        wanted_debt.clone(),
        &EgldOrEsdtTokenIdentifier::from(EGLD_TOKEN.as_bytes()),
        PositionMode::Multiply,
        steps.clone(),
        OptionalValue::None,
        payments,
    );

    let total_debt = state.get_borrow_amount_for_token(2, EGLD_TOKEN);
    assert_eq!(total_debt.into_raw_units().clone(), wanted_debt);
    let market_revenue = state.get_market_revenue(state.egld_market.clone());
    assert_eq!(
        market_revenue.into_raw_units().clone(),
        BigUint::from(WAD) / 2u64
    );

    let mut nft_payment = ManagedVec::<StaticApi, EgldOrEsdtTokenPayment<StaticApi>>::new();
    nft_payment.push(EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::from(ACCOUNT_TOKEN.as_bytes()),
        2,
        BigUint::from(1u64),
    ));
    state.multiply(
        &borrower,
        1,
        &EgldOrEsdtTokenIdentifier::from(XEGLD_TOKEN.as_bytes()),
        wanted_debt.clone(),
        &EgldOrEsdtTokenIdentifier::from(EGLD_TOKEN.as_bytes()),
        PositionMode::Multiply,
        steps,
        OptionalValue::None,
        nft_payment.clone(),
    );

    let mut steps_swap = ManagedArgBuffer::<StaticApi>::new();
    steps_swap.push_arg(EgldOrEsdtTokenIdentifier::<StaticApi>::from(
        EGLD_TOKEN.as_bytes(),
    ));
    steps_swap.push_arg(BigUint::<StaticApi>::from(100u64) * BigUint::from(WAD));
    state.swap_debt(
        &borrower,
        &EgldOrEsdtTokenIdentifier::from(EGLD_TOKEN.as_bytes()),
        &wanted_debt,
        &EgldOrEsdtTokenIdentifier::from(XEGLD_TOKEN.as_bytes()),
        steps_swap,
        nft_payment,
    );
}

#[test]
fn repay_debt_with_collateral_full_close_position() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Supplier provides XEGLD liquidity with E-Mode category 1
    state.supply_asset(
        &supplier,
        XEGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::Some(1), // E-Mode category 1
        false,
    );
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(200u64),
        EGLD_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::Some(1), // E-Mode category 1
        false,
    );

    // Borrower supplies EGLD as collateral with E-Mode category 1
    let mut steps = ManagedArgBuffer::<StaticApi>::new();
    steps.push_arg(EgldOrEsdtTokenIdentifier::<StaticApi>::from(
        XEGLD_TOKEN.as_bytes(),
    ));
    steps.push_arg(BigUint::<StaticApi>::from(100u64) * BigUint::from(WAD));

    let mut payments = ManagedVec::<StaticApi, EgldOrEsdtTokenPayment<StaticApi>>::new();
    payments.push(EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::from(XEGLD_TOKEN.as_bytes()),
        0,
        BigUint::from(20u64) * BigUint::from(WAD),
    ));
    let wanted_debt = BigUint::from(100u64) * BigUint::from(WAD);
    state.multiply(
        &borrower,
        1,
        &EgldOrEsdtTokenIdentifier::from(XEGLD_TOKEN.as_bytes()),
        wanted_debt.clone(),
        &EgldOrEsdtTokenIdentifier::from(EGLD_TOKEN.as_bytes()),
        PositionMode::Multiply,
        steps.clone(),
        OptionalValue::None,
        payments,
    );

    let total_debt = state.get_borrow_amount_for_token(2, EGLD_TOKEN);
    assert_eq!(total_debt.into_raw_units().clone(), wanted_debt);
    let market_revenue = state.get_market_revenue(state.egld_market.clone());
    assert_eq!(
        market_revenue.into_raw_units().clone(),
        BigUint::from(WAD) / 2u64
    );

    let mut nft_payment = ManagedVec::<StaticApi, EgldOrEsdtTokenPayment<StaticApi>>::new();
    nft_payment.push(EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::from(ACCOUNT_TOKEN.as_bytes()),
        2,
        BigUint::from(1u64),
    ));
    state.multiply(
        &borrower,
        1,
        &EgldOrEsdtTokenIdentifier::from(XEGLD_TOKEN.as_bytes()),
        wanted_debt.clone(),
        &EgldOrEsdtTokenIdentifier::from(EGLD_TOKEN.as_bytes()),
        PositionMode::Multiply,
        steps,
        OptionalValue::None,
        nft_payment.clone(),
    );
    let total_debt = state.get_borrow_amount_for_token(2, EGLD_TOKEN);
    println!("total_debt: {:?}", total_debt);
    let mut steps_swap = ManagedArgBuffer::<StaticApi>::new();
    steps_swap.push_arg(EgldOrEsdtTokenIdentifier::<StaticApi>::from(
        EGLD_TOKEN.as_bytes(),
    ));
    steps_swap.push_arg(total_debt.into_raw_units().clone());
    state.swap_debt(
        &borrower,
        &EgldOrEsdtTokenIdentifier::from(EGLD_TOKEN.as_bytes()),
        &wanted_debt,
        &EgldOrEsdtTokenIdentifier::from(XEGLD_TOKEN.as_bytes()),
        steps_swap.clone(),
        nft_payment.clone(),
    );
    let total_collateral = state.get_collateral_amount_for_token(2, XEGLD_TOKEN);
    println!("total_collateral: {:?}", total_collateral);
    let total_debt = state.get_borrow_amount_for_token(2, XEGLD_TOKEN);
    println!("total_debt: {:?}", total_debt);
    let mut repay_steps = ManagedArgBuffer::<StaticApi>::new();
    repay_steps.push_arg(EgldOrEsdtTokenIdentifier::<StaticApi>::from(
        XEGLD_TOKEN.as_bytes(),
    ));
    repay_steps.push_arg(total_debt.into_raw_units().clone());
    state.repay_debt_with_collateral(
        &borrower,
        &EgldOrEsdtTokenIdentifier::from(XEGLD_TOKEN.as_bytes()),
        total_collateral.into_raw_units().clone() - 1u64,
        &EgldOrEsdtTokenIdentifier::from(XEGLD_TOKEN.as_bytes()),
        true,
        OptionalValue::Some(repay_steps),
        nft_payment,
    );
}

#[test]
fn repay_debt_with_collateral_partial() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Supplier provides XEGLD liquidity with E-Mode category 1
    state.supply_asset(
        &supplier,
        XEGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::Some(1), // E-Mode category 1
        false,
    );
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(200u64),
        EGLD_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::Some(1), // E-Mode category 1
        false,
    );

    // Borrower supplies EGLD as collateral with E-Mode category 1
    let mut steps = ManagedArgBuffer::<StaticApi>::new();
    steps.push_arg(EgldOrEsdtTokenIdentifier::<StaticApi>::from(
        XEGLD_TOKEN.as_bytes(),
    ));
    steps.push_arg(BigUint::<StaticApi>::from(100u64) * BigUint::from(WAD));

    let mut payments = ManagedVec::<StaticApi, EgldOrEsdtTokenPayment<StaticApi>>::new();
    payments.push(EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::from(XEGLD_TOKEN.as_bytes()),
        0,
        BigUint::from(20u64) * BigUint::from(WAD),
    ));
    let wanted_debt = BigUint::from(100u64) * BigUint::from(WAD);
    state.multiply(
        &borrower,
        1,
        &EgldOrEsdtTokenIdentifier::from(XEGLD_TOKEN.as_bytes()),
        wanted_debt.clone(),
        &EgldOrEsdtTokenIdentifier::from(EGLD_TOKEN.as_bytes()),
        PositionMode::Multiply,
        steps.clone(),
        OptionalValue::None,
        payments,
    );

    let total_debt = state.get_borrow_amount_for_token(2, EGLD_TOKEN);
    assert_eq!(total_debt.into_raw_units().clone(), wanted_debt);
    let market_revenue = state.get_market_revenue(state.egld_market.clone());
    assert_eq!(
        market_revenue.into_raw_units().clone(),
        BigUint::from(WAD) / 2u64
    );

    let mut nft_payment = ManagedVec::<StaticApi, EgldOrEsdtTokenPayment<StaticApi>>::new();
    nft_payment.push(EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::from(ACCOUNT_TOKEN.as_bytes()),
        2,
        BigUint::from(1u64),
    ));
    state.multiply(
        &borrower,
        1,
        &EgldOrEsdtTokenIdentifier::from(XEGLD_TOKEN.as_bytes()),
        wanted_debt.clone(),
        &EgldOrEsdtTokenIdentifier::from(EGLD_TOKEN.as_bytes()),
        PositionMode::Multiply,
        steps,
        OptionalValue::None,
        nft_payment.clone(),
    );
    let total_debt = state.get_borrow_amount_for_token(2, EGLD_TOKEN);
    println!("total_debt: {:?}", total_debt);
    let mut steps_swap = ManagedArgBuffer::<StaticApi>::new();
    steps_swap.push_arg(EgldOrEsdtTokenIdentifier::<StaticApi>::from(
        EGLD_TOKEN.as_bytes(),
    ));
    steps_swap.push_arg(total_debt.into_raw_units().clone());
    state.swap_debt(
        &borrower,
        &EgldOrEsdtTokenIdentifier::from(EGLD_TOKEN.as_bytes()),
        &wanted_debt,
        &EgldOrEsdtTokenIdentifier::from(XEGLD_TOKEN.as_bytes()),
        steps_swap.clone(),
        nft_payment.clone(),
    );
    let total_collateral = state.get_collateral_amount_for_token(2, XEGLD_TOKEN);
    println!("total_collateral: {:?}", total_collateral);
    let total_debt = state.get_borrow_amount_for_token(2, XEGLD_TOKEN);
    println!("total_debt: {:?}", total_debt);
    let mut repay_steps = ManagedArgBuffer::<StaticApi>::new();
    repay_steps.push_arg(EgldOrEsdtTokenIdentifier::<StaticApi>::from(
        XEGLD_TOKEN.as_bytes(),
    ));
    repay_steps.push_arg(total_debt.into_raw_units().clone() / 5u64);
    state.repay_debt_with_collateral(
        &borrower,
        &EgldOrEsdtTokenIdentifier::from(XEGLD_TOKEN.as_bytes()),
        total_collateral.into_raw_units().clone() / 5u64,
        &EgldOrEsdtTokenIdentifier::from(XEGLD_TOKEN.as_bytes()),
        false,
        OptionalValue::Some(repay_steps),
        nft_payment,
    );
}

#[test]
fn swap_collateral() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Supplier provides XEGLD liquidity with E-Mode category 1
    state.supply_asset(
        &supplier,
        XEGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::Some(1), // E-Mode category 1
        false,
    );
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(200u64),
        EGLD_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::Some(1), // E-Mode category 1
        false,
    );

    // Borrower supplies EGLD as collateral with E-Mode category 1
    let mut steps = ManagedArgBuffer::<StaticApi>::new();
    steps.push_arg(EgldOrEsdtTokenIdentifier::<StaticApi>::from(
        XEGLD_TOKEN.as_bytes(),
    ));
    steps.push_arg(BigUint::<StaticApi>::from(100u64) * BigUint::from(WAD));

    let mut payments = ManagedVec::<StaticApi, EgldOrEsdtTokenPayment<StaticApi>>::new();
    payments.push(EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::from(XEGLD_TOKEN.as_bytes()),
        0,
        BigUint::from(20u64) * BigUint::from(WAD),
    ));
    let wanted_debt = BigUint::from(100u64) * BigUint::from(WAD);
    state.multiply(
        &borrower,
        1,
        &EgldOrEsdtTokenIdentifier::from(XEGLD_TOKEN.as_bytes()),
        wanted_debt.clone(),
        &EgldOrEsdtTokenIdentifier::from(EGLD_TOKEN.as_bytes()),
        PositionMode::Multiply,
        steps.clone(),
        OptionalValue::None,
        payments,
    );

    let total_debt = state.get_borrow_amount_for_token(2, EGLD_TOKEN);
    assert_eq!(total_debt.into_raw_units().clone(), wanted_debt);
    let market_revenue = state.get_market_revenue(state.egld_market.clone());
    assert_eq!(
        market_revenue.into_raw_units().clone(),
        BigUint::from(WAD) / 2u64
    );

    let mut nft_payment = ManagedVec::<StaticApi, EgldOrEsdtTokenPayment<StaticApi>>::new();
    nft_payment.push(EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::from(ACCOUNT_TOKEN.as_bytes()),
        2,
        BigUint::from(1u64),
    ));
    state.multiply(
        &borrower,
        1,
        &EgldOrEsdtTokenIdentifier::from(XEGLD_TOKEN.as_bytes()),
        wanted_debt.clone(),
        &EgldOrEsdtTokenIdentifier::from(EGLD_TOKEN.as_bytes()),
        PositionMode::Multiply,
        steps,
        OptionalValue::None,
        nft_payment.clone(),
    );

    let mut steps_swap = ManagedArgBuffer::<StaticApi>::new();
    steps_swap.push_arg(EgldOrEsdtTokenIdentifier::<StaticApi>::from(
        EGLD_TOKEN.as_bytes(),
    ));
    steps_swap.push_arg(BigUint::<StaticApi>::from(100u64) * BigUint::from(WAD));
    let total_collateral = state.get_collateral_amount_for_token(2, XEGLD_TOKEN);
    state.swap_collateral(
        &borrower,
        &EgldOrEsdtTokenIdentifier::from(XEGLD_TOKEN.as_bytes()),
        total_collateral.into_raw_units().clone() / 5u64,
        &EgldOrEsdtTokenIdentifier::from(EGLD_TOKEN.as_bytes()),
        steps_swap,
        nft_payment,
    );
}
