use common_constants::WAD_PRECISION;
use multiversx_sc::types::{
    EgldOrEsdtTokenIdentifier, ManagedDecimal, ManagedMapEncoded, MultiValueEncoded,
};
use multiversx_sc_scenario::{
    api::StaticApi,
    imports::{BigUint, OptionalValue, TestAddress},
};
pub mod constants;
pub mod proxys;
pub mod setup;
use constants::*;
use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use setup::*;

/// Tests that dust amounts remain minimal after complete market exit by multiple users.
///
/// Covers:
/// - Multi-user market exit with interest accrual
/// - Dust accumulation from rounding
/// - Reserve and revenue consistency after full exit
/// - Protocol fee collection accuracy
/// - High frequency market updates
#[test]
fn market_exit_dust_accumulation_minimal() {
    let mut state = LendingPoolTestState::new();
    let supplier1 = TestAddress::new("supplier");
    let supplier2 = TestAddress::new("supplier2");
    let borrower1 = TestAddress::new("borrower");
    let borrower2 = TestAddress::new("borrower2");

    state.change_timestamp(0);
    setup_accounts(&mut state, supplier1, borrower1);
    setup_accounts(&mut state, supplier2, borrower2);

    // First pair - supplier1 and borrower1
    state.supply_asset(
        &supplier1,
        EGLD_TOKEN,
        BigUint::from(100000u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.supply_asset(
        &borrower1,
        EGLD_TOKEN,
        BigUint::from(100000u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.borrow_asset(
        &borrower1,
        EGLD_TOKEN,
        BigUint::from(50000u64),
        2,
        EGLD_DECIMALS,
    );

    // Second pair - supplier2 and borrower2
    state.supply_asset(
        &supplier2,
        EGLD_TOKEN,
        BigUint::from(100000u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.supply_asset(
        &borrower2,
        EGLD_TOKEN,
        BigUint::from(100000u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.borrow_asset(
        &borrower2,
        EGLD_TOKEN,
        BigUint::from(50000u64),
        4,
        EGLD_DECIMALS,
    );

    // Record initial market state
    let utilization_ratio = state.get_market_utilization(state.egld_market.clone());

    // Verify 25% utilization
    assert_eq!(
        utilization_ratio,
        ManagedDecimal::from_raw_units(BigUint::from(250000000000000000000000000u128), 27)
    );

    // Update markets frequently over one day (every 6 seconds)
    let mut markets = MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN));
    let update_frequency = 60; // seconds
    let total_updates = SECONDS_PER_DAY / update_frequency;

    for i in 1..=total_updates {
        state.change_timestamp(i * update_frequency);
        state.update_markets(&OWNER_ADDRESS, markets.clone());
        state.claim_revenue(EGLD_TOKEN);
    }

    // Get final positions
    let final_supply_supplier1 = state.get_collateral_amount_for_token(1, EGLD_TOKEN);
    let final_supply_borrower1 = state.get_collateral_amount_for_token(2, EGLD_TOKEN);
    let final_borrow_borrower1 = state.get_borrow_amount_for_token(2, EGLD_TOKEN);
    let final_supply_supplier2 = state.get_collateral_amount_for_token(3, EGLD_TOKEN);
    let final_supply_borrower2 = state.get_collateral_amount_for_token(4, EGLD_TOKEN);
    let final_borrow_borrower2 = state.get_borrow_amount_for_token(4, EGLD_TOKEN);

    // Full exit for all users
    state.repay_asset_deno(
        &borrower1,
        &EGLD_TOKEN,
        final_borrow_borrower1.into_raw_units().clone(),
        2,
    );

    state.withdraw_asset_den(
        &supplier1,
        EGLD_TOKEN,
        final_supply_supplier1.into_raw_units().clone(),
        1,
    );

    state.withdraw_asset_den(
        &borrower1,
        EGLD_TOKEN,
        final_supply_borrower1.into_raw_units().clone(),
        2,
    );

    state.repay_asset_deno(
        &borrower2,
        &EGLD_TOKEN,
        final_borrow_borrower2.into_raw_units().clone(),
        4,
    );

    state.withdraw_asset_den(
        &supplier2,
        EGLD_TOKEN,
        final_supply_supplier2.into_raw_units().clone(),
        3,
    );

    state.withdraw_asset_den(
        &borrower2,
        EGLD_TOKEN,
        final_supply_borrower2.into_raw_units().clone(),
        4,
    );

    // Verify dust is minimal
    let protocol_revenue = state.get_market_revenue(state.egld_market.clone());
    let reserves = state.get_market_reserves(state.egld_market.clone());

    println!("reserves:         {}", reserves);
    println!("protocol_revenue: {}", protocol_revenue);
    let dust = if reserves >= protocol_revenue {
        reserves - protocol_revenue
    } else {
        protocol_revenue - reserves
    };

    let supplied = state.get_market_supplied(state.egld_market.clone());
    println!("supplied: {}", supplied);
    println!("dust: {}", dust);
    // Dust should be less than 5 wei (extremely small)
    assert!(dust <= ManagedDecimal::from_raw_units(BigUint::from(1u64), WAD_PRECISION));
}

const SEED: u64 = 69696; // Fixed seed for reproducible tests

/// Simulates many users performing random actions to stress test the protocol.
///
/// Covers:
/// - Large-scale user interactions (1000 users)
/// - Random supply, borrow, repay, withdraw actions
/// - Market updates under various utilization rates
/// - Protocol robustness with concurrent operations
/// - Final settlement and clean exit
/// - Interest accrual over simulated time
/// - Edge cases from random action combinations
#[test]
fn stress_test_random_user_actions_large_scale() {
    let mut state = LendingPoolTestState::new();
    let mut rng = ChaCha8Rng::seed_from_u64(SEED);

    const NUM_ACTIONS: usize = 4000;
    const MAX_TIME_JUMP_SECONDS: u64 = SECONDS_PER_DAY * 10;
    const NUM_USERS: usize = 1000;
    const MAX_AMOUNT: u64 = 100_000;

    // Create user accounts
    let mut borrower_names = Vec::with_capacity(NUM_USERS);
    let mut supplier_names = Vec::with_capacity(NUM_USERS);

    for i in 0..NUM_USERS {
        borrower_names.push(format!("borrower{}", i));
        supplier_names.push(format!("supplier{}", i));
    }

    let mut borrowers = Vec::with_capacity(NUM_USERS);
    let mut suppliers = Vec::with_capacity(NUM_USERS);
    let mut all_users = Vec::with_capacity(NUM_USERS * 2);
    let mut user_nonces: ManagedMapEncoded<StaticApi, TestAddress, u64> = ManagedMapEncoded::new();
    let mut nonce_counter: u64 = 0;

    // Initialize all user accounts
    for i in 0..NUM_USERS {
        let borrower = TestAddress::new(borrower_names[i].as_str());
        let supplier = TestAddress::new(supplier_names[i].as_str());

        borrowers.push(borrower);
        suppliers.push(supplier);
        all_users.push(borrower);
        all_users.push(supplier);

        setup_account(&mut state, borrower);
        setup_account(&mut state, supplier);
    }

    // Prepare markets for updates
    let mut markets = MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN));

    // Simulation loop
    let mut current_timestamp = 0u64;
    state.change_timestamp(current_timestamp);

    for action_index in 0..NUM_ACTIONS {
        // Advance time randomly
        let time_increase = rng.random_range(1..=MAX_TIME_JUMP_SECONDS);
        current_timestamp += time_increase;
        state.change_timestamp(current_timestamp);
        state.update_markets(&OWNER_ADDRESS, markets.clone());

        // Update markets periodically
        if action_index % 10 == 0 {
            let mut markets = MultiValueEncoded::new();
            markets.push(EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN));
            state.update_markets(&OWNER_ADDRESS, markets);
        }

        // if action_index % 10 == 1 {
        state.claim_revenue(EGLD_TOKEN);
        // }

        // Select random user
        let user_index = rng.random_range(0..all_users.len());
        let user_addr = all_users[user_index];
        let is_borrower = borrowers.contains(&user_addr);

        // Determine action based on probability
        let action_type = rng.random_range(0..100);

        if is_borrower {
            // Borrower actions: Supply (30%), Borrow (30%), Repay (20%), Withdraw (20%)
            if action_type < 30 {
                // Supply action
                let amount = BigUint::from(rng.random_range(1_000..=MAX_AMOUNT));
                if user_nonces.contains(&user_addr) {
                    let nonce = user_nonces.get(&user_addr);
                    state.supply_asset(
                        &user_addr,
                        EGLD_TOKEN,
                        amount,
                        EGLD_DECIMALS,
                        OptionalValue::Some(nonce),
                        OptionalValue::None,
                        false,
                    );
                } else {
                    state.supply_asset(
                        &user_addr,
                        EGLD_TOKEN,
                        amount,
                        EGLD_DECIMALS,
                        OptionalValue::None,
                        OptionalValue::None,
                        false,
                    );
                    nonce_counter += 1;
                    user_nonces.put(&user_addr, &nonce_counter);
                }
            } else if action_type < 60 {
                // Borrow action
                if user_nonces.contains(&user_addr) {
                    let nonce = user_nonces.get(&user_addr);
                    let total_borrow = state.get_total_borrow_in_egld(nonce);
                    let ltv_collateral = state.get_ltv_collateral_in_egld(nonce);

                    if ltv_collateral > total_borrow {
                        let available_borrow = ltv_collateral - total_borrow;

                        state.borrow_asset(
                            &user_addr,
                            EGLD_TOKEN,
                            available_borrow.into_raw_units().clone() / BigUint::from(WAD),
                            nonce,
                            EGLD_DECIMALS,
                        );
                    }
                }
            } else if action_type < 80 {
                // Repay action
                if user_nonces.contains(&user_addr) {
                    let nonce = user_nonces.get(&user_addr);
                    let current_borrow = state.get_total_borrow_in_egld(nonce);
                    if current_borrow.into_raw_units() > &BigUint::zero() {
                        state.repay_asset_deno(
                            &user_addr,
                            &EGLD_TOKEN,
                            current_borrow.into_raw_units().clone(),
                            nonce,
                        );
                        assert!(
                            state.get_total_borrow_in_egld(nonce).into_raw_units()
                                == &BigUint::zero()
                        );
                    }
                }
            } else {
                // Withdraw action
                if user_nonces.contains(&user_addr) {
                    let nonce = user_nonces.get(&user_addr);
                    let current_supply = state.get_total_collateral_in_egld(nonce);
                    let total_borrow = state.get_total_borrow_in_egld(nonce);

                    if current_supply.into_raw_units() > &BigUint::zero()
                        && total_borrow.into_raw_units() == &BigUint::zero()
                    {
                        let max_withdraw = current_supply;
                        state.withdraw_asset_den(
                            &user_addr,
                            EGLD_TOKEN,
                            max_withdraw.into_raw_units().clone(),
                            nonce,
                        );
                        user_nonces.remove(&user_addr);
                    }
                }
            }
        } else {
            // Supplier actions: Supply (70%), Withdraw (30%)
            if action_type < 70 {
                // Supply action
                let amount = BigUint::from(rng.random_range(1_000..=MAX_AMOUNT));
                if user_nonces.contains(&user_addr) {
                    let nonce = user_nonces.get(&user_addr);
                    state.supply_asset(
                        &user_addr,
                        EGLD_TOKEN,
                        amount,
                        EGLD_DECIMALS,
                        OptionalValue::Some(nonce),
                        OptionalValue::None,
                        false,
                    );
                } else {
                    state.supply_asset(
                        &user_addr,
                        EGLD_TOKEN,
                        amount,
                        EGLD_DECIMALS,
                        OptionalValue::None,
                        OptionalValue::None,
                        false,
                    );
                    nonce_counter += 1;
                    user_nonces.put(&user_addr, &nonce_counter);
                }
            } else {
                // Withdraw action
                if user_nonces.contains(&user_addr) {
                    let nonce = user_nonces.get(&user_addr);
                    let current_supply = state.get_total_collateral_in_egld(nonce);
                    if current_supply.into_raw_units() > &BigUint::zero() {
                        state.withdraw_asset_den(
                            &user_addr,
                            EGLD_TOKEN,
                            current_supply.into_raw_units().clone(),
                            nonce,
                        );
                        assert!(
                            state.get_total_collateral_in_egld(nonce).into_raw_units()
                                == &BigUint::zero()
                        );
                        assert!(
                            state.get_total_borrow_in_egld(nonce).into_raw_units()
                                == &BigUint::zero()
                        );
                        user_nonces.remove(&user_addr);
                    }
                }
            }
        }
    }

    // Final settlement - clean exit for all users
    let final_timestamp = current_timestamp + SECONDS_PER_DAY;
    state.change_timestamp(final_timestamp);
    state.update_markets(&OWNER_ADDRESS, markets.clone());

    for user_addr in &all_users {
        if user_nonces.contains(user_addr) {
            let nonce = user_nonces.get(user_addr);

            // Repay all debt if borrower
            if borrowers.contains(user_addr) {
                let final_borrow = state.get_total_borrow_in_egld(nonce);
                if final_borrow.into_raw_units() > &BigUint::zero() {
                    state.repay_asset_deno(
                        user_addr,
                        &EGLD_TOKEN,
                        final_borrow.into_raw_units().clone(),
                        nonce,
                    );
                }
            }

            // Withdraw all supply
            let final_supply = state.get_total_collateral_in_egld(nonce);
            if final_supply.into_raw_units() > &BigUint::zero() {
                state.withdraw_asset_den(
                    user_addr,
                    EGLD_TOKEN,
                    final_supply.into_raw_units().clone(),
                    nonce,
                );
                user_nonces.remove(user_addr);
            }

            assert!(state.get_total_collateral_in_egld(nonce).into_raw_units() == &BigUint::zero());
            assert!(state.get_total_borrow_in_egld(nonce).into_raw_units() == &BigUint::zero());
        }
    }

    // Verify market integrity after simulation
    let protocol_revenue = state.get_market_revenue(state.egld_market.clone());
    println!("protocol_revenue: {}", protocol_revenue);
    let reserves = state.get_market_reserves(state.egld_market.clone());
    println!("reserves: {}", reserves);
    let final_utilization = state.get_market_utilization(state.egld_market.clone());
    println!("final_utilization: {}", final_utilization);
    let supplied = state.get_market_supplied(state.egld_market.clone());
    println!("supplied: {}", supplied);
    let borrowed = state.get_market_borrowed(state.egld_market.clone());
    println!("borrowed: {}", borrowed);

    // Basic sanity checks
    assert!(reserves.into_raw_units() >= &BigUint::zero());
    assert!(protocol_revenue.into_raw_units() >= &BigUint::zero());
    // Use precision tolerance (allow up to 1000 wei difference):
    let diff = if protocol_revenue > reserves {
        protocol_revenue - reserves
    } else {
        reserves - protocol_revenue
    };
    assert!(diff <= ManagedDecimal::from_raw_units(BigUint::from(1000u64), WAD_PRECISION));

    assert!(supplied.into_raw_units() >= &BigUint::zero());
    assert!(borrowed.into_raw_units() == &BigUint::zero());
    assert!(final_utilization == ManagedDecimal::from_raw_units(BigUint::zero(), 27));

    state.claim_revenue(EGLD_TOKEN);
    // Verify market integrity after simulation
    let protocol_revenue = state.get_market_revenue(state.egld_market.clone());
    println!("protocol_revenue: {}", protocol_revenue);
    let reserves = state.get_market_reserves(state.egld_market.clone());
    println!("reserves: {}", reserves);
    let supplied = state.get_market_supplied(state.egld_market.clone());
    println!("supplied: {}", supplied);
    let borrowed = state.get_market_borrowed(state.egld_market.clone());
    println!("borrowed: {}", borrowed);

    assert!(protocol_revenue.into_raw_units() == &BigUint::zero());

    assert!(supplied.into_raw_units() == &BigUint::zero());
    assert!(borrowed.into_raw_units() == &BigUint::zero());
}
