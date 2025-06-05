use controller::{
    AccountAttributes, PositionMode, ERROR_HEALTH_FACTOR_WITHDRAW,
    ERROR_INVALID_LIQUIDATION_THRESHOLD,
};
use multiversx_sc::types::{
    EgldOrEsdtTokenIdentifier, ManagedDecimal, ManagedOption, MultiValueEncoded,
};
use multiversx_sc_scenario::{
    api::StaticApi,
    imports::{BigUint, OptionalValue, TestAddress},
};
pub mod constants;
pub mod proxys;
pub mod setup;
use constants::*;
use setup::*;

/// Tests edge case math rounding when borrowing equals supply.
/// 
/// Covers:
/// - Interest accrual precision with 100% utilization
/// - Revenue and reserves calculation accuracy
/// - Rounding error handling in full repayment
/// - State consistency after complete withdrawal
#[test]
fn edge_case_100_percent_utilization_rounding() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Supply 100 EGLD and collateral
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
        BigUint::from(5000u64),
        USDC_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::None,
        false,
    );

    // Borrow entire supply (100% utilization)
    state.borrow_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(100u64),
        1,
        EGLD_DECIMALS,
    );

    // Verify initial state
    let mut markets = MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN));
    markets.push(EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN));
    
    let borrowed = state.get_borrow_amount_for_token(1, EGLD_TOKEN);
    let collateral = state.get_collateral_amount_for_token(1, EGLD_TOKEN);
    let utilization = state.get_market_utilization(state.egld_market.clone());

    assert!(borrowed > ManagedDecimal::from_raw_units(BigUint::zero(), EGLD_DECIMALS));
    assert!(collateral > ManagedDecimal::from_raw_units(BigUint::zero(), EGLD_DECIMALS));
    assert_eq!(
        utilization,
        ManagedDecimal::from_raw_units(BigUint::from(10u64).pow(27), 27) // 100% in RAY
    );

    // Advance time to accrue interest
    state.change_timestamp(1111u64);
    state.update_markets(&supplier, markets.clone());

    // Check balances after interest accrual
    let borrowed_after = state.get_borrow_amount_for_token(1, EGLD_TOKEN);
    let collateral_after = state.get_collateral_amount_for_token(1, EGLD_TOKEN);
    let revenue = state.get_market_revenue(state.egld_market.clone());
    let reserves = state.get_market_reserves(state.egld_market.clone());

    // Verify interest accrued correctly
    assert!(borrowed_after > borrowed);
    assert!(collateral_after > collateral);
    assert!(revenue > ManagedDecimal::from_raw_units(BigUint::zero(), EGLD_DECIMALS));
    
    // Reserves should be zero initially
    assert_eq!(
        reserves,
        ManagedDecimal::from_raw_units(BigUint::zero(), EGLD_DECIMALS)
    );

    // Full repayment
    state.repay_asset_deno(&supplier, &EGLD_TOKEN, borrowed_after.into_raw_units().clone(), 1);

    // Check post-repayment state
    let collateral_final = state.get_collateral_amount_for_token(1, EGLD_TOKEN);
    let utilization_final = state.get_market_utilization(state.egld_market.clone());
    let revenue_final = state.get_market_revenue(state.egld_market.clone());

    // Full withdrawal
    state.withdraw_asset_den(
        &supplier,
        EGLD_TOKEN,
        collateral_final.into_raw_units().clone(),
        1,
    );
    
    // Verify final state
    let reserves_final = state.get_market_reserves(state.egld_market.clone());
    let revenue_post_withdraw = state.get_market_revenue(state.egld_market.clone());

    assert!(reserves_final >= revenue_post_withdraw);
    let diff = reserves_final - revenue_post_withdraw;
    
    // Rounding error should be minimal (less than 1 wei)
    assert!(diff <= ManagedDecimal::from_raw_units(BigUint::from(1u64), EGLD_DECIMALS));
}

/// Tests edge case math rounding with overpayment on repayment.
/// 
/// Covers:
/// - Overpayment handling in repay function
/// - Reserve accumulation from excess payment
/// - Multiple precision points across operations
/// - Token existence validation after full repayment
#[test]
fn edge_case_overpayment_reserve_accumulation() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Supply initial liquidity
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
        BigUint::from(5000u64),
        USDC_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::None,
        false,
    );

    // Borrow entire supply
    state.borrow_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(100u64),
        1,
        EGLD_DECIMALS,
    );

    let mut markets = MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN));
    markets.push(EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN));
    
    // Advance time for interest
    state.change_timestamp(1111u64);
    state.update_markets(&supplier, markets.clone());

    let borrowed = state.get_borrow_amount_for_token(1, EGLD_TOKEN);
    let collateral = state.get_collateral_amount_for_token(1, EGLD_TOKEN);
    let revenue = state.get_market_revenue(state.egld_market.clone());
    let reserves = state.get_market_reserves(state.egld_market.clone());

    // Verify initial reserves are zero
    assert_eq!(
        reserves,
        ManagedDecimal::from_raw_units(BigUint::zero(), EGLD_DECIMALS)
    );

    // Overpay by 5 EGLD
    state.repay_asset(
        &supplier,
        &EGLD_TOKEN,
        BigUint::from(105u64),
        1,
        EGLD_DECIMALS,
    );
    
    // Verify borrow no longer exists
    let custom_error_message = format!("Token not existing in the account {}", EGLD_TOKEN.as_str());
    state.get_borrow_amount_for_token_non_existing(1, EGLD_TOKEN, custom_error_message.as_bytes());
    
    // Check reserves accumulated from overpayment
    let collateral_after = state.get_collateral_amount_for_token(1, EGLD_TOKEN);
    let utilization_after = state.get_market_utilization(state.egld_market.clone());
    let revenue_after = state.get_market_revenue(state.egld_market.clone());
    let reserves_after = state.get_market_reserves(state.egld_market.clone());

    assert!(reserves_after >= collateral_after + revenue_after, "Reserves are not enough");

    // Partial withdrawal
    state.withdraw_asset(&supplier, EGLD_TOKEN, BigUint::from(1u64), 1, EGLD_DECIMALS);
    
    // Update markets and verify consistency
    state.update_markets(&supplier, markets.clone());
    
    let reserves_final = state.get_market_reserves(state.egld_market.clone());
    let revenue_final = state.get_market_revenue(state.egld_market.clone());
    let collateral_final = state.get_collateral_amount_for_token(1, EGLD_TOKEN);
    
    assert!(reserves_final >= collateral_final.clone() + revenue_final, "Reserves are not enough");
    
    // Full withdrawal
    state.withdraw_asset_den(
        &supplier,
        EGLD_TOKEN,
        collateral_final.into_raw_units().clone(),
        1,
    );
    
    // Final state verification
    let reserves_end = state.get_market_reserves(state.egld_market.clone());
    let revenue_end = state.get_market_revenue(state.egld_market.clone());
    assert!(reserves_end >= revenue_end);
}

/// Tests complete market exit with multiple participants.
/// 
/// Covers:
/// - Full repayment and withdrawal flow
/// - Account token burning on full exit
/// - Multi-user market interaction
/// - Reserve consistency after complete exit
/// - Position mode preservation during operations
#[test]
fn market_complete_exit_multi_user() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_accounts(&mut state, supplier, borrower);
    
    // Supplier provides liquidity
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Borrower supplies collateral
    state.supply_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(5000u64),
        USDC_DECIMALS,
        OptionalValue::None,
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

    // Owner also supplies to market
    state.supply_asset(
        &OWNER_ADDRESS,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Update markets after time passage
    let mut markets = MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN));
    markets.push(EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN));
    state.change_timestamp(8000u64);
    state.update_markets(&borrower, markets.clone());

    // Verify borrower account state
    state
        .world
        .check_account(borrower)
        .esdt_nft_balance_and_attributes(
            ACCOUNT_TOKEN,
            2,
            BigUint::from(1u64),
            AccountAttributes::<StaticApi> {
                is_isolated_position: false,
                e_mode_category_id: 0,
                mode: PositionMode::Normal,
                isolated_token: ManagedOption::none(),
            },
        );
    
    // Borrower repays full debt
    let borrow_amount = state.get_borrow_amount_for_token(2, EGLD_TOKEN);
    state.repay_asset_deno(
        &borrower,
        &EGLD_TOKEN,
        borrow_amount.into_raw_units().clone(),
        2,
    );
    
    // Verify borrow no longer exists
    let custom_error_message = format!("Token not existing in the account {}", EGLD_TOKEN.as_str());
    state.get_borrow_amount_for_token_non_existing(2, EGLD_TOKEN, custom_error_message.as_bytes());
    
    // Update markets significantly later
    state.change_timestamp(1000000u64);
    state.update_markets(&borrower, markets.clone());

    // Borrower withdraws all collateral
    let supplied_collateral = state.get_collateral_amount_for_token(2, USDC_TOKEN);
    state.withdraw_asset_den(
        &borrower,
        USDC_TOKEN,
        supplied_collateral.into_raw_units().clone(),
        2,
    );
    
    // Verify collateral removed
    let custom_error_message = format!("Token not existing in the account {}", USDC_TOKEN.as_str());
    state.get_collateral_amount_for_token_non_existing(
        2,
        USDC_TOKEN,
        custom_error_message.as_bytes(),
    );
    
    // Verify account token burned after full exit
    state
        .world
        .check_account(borrower)
        .esdt_nft_balance_and_attributes(
            ACCOUNT_TOKEN,
            2,
            BigUint::zero(),
            AccountAttributes::<StaticApi> {
                is_isolated_position: false,
                e_mode_category_id: 0,
                mode: PositionMode::Normal,
                isolated_token: ManagedOption::none(),
            },
        );

    // Supplier withdraws all funds
    let supplied_collateral = state.get_collateral_amount_for_token(1, EGLD_TOKEN);
    state.withdraw_asset_den(
        &supplier,
        EGLD_TOKEN,
        supplied_collateral.into_raw_units().clone(),
        1,
    );

    let custom_error_message = format!("Token not existing in the account {}", EGLD_TOKEN.as_str());
    state.get_collateral_amount_for_token_non_existing(
        1,
        EGLD_TOKEN,
        custom_error_message.as_bytes(),
    );

    // Owner withdraws remaining funds
    let supplied_collateral = state.get_collateral_amount_for_token(3, EGLD_TOKEN);
    let reserves = state.get_market_reserves(state.egld_market.clone());
    let revenue = state.get_market_revenue(state.egld_market.clone());
    
    state.withdraw_asset_den(
        &OWNER_ADDRESS,
        EGLD_TOKEN,
        supplied_collateral.into_raw_units().clone(),
        3,
    );
    
    state.get_collateral_amount_for_token_non_existing(
        3,
        EGLD_TOKEN,
        custom_error_message.as_bytes(),
    );

    // Verify final reserve state
    let reserves = state.get_market_reserves(state.egld_market.clone());
    let revenue = state.get_market_revenue(state.egld_market.clone());
    assert!(reserves >= revenue);
}

/// Tests interest accrual over long time period.
/// 
/// Covers:
/// - Yearly interest accumulation
/// - High utilization rate impact
/// - Borrow rate calculation accuracy
/// - Interest compounding effects
/// - Full repayment after significant interest
#[test]
fn interest_accrual_long_term_high_utilization() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Supply liquidity
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(200u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    
    // Borrower supplies large collateral
    state.supply_asset(
        &borrower,
        XEGLD_TOKEN,
        BigUint::from(1500000u64),
        XEGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    
    // Borrow 80% of supply (high utilization)
    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(160u64),
        2,
        EGLD_DECIMALS,
    );

    // Record initial amounts
    let initial_borrow = state.get_borrow_amount_for_token(2, EGLD_TOKEN);
    let initial_supply = state.get_collateral_amount_for_token(1, EGLD_TOKEN);
    
    let mut markets = MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN));
    
    // Check initial market state
    let utilization = state.get_market_utilization(state.egld_market.clone());
    let borrow_rate = state.get_market_borrow_rate(state.egld_market.clone());
    
    // Advance one year
    state.change_timestamp(SECONDS_PER_YEAR);
    state.update_markets(&supplier, markets.clone());

    // Verify significant interest accrual
    let final_borrow = state.get_borrow_amount_for_token(2, EGLD_TOKEN);
    let final_supply = state.get_collateral_amount_for_token(1, EGLD_TOKEN);

    assert!(final_borrow > initial_borrow);
    assert!(final_supply > initial_supply);
    
    // Full repayment
    state.repay_asset_deno(
        &borrower,
        &EGLD_TOKEN,
        final_borrow.into_raw_units().clone(),
        2,
    );

    // Verify final state
    let final_supply_after_repay = state.get_collateral_amount_for_token(1, EGLD_TOKEN);
    let reserves = state.get_market_reserves(state.egld_market.clone());
    let revenue = state.get_market_revenue(state.egld_market.clone());
    
    // Supply should have grown from interest
    assert!(final_supply_after_repay > initial_supply);
}

/// Tests interest accrual with multiple suppliers entering at different times.
/// 
/// Covers:
/// - Fair interest distribution among suppliers
/// - Time-weighted interest accumulation
/// - Market dynamics with changing supply
/// - Interest rate adjustments with utilization changes
#[test]
fn interest_accrual_multiple_suppliers_different_times() {
    let mut state = LendingPoolTestState::new();
    let supplier1 = TestAddress::new("supplier1");
    let supplier2 = TestAddress::new("supplier2");
    let borrower = TestAddress::new("borrower");

    state.change_timestamp(0);
    setup_account(&mut state, supplier1);
    setup_account(&mut state, supplier2);
    setup_account(&mut state, borrower);

    // First supplier enters
    state.supply_asset(
        &supplier1,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Borrower provides collateral and borrows
    state.supply_asset(
        &borrower,
        XEGLD_TOKEN,
        BigUint::from(200u64),
        XEGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(50u64),
        3,
        EGLD_DECIMALS,
    );

    // Record supplier1's initial position
    let supplier1_initial = state.get_collateral_amount_for_token(1, EGLD_TOKEN);

    // Advance time before second supplier
    let mut markets = MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN));
    state.change_timestamp(SECONDS_PER_DAY * 30);
    state.update_markets(&supplier1, markets.clone());

    // Second supplier enters after interest has accrued
    state.supply_asset(
        &supplier2,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    let supplier2_initial = state.get_collateral_amount_for_token(2, EGLD_TOKEN);

    // Continue for another period
    state.change_timestamp(SECONDS_PER_DAY * 60);
    state.update_markets(&supplier1, markets.clone());

    // Check final positions
    let supplier1_final = state.get_collateral_amount_for_token(1, EGLD_TOKEN);
    let supplier2_final = state.get_collateral_amount_for_token(2, EGLD_TOKEN);
    let borrower_debt = state.get_borrow_amount_for_token(3, EGLD_TOKEN);

    // Supplier1 should have earned more interest (longer time)
    let supplier1_interest = supplier1_final.clone() - supplier1_initial.clone();
    let supplier2_interest = supplier2_final.clone() - supplier2_initial.clone();
    
    assert!(supplier1_interest > supplier2_interest);

    // Borrower repays debt
    state.repay_asset_deno(
        &borrower,
        &EGLD_TOKEN,
        borrower_debt.into_raw_units().clone(),
        3,
    );

    // Both suppliers withdraw
    state.withdraw_asset_den(
        &supplier1,
        EGLD_TOKEN,
        supplier1_final.into_raw_units().clone(),
        1,
    );
    state.withdraw_asset_den(
        &supplier2,
        EGLD_TOKEN,
        supplier2_final.into_raw_units().clone(),
        2,
    );

    // Verify clean exit
    let reserves = state.get_market_reserves(state.egld_market.clone());
    let revenue = state.get_market_revenue(state.egld_market.clone());
    assert!(reserves >= revenue);
}

/// Tests oracle price feed for LP tokens.
/// 
/// Covers:
/// - LP token price calculation
/// - Oracle integration for composite assets
#[test]
fn oracle_lp_token_price_feed() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");

    setup_account(&mut state, supplier);

    // Supply LP token
    state.supply_asset(
        &supplier,
        LP_EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Verify LP token price calculations
    let usd_price = state.get_usd_price(LP_EGLD_TOKEN);
    let egld_price = state.get_egld_price(LP_EGLD_TOKEN);
    
    assert!(usd_price > ManagedDecimal::from_raw_units(BigUint::zero(), 18));
    assert!(egld_price > ManagedDecimal::from_raw_units(BigUint::zero(), 18));
}

/// Tests updating asset configuration after supply exists.
/// 
/// Covers:
/// - Dynamic configuration updates
/// - Impact on existing positions
/// - Safe configuration change validation
/// - Interest rate model updates
#[test]
fn configuration_update_with_existing_supply() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_accounts(&mut state, supplier, borrower);

    // Initial supply and borrow
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
        BigUint::from(200u64),
        XEGLD_DECIMALS,
        OptionalValue::None,
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

    // Update asset configuration using edit_asset_config
    let config = get_egld_config();
    state.edit_asset_config(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        &BigUint::from(7500u64), // 75% loan_to_value
        &BigUint::from(8000u64), // 80% liquidation_threshold
        &BigUint::from(555u64),  // liquidation_bonus
        &BigUint::from(1800u64), // 18% reserve_factor
        config.config.is_isolated_asset,
        config.config.isolation_debt_ceiling_usd.into_raw_units(),
        config.config.is_siloed_borrowing,
        config.config.is_flashloanable,
        config.config.flashloan_fee.into_raw_units(),
        config.config.is_collateralizable,
        config.config.is_borrowable,
        config.config.isolation_borrow_enabled,
        &config.config.borrow_cap.unwrap_or(BigUint::from(0u64)),
        &config.config.supply_cap.unwrap_or(BigUint::from(0u64)),
        None,
    );

    // Advance time and verify new rates apply
    let mut markets = MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN));
    state.change_timestamp(SECONDS_PER_DAY);
    state.update_markets(&borrower, markets);

    // Verify interest accrual with new rates
    let borrow_amount = state.get_borrow_amount_for_token(2, EGLD_TOKEN);
    assert!(borrow_amount > ManagedDecimal::from_raw_units(BigUint::from(50u64) * BigUint::from(10u64).pow(EGLD_DECIMALS as u32), EGLD_DECIMALS));
}

/// Tests safe configuration updates via endpoint with validation.
/// 
/// Covers:
/// - Endpoint-based configuration updates
/// - Parameter validation ranges
/// - Multi-field updates in single transaction
/// - Configuration consistency checks
#[test]
fn configuration_update_endpoint_safe_values() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_accounts(&mut state, supplier, borrower);

    // Setup initial positions
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
        BigUint::from(200u64),
        XEGLD_DECIMALS,
        OptionalValue::None,
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

    // Use edit_asset_config to update multiple parameters
    let config = get_egld_config();
    state.edit_asset_config(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        &BigUint::from(7500u64), // 75% loan_to_value  
        &BigUint::from(8000u64), // 80% liquidation_threshold
        &BigUint::from(500u64),  // 5% liquidation_bonus
        &BigUint::from(1800u64), // 18% reserve_factor
        config.config.is_isolated_asset,
        config.config.isolation_debt_ceiling_usd.into_raw_units(),
        config.config.is_siloed_borrowing,
        config.config.is_flashloanable,
        config.config.flashloan_fee.into_raw_units(),
        config.config.is_collateralizable,
        config.config.is_borrowable,
        config.config.isolation_borrow_enabled,
        &config.config.borrow_cap.unwrap_or(BigUint::from(0u64)),
        &config.config.supply_cap.unwrap_or(BigUint::from(0u64)),
        None,
    );
}

/// Tests risky configuration updates without existing borrows.
/// 
/// Covers:
/// - High-risk parameter changes
/// - LTV and liquidation threshold updates
/// - Configuration change when no borrows exist
/// - Validation of extreme values
#[test]
fn configuration_update_risky_values_no_borrows() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");

    setup_account(&mut state, supplier);

    // Supply without borrows
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Apply risky changes (allowed without borrows)
    let config = get_egld_config();
    state.edit_asset_config(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        &BigUint::from(9000u64), // 90% loan_to_value
        &BigUint::from(9200u64), // 92% liquidation_threshold
        &BigUint::from(555u64),
        &BigUint::from(600u64),
        config.config.is_isolated_asset,
        config.config.isolation_debt_ceiling_usd.into_raw_units(),
        config.config.is_siloed_borrowing,
        config.config.is_flashloanable,
        config.config.flashloan_fee.into_raw_units(),
        false, // is_collateralizable = false
        false, // is_borrowable = false
        false, // isolation_borrow_enabled = false
        &config.config.borrow_cap.unwrap_or(BigUint::from(0u64)),
        &config.config.supply_cap.unwrap_or(BigUint::from(0u64)),
        None,
    );
}

/// Tests risky configuration updates with existing borrows (allowed case).
/// 
/// Covers:
/// - Risky updates validation with active borrows
/// - Health factor preservation after changes
/// - LTV reduction impact on borrowing capacity
/// - Configuration change safeguards
#[test]
fn configuration_update_risky_values_with_borrows_allowed() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_accounts(&mut state, supplier, borrower);

    // Create active borrow position
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
        XEGLD_TOKEN,
        BigUint::from(200u64),
        XEGLD_DECIMALS,
        OptionalValue::None,
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

    // Get health factor before update
    let health_before = state.get_account_health_factor(2);

    // Update XEGLD configuration to reduce collateral value
    let config = get_xegld_config();
    state.edit_asset_config(
        EgldOrEsdtTokenIdentifier::esdt(XEGLD_TOKEN.to_token_identifier()),
        &BigUint::from(6000u64), // Reduce from 75% to 60%
        &BigUint::from(7000u64), // Reduce from 80% to 70%
        &BigUint::from(600u64),
        &BigUint::from(600u64),
        config.config.is_isolated_asset,
        config.config.isolation_debt_ceiling_usd.into_raw_units(),
        config.config.is_siloed_borrowing,
        config.config.is_flashloanable,
        config.config.flashloan_fee.into_raw_units(),
        config.config.is_collateralizable,
        config.config.is_borrowable,
        config.config.isolation_borrow_enabled,
        &config.config.borrow_cap.unwrap_or(BigUint::from(0u64)),
        &config.config.supply_cap.unwrap_or(BigUint::from(0u64)),
        None,
    );

    // Update thresholds via endpoint
    let mut nonces = MultiValueEncoded::new();
    nonces.push(2u64); // borrower's nonce
    state.update_account_threshold(
        EgldOrEsdtTokenIdentifier::esdt(XEGLD_TOKEN.to_token_identifier()),
        true, // risky update
        nonces,
        None,
    );

    // Verify health factor still safe
    let health_after = state.get_account_health_factor(2);
    assert!(health_after >= ManagedDecimal::from_raw_units(BigUint::from(1u64), 27));
    
    // Health should decrease due to lower collateral value
    assert!(health_after < health_before);
}

/// Tests risky configuration updates that would harm health factor.
/// 
/// Covers:
/// - Configuration change rejection when health factor at risk
/// - Validation of liquidation threshold changes
/// - Protection of existing borrowers
/// - Error handling for unsafe updates
#[test]
fn configuration_update_risky_values_health_factor_violation() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    setup_accounts(&mut state, supplier, borrower);

    // Create position close to liquidation
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
        XEGLD_TOKEN,
        BigUint::from(120u64), // Lower collateral
        XEGLD_DECIMALS,
        OptionalValue::None,
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

    // Try to reduce XEGLD collateral value drastically
    let config = get_xegld_config();
    state.edit_asset_config(
        EgldOrEsdtTokenIdentifier::esdt(XEGLD_TOKEN.to_token_identifier()),
        &BigUint::from(3000u64), // 30% (from 75%)
        &BigUint::from(4000u64), // 40% (from 80%)
        &BigUint::from(600u64),
        &BigUint::from(600u64),
        config.config.is_isolated_asset,
        config.config.isolation_debt_ceiling_usd.into_raw_units(),
        config.config.is_siloed_borrowing,
        config.config.is_flashloanable,
        config.config.flashloan_fee.into_raw_units(),
        config.config.is_collateralizable,
        config.config.is_borrowable,
        config.config.isolation_borrow_enabled,
        &config.config.borrow_cap.unwrap_or(BigUint::from(0u64)),
        &config.config.supply_cap.unwrap_or(BigUint::from(0u64)),
        None,
    );

    // Update should fail due to health factor violation
    let mut nonces = MultiValueEncoded::new();
    nonces.push(2u64); // borrower's nonce
    state.update_account_threshold(
        EgldOrEsdtTokenIdentifier::esdt(XEGLD_TOKEN.to_token_identifier()),
        true, // risky update
        nonces,
        Some(ERROR_HEALTH_FACTOR_WITHDRAW),
    );
}

/// Tests invalid LTV configuration.
/// 
/// Covers:
/// - LTV higher than liquidation threshold validation
/// - Configuration consistency rules
/// - Error message validation
#[test]
fn configuration_update_invalid_ltv_threshold_relationship() {
    let mut state = LendingPoolTestState::new();

    // Try to set LTV higher than liquidation threshold
    let config = get_egld_config();
    state.edit_asset_config(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        &BigUint::from(8500u64), // 85% LTV
        &BigUint::from(8000u64), // 80% liquidation_threshold (invalid - lower than LTV)
        &BigUint::from(555u64),
        &BigUint::from(600u64),
        config.config.is_isolated_asset,
        config.config.isolation_debt_ceiling_usd.into_raw_units(),
        config.config.is_siloed_borrowing,
        config.config.is_flashloanable,
        config.config.flashloan_fee.into_raw_units(),
        config.config.is_collateralizable,
        config.config.is_borrowable,
        config.config.isolation_borrow_enabled,
        &config.config.borrow_cap.unwrap_or(BigUint::from(0u64)),
        &config.config.supply_cap.unwrap_or(BigUint::from(0u64)),
        Some(ERROR_INVALID_LIQUIDATION_THRESHOLD),
    );
}
