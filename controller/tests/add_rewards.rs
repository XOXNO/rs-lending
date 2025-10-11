use common_constants::RAY;
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
fn add_rewards_increases_supply_index_proportionally() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");

    state.change_timestamp(0);
    setup_account(&mut state, supplier);

    // 1) Provide initial USDC liquidity: 1000 USDC
    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(1000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    let usdc_pool = state.usdc_market.clone();

    // Snapshot before adding rewards
    let pre_index = state.market_supply_index(usdc_pool.clone());
    let pre_index_raw = pre_index.into_raw_units().clone();
    let pre_supplied_actual = state.market_supplied_amount(usdc_pool.clone());
    let pre_supplied_raw = pre_supplied_actual.into_raw_units().clone();

    // 2) Add 100 USDC in rewards as owner via controller.addRewards
    let reward_raw = BigUint::from(100u64) * BigUint::from(10u64).pow(USDC_DECIMALS as u32);
    state.add_rewards(&OWNER_ADDRESS, USDC_TOKEN, reward_raw.clone());

    // Compute expected index using the same half-up rounding scheme as on-chain
    let ray = BigUint::from(RAY);
    // ratio_ray = round_half_up(reward / total * RAY)
    let ratio_ray = half_up_div(&(reward_raw * &ray), &pre_supplied_raw);
    let factor_ray = &ray + &ratio_ray;
    // expected_index = round_half_up(pre_index * factor_ray / RAY)
    let numer = pre_index_raw * &factor_ray;
    let expected_index_raw = half_up_div(&numer, &ray);

    // Read post state
    let post_index = state.market_supply_index(usdc_pool.clone());
    println!("post index: {:?}", post_index);
    let post_index_raw = post_index.into_raw_units().clone();
    let post_supplied_actual = state.market_supplied_amount(usdc_pool);
    println!("post supplied actual: {:?}", post_supplied_actual);
    let post_supplied_raw = post_supplied_actual.into_raw_units().clone();

    // Supply index increased as expected (within tiny rounding tolerance)
    let tol = small_ray_tolerance();
    let within = |a: &BigUint<StaticApi>, b: &BigUint<StaticApi>| -> bool {
        if a >= b {
            (a.clone() - b).le(&tol)
        } else {
            (b.clone() - a).le(&tol)
        }
    };
    assert!(
        within(&post_index_raw, &expected_index_raw),
        "supply index mismatch after rewards: expected≈{expected_index_raw:?} actual={post_index_raw:?} tol={tol:?}"
    );

    // Total supplied amount should reflect +100 USDC
    let expected_supplied =
        pre_supplied_raw + BigUint::from(100u64) * BigUint::from(10u64).pow(USDC_DECIMALS as u32);
    println!("expected_supplied: {:?}", expected_supplied.to_display());
    println!("post_supplied_raw: {:?}", post_supplied_raw.to_display());
    assert_eq!(
        post_supplied_raw, expected_supplied,
        "total supplied amount did not increase by the reward"
    );
}
