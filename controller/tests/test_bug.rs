use common_constants::BPS_PRECISION;
use multiversx_sc::types::ManagedDecimal;
use multiversx_sc_scenario::imports::{BigUint, OptionalValue, StaticApi, TestAddress};

pub mod constants;
pub mod proxys;
pub mod setup;
use constants::*;
use setup::*;

fn ray_dust_tolerance() -> BigUint<StaticApi> {
    BigUint::from(10u64).pow(22)
}

fn build_liquidation_case() -> (
    LendingPoolTestState,
    TestAddress<'static>,
    u64,
    BigUint<StaticApi>,
) {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_account(&mut state, supplier);
    setup_account(&mut state, borrower);

    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(120u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(500_000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.supply_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(2_600u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    let borrower_nonce = state.last_account_nonce();

    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(48u64),
        borrower_nonce,
        EGLD_DECIMALS,
    );

    let mut current_ts = SECONDS_PER_DAY * 440;
    state.change_timestamp(current_ts);
    let mut markets = multiversx_sc::types::MultiValueEncoded::new();
    use multiversx_sc::types::EgldOrEsdtTokenIdentifier as TokenId;
    markets.push(TokenId::esdt(EGLD_TOKEN.to_token_identifier()));
    markets.push(TokenId::esdt(USDC_TOKEN.to_token_identifier()));
    state.update_markets(&borrower, markets.clone());

    let mut tries = 0u32;
    loop {
        let hf_now = state.account_health_factor(borrower_nonce);
        let done = hf_now < ManagedDecimal::from_raw_units(BigUint::from(95_000u64), BPS_PRECISION);
        if done || tries >= 30 {
            break;
        }
        tries += 1;
        current_ts += SECONDS_PER_DAY * 30;
        state.change_timestamp(current_ts);
        state.update_markets(&borrower, markets.clone());
    }

    let total_pay_deno = state
        .borrow_amount_for_token(borrower_nonce, EGLD_TOKEN)
        .into_raw_units()
        .clone()
        / 4u32;

    (state, borrower, borrower_nonce, total_pay_deno)
}

#[test]
fn liquidation_micro_payments_do_not_gain_extra_collateral() {
    let (mut single_state, _single_borrower, single_nonce, total_pay_deno) =
        build_liquidation_case();
    let (mut micro_state, _micro_borrower, micro_nonce, _) = build_liquidation_case();

    let liquidator = TestAddress::new("liquidator");
    for state in [&mut single_state, &mut micro_state] {
        state.world.account(liquidator).nonce(1).esdt_balance(
            EGLD_TOKEN,
            BigUint::from(1_000_000u64) * BigUint::from(10u64).pow(EGLD_DECIMALS as u32),
        );
    }

    let pre_single_collateral = single_state
        .total_collateral_in_egld(single_nonce)
        .into_raw_units()
        .clone();
    let single_health_before = single_state.account_health_factor(single_nonce);
    single_state.liquidate_account_dem_bulk(
        &liquidator,
        vec![(&EGLD_TOKEN, &total_pay_deno)],
        single_nonce,
    );
    let post_single_collateral = single_state
        .total_collateral_in_egld(single_nonce)
        .into_raw_units()
        .clone();
    let single_health_after = single_state.account_health_factor(single_nonce);

    let first_payment = &total_pay_deno / 2u32;
    let second_payment = total_pay_deno.clone() - first_payment.clone();
    let micro_health_before = micro_state.account_health_factor(micro_nonce);
    micro_state.liquidate_account_dem_bulk(
        &liquidator,
        vec![(&EGLD_TOKEN, &first_payment)],
        micro_nonce,
    );
    micro_state.liquidate_account_dem_bulk(
        &liquidator,
        vec![(&EGLD_TOKEN, &second_payment)],
        micro_nonce,
    );
    let post_micro_collateral = micro_state
        .total_collateral_in_egld(micro_nonce)
        .into_raw_units()
        .clone();
    let micro_health_after = micro_state.account_health_factor(micro_nonce);

    assert!(pre_single_collateral >= post_single_collateral);
    assert!(pre_single_collateral >= post_micro_collateral);
    assert!(
        post_micro_collateral >= post_single_collateral,
        "Micro liquidation should leave at least as much collateral as single repayment",
    );
    let collateral_difference = if post_micro_collateral >= post_single_collateral {
        post_micro_collateral.clone() - post_single_collateral.clone()
    } else {
        post_single_collateral.clone() - post_micro_collateral.clone()
    };
    assert!(
        collateral_difference <= ray_dust_tolerance(),
        "Collateral difference must stay within dust tolerance",
    );

    let single_debt = single_state
        .total_borrow_in_egld(single_nonce)
        .into_raw_units()
        .clone();
    let micro_debt = micro_state
        .total_borrow_in_egld(micro_nonce)
        .into_raw_units()
        .clone();
    assert!(single_debt <= ray_dust_tolerance());
    assert!(micro_debt <= ray_dust_tolerance());
    assert!(
        single_health_after >= single_health_before,
        "Single liquidation must not worsen borrower health",
    );
    assert!(
        micro_health_after >= micro_health_before,
        "Micro liquidation must not worsen borrower health",
    );
    assert!(
        micro_health_after >= single_health_after,
        "Micro liquidation should produce equal or better health factor",
    );
}
