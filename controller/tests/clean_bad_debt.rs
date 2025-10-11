use common_constants::RAY;
use multiversx_sc::types::{EgldOrEsdtTokenIdentifier, MultiValueEncoded};
use multiversx_sc_scenario::imports::{BigUint, OptionalValue, StaticApi, TestAddress};

pub mod constants;
pub mod proxys;
pub mod setup;
use constants::*;
use setup::*;

fn small_ray_tolerance() -> BigUint<StaticApi> {
    // ~1e22 raw (1e-5 of 1 RAY) as used in other tests
    BigUint::from(10u64).pow(22)
}

fn half_up_div(numer: &BigUint<StaticApi>, denom: &BigUint<StaticApi>) -> BigUint<StaticApi> {
    if denom == &BigUint::zero() {
        return BigUint::zero();
    }
    (numer + &(denom / 2u64)) / denom
}

#[test]
fn clean_bad_debt_keeps_borrow_index_constant() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");
    let liquidator = TestAddress::new("liquidator");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Ensure liquidator account exists and has sufficient USDC for repayments
    state.world.account(liquidator).nonce(1).esdt_balance(
        USDC_TOKEN,
        BigUint::from(1_000_000u64) * BigUint::from(10u64).pow(USDC_DECIMALS as u32),
    );

    // Provide USDC liquidity for borrowing
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
        &supplier,
        USDC_TOKEN,
        BigUint::from(1000u64),
        USDC_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::None,
        false,
    );

    // Borrower collateral and borrow
    state.supply_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(20u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    state.borrow_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(500u64),
        2,
        USDC_DECIMALS,
    );

    // Accrue large interest then sync the market once
    state.change_timestamp(880_000_000u64);
    let mut markets = MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN));
    state.update_markets(&supplier, markets);

    // Liquidate most collateral to leave bad debt and dust collateral
    state.liquidate_account(
        &liquidator,
        &USDC_TOKEN,
        BigUint::from(770u64),
        2,
        USDC_DECIMALS,
    );

    let usdc_pool = state.pool_address(EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN));
    let pre_borrow_index = state.market_borrow_index(usdc_pool.clone());
    let pre_supply_index = state.market_supply_index(usdc_pool.clone());

    // Clean bad debt (internally calls pool.seize_position)
    state.clean_bad_debt(2);

    let post_borrow_index = state.market_borrow_index(usdc_pool.clone());
    let post_supply_index = state.market_supply_index(usdc_pool);

    // Borrow index must not change during clean (no time advance)
    assert_eq!(
        post_borrow_index, pre_borrow_index,
        "borrow index changed during cleanBadDebt"
    );
    // Supply index must strictly decrease due to socialized loss
    assert!(
        post_supply_index < pre_supply_index,
        "supply index did not decrease during cleanBadDebt"
    );
}

#[test]
fn clean_bad_debt_supply_index_exact_factor() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");
    let liquidator = TestAddress::new("liquidator");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Ensure liquidator account exists and has sufficient USDC for repayments
    state.world.account(liquidator).nonce(1).esdt_balance(
        USDC_TOKEN,
        BigUint::from(1_000_000u64) * BigUint::from(10u64).pow(USDC_DECIMALS as u32),
    );

    // Provide USDC liquidity
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
        &supplier,
        USDC_TOKEN,
        BigUint::from(1000u64),
        USDC_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::None,
        false,
    );

    // Borrower collateral and borrow
    state.supply_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(20u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    state.borrow_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(500u64),
        2,
        USDC_DECIMALS,
    );

    // Accrue interest and sync once
    state.change_timestamp(880_000_000u64);
    let mut markets = MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN));
    state.update_markets(&supplier, markets);

    // Liquidate to leave bad debt and dust collateral
    state.liquidate_account(
        &liquidator,
        &USDC_TOKEN,
        BigUint::from(760u64),
        2,
        USDC_DECIMALS,
    );

    // Snapshot before clean
    let usdc_pool = state.pool_address(EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN));
    let supply_index_before = state.market_supply_index(usdc_pool.clone());
    let supplied_amount_before = state.market_supplied_amount(usdc_pool.clone());
    let bad_debt_before = state.borrow_amount_for_token(2, USDC_TOKEN);

    // Compute expected new supply index using the same half-up rounding scheme
    let ray = BigUint::from(RAY);
    let total_raw = supplied_amount_before.into_raw_units().clone();
    let debt_raw = bad_debt_before.into_raw_units().clone();
    let capped_bad_debt = if debt_raw > total_raw {
        total_raw.clone()
    } else {
        debt_raw
    };
    let remaining_raw = total_raw.clone() - capped_bad_debt;

    // reduction_factor_ray = round_half_up(remaining / total * RAY)
    let reduction_numer = remaining_raw * &ray;
    let reduction_factor_ray = half_up_div(&reduction_numer, &total_raw);

    // expected_index = round_half_up(supply_index_before * reduction_factor_ray / RAY)
    let sib_raw = supply_index_before.into_raw_units().clone();
    let numer = sib_raw * &reduction_factor_ray;
    let expected_index_raw = half_up_div(&numer, &ray);

    // Execute clean
    state.clean_bad_debt(2);

    let supply_index_after = state.market_supply_index(usdc_pool);
    let after_raw = supply_index_after.into_raw_units().clone();

    // Allow tiny rounding drift tolerance
    let tol = small_ray_tolerance();
    let within = |a: &BigUint<StaticApi>, b: &BigUint<StaticApi>| -> bool {
        if a >= b {
            (a.clone() - b).le(&tol)
        } else {
            (b.clone() - a).le(&tol)
        }
    };
    assert!(
        within(&after_raw, &expected_index_raw),
        "supply index post-clean differs from expected (±tol). expected={expected_index_raw:?} actual={after_raw:?} tol={tol:?}"
    );
}

#[test]
fn clean_bad_debt_supply_index_clamped_min() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");
    let liquidator = TestAddress::new("liquidator");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Ensure liquidator account exists and has sufficient USDC for repayments
    state.world.account(liquidator).nonce(1).esdt_balance(
        USDC_TOKEN,
        BigUint::from(1_000_000u64) * BigUint::from(10u64).pow(USDC_DECIMALS as u32),
    );

    // Provide minimal liquidity above the borrow so the position can be created
    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(500u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.supply_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(20u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    state.borrow_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(500u64),
        2,
        USDC_DECIMALS,
    );

    // Accrue extremely large interest and sync (amplify debt relative to supply)
    state.change_timestamp(8_800_000_000u64);
    let mut markets = MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN));
    state.update_markets(&supplier, markets);

    // Perform a large liquidation payment; engine caps effective repayment to its
    // computed maximum and refunds the excess. This maximizes seizure so the
    // remaining collateral is <= $5 (dust), enabling bad-debt cleanup.
    state.liquidate_account(
        &liquidator,
        &USDC_TOKEN,
        BigUint::from(50_000u64),
        2,
        USDC_DECIMALS,
    );

    let usdc_pool = state.pool_address(EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN));

    state.clean_bad_debt(2);

    // Expect supply index to be reduced significantly and never below the hard
    // minimum raw value 1 (RAY floor)
    let supply_index_after_raw = state
        .market_supply_index(usdc_pool)
        .into_raw_units()
        .clone();
    assert!(supply_index_after_raw >= 1u64);
}

#[test]
fn clean_bad_debt_affects_only_impacted_market() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");
    let liquidator = TestAddress::new("liquidator");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Ensure liquidator account exists and has sufficient USDC for repayments
    state.world.account(liquidator).nonce(1).esdt_balance(
        USDC_TOKEN,
        BigUint::from(1_000_000u64) * BigUint::from(10u64).pow(USDC_DECIMALS as u32),
    );

    // Supply into two markets
    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(2000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
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

    // Borrower posts EGLD collateral and borrows USDC only
    state.supply_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(50u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    state.borrow_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(800u64),
        2,
        USDC_DECIMALS,
    );

    // Accrue substantial interest and sync both markets once to make position liquidatable
    state.change_timestamp(880_000_000u64);
    let mut markets = MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN));
    markets.push(EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN));
    state.update_markets(&supplier, markets);

    // Liquidate most collateral to leave USDC bad debt
    state.liquidate_account(
        &liquidator,
        &USDC_TOKEN,
        BigUint::from(50_000u64),
        2,
        USDC_DECIMALS,
    );

    let usdc_pool = state.pool_address(EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN));
    let egld_pool = state.pool_address(EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN));

    let pre_usdc_si = state.market_supply_index(usdc_pool.clone());
    let pre_egld_si = state.market_supply_index(egld_pool.clone());
    let pre_usdc_bi = state.market_borrow_index(usdc_pool.clone());
    let pre_egld_bi = state.market_borrow_index(egld_pool.clone());

    state.clean_bad_debt(2);

    let post_usdc_si = state.market_supply_index(usdc_pool.clone());
    let post_egld_si = state.market_supply_index(egld_pool.clone());
    let post_usdc_bi = state.market_borrow_index(usdc_pool);
    let post_egld_bi = state.market_borrow_index(egld_pool);

    // USDC supply index must decrease, EGLD supply index must remain unchanged
    assert!(
        post_usdc_si < pre_usdc_si,
        "USDC supply index should decrease"
    );
    assert_eq!(
        post_egld_si, pre_egld_si,
        "EGLD supply index should be unchanged"
    );

    // Borrow indexes for both markets should remain unchanged (no time advance)
    assert_eq!(post_usdc_bi, pre_usdc_bi, "USDC borrow index changed");
    assert_eq!(post_egld_bi, pre_egld_bi, "EGLD borrow index changed");
}
