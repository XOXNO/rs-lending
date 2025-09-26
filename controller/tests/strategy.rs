use controller::{
    PositionMode, ERROR_ASSETS_ARE_THE_SAME, ERROR_INVALID_NUMBER_OF_ESDT_TRANSFERS,
    ERROR_INVALID_POSITION_MODE, ERROR_MULTIPLY_REQUIRE_EXTRA_STEPS,
    ERROR_SWAP_COLLATERAL_NOT_SUPPORTED, ERROR_SWAP_DEBT_NOT_SUPPORTED,
};
use multiversx_sc::types::{
    EgldOrEsdtTokenIdentifier, EgldOrEsdtTokenPayment, ManagedArgBuffer, ManagedVec,
};
use multiversx_sc_scenario::{
    api::StaticApi,
    imports::{BigUint, OptionalValue, TestAddress, TestTokenIdentifier},
};
pub mod constants;
pub mod proxys;
pub mod setup;
use constants::*;
use setup::*;

fn leverage_steps(
    token: &TestTokenIdentifier,
    amount_raw: BigUint<StaticApi>,
) -> ManagedArgBuffer<StaticApi> {
    let mut steps = ManagedArgBuffer::<StaticApi>::new();
    steps.push_arg(EgldOrEsdtTokenIdentifier::<StaticApi>::from(
        token.as_bytes(),
    ));
    steps.push_arg(amount_raw);
    steps
}

fn single_payment(
    token: &TestTokenIdentifier,
    amount_raw: BigUint<StaticApi>,
) -> ManagedVec<StaticApi, EgldOrEsdtTokenPayment<StaticApi>> {
    let mut payments = ManagedVec::<StaticApi, EgldOrEsdtTokenPayment<StaticApi>>::new();
    payments.push(EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::from(token.as_bytes()),
        0,
        amount_raw,
    ));
    payments
}

fn account_nft_payment(
    account_nonce: u64,
) -> ManagedVec<StaticApi, EgldOrEsdtTokenPayment<StaticApi>> {
    let mut payments = ManagedVec::<StaticApi, EgldOrEsdtTokenPayment<StaticApi>>::new();
    payments.push(EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::from(ACCOUNT_TOKEN.as_bytes()),
        account_nonce,
        BigUint::from(1u64),
    ));
    payments
}

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
    state.assert_collateral_raw_eq(
        1,
        &XEGLD_TOKEN,
        scaled_amount(100, XEGLD_DECIMALS),
        "Supplier XEGLD liquidity should be recorded",
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
    state.assert_collateral_raw_eq(
        1,
        &EGLD_TOKEN,
        scaled_amount(100, EGLD_DECIMALS),
        "Supplier EGLD liquidity should be recorded",
    );

    // Borrower supplies EGLD as collateral with E-Mode category 1
    let steps = leverage_steps(
        &XEGLD_TOKEN,
        BigUint::<StaticApi>::from(100u64) * BigUint::from(WAD),
    );

    let payments = single_payment(&XEGLD_TOKEN, BigUint::from(20u64) * BigUint::from(WAD));
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

    state.assert_borrow_raw_eq(
        2,
        &EGLD_TOKEN,
        wanted_debt.clone(),
        "Multiply should record EGLD debt equal to requested leverage",
    );
    let market_revenue = state.market_revenue(state.egld_market.clone());
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
    state.assert_collateral_raw_eq(
        1,
        &XEGLD_TOKEN,
        scaled_amount(100, XEGLD_DECIMALS),
        "Supplier XEGLD liquidity should be recorded",
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
    state.assert_collateral_raw_eq(
        1,
        &EGLD_TOKEN,
        scaled_amount(100, EGLD_DECIMALS),
        "Supplier EGLD liquidity should be recorded",
    );

    // Borrower supplies EGLD as collateral with E-Mode category 1
    let steps = leverage_steps(
        &XEGLD_TOKEN,
        BigUint::<StaticApi>::from(100u64) * BigUint::from(WAD),
    );

    let payments = single_payment(&EGLD_TOKEN, BigUint::from(20u64) * BigUint::from(WAD));
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

    state.assert_borrow_raw_eq(
        2,
        &EGLD_TOKEN,
        wanted_debt.clone(),
        "Multiply should borrow EGLD equal to target leverage",
    );
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

    let total_debt = state.borrow_amount_for_token(2, EGLD_TOKEN);
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

    let total_debt = state.borrow_amount_for_token(2, EGLD_TOKEN);
    assert_eq!(total_debt.into_raw_units().clone(), wanted_debt);
    let market_revenue = state.market_revenue(state.egld_market.clone());
    assert_eq!(
        market_revenue.into_raw_units().clone(),
        BigUint::from(WAD) / 2u64
    );

    let nft_payment = account_nft_payment(2);
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
fn multiply_strategy_invalid_mode_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Prepare minimal steps; they won’t be used due to early revert
    let mut steps = ManagedArgBuffer::<StaticApi>::new();
    steps.push_arg(EgldOrEsdtTokenIdentifier::<StaticApi>::from(
        XEGLD_TOKEN.as_bytes(),
    ));
    steps.push_arg(BigUint::<StaticApi>::from(100u64) * BigUint::from(WAD));

    // Payment in collateral token to satisfy initial validations
    let mut payments = ManagedVec::<StaticApi, EgldOrEsdtTokenPayment<StaticApi>>::new();
    payments.push(EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::from(XEGLD_TOKEN.as_bytes()),
        0,
        BigUint::from(20u64) * BigUint::from(WAD),
    ));

    // Use PositionMode::Normal which is not allowed for multiply
    state.multiply_error(
        &borrower,
        1,
        &EgldOrEsdtTokenIdentifier::from(XEGLD_TOKEN.as_bytes()),
        BigUint::from(80u64) * BigUint::from(WAD),
        &EgldOrEsdtTokenIdentifier::from(EGLD_TOKEN.as_bytes()),
        PositionMode::Normal,
        steps,
        OptionalValue::None,
        payments,
        ERROR_INVALID_POSITION_MODE,
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

    let total_debt = state.borrow_amount_for_token(2, EGLD_TOKEN);
    assert_eq!(total_debt.into_raw_units().clone(), wanted_debt);
    let market_revenue = state.market_revenue(state.egld_market.clone());
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

    let total_debt = state.borrow_amount_for_token(2, EGLD_TOKEN);
    assert_eq!(total_debt.into_raw_units().clone(), wanted_debt);
    let market_revenue = state.market_revenue(state.egld_market.clone());
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
    let total_debt = state.borrow_amount_for_token(2, EGLD_TOKEN);
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
    let total_collateral = state.collateral_amount_for_token(2, XEGLD_TOKEN);
    println!("total_collateral: {:?}", total_collateral);
    let total_debt = state.borrow_amount_for_token(2, XEGLD_TOKEN);
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

    let total_debt = state.borrow_amount_for_token(2, EGLD_TOKEN);
    assert_eq!(total_debt.into_raw_units().clone(), wanted_debt);
    let market_revenue = state.market_revenue(state.egld_market.clone());
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
    let total_debt = state.borrow_amount_for_token(2, EGLD_TOKEN);
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
    let total_collateral = state.collateral_amount_for_token(2, XEGLD_TOKEN);
    println!("total_collateral: {:?}", total_collateral);
    let total_debt = state.borrow_amount_for_token(2, XEGLD_TOKEN);
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

    let total_debt = state.borrow_amount_for_token(2, EGLD_TOKEN);
    assert_eq!(total_debt.into_raw_units().clone(), wanted_debt);
    let market_revenue = state.market_revenue(state.egld_market.clone());
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
    let total_collateral = state.collateral_amount_for_token(2, XEGLD_TOKEN);
    state.swap_collateral(
        &borrower,
        &EgldOrEsdtTokenIdentifier::from(XEGLD_TOKEN.as_bytes()),
        total_collateral.into_raw_units().clone() / 5u64,
        &EgldOrEsdtTokenIdentifier::from(EGLD_TOKEN.as_bytes()),
        steps_swap,
        nft_payment,
    );
}

/// Tests that calling a strategy endpoint that requires the account NFT
/// with a non-NFT first payment triggers ERROR_INVALID_NUMBER_OF_ESDT_TRANSFERS
/// via the else-branch in validate_supply_payment.
#[test]
fn swap_debt_missing_account_nft_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Build a wrong first payment: regular token (XEGLD), not the account NFT
    let mut wrong_payment = ManagedVec::<StaticApi, EgldOrEsdtTokenPayment<StaticApi>>::new();
    wrong_payment.push(EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::from(XEGLD_TOKEN.as_bytes()),
        0,
        BigUint::from(1u64) * BigUint::from(WAD),
    ));

    // Steps buffer (won't be reached; error triggers in validate_supply_payment)
    let mut steps = ManagedArgBuffer::<StaticApi>::new();
    steps.push_arg(EgldOrEsdtTokenIdentifier::<StaticApi>::from(
        EGLD_TOKEN.as_bytes(),
    ));
    steps.push_arg(BigUint::<StaticApi>::from(1u64) * BigUint::from(WAD));

    let new_debt_amount_raw = BigUint::from(1u64) * BigUint::from(WAD);

    // Expect error because account NFT is required but first payment is not the NFT
    state.swap_debt_error(
        &borrower,
        &EgldOrEsdtTokenIdentifier::from(EGLD_TOKEN.as_bytes()),
        &new_debt_amount_raw,
        &EgldOrEsdtTokenIdentifier::from(XEGLD_TOKEN.as_bytes()),
        steps,
        wrong_payment,
        ERROR_INVALID_NUMBER_OF_ESDT_TRANSFERS,
    );
}

/// Tests that multiply with a random payment token but missing extra steps
/// fails with ERROR_MULTIPLY_REQUIRE_EXTRA_STEPS.
#[test]
fn multiply_random_payment_missing_steps_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Borrower pays in a random token (XOXNO), different from collateral and debt
    let mut payments = ManagedVec::<StaticApi, EgldOrEsdtTokenPayment<StaticApi>>::new();
    payments.push(EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::from(XOXNO_TOKEN.as_bytes()),
        0,
        BigUint::from(20u64) * BigUint::from(WAD),
    ));

    // Steps for debt->collateral (won't be used due to error)
    let mut steps = ManagedArgBuffer::<StaticApi>::new();
    steps.push_arg(EgldOrEsdtTokenIdentifier::<StaticApi>::from(
        XEGLD_TOKEN.as_bytes(),
    ));
    steps.push_arg(BigUint::<StaticApi>::from(100u64) * BigUint::from(WAD));

    // Missing steps_payment on purpose to trigger the require
    state.multiply_error(
        &borrower,
        1,
        &EgldOrEsdtTokenIdentifier::from(XEGLD_TOKEN.as_bytes()),
        BigUint::from(80u64) * BigUint::from(WAD),
        &EgldOrEsdtTokenIdentifier::from(EGLD_TOKEN.as_bytes()),
        PositionMode::Multiply,
        steps,
        OptionalValue::None, // missing extra steps for random payment
        payments,
        ERROR_MULTIPLY_REQUIRE_EXTRA_STEPS,
    );
}

/// Tests that swap_debt with the same existing and new debt token
/// fails with ERROR_SWAP_DEBT_NOT_SUPPORTED.
#[test]
fn swap_debt_same_token_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Create a simple account for borrower to obtain NFT
    state.supply_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(1u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Build NFT payment (account nonce = 1 as first minted for borrower)
    let mut nft_payment = ManagedVec::<StaticApi, EgldOrEsdtTokenPayment<StaticApi>>::new();
    nft_payment.push(EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::from(ACCOUNT_TOKEN.as_bytes()),
        1,
        BigUint::from(1u64),
    ));

    // Steps buffer
    let mut steps = ManagedArgBuffer::<StaticApi>::new();
    steps.push_arg(EgldOrEsdtTokenIdentifier::<StaticApi>::from(
        EGLD_TOKEN.as_bytes(),
    ));
    steps.push_arg(BigUint::<StaticApi>::from(1u64) * BigUint::from(WAD));

    // existing_debt_token == new_debt_token triggers error
    state.swap_debt_error(
        &borrower,
        &EgldOrEsdtTokenIdentifier::from(EGLD_TOKEN.as_bytes()),
        &(BigUint::from(1u64) * BigUint::from(WAD)),
        &EgldOrEsdtTokenIdentifier::from(EGLD_TOKEN.as_bytes()),
        steps,
        nft_payment,
        ERROR_SWAP_DEBT_NOT_SUPPORTED,
    );
}

/// Tests that swap_debt involving a siloed token fails with ERROR_SWAP_DEBT_NOT_SUPPORTED.
#[test]
fn swap_debt_with_siloed_token_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Create account for borrower
    state.supply_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(1u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    let mut nft_payment = ManagedVec::<StaticApi, EgldOrEsdtTokenPayment<StaticApi>>::new();
    nft_payment.push(EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::from(ACCOUNT_TOKEN.as_bytes()),
        1,
        BigUint::from(1u64),
    ));

    let mut steps = ManagedArgBuffer::<StaticApi>::new();
    steps.push_arg(EgldOrEsdtTokenIdentifier::<StaticApi>::from(
        SILOED_TOKEN.as_bytes(),
    ));
    steps.push_arg(BigUint::<StaticApi>::from(1u64) * BigUint::from(WAD));

    // Using a siloed token as new debt triggers the siloed check
    state.swap_debt_error(
        &borrower,
        &EgldOrEsdtTokenIdentifier::from(EGLD_TOKEN.as_bytes()),
        &(BigUint::from(1u64) * BigUint::from(WAD)),
        &EgldOrEsdtTokenIdentifier::from(SILOED_TOKEN.as_bytes()),
        steps,
        nft_payment,
        ERROR_SWAP_DEBT_NOT_SUPPORTED,
    );
}

/// Tests that swap_collateral with same current and new asset fails with ERROR_ASSETS_ARE_THE_SAME.
#[test]
fn swap_collateral_same_token_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Create account for borrower
    state.supply_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(1u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    let mut nft_payment = ManagedVec::<StaticApi, EgldOrEsdtTokenPayment<StaticApi>>::new();
    nft_payment.push(EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::from(ACCOUNT_TOKEN.as_bytes()),
        1,
        BigUint::from(1u64),
    ));

    let mut steps = ManagedArgBuffer::<StaticApi>::new();
    steps.push_arg(EgldOrEsdtTokenIdentifier::<StaticApi>::from(
        EGLD_TOKEN.as_bytes(),
    ));
    steps.push_arg(BigUint::<StaticApi>::from(1u64) * BigUint::from(WAD));

    // Same asset for current and new triggers ERROR_ASSETS_ARE_THE_SAME
    state.swap_collateral_error(
        &borrower,
        &EgldOrEsdtTokenIdentifier::from(EGLD_TOKEN.as_bytes()),
        BigUint::from(1u64) * BigUint::from(WAD),
        &EgldOrEsdtTokenIdentifier::from(EGLD_TOKEN.as_bytes()),
        steps,
        nft_payment,
        ERROR_ASSETS_ARE_THE_SAME,
    );
}

/// Tests that swap_collateral fails when account is isolated.
#[test]
fn swap_collateral_isolated_account_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Create isolated account by supplying isolated asset
    state.supply_asset(
        &borrower,
        ISOLATED_TOKEN,
        BigUint::from(1u64),
        ISOLATED_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    let mut nft_payment = ManagedVec::<StaticApi, EgldOrEsdtTokenPayment<StaticApi>>::new();
    nft_payment.push(EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::from(ACCOUNT_TOKEN.as_bytes()),
        1,
        BigUint::from(1u64),
    ));

    let mut steps = ManagedArgBuffer::<StaticApi>::new();
    steps.push_arg(EgldOrEsdtTokenIdentifier::<StaticApi>::from(
        EGLD_TOKEN.as_bytes(),
    ));
    steps.push_arg(BigUint::<StaticApi>::from(1u64) * BigUint::from(WAD));

    // Different assets but isolated account should still fail
    state.swap_collateral_error(
        &borrower,
        &EgldOrEsdtTokenIdentifier::from(EGLD_TOKEN.as_bytes()),
        BigUint::from(1u64) * BigUint::from(WAD),
        &EgldOrEsdtTokenIdentifier::from(XEGLD_TOKEN.as_bytes()),
        steps,
        nft_payment,
        ERROR_SWAP_COLLATERAL_NOT_SUPPORTED,
    );
}

/// Tests that swap_collateral to an isolated target asset fails for regular (non-isolated) accounts.
#[test]
fn swap_collateral_target_isolated_asset_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Create regular (non-isolated) account
    state.supply_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(1u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    let mut nft_payment = ManagedVec::<StaticApi, EgldOrEsdtTokenPayment<StaticApi>>::new();
    nft_payment.push(EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::from(ACCOUNT_TOKEN.as_bytes()),
        1,
        BigUint::from(1u64),
    ));

    let mut steps = ManagedArgBuffer::<StaticApi>::new();
    steps.push_arg(EgldOrEsdtTokenIdentifier::<StaticApi>::from(
        ISOLATED_TOKEN.as_bytes(),
    ));
    steps.push_arg(BigUint::<StaticApi>::from(1u64) * BigUint::from(WAD));

    // Swapping to an isolated asset should be blocked
    state.swap_collateral_error(
        &borrower,
        &EgldOrEsdtTokenIdentifier::from(EGLD_TOKEN.as_bytes()),
        BigUint::from(1u64) * BigUint::from(WAD),
        &EgldOrEsdtTokenIdentifier::from(ISOLATED_TOKEN.as_bytes()),
        steps,
        nft_payment,
        ERROR_SWAP_COLLATERAL_NOT_SUPPORTED,
    );
}
