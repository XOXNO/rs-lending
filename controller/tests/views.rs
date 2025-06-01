use common_constants::RAY;
use controller::{BPS_PRECISION, RAY_PRECISION, WAD_PRECISION};
use multiversx_sc::types::{EgldOrEsdtTokenIdentifier, EgldOrEsdtTokenPayment, ManagedDecimal, ManagedVec, MultiValueEncoded};
use multiversx_sc_scenario::imports::{BigUint, OptionalValue, TestAddress};
pub mod constants;
pub mod proxys;
pub mod setup;
use constants::*;
use setup::*;

// Basic Operations
#[test]
fn views_tests() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    // Setup accounts
    setup_accounts(&mut state, supplier, borrower);
    // Total Supplied 5000$
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
        XEGLD_TOKEN,
        BigUint::from(100u64), // 2500$
        XEGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(45u64),
        2,
        EGLD_DECIMALS,
    );
    // Verify amounts
    let borrowed = state.get_total_borrow_in_egld(2);
    let collateral = state.get_total_collateral_in_egld(2);
    let collateral_weighted = state.get_liquidation_collateral_available(2);
    let health_factor = state.get_account_health_factor(2);
    let utilisation = state.get_market_utilization(state.egld_market.clone());
    let borrow_rate = state.get_market_borrow_rate(state.egld_market.clone());
    let deposit_rate = state.get_market_supply_rate(state.egld_market.clone());
    let usd_price = state.get_usd_price(EGLD_TOKEN);
    let egld_price = state.get_egld_price(EGLD_TOKEN);
    println!("usd_price: {:?}", usd_price);
    println!("egld_price: {:?}", egld_price);
    println!("borrowed: {:?}", borrowed);
    println!("collateral: {:?}", collateral);
    println!("collateral_weighted: {:?}", collateral_weighted);
    println!("health_factor: {:?}", health_factor);
    println!("utilisation: {:?}", utilisation);
    println!(
        "borrow_rate: {:?}",
        borrow_rate
            * ManagedDecimal::from_raw_units(BigUint::from(MS_PER_YEAR), 0)
            * ManagedDecimal::from_raw_units(BigUint::from(100u64), 0)
    );
    println!(
        "deposit_rate: {:?}",
        deposit_rate
            * ManagedDecimal::from_raw_units(BigUint::from(MS_PER_YEAR), 0)
            * ManagedDecimal::from_raw_units(BigUint::from(100u64), 0)
    );

    assert_eq!(
        utilisation,
        ManagedDecimal::from_raw_units(
            BigUint::from(450000000000000000000000000u128),
            RAY_PRECISION
        )
    );
    assert_eq!(
        usd_price,
        ManagedDecimal::from_raw_units(BigUint::from(40000000000000000000u128), WAD_PRECISION)
    );
    assert_eq!(
        egld_price,
        ManagedDecimal::from_raw_units(BigUint::from(1000000000000000000u128), WAD_PRECISION)
    );
}

#[test]
fn test_liquidation_estimations_view() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    // Setup accounts
    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Supply assets
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

    // Supply XEGLD so borrower can borrow it later
    state.supply_asset(
        &supplier,
        XEGLD_TOKEN,
        BigUint::from(50u64),
        XEGLD_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::None,
        false,
    );

    // Borrower supplies collateral
    state.supply_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(3000u64), // $3000 USDC as collateral
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Borrower takes loans
    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(50u64), // $2000 (increased from $1200)
        2,
        EGLD_DECIMALS,
    );

    state.borrow_asset(
        &borrower,
        XEGLD_TOKEN,
        BigUint::from(5u64), // $250 (reduced from $500)
        2,
        XEGLD_DECIMALS,
    );

    // Initial health check
    let initial_health = state.get_account_health_factor(2);
    println!("Initial health factor: {:?}", initial_health);

    // Fast forward time to accrue interest and put position in bad health
    state.change_timestamp(SECONDS_PER_DAY * 700); // Increased from 600 days
    let mut markets = MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN));
    markets.push(EgldOrEsdtTokenIdentifier::esdt(XEGLD_TOKEN));
    state.update_markets(&borrower, markets);

    // Check if position can be liquidated
    let can_liquidate = state.can_be_liquidated(2);
    let health_after = state.get_account_health_factor(2);
    println!("Health factor after time: {:?}", health_after);
    println!("Can be liquidated: {:?}", can_liquidate);
    assert!(can_liquidate);

    // Prepare liquidation payments
    let borrowed_egld = state.get_borrow_amount_for_token(2, EGLD_TOKEN);
    let borrowed_xegld = state.get_borrow_amount_for_token(2, XEGLD_TOKEN);
    println!("Borrowed EGLD: {:?}", borrowed_egld);
    println!("Borrowed XEGLD: {:?}", borrowed_xegld);

    let mut debt_payments = ManagedVec::new();
    debt_payments.push(EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        0,
        borrowed_egld.into_raw_units() / 2u64, // Partial repayment
    ));
    debt_payments.push(EgldOrEsdtTokenPayment::new(
        EgldOrEsdtTokenIdentifier::esdt(XEGLD_TOKEN.to_token_identifier()),
        0,
        borrowed_xegld.into_raw_units() / 2u64, // Partial repayment
    ));

    // Call liquidation estimations view
    let (seized_collaterals, protocol_fees, refunds, max_egld_payment, bonus_rate) = 
        state.liquidation_estimations(2, debt_payments);

    println!("\n=== Liquidation Estimations ===");
    println!("Seized collaterals count: {}", seized_collaterals.len());
    for (i, collateral) in seized_collaterals.iter().enumerate() {
        println!("Seized collateral {}: {:?} amount: {:?}", 
            i, 
            collateral.token_identifier,
            collateral.amount
        );
    }
    
    println!("\nProtocol fees count: {}", protocol_fees.len());
    for (i, fee) in protocol_fees.iter().enumerate() {
        println!("Protocol fee {}: {:?} amount: {:?}", 
            i,
            fee.token_identifier,
            fee.amount
        );
    }
    
    println!("\nRefunds count: {}", refunds.len());
    for (i, refund) in refunds.iter().enumerate() {
        println!("Refund {}: {:?} amount: {:?}", 
            i,
            refund.token_identifier,
            refund.amount
        );
    }
    
    println!("\nMax EGLD payment: {:?}", max_egld_payment);
    println!("Bonus rate: {:?}", bonus_rate);

    // Verify estimations
    assert!(seized_collaterals.len() > 0);
    assert!(protocol_fees.len() > 0);
    assert!(max_egld_payment > ManagedDecimal::from_raw_units(BigUint::zero(), RAY_PRECISION));
    assert!(bonus_rate > ManagedDecimal::from_raw_units(BigUint::zero(), BPS_PRECISION));
}

#[test]
fn test_all_market_views() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");

    // Setup account
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

    // Test get_all_market_indexes
    let mut assets = MultiValueEncoded::new();
    assets.push(EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN));
    assets.push(EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN));
    assets.push(EgldOrEsdtTokenIdentifier::esdt(XEGLD_TOKEN));

    let market_indexes = state.get_all_market_indexes(assets.clone());

    println!("\n=== Market Indexes ===");
    for (i, index) in market_indexes.iter().enumerate() {
        println!("Market {}: {:?}", i, index.asset_id);
        println!("  Supply index: {:?}", index.supply_index);
        println!("  Borrow index: {:?}", index.borrow_index);
        println!("  EGLD price: {:?}", index.egld_price);
        println!("  USD price: {:?}", index.usd_price);
    }

    assert_eq!(market_indexes.len(), 3);
    
    // Verify initial indexes
    for index in &market_indexes {
        assert!(index.supply_index >= ManagedDecimal::from_raw_units(BigUint::from(RAY), RAY_PRECISION));
        assert!(index.borrow_index >= ManagedDecimal::from_raw_units(BigUint::from(RAY), RAY_PRECISION));
        assert!(index.egld_price > ManagedDecimal::from_raw_units(BigUint::zero(), WAD_PRECISION));
        assert!(index.usd_price > ManagedDecimal::from_raw_units(BigUint::zero(), WAD_PRECISION));
    }

    // Test get_all_markets
    let markets = state.get_all_markets(assets);

    println!("\n=== All Markets ===");
    for (i, market) in markets.iter().enumerate() {
        println!("Market {}: {:?}", i, market.asset_id);
        println!("  Contract address: {:?}", market.market_contract_address);
        println!("  Price in EGLD: {:?}", market.price_in_egld);
        println!("  Price in USD: {:?}", market.price_in_usd);
    }

    assert_eq!(markets.len(), 3);
    
    // Verify market data
    for market in &markets {
        assert!(!market.market_contract_address.is_zero());
        assert!(market.price_in_egld > ManagedDecimal::from_raw_units(BigUint::zero(), WAD_PRECISION));
        assert!(market.price_in_usd > ManagedDecimal::from_raw_units(BigUint::zero(), WAD_PRECISION));
    }
}

#[test]
fn test_position_views() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    // Setup accounts
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

    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(50u64),
        2,
        EGLD_DECIMALS,
    );

    // Test get_collateral_amount_for_token
    let usdc_collateral = state.get_collateral_amount_for_token(2, USDC_TOKEN);
    let xegld_collateral = state.get_collateral_amount_for_token(2, XEGLD_TOKEN);
    
    println!("\n=== Collateral Amounts ===");
    println!("USDC collateral: {:?}", usdc_collateral);
    println!("XEGLD collateral: {:?}", xegld_collateral);
    
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

    // Test get_borrow_amount_for_token
    let egld_borrow = state.get_borrow_amount_for_token(2, EGLD_TOKEN);
    
    println!("\n=== Borrow Amounts ===");
    println!("EGLD borrow: {:?}", egld_borrow);
    
    assert_eq!(
        egld_borrow,
        ManagedDecimal::from_raw_units(
            BigUint::from(50u64) * BigUint::from(10u64).pow(EGLD_DECIMALS as u32),
            EGLD_DECIMALS
        )
    );

    // Test aggregate position views
    let total_borrow_egld = state.get_total_borrow_in_egld(2);
    let total_collateral_egld = state.get_total_collateral_in_egld(2);
    let liquidation_collateral = state.get_liquidation_collateral_available(2);
    let ltv_collateral = state.get_ltv_collateral_in_egld(2);
    
    println!("\n=== Aggregate Position Data ===");
    println!("Total borrow in EGLD: {:?}", total_borrow_egld);
    println!("Total collateral in EGLD: {:?}", total_collateral_egld);
    println!("Liquidation collateral available: {:?}", liquidation_collateral);
    println!("LTV collateral in EGLD: {:?}", ltv_collateral);
    
    // Verify relationships
    assert!(total_borrow_egld > ManagedDecimal::from_raw_units(BigUint::zero(), RAY_PRECISION));
    assert!(total_collateral_egld > ManagedDecimal::from_raw_units(BigUint::zero(), RAY_PRECISION));
    assert!(liquidation_collateral > ManagedDecimal::from_raw_units(BigUint::zero(), RAY_PRECISION));
    assert!(ltv_collateral > ManagedDecimal::from_raw_units(BigUint::zero(), RAY_PRECISION));
    
    // LTV collateral should be less than liquidation collateral (more conservative)
    assert!(ltv_collateral < liquidation_collateral);
    // Liquidation collateral should be less than total collateral (due to weighting)
    assert!(liquidation_collateral < total_collateral_egld);
}

#[test]
fn test_price_views() {
    let mut state = LendingPoolTestState::new();
    
    // Test various token prices
    let tokens = vec![
        (EGLD_TOKEN, EGLD_PRICE_IN_DOLLARS),
        (USDC_TOKEN, USDC_PRICE_IN_DOLLARS),
        // (XEGLD_TOKEN, XEGLD_PRICE_IN_DOLLARS),
        (SEGLD_TOKEN, SEGLD_PRICE_IN_DOLLARS),
    ];
    
    println!("\n=== Token Prices ===");
    for (token, expected_usd) in tokens {
        let usd_price = state.get_usd_price(token);
        let egld_price = state.get_egld_price(token);
        
        println!("{:?}:", token);
        println!("  USD price: {:?}", usd_price);
        println!("  EGLD price: {:?}", egld_price);
        
        // Verify USD price is close to expected
        let expected_usd_decimal = ManagedDecimal::from_raw_units(
            BigUint::from(expected_usd) * BigUint::from(10u64).pow(WAD_PRECISION as u32),
            WAD_PRECISION
        );
        
        // Allow for small price variations
        let diff = if usd_price > expected_usd_decimal {
            usd_price.clone() - expected_usd_decimal.clone()
        } else {
            expected_usd_decimal.clone() - usd_price.clone()
        };
        
        let tolerance = expected_usd_decimal.clone() / 100usize; // 1% tolerance
        println!("Diff:      {:?}", diff);
        println!("Tolerance: {:?}", tolerance);
        assert!(diff < tolerance, "Price deviation too large for {:?}", token);
    }
    
    // Test EGLD price should be 1 EGLD = 1 EGLD
    let egld_in_egld = state.get_egld_price(EGLD_TOKEN);
    assert_eq!(
        egld_in_egld,
        ManagedDecimal::from_raw_units(BigUint::from(10u64).pow(WAD_PRECISION as u32), WAD_PRECISION)
    );
}

#[test]
fn test_view_error_cases() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    
    setup_account(&mut state, supplier);
    
    // Create a simple position
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(10u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    let custom_error_message = format!("Token not existing in the account {}", USDC_TOKEN.as_str());

    // Test error for non-existent collateral token
    state.get_collateral_amount_for_token_non_existing(
        1,
        USDC_TOKEN,
        custom_error_message.as_bytes(),
    );
}

#[test]
fn test_complex_liquidation_estimation_scenario() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    // Setup accounts
    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Create complex position with multiple assets
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

    // Fast forward to create bad debt
    state.change_timestamp(SECONDS_PER_DAY * 15000);
    let mut markets = MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN));
    markets.push(EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN));
    markets.push(EgldOrEsdtTokenIdentifier::esdt(CAPPED_TOKEN));
    state.update_markets(&borrower, markets);

    // Get current position state
    let health_factor = state.get_account_health_factor(2);
    let can_liquidate = state.can_be_liquidated(2);
    
    println!("\n=== Position State Before Liquidation ===");
    println!("Health factor: {:?}", health_factor);
    println!("Can be liquidated: {:?}", can_liquidate);
    
    assert!(can_liquidate);
    assert!(health_factor < ManagedDecimal::from_raw_units(BigUint::from(10u64).pow(WAD_PRECISION as u32), WAD_PRECISION));

    // Get all borrowed amounts
    let borrowed_egld = state.get_borrow_amount_for_token(2, EGLD_TOKEN);
    let borrowed_usdc = state.get_borrow_amount_for_token(2, USDC_TOKEN);
    let borrowed_capped = state.get_borrow_amount_for_token(2, CAPPED_TOKEN);

    // Prepare debt payments for full liquidation attempt
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
    let (seized_collaterals, protocol_fees, refunds, max_egld_payment, bonus_rate) = 
        state.liquidation_estimations(2, debt_payments);

    println!("\n=== Complex Liquidation Estimations ===");
    println!("Number of seized collateral types: {}", seized_collaterals.len());
    println!("Total seized collateral value in EGLD:");
    
    let mut total_seized_egld = ManagedDecimal::from_raw_units(BigUint::zero(), RAY_PRECISION);
    for collateral in &seized_collaterals {
        let token_price = if collateral.token_identifier == EgldOrEsdtTokenIdentifier::esdt(XEGLD_TOKEN.to_token_identifier()) {
            state.get_egld_price(XEGLD_TOKEN)
        } else if collateral.token_identifier == EgldOrEsdtTokenIdentifier::esdt(SEGLD_TOKEN.to_token_identifier()) {
            state.get_egld_price(SEGLD_TOKEN)
        } else {
            ManagedDecimal::from_raw_units(BigUint::from(10u64).pow(WAD_PRECISION as u32), WAD_PRECISION)
        };
        
        let amount_decimal = ManagedDecimal::from_raw_units(collateral.amount.clone(), XEGLD_DECIMALS);
        let value_in_egld = amount_decimal * token_price / ManagedDecimal::from_raw_units(BigUint::from(10u64).pow(WAD_PRECISION as u32), WAD_PRECISION);
        total_seized_egld = total_seized_egld + value_in_egld;
        
        println!("  {:?}: {:?} (raw)", collateral.token_identifier, collateral.amount);
    }
    
    println!("\nTotal protocol fees:");
    for fee in &protocol_fees {
        println!("  {:?}: {:?} (raw)", fee.token_identifier, fee.amount);
    }
    
    println!("\nRefunds:");
    if refunds.len() == 0 {
        println!("  No refunds (full liquidation)");
    } else {
        for refund in &refunds {
            println!("  {:?}: {:?} (raw)", refund.token_identifier, refund.amount);
        }
    }
    
    println!("\nMax EGLD payment required: {:?}", max_egld_payment);
    println!("Liquidation bonus rate: {:?}", bonus_rate);

    // Verify complex liquidation results
    assert_eq!(seized_collaterals.len(), 2); // Should seize both XEGLD and SEGLD
    assert_eq!(protocol_fees.len(), 2); // Protocol fees for each seized asset
    assert!(max_egld_payment > ManagedDecimal::from_raw_units(BigUint::zero(), RAY_PRECISION));
    assert!(bonus_rate > ManagedDecimal::from_raw_units(BigUint::from(100u64), BPS_PRECISION)); // > 1%
}
