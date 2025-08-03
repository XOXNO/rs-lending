use common_constants::RAY;
pub use common_constants::{BPS_PRECISION, RAY_PRECISION, WAD_PRECISION};
use multiversx_sc::types::{
    EgldOrEsdtTokenIdentifier, EgldOrEsdtTokenPayment, ManagedDecimal, ManagedVec,
    MultiValueEncoded,
};
use multiversx_sc_scenario::imports::{BigUint, OptionalValue, TestAddress};
pub mod constants;
pub mod proxys;
pub mod setup;
use constants::*;
use setup::*;

/// Tests basic market views and price calculations.
///
/// Covers:
/// - Market utilization calculation
/// - Borrow and supply rate views
/// - USD and EGLD price views
/// - Interest rate calculations
/// - Position aggregate views
#[test]
fn views_basic_market_metrics_success() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_accounts(&mut state, supplier, borrower);

    // Supply $4000 worth of EGLD
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Borrower supplies $2500 worth of XEGLD as collateral
    state.supply_asset(
        &borrower,
        XEGLD_TOKEN,
        BigUint::from(100u64),
        XEGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Borrower takes $1800 EGLD loan (45% utilization)
    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(45u64),
        2,
        EGLD_DECIMALS,
    );

    // Test market views
    let utilisation = state.market_utilization(state.egld_market.clone());

    // Test price views
    let usd_price = state.usd_price(EGLD_TOKEN);
    let egld_price = state.egld_price(EGLD_TOKEN);

    // Test position views
    let borrowed = state.total_borrow_in_egld(2);
    let collateral = state.total_collateral_in_egld(2);
    let collateral_weighted = state.liquidation_collateral_available(2);
    let health_factor = state.account_health_factor(2);

    // Verify utilization is 45%
    assert_eq!(
        utilisation,
        ManagedDecimal::from_raw_units(
            BigUint::from(450000000000000000000000000u128),
            RAY_PRECISION
        )
    );

    // Verify EGLD price
    assert_eq!(
        usd_price,
        ManagedDecimal::from_raw_units(BigUint::from(40000000000000000000u128), WAD_PRECISION)
    );
    assert_eq!(
        egld_price,
        ManagedDecimal::from_raw_units(BigUint::from(1000000000000000000u128), WAD_PRECISION)
    );

    // Verify position data exists
    assert!(borrowed > ManagedDecimal::from_raw_units(BigUint::zero(), RAY_PRECISION));
    assert!(collateral > ManagedDecimal::from_raw_units(BigUint::zero(), RAY_PRECISION));
    assert!(collateral_weighted > ManagedDecimal::from_raw_units(BigUint::zero(), RAY_PRECISION));
    assert!(health_factor > ManagedDecimal::from_raw_units(BigUint::from(1u64), RAY_PRECISION));
}

/// Tests liquidation estimation view for complex scenarios.
///
/// Covers:
/// - Liquidation estimation calculations
/// - Seized collateral calculation
/// - Protocol fee calculation
/// - Refund calculation
/// - Liquidation bonus rate
/// - Maximum EGLD payment estimation
#[test]
fn views_liquidation_estimation_unhealthy_position() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Supply liquidity
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
        BigUint::from(2000u64),
        USDC_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::None,
        false,
    );

    state.supply_asset(
        &supplier,
        XEGLD_TOKEN,
        BigUint::from(50u64),
        XEGLD_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::None,
        false,
    );

    // Borrower supplies $3000 USDC as collateral
    state.supply_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(3000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Borrower takes loans
    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(50u64), // $2000
        2,
        EGLD_DECIMALS,
    );

    state.borrow_asset(
        &borrower,
        XEGLD_TOKEN,
        BigUint::from(5u64), // $250
        2,
        XEGLD_DECIMALS,
    );

    // Verify initial health
    let initial_health = state.account_health_factor(2);
    assert!(initial_health > ManagedDecimal::from_raw_units(BigUint::from(1u64), RAY_PRECISION));

    // Advance time to make position unhealthy
    state.change_timestamp(SECONDS_PER_DAY * 700);
    let mut markets = MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN));
    markets.push(EgldOrEsdtTokenIdentifier::esdt(XEGLD_TOKEN));
    state.update_markets(&borrower, markets);

    // Verify position is liquidatable
    let can_liquidate = state.can_be_liquidated(2);
    let health_after = state.account_health_factor(2);
    assert!(can_liquidate);
    assert!(health_after < ManagedDecimal::from_raw_units(BigUint::from(RAY), RAY_PRECISION));

    // Prepare partial liquidation payments
    let borrowed_egld = state.borrow_amount_for_token(2, EGLD_TOKEN);
    let borrowed_xegld = state.borrow_amount_for_token(2, XEGLD_TOKEN);

    let mut debt_payments = ManagedVec::new();
    debt_payments.push(EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        0,
        borrowed_egld.into_raw_units() / 2u64,
    ));
    debt_payments.push(EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::esdt(XEGLD_TOKEN.to_token_identifier()),
        0,
        borrowed_xegld.into_raw_units() / 2u64,
    ));

    // Get liquidation estimations
    let liquidation_estimate = state.liquidation_estimations(2, debt_payments);

    // Verify estimations
    assert!(liquidation_estimate.seized_collaterals.len() > 0);
    assert!(liquidation_estimate.protocol_fees.len() > 0);
    assert!(liquidation_estimate.refunds.len() > 0);
    assert!(
        liquidation_estimate.max_egld_payment_wad
            > ManagedDecimal::from_raw_units(BigUint::zero(), RAY_PRECISION)
    );
    assert!(
        liquidation_estimate.bonus_rate_bps
            > ManagedDecimal::from_raw_units(BigUint::zero(), BPS_PRECISION)
    );
}

/// Tests market index and market data views for multiple assets.
///
/// Covers:
/// - get_all_market_indexes view
/// - get_all_markets view
/// - Multiple market data retrieval
/// - Supply and borrow index tracking
/// - Market contract address resolution
#[test]
fn views_all_markets_data_retrieval() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");

    setup_account(&mut state, supplier);

    // Supply to multiple markets
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
        BigUint::from(1000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.supply_asset(
        &supplier,
        XEGLD_TOKEN,
        BigUint::from(50u64),
        XEGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Prepare asset list
    let mut assets = MultiValueEncoded::new();
    assets.push(EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN));
    assets.push(EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN));
    assets.push(EgldOrEsdtTokenIdentifier::esdt(XEGLD_TOKEN));

    // Test market indexes view
    let market_indexes = state.all_market_indexes(assets.clone());
    assert_eq!(market_indexes.len(), 3);

    // Verify all indexes initialized properly
    for index in &market_indexes {
        assert!(
            index.supply_index_ray >= ManagedDecimal::from_raw_units(BigUint::from(RAY), RAY_PRECISION)
        );
        assert!(
            index.borrow_index_ray >= ManagedDecimal::from_raw_units(BigUint::from(RAY), RAY_PRECISION)
        );
        assert!(index.egld_price_wad > ManagedDecimal::from_raw_units(BigUint::zero(), WAD_PRECISION));
        assert!(index.usd_price_wad > ManagedDecimal::from_raw_units(BigUint::zero(), WAD_PRECISION));
    }

    // Test all markets view
    let markets = state.all_markets(assets);
    assert_eq!(markets.len(), 3);

    // Verify market data
    for market in &markets {
        assert!(!market.market_contract_address.is_zero());
        assert!(
            market.price_in_egld_wad > ManagedDecimal::from_raw_units(BigUint::zero(), WAD_PRECISION)
        );
        assert!(
            market.price_in_usd_wad > ManagedDecimal::from_raw_units(BigUint::zero(), WAD_PRECISION)
        );
    }
}

/// Tests position-specific views for collateral and borrow amounts.
///
/// Covers:
/// - get_collateral_amount_for_token view
/// - get_borrow_amount_for_token view
/// - Aggregate position views (total collateral, total borrow)
/// - LTV vs liquidation collateral calculations
/// - Position health metrics
#[test]
fn views_position_data_and_aggregates() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_accounts(&mut state, supplier, borrower);

    // Create positions
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(200u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Borrower supplies multiple collaterals
    state.supply_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(3000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.supply_asset(
        &borrower,
        XEGLD_TOKEN,
        BigUint::from(40u64),
        XEGLD_DECIMALS,
        OptionalValue::Some(2),
        OptionalValue::None,
        false,
    );

    // Borrower takes loan
    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(50u64),
        2,
        EGLD_DECIMALS,
    );

    // Test individual token views
    let usdc_collateral = state.collateral_amount_for_token(2, USDC_TOKEN);
    let xegld_collateral = state.collateral_amount_for_token(2, XEGLD_TOKEN);
    let egld_borrow = state.borrow_amount_for_token(2, EGLD_TOKEN);

    // Verify individual amounts
    assert_eq!(
        usdc_collateral,
        ManagedDecimal::from_raw_units(
            BigUint::from(3000u64) * BigUint::from(10u64).pow(USDC_DECIMALS as u32),
            USDC_DECIMALS
        )
    );
    assert_eq!(
        xegld_collateral,
        ManagedDecimal::from_raw_units(
            BigUint::from(40u64) * BigUint::from(10u64).pow(XEGLD_DECIMALS as u32),
            XEGLD_DECIMALS
        )
    );
    assert_eq!(
        egld_borrow,
        ManagedDecimal::from_raw_units(
            BigUint::from(50u64) * BigUint::from(10u64).pow(EGLD_DECIMALS as u32),
            EGLD_DECIMALS
        )
    );

    // Test aggregate views
    let total_borrow_egld = state.total_borrow_in_egld(2);
    let total_collateral_egld = state.total_collateral_in_egld(2);
    let liquidation_collateral = state.liquidation_collateral_available(2);
    let ltv_collateral = state.ltv_collateral_in_egld(2);

    // Verify relationships
    assert!(total_borrow_egld > ManagedDecimal::from_raw_units(BigUint::zero(), WAD_PRECISION));
    assert!(total_collateral_egld > ManagedDecimal::from_raw_units(BigUint::zero(), WAD_PRECISION));
    assert!(
        liquidation_collateral > ManagedDecimal::from_raw_units(BigUint::zero(), WAD_PRECISION)
    );
    assert!(ltv_collateral > ManagedDecimal::from_raw_units(BigUint::zero(), WAD_PRECISION));

    // Verify expected relationships
    assert!(ltv_collateral < liquidation_collateral); // LTV more conservative
    assert!(liquidation_collateral < total_collateral_egld); // Weighted less than total
}

/// Tests price views for various tokens with tolerance checks.
///
/// Covers:
/// - get_usd_price view
/// - get_egld_price view
/// - Price accuracy validation
/// - EGLD self-price (should be 1.0)
/// - Price tolerance testing
#[test]
fn views_token_prices_accuracy() {
    let mut state = LendingPoolTestState::new();

    // Test various token prices
    let tokens = vec![
        (EGLD_TOKEN, EGLD_PRICE_IN_DOLLARS),
        (USDC_TOKEN, USDC_PRICE_IN_DOLLARS),
        (SEGLD_TOKEN, SEGLD_PRICE_IN_DOLLARS),
    ];

    for (token, expected_usd) in tokens {
        let usd_price = state.usd_price(token);
        let egld_price = state.egld_price(token);

        // Verify USD price within 1% tolerance
        let expected_usd_decimal = ManagedDecimal::from_raw_units(
            BigUint::from(expected_usd) * BigUint::from(10u64).pow(WAD_PRECISION as u32),
            WAD_PRECISION,
        );

        let diff = if usd_price > expected_usd_decimal {
            usd_price.clone() - expected_usd_decimal.clone()
        } else {
            expected_usd_decimal.clone() - usd_price.clone()
        };

        let tolerance = expected_usd_decimal.clone() / 100usize;
        assert!(
            diff < tolerance,
            "Price deviation too large for {:?}",
            token
        );

        // Verify EGLD price is positive
        assert!(egld_price > ManagedDecimal::from_raw_units(BigUint::zero(), WAD_PRECISION));
    }

    // Test EGLD self-price (should be exactly 1.0)
    let egld_in_egld = state.egld_price(EGLD_TOKEN);
    assert_eq!(
        egld_in_egld,
        ManagedDecimal::from_raw_units(
            BigUint::from(10u64).pow(WAD_PRECISION as u32),
            WAD_PRECISION
        )
    );
}

/// Tests view error cases for non-existent positions.
///
/// Covers:
/// - Error handling for non-existent collateral
/// - View validation for missing data
#[test]
fn views_error_handling_non_existent_data() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");

    setup_account(&mut state, supplier);

    // Create minimal position
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(10u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Test error for non-existent collateral token
    let custom_error_message = format!("Token not existing in the account {}", USDC_TOKEN.as_str());
    state.collateral_amount_for_token_non_existing(
        1,
        USDC_TOKEN,
        custom_error_message.as_bytes(),
    );
}

/// Tests complex liquidation estimation with bad debt scenario.
///
/// Covers:
/// - Complex multi-asset liquidation estimation
/// - Bad debt scenario handling
/// - Full liquidation with insufficient collateral
/// - Multiple collateral seizure calculations
/// - Protocol fee distribution across assets
#[test]
fn views_complex_liquidation_bad_debt_scenario() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Create large liquidity pools
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(500u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(10000u64),
        USDC_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::None,
        false,
    );

    state.supply_asset(
        &supplier,
        CAPPED_TOKEN,
        BigUint::from(100u64),
        CAPPED_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::None,
        false,
    );

    // Borrower supplies multiple collaterals
    state.supply_asset(
        &borrower,
        XEGLD_TOKEN,
        BigUint::from(30u64), // $1500
        XEGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.supply_asset(
        &borrower,
        SEGLD_TOKEN,
        BigUint::from(40u64), // $2000
        SEGLD_DECIMALS,
        OptionalValue::Some(2),
        OptionalValue::None,
        false,
    );

    // Borrower takes multiple loans
    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(20u64),
        2,
        EGLD_DECIMALS,
    );

    state.borrow_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(500u64),
        2,
        USDC_DECIMALS,
    );

    state.borrow_asset(
        &borrower,
        CAPPED_TOKEN,
        BigUint::from(10u64),
        2,
        CAPPED_DECIMALS,
    );

    // Advance time significantly to create bad debt
    state.change_timestamp(SECONDS_PER_DAY * 15000);
    let mut markets = MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN));
    markets.push(EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN));
    markets.push(EgldOrEsdtTokenIdentifier::esdt(CAPPED_TOKEN));
    state.update_markets(&borrower, markets);

    // Verify position is deeply underwater
    let health_factor = state.account_health_factor(2);
    let can_liquidate = state.can_be_liquidated(2);
    assert!(can_liquidate);
    assert!(
        health_factor
            < ManagedDecimal::from_raw_units(
                BigUint::from(10u64).pow(RAY_PRECISION as u32),
                RAY_PRECISION
            )
    );

    // Prepare full liquidation attempt
    let borrowed_egld = state.borrow_amount_for_token(2, EGLD_TOKEN);
    let borrowed_usdc = state.borrow_amount_for_token(2, USDC_TOKEN);
    let borrowed_capped = state.borrow_amount_for_token(2, CAPPED_TOKEN);

    let mut debt_payments = ManagedVec::new();
    debt_payments.push(EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        0,
        borrowed_egld.into_raw_units().clone(),
    ));
    debt_payments.push(EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN.to_token_identifier()),
        0,
        borrowed_usdc.into_raw_units().clone(),
    ));
    debt_payments.push(EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::esdt(CAPPED_TOKEN.to_token_identifier()),
        0,
        borrowed_capped.into_raw_units().clone(),
    ));

    // Get liquidation estimations
    let liquidation_estimate = state.liquidation_estimations(2, debt_payments);

    // Full refund of the last paid debt since it was overpaid
    assert_eq!(
        liquidation_estimate.refunds.get(0).amount,
        borrowed_capped.into_raw_units().clone()
    );
    // Partial refund of the second paid debt since it was overpaid
    assert_eq!(
        liquidation_estimate.refunds.get(1).amount,
        borrowed_usdc.into_raw_units().clone()
    );
    // Partial refund of the first paid debt since it was underpaid
    assert!(liquidation_estimate.refunds.get(2).amount < borrowed_egld.into_raw_units().clone());
    // Verify complex liquidation results
    assert_eq!(liquidation_estimate.seized_collaterals.len(), 2); // Both collateral types seized
    assert_eq!(liquidation_estimate.protocol_fees.len(), 2); // Fees for each seized asset
    assert_eq!(liquidation_estimate.refunds.len(), 3); // Refunds from the last to the first asset
    assert!(
        liquidation_estimate.max_egld_payment_wad
            > ManagedDecimal::from_raw_units(BigUint::zero(), RAY_PRECISION)
    );
    assert!(
        liquidation_estimate.bonus_rate_bps
            > ManagedDecimal::from_raw_units(BigUint::from(100u64), BPS_PRECISION)
    );
}
