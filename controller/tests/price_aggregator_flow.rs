use multiversx_sc::types::ManagedBuffer;
use multiversx_sc_scenario::imports::{
    BigUint, EgldOrEsdtTokenIdentifier, ScenarioTxRun, TestAddress,
};

pub mod constants;
pub mod proxys;
pub mod setup;
use common_constants::EGLD_TICKER;
use constants::*;
use multiversx_sc_scenario::imports::ReturnsResult;
use setup::*;

#[test]
fn aggregator_happy_path_and_pause() {
    let mut state = LendingPoolTestState::new();

    // Deploy another aggregator for isolation
    let owner = OWNER_ADDRESS;
    let agg = state.price_aggregator_sc.clone();

    // Oracle submission (already unpaused in setup)
    state
        .world
        .tx()
        .from(ORACLE_ADDRESS_1)
        .to(agg.clone())
        .typed(proxys::proxy_aggregator::PriceAggregatorProxy)
        .submit(
            ManagedBuffer::from(EGLD_TICKER),
            ManagedBuffer::from(DOLLAR_TICKER),
            0u64,
            BigUint::from(EGLD_PRICE_IN_DOLLARS as u64) * BigUint::from(WAD),
        )
        .run();

    // Pause and verify submissions are blocked
    state
        .world
        .tx()
        .from(owner)
        .to(agg.clone())
        .typed(proxys::proxy_aggregator::PriceAggregatorProxy)
        .pause_endpoint()
        .run();

    state
        .world
        .tx()
        .from(ORACLE_ADDRESS_1)
        .to(agg)
        .typed(proxys::proxy_aggregator::PriceAggregatorProxy)
        .submit(
            ManagedBuffer::from(EGLD_TICKER),
            ManagedBuffer::from(DOLLAR_TICKER),
            0u64,
            BigUint::from(1u64),
        )
        .returns(multiversx_sc_scenario::imports::ExpectMessage(
            "Contract is paused",
        ))
        .run();
}

#[test]
fn aggregator_submission_count_invalid_errors() {
    let mut state = LendingPoolTestState::new();
    let agg = state.price_aggregator_sc.clone();

    // Too high submission count (greater than oracles length)
    state
        .world
        .tx()
        .from(OWNER_ADDRESS)
        .to(agg.clone())
        .typed(proxys::proxy_aggregator::PriceAggregatorProxy)
        .set_submission_count(10usize)
        .returns(multiversx_sc_scenario::imports::ExpectMessage(
            "Invalid submission count",
        ))
        .run();

    // Too low submission count (below min length)
    state
        .world
        .tx()
        .from(OWNER_ADDRESS)
        .to(agg)
        .typed(proxys::proxy_aggregator::PriceAggregatorProxy)
        .set_submission_count(0usize)
        .returns(multiversx_sc_scenario::imports::ExpectMessage(
            "Invalid submission count",
        ))
        .run();
}

#[test]
fn aggregator_latest_price_feed_view() {
    let mut state = LendingPoolTestState::new();
    let agg = state.price_aggregator_sc.clone();

    // Should have price for EGLD/USD after setup
    let _pf = state
        .world
        .query()
        .to(agg)
        .typed(proxys::proxy_aggregator::PriceAggregatorProxy)
        .latest_price_feed(
            ManagedBuffer::from(EGLD_TICKER),
            ManagedBuffer::from(DOLLAR_TICKER),
        )
        .returns(ReturnsResult)
        .run();
}

#[test]
fn aggregator_only_oracles_can_submit() {
    let mut state = LendingPoolTestState::new();
    let agg = state.price_aggregator_sc.clone();

    // A random non-oracle address should be blocked
    let stranger = TestAddress::new("stranger");
    state.world.account(stranger).nonce(1);

    state
        .world
        .tx()
        .from(stranger)
        .to(agg)
        .typed(proxys::proxy_aggregator::PriceAggregatorProxy)
        .submit(
            ManagedBuffer::from(EGLD_TICKER),
            ManagedBuffer::from(DOLLAR_TICKER),
            0u64,
            BigUint::from(1u64),
        )
        .returns(multiversx_sc_scenario::imports::ExpectMessage(
            "Only oracles allowed",
        ))
        .run();
}

#[test]
fn aggregator_timestamp_validation_errors() {
    let mut state = LendingPoolTestState::new();
    let agg = state.price_aggregator_sc.clone();

    // Move time forward
    state.world.current_block().block_timestamp(100);

    // Too old first submission (exceeds FIRST_SUBMISSION_TIMESTAMP_MAX_DIFF_SECONDS = 30)
    state
        .world
        .tx()
        .from(ORACLE_ADDRESS_1)
        .to(agg.clone())
        .typed(proxys::proxy_aggregator::PriceAggregatorProxy)
        .submit(
            ManagedBuffer::from(EGLD_TICKER),
            ManagedBuffer::from(DOLLAR_TICKER),
            0u64,
            BigUint::from(1u64),
        )
        .returns(multiversx_sc_scenario::imports::ExpectMessage(
            "First submission too old",
        ))
        .run();

    // Timestamp from the future
    state
        .world
        .tx()
        .from(ORACLE_ADDRESS_1)
        .to(agg)
        .typed(proxys::proxy_aggregator::PriceAggregatorProxy)
        .submit(
            ManagedBuffer::from(EGLD_TICKER),
            ManagedBuffer::from(DOLLAR_TICKER),
            1_000_000u64, // future compared to current 100
            BigUint::from(1u64),
        )
        .returns(multiversx_sc_scenario::imports::ExpectMessage(
            "Timestamp is from the future",
        ))
        .run();
}

#[test]
fn aggregator_discard_stale_round_and_finalize() {
    let mut state = LendingPoolTestState::new();
    let agg = state.price_aggregator_sc.clone();

    // First submission starts a round at t=0 by oracle1
    state
        .world
        .tx()
        .from(ORACLE_ADDRESS_1)
        .to(agg.clone())
        .typed(proxys::proxy_aggregator::PriceAggregatorProxy)
        .submit(
            ManagedBuffer::from(EGLD_TICKER),
            ManagedBuffer::from(DOLLAR_TICKER),
            0u64,
            BigUint::from(38u64) * BigUint::from(WAD),
        )
        .run();

    // Advance beyond MAX_ROUND_DURATION_SECONDS (1800) -> stale, will discard on next submission
    state.world.current_block().block_timestamp(2000);

    // Submissions after stale should discard previous and start a fresh round at t=2000
    for (addr, price) in [
        (ORACLE_ADDRESS_2, 40u64),
        (ORACLE_ADDRESS_3, 42u64),
        (ORACLE_ADDRESS_4, 44u64),
        (ORACLE_ADDRESS_1, 41u64), // oracle1 submits again in the fresh round
    ] {
        state
            .world
            .tx()
            .from(addr)
            .to(agg.clone())
            .typed(proxys::proxy_aggregator::PriceAggregatorProxy)
            .submit(
                ManagedBuffer::from(EGLD_TICKER),
                ManagedBuffer::from(DOLLAR_TICKER),
                2000u64,
                BigUint::from(price) * BigUint::from(WAD),
            )
            .run();
    }

    // A round should have been created at t=2000
    let pf = state
        .world
        .query()
        .to(agg)
        .typed(proxys::proxy_aggregator::PriceAggregatorProxy)
        .latest_price_feed(
            ManagedBuffer::from(EGLD_TICKER),
            ManagedBuffer::from(DOLLAR_TICKER),
        )
        .returns(ReturnsResult)
        .run();

    assert_eq!(pf.timestamp, 2000);
}

#[test]
fn oracle_tolerance_boundary_case_uses_safe_price_in_views() {
    let mut state = LendingPoolTestState::new();

    // Push USDC aggregator to 2x to exceed last tolerance vs safe price
    state.change_price(USDC_TICKER, USDC_PRICE_IN_DOLLARS * 2, 0u64);

    // Query all_market_indexes for USDC and check tolerance flags and price source
    let mut assets = multiversx_sc::types::MultiValueEncoded::new();
    assets.push(EgldOrEsdtTokenIdentifier::esdt(
        USDC_TOKEN.to_token_identifier(),
    ));

    let views = state
        .world
        .query()
        .to(state.lending_sc.clone())
        .typed(proxys::proxy_lending_pool::ControllerProxy)
        .all_market_indexes(assets)
        .returns(ReturnsResult)
        .run();

    assert_eq!(views.len(), 1);
    let v = views.get(0);
    // Tolerance should be violated for both bounds
    assert!(!v.within_first_tolerance);
    assert!(!v.within_second_tolerance);
    // Final price falls back to safe price in views (allow_unsafe_price=true)
    assert_eq!(v.egld_price_wad, v.safe_price_egld_wad);
}

#[test]
fn oracle_lp_high_deviation_returns_average() {
    let mut state = LendingPoolTestState::new();

    // Manipulate USDC aggregator price to 2x to cause high deviation vs safe price
    state.change_price(USDC_TICKER, USDC_PRICE_IN_DOLLARS * 2, 0u64);

    let mut assets = multiversx_sc::types::MultiValueEncoded::new();
    assets.push(EgldOrEsdtTokenIdentifier::esdt(
        LP_EGLD_TOKEN.to_token_identifier(),
    ));

    let views = state
        .world
        .query()
        .to(state.lending_sc.clone())
        .typed(proxys::proxy_lending_pool::ControllerProxy)
        .all_market_indexes(assets)
        .returns(ReturnsResult)
        .run();

    assert_eq!(views.len(), 1);
    let v = views.get(0);
    // Both tolerances violated
    assert!(!v.within_first_tolerance);
    assert!(!v.within_second_tolerance);
    // Final price should be the average between safe and off-chain prices for LPs in views
    let avg = (v.safe_price_egld_wad.clone() + v.aggregator_price_egld_wad.clone()) / 2usize;
    assert_eq!(v.egld_price_wad, avg);
}

#[test]
fn lxoxno_inherits_underlying_tolerance_flags() {
    let mut state = LendingPoolTestState::new();

    // Cause high deviation on XOXNO vs its safe price (2x)
    state.change_price(XOXNO_TICKER, XOXNO_PRICE_IN_DOLLARS * 2, 0u64);

    // Ensure views can read indexes by actually creating a market for LXOXNO.
    // Uses the same helper used during global setup to avoid scenario pitfalls.
    let _lxoxno_market = setup::setup_market(
        &mut state.world,
        &state.lending_sc,
        EgldOrEsdtTokenIdentifier::esdt(LXOXNO_TOKEN.to_token_identifier()),
        get_lxoxno_config(),
    );

    // Query only LXOXNO market index
    let mut assets = multiversx_sc::types::MultiValueEncoded::new();
    assets.push(EgldOrEsdtTokenIdentifier::esdt(
        LXOXNO_TOKEN.to_token_identifier(),
    ));

    let views = state
        .world
        .query()
        .to(state.lending_sc.clone())
        .typed(proxys::proxy_lending_pool::ControllerProxy)
        .all_market_indexes(assets)
        .returns(ReturnsResult)
        .run();

    assert_eq!(views.len(), 1);
    let v = views.get(0);
    // LXOXNO inherits XOXNO tolerance status
    assert!(!v.within_first_tolerance);
    assert!(!v.within_second_tolerance);
}

#[test]
fn lxoxno_view_prices_consistency_and_usd_median() {
    let mut state = LendingPoolTestState::new();

    // Ensure LXOXNO market exists so views can read indexes
    let _lxoxno_market = setup::setup_market(
        &mut state.world,
        &state.lending_sc,
        EgldOrEsdtTokenIdentifier::esdt(LXOXNO_TOKEN.to_token_identifier()),
        get_lxoxno_config(),
    );

    // Query LXOXNO price in EGLD and USD via views
    let lx_egld = state.egld_price(LXOXNO_TOKEN);
    let lx_usd = state.usd_price(LXOXNO_TOKEN);

    // Fetch EGLD/USD aggregator price (median of oracle submissions)
    let egld_usd_feed = state
        .world
        .query()
        .to(state.price_aggregator_sc.clone())
        .typed(proxys::proxy_aggregator::PriceAggregatorProxy)
        .latest_price_feed(
            ManagedBuffer::from(EGLD_TICKER),
            ManagedBuffer::from(DOLLAR_TICKER),
        )
        .returns(ReturnsResult)
        .run();

    // USD price should equal EGLD price * EGLD/USD (WAD math)
    let lx_egld_raw = lx_egld.into_raw_units().clone();
    let egld_usd_raw = egld_usd_feed.price;
    let expected_usd = (lx_egld_raw.clone() * egld_usd_raw) / BigUint::from(WAD);
    assert_eq!(lx_usd.into_raw_units(), &expected_usd);

    // With initial exchange rate = 1, LXOXNO and XOXNO EGLD prices match
    let xoxno_egld = state.egld_price(XOXNO_TOKEN);
    assert_eq!(lx_egld.into_raw_units(), xoxno_egld.into_raw_units());
}
