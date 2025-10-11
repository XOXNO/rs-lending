use multiversx_sc::types::{EgldOrEsdtTokenIdentifier, ManagedVec, MultiValueEncoded};
use multiversx_sc_scenario::imports::{BigUint, OptionalValue, StaticApi, TestAddress};

pub mod constants;
pub mod proxys;
pub mod setup;
use constants::*;
use setup::*;

fn build_multicollateral_case() -> (
    LendingPoolTestState,
    TestAddress<'static>,
    u64,
    BigUint<StaticApi>,
) {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Debt liquidity (EGLD, USDC)
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(150u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(10_000u64),
        USDC_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::None,
        false,
    );

    // Borrower collateral across two assets (ensures multi-asset seizure during liquidation)
    state.supply_asset(
        &borrower,
        XEGLD_TOKEN,
        BigUint::from(20u64),
        XEGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    state.supply_asset(
        &borrower,
        SEGLD_TOKEN,
        BigUint::from(80u64),
        SEGLD_DECIMALS,
        OptionalValue::Some(2),
        OptionalValue::None,
        false,
    );

    // Borrow EGLD
    let borrower_nonce = 2u64;
    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(39u64),
        borrower_nonce,
        EGLD_DECIMALS,
    );
    state.borrow_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(1000u64),
        borrower_nonce,
        USDC_DECIMALS,
    );

    // Advance time significantly to make the position unhealthy and update markets
    let mut current_ts = SECONDS_PER_DAY * 440;
    state.change_timestamp(current_ts);
    let mut markets = MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN));
    markets.push(EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN));
    state.update_markets(&borrower, markets);

    // Force into liquidatable territory if needed
    let mut tries = 0u32;
    while !state.can_be_liquidated(borrower_nonce) && tries < 20 {
        tries += 1;
        current_ts += SECONDS_PER_DAY * 4040; // large jump to accumulate interest
        state.change_timestamp(current_ts);
        let mut mk = MultiValueEncoded::new();
        mk.push(EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN));
        mk.push(EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN));
        state.update_markets(&borrower, mk);
    }

    let total_pay_deno = state
        .borrow_amount_for_token(borrower_nonce, EGLD_TOKEN)
        .into_raw_units()
        .clone()
        / 2u32; // repay ~50% of EGLD debt

    (state, borrower, borrower_nonce, total_pay_deno)
}

/// View-only regression: splitting the same EGLD payment into multiple entries
/// within a single liquidationEstimations call should not increase seized collateral
/// versus a single entry, even when multiple collateral assets are available.
#[test]
fn liquidation_multicollateral_split_consistency_view() {
    let (mut state, _b, n, total_pay_deno) = build_multicollateral_case();

    // One-shot estimation
    let mut one = ManagedVec::new();
    one.push(multiversx_sc::types::EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        0,
        total_pay_deno.clone(),
    ));
    let est_one = state.liquidation_estimations(n, one);

    // Split into two equal halves estimation
    let half = &total_pay_deno / 2u32;
    let rest = total_pay_deno.clone() - half.clone();
    let mut two = ManagedVec::new();
    two.push(multiversx_sc::types::EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        0,
        half,
    ));
    two.push(multiversx_sc::types::EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        0,
        rest,
    ));
    let est_two = state.liquidation_estimations(n, two);

    // Compare total seized amounts per token id
    assert_eq!(
        est_one.seized_collaterals.len(),
        est_two.seized_collaterals.len()
    );
    for i in 0..est_one.seized_collaterals.len() {
        let a = est_one.seized_collaterals.get(i);
        let b = est_two.seized_collaterals.get(i);
        assert!(a.token_identifier == b.token_identifier);
        // Allow tiny dust difference due to rounding
        let amt_a = a.amount.clone();
        let amt_b = b.amount.clone();
        if amt_a >= amt_b {
            assert!((amt_a - amt_b) <= 10u64);
        } else {
            assert!((amt_b - amt_a) <= 10u64);
        }
    }
}
