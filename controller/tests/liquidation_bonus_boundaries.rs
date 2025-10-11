use common_constants::BPS_PRECISION;
use multiversx_sc::types::{
    EgldOrEsdtTokenIdentifier, EgldOrEsdtTokenPayment, ManagedDecimal, ManagedVec,
};
use multiversx_sc_scenario::imports::{BigUint, OptionalValue, StaticApi, TestAddress};

pub mod constants;
pub mod proxys;
pub mod setup;
use constants::*;
use setup::*;

/// Sets up a simple single-asset position (EGLD as both collateral and debt) that
/// becomes liquidatable after advancing time and updating markets.
fn setup_unhealthy_single_asset_position() -> (
    LendingPoolTestState,
    TestAddress<'static>,
    u64,
    TestAddress<'static>,
) {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");
    let liquidator = TestAddress::new("liquidator");

    state.world.account(liquidator).nonce(1).esdt_balance(
        EGLD_TOKEN,
        BigUint::from(1_000_000u64) * BigUint::from(10u64).pow(EGLD_DECIMALS as u32),
    );

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Supplier provides EGLD liquidity, borrower supplies EGLD as collateral then borrows EGLD
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
    // Supplier minted first, so borrower NFT nonce = 2 here
    let borrower_nonce = 2u64;

    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(75u64),
        borrower_nonce,
        EGLD_DECIMALS,
    );

    // Advance time to make position unhealthy and update markets
    state.change_timestamp(SECONDS_PER_YEAR + SECONDS_PER_DAY * 1200);
    let mut markets = multiversx_sc::types::MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN));
    state.update_markets(&borrower, markets);

    (state, borrower, borrower_nonce, liquidator)
}

/// When repayment is capped but within the 100 bps tolerance, the effective bonus
/// should remain the scaled (dynamic) bonus – equal to the uncapped simulation.
#[test]
fn liquidation_bonus_within_tolerance_keeps_scaled_bonus() {
    let (mut state, _borrower, nonce, _liq) = setup_unhealthy_single_asset_position();

    // Baseline (uncapped) estimation
    let empty = ManagedVec::<StaticApi, EgldOrEsdtTokenPayment<StaticApi>>::new();
    let base_est = state.liquidation_estimations(nonce, empty);

    // Build a capped payment set at 0.5% below the estimate (within 100 bps tolerance)
    let target_wad = base_est.max_egld_payment_wad.into_raw_units().clone();
    // 0.5% underpayment -> multiply by 9950 / 10000
    let within_amount = target_wad.clone() * 9_950u64 / 10_000u64;

    let mut payments = ManagedVec::new();
    payments.push(EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        0,
        within_amount,
    ));

    let within_est = state.liquidation_estimations(nonce, payments);
    let base_bps = ManagedDecimal::from_raw_units(BigUint::from(LIQ_BONUS), BPS_PRECISION);

    // Expect scaled bonus to remain applied and equal to baseline (uncapped) estimation
    assert!(within_est.bonus_rate_bps >= base_bps);
    assert!(within_est.bonus_rate_bps == base_est.bonus_rate_bps);
}

/// For a capped payment above the 100 bps shortfall, the algorithm may fall back
/// to the base bonus only if the projected health factor does not improve; otherwise
/// it keeps the scaled bonus. We assert it picks one of those two valid outcomes.
#[test]
fn liquidation_bonus_above_tolerance_fallback_or_scaled() {
    let (mut state, _borrower, nonce, _liq) = setup_unhealthy_single_asset_position();

    // Baseline (uncapped) estimation
    let empty = ManagedVec::<StaticApi, EgldOrEsdtTokenPayment<StaticApi>>::new();
    let base_est = state.liquidation_estimations(nonce, empty);

    // Build a capped payment set at 2% below the estimate (> 100 bps tolerance)
    let target_wad = base_est.max_egld_payment_wad.into_raw_units().clone();
    let above_amount = target_wad.clone() * 9_800u64 / 10_000u64;

    let mut payments = ManagedVec::new();
    payments.push(EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        0,
        above_amount,
    ));

    let est = state.liquidation_estimations(nonce, payments);
    let base_bps = ManagedDecimal::from_raw_units(BigUint::from(LIQ_BONUS), BPS_PRECISION);

    // Valid outcomes: either fallback to base bonus or keep the uncapped scaled bonus
    assert!(est.bonus_rate_bps >= base_bps);
    assert!(
        est.bonus_rate_bps == base_bps || est.bonus_rate_bps == base_est.bonus_rate_bps,
        "Bonus must be either base (fallback) or the uncapped scaled bonus"
    );
}

/// Paying exactly the estimated amount must be treated as uncapped (min picks estimate),
/// therefore the applied bonus equals the uncapped scaled bonus and health factor improves.
#[test]
fn liquidation_exact_estimate_is_uncapped_and_improves_hf() {
    let (mut state, _borrower, nonce, liq) = setup_unhealthy_single_asset_position();

    // Baseline (uncapped) estimation
    let empty = ManagedVec::<StaticApi, EgldOrEsdtTokenPayment<StaticApi>>::new();
    let base_est = state.liquidation_estimations(nonce, empty);

    // Use exactly the estimated payment
    let exact_amount = base_est.max_egld_payment_wad.into_raw_units().clone();

    // View with explicit exact payment should match uncapped bonus
    let mut payments = ManagedVec::new();
    payments.push(EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        0,
        exact_amount.clone(),
    ));
    let with_exact = state.liquidation_estimations(nonce, payments);
    assert!(with_exact.bonus_rate_bps == base_est.bonus_rate_bps);

    // Execute and assert HF improves
    let hf_before = state.account_health_factor(nonce);
    state.liquidate_account_dem(&liq, &EGLD_TOKEN, exact_amount, nonce);
    let hf_after = state.account_health_factor(nonce);
    assert!(hf_after > hf_before);
}

/// A zero-amount payment in the estimation view should behave like providing no payments:
/// the algorithm chooses the repayment and yields identical bonus and max payment.
#[test]
fn liquidation_zero_payment_estimation_matches_empty() {
    let (mut state, _borrower, nonce, _liq) = setup_unhealthy_single_asset_position();

    let empty = ManagedVec::<StaticApi, EgldOrEsdtTokenPayment<StaticApi>>::new();
    let est_empty = state.liquidation_estimations(nonce, empty);

    let mut zero = ManagedVec::new();
    zero.push(EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        0,
        BigUint::zero(),
    ));
    let est_zero = state.liquidation_estimations(nonce, zero);

    assert!(est_zero.bonus_rate_bps == est_empty.bonus_rate_bps);
    assert!(est_zero.max_egld_payment_wad == est_empty.max_egld_payment_wad);
}
