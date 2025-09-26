use common_constants::BPS_PRECISION;
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

fn setup_liquidatable_positions(
    state: &mut LendingPoolTestState,
) -> (TestAddress<'static>, u64, u64) {
    let supplier = TestAddress::new("supplier");
    let borrower1 = TestAddress::new("borrower1");
    let borrower2 = TestAddress::new("borrower2");

    state.change_timestamp(0);
    setup_account(state, supplier);
    setup_account(state, borrower1);
    setup_account(state, borrower2);

    // Debt liquidity (EGLD)
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(120u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    // Collateral liquidity (USDC)
    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(500_000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Borrower1 and Borrower2 – identical setup
    state.supply_asset(
        &borrower1,
        USDC_TOKEN,
        BigUint::from(2_600u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    let nonce1 = state.last_account_nonce();
    state.borrow_asset(
        &borrower1,
        EGLD_TOKEN,
        BigUint::from(48u64),
        nonce1,
        EGLD_DECIMALS,
    );

    state.supply_asset(
        &borrower2,
        USDC_TOKEN,
        BigUint::from(2_600u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    let nonce2 = state.last_account_nonce();
    state.borrow_asset(
        &borrower2,
        EGLD_TOKEN,
        BigUint::from(48u64),
        nonce2,
        EGLD_DECIMALS,
    );

    // Interest accumulation + reindexing until HF < ~0.95 for both positions
    let mut current_ts = SECONDS_PER_DAY * 440;
    state.change_timestamp(current_ts);
    let mut markets = multiversx_sc::types::MultiValueEncoded::new();
    use multiversx_sc::types::EgldOrEsdtTokenIdentifier as TokenId;
    markets.push(TokenId::esdt(EGLD_TOKEN.to_token_identifier()));
    markets.push(TokenId::esdt(USDC_TOKEN.to_token_identifier()));
    state.update_markets(&supplier, markets);

    let mut tries = 0u32;
    loop {
        let hf1_now = state.account_health_factor(nonce1);
        let hf2_now = state.account_health_factor(nonce2);
        let done = (hf1_now
            < ManagedDecimal::from_raw_units(BigUint::from(95_000u64), BPS_PRECISION))
            && (hf2_now < ManagedDecimal::from_raw_units(BigUint::from(95_000u64), BPS_PRECISION));
        if done || tries >= 30 {
            break;
        }
        tries += 1;
        current_ts += SECONDS_PER_DAY * 30;
        state.change_timestamp(current_ts);
        let mut markets = multiversx_sc::types::MultiValueEncoded::new();
        use multiversx_sc::types::EgldOrEsdtTokenIdentifier as TokenId;
        markets.push(TokenId::esdt(EGLD_TOKEN.to_token_identifier()));
        markets.push(TokenId::esdt(USDC_TOKEN.to_token_identifier()));
        state.update_markets(&supplier, markets);
    }

    (supplier, nonce1, nonce2)
}

fn delta_for_n_micro(
    state: &mut LendingPoolTestState,
    nonce1: u64,
    nonce2: u64,
    n: u64,
) -> BigUint<StaticApi> {
    // Collateral state before
    let pre_usdc_b1 = state
        .collateral_amount_for_token(nonce1, USDC_TOKEN)
        .clone();
    let pre_usdc_b2 = state
        .collateral_amount_for_token(nonce2, USDC_TOKEN)
        .clone();

    // Repayment amount ≈ half of current debt
    let debt1 = state
        .borrow_amount_for_token(nonce1, EGLD_TOKEN)
        .into_raw_units()
        .clone();
    let total_pay_deno = &debt1 / 2u32;

    // Liquidator with large funds
    let liquidator = TestAddress::new("liquidator_n_");
    state.world.account(liquidator).nonce(1).esdt_balance(
        EGLD_TOKEN,
        BigUint::from(1_000_000_000u64) * BigUint::from(10u64).pow(EGLD_DECIMALS as u32),
    );

    // CASE A: single payment
    state.liquidate_account_dem_bulk(&liquidator, vec![(&EGLD_TOKEN, &total_pay_deno)], nonce1);
    let post_usdc_b1 = state
        .collateral_amount_for_token(nonce1, USDC_TOKEN)
        .clone();
    let seized_single = pre_usdc_b1.clone() - post_usdc_b1.clone();

    // CASE B: N micro-payments summing to the same amount
    let micro = &total_pay_deno / n;
    for _ in 0..(n - 1) {
        state.liquidate_account_dem_bulk(&liquidator, vec![(&EGLD_TOKEN, &micro)], nonce2);
    }
    let last = total_pay_deno.clone() - (&micro * (n - 1));
    state.liquidate_account_dem_bulk(&liquidator, vec![(&EGLD_TOKEN, &last)], nonce2);

    let post_usdc_b2 = state
        .collateral_amount_for_token(nonce2, USDC_TOKEN)
        .clone();
    let seized_micro = pre_usdc_b2.clone() - post_usdc_b2.clone();

    // delta = over-seizure thanks to micro-payments
    let delta = seized_micro.clone() - seized_single.clone();
    println!(
        "n = {} -> delta(seized_micro - seized_single) = {:?}",
        n, delta
    );
    let seized_micro_raw = seized_micro.into_raw_units();
    let seized_single_raw = seized_single.into_raw_units();
    if seized_micro_raw.clone() > seized_single_raw.clone() {
        let diff = seized_micro_raw - seized_single_raw;
        println!("   over-seized raw units = {:?}", diff);
        diff
    } else {
        BigUint::zero()
    }
}

#[test]
fn poc_liquidation_micro_payments_escalation() {
    let test_ns = [2u64, 5, 10, 20, 50];
    let mut all_within_tolerance = true;
    let tolerance = BigUint::from(10u64);
    for &n in &test_ns {
        // Build state from scratch for each n value to avoid side effects
        let mut local_state = LendingPoolTestState::new();
        let (_s, n1, n2) = setup_liquidatable_positions(&mut local_state);
        let delta_raw = delta_for_n_micro(&mut local_state, n1, n2, n);
        if delta_raw > tolerance {
            all_within_tolerance = false;
        }
    }
    assert!(
        all_within_tolerance,
        "Micro-liquidations should not seize materially more collateral"
    );
}

#[test]
fn poc_liquidation_micro_payments_escalation_table() {
    let test_ns = [2u64, 5, 10, 20, 50];
    let mut all_within_tolerance = true;
    let mut last_within_tolerance = false;
    let tolerance = BigUint::from(10u64);
    for &n in &test_ns {
        let mut local_state = LendingPoolTestState::new();
        let (_s, n1, n2) = setup_liquidatable_positions(&mut local_state);
        let delta_raw = delta_for_n_micro(&mut local_state, n1, n2, n);
        if delta_raw > tolerance {
            all_within_tolerance = false;
        }
        if n == *test_ns.last().unwrap() {
            last_within_tolerance = delta_raw <= tolerance;
        }
    }
    assert!(
        all_within_tolerance,
        "Micro-liquidations must not seize materially more collateral"
    );
    assert!(
        last_within_tolerance,
        "Largest N should still avoid over-seizure"
    );
}
