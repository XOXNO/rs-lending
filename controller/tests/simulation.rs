use controller::WAD_PRECISION;
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
use rand::prelude::*; // <-- Import rand traits
use rand_chacha::ChaCha8Rng;
use setup::*;

#[test]
fn test_leave_low_dust_in_market() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let supplier2 = TestAddress::new("supplier2");
    let borrower = TestAddress::new("borrower");
    let borrower2 = TestAddress::new("borrower2");

    // Setup initial state
    state.change_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);
    setup_accounts(&mut state, supplier2, borrower2);

    // Initial supply and borrow
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(100000u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.supply_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(100000u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(50000u64),
        2,
        EGLD_DECIMALS,
    );

    // // Initial supply and borrow
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
    // Borrow from second supplier
    state.borrow_asset(
        &borrower2,
        EGLD_TOKEN,
        BigUint::from(50000u64),
        4,
        EGLD_DECIMALS,
    );

    let utilization_ratio = state.get_market_utilization(state.egld_market.clone());
    let borrow_rate = state.get_market_borrow_rate(state.egld_market.clone());
    let supply_rate = state.get_market_supply_rate(state.egld_market.clone());
    println!("utilization_ratio: {:?}", utilization_ratio);
    println!("borrow_rate: {:?}", borrow_rate);
    println!("supply_rate: {:?}", supply_rate);

    // Record initial amounts
    let initial_supply_borrower = state.get_collateral_amount_for_token(2, EGLD_TOKEN);
    let initial_supply_supplier = state.get_collateral_amount_for_token(1, EGLD_TOKEN);
    let initial_borrow_borrower = state.get_borrow_amount_for_token(2, EGLD_TOKEN);

    println!("initial_supply_borrower: {:?}", initial_supply_borrower);
    println!("initial_supply_supplier: {:?}", initial_supply_supplier);
    println!("initial_borrow_borrower: {:?}", initial_borrow_borrower);

    let mut markets = MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN));
    let days = (SECONDS_PER_YEAR * 4) / SECONDS_PER_DAY;
    // Simulate hourly updates for 2 years
    for day in 1..=days {
        state.change_timestamp(day * SECONDS_PER_DAY);
        state.update_markets(&OWNER_ADDRESS, markets.clone());
    }

    let final_supply_supplier = state.get_collateral_amount_for_token(1, EGLD_TOKEN);
    let final_supply_borrower = state.get_collateral_amount_for_token(2, EGLD_TOKEN);
    let final_borrow_borrower = state.get_borrow_amount_for_token(2, EGLD_TOKEN);
    let final_supply_supplier2 = state.get_collateral_amount_for_token(3, EGLD_TOKEN);
    let final_supply_borrower2 = state.get_collateral_amount_for_token(4, EGLD_TOKEN);
    let final_borrow_borrower2 = state.get_borrow_amount_for_token(4, EGLD_TOKEN);
    println!("final_supply_supplier: {:?}", final_supply_supplier);
    println!("final_supply_borrower: {:?}", final_supply_borrower);
    println!("final_borrow_borrower: {:?}", final_borrow_borrower);
    println!("final_supply_supplier2: {:?}", final_supply_supplier2);
    println!("final_supply_borrower2: {:?}", final_supply_borrower2);
    println!("final_borrow_borrower2: {:?}", final_borrow_borrower2);
    state.repay_asset_deno(
        &borrower,
        &EGLD_TOKEN,
        final_borrow_borrower.into_raw_units().clone(),
        2,
    );

    state.withdraw_asset_den(
        &supplier,
        EGLD_TOKEN,
        final_supply_supplier.into_raw_units().clone(),
        1,
    );

    state.withdraw_asset_den(
        &borrower,
        EGLD_TOKEN,
        final_supply_borrower.into_raw_units().clone(),
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

    let protocol_revenue = state.get_market_revenue(state.egld_market.clone());
    println!("protocol_revenue: {:?}", protocol_revenue);
    let reserves = state.get_market_reserves(state.egld_market.clone());
    println!("reserves: {:?}", reserves);

    let diff = reserves - protocol_revenue;
    println!("dust: {:?}", diff);
}

const SEED: u64 = 696969; // Use a fixed seed for reproducible tests

#[test]
fn simulate_many_users_random_actions() {
    let mut state = LendingPoolTestState::new();
    let mut rng = ChaCha8Rng::seed_from_u64(SEED); // Initialize RNG

    let num_actions = 1000; // Number of random actions to simulate
    let max_simulation_time_increase_seconds = SECONDS_PER_DAY * 7; // Max time jump per action

    println!("Setting up users...");
    // Create Suppliers
    let count_accounts = 1000;
    let mut borrowers = vec![];
    let mut suppliers = vec![];
    let mut borrower_names = vec![]; // Store names here
    let mut supplier_names = vec![]; // Store addresses here
    let mut all_users = vec![]; // Combined list for easy random selection
    let mut user_nonces: ManagedMapEncoded<StaticApi, TestAddress, u64> = ManagedMapEncoded::new(); // Map Address -> Nonce
    let mut nonce_counter: u64 = 0; // Start nonces from 1
    for i in 0..count_accounts {
        borrower_names.push(format!("borrower{}", i)); // Create and store the String
        supplier_names.push(format!("supplier{}", i)); // Create and store the String
    }

    let mut markets = MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN));
    // Now create TestAddress using references to the stored names
    for i in 0..count_accounts {
        let borrower = TestAddress::new(borrower_names[i].as_str());
        borrowers.push(borrower);
        let supplier = TestAddress::new(supplier_names[i].as_str());
        suppliers.push(supplier);
        all_users.push(borrower);
        all_users.push(supplier);
        setup_account(&mut state, borrower);
        setup_account(&mut state, supplier);
    }
    // --- Simulation Loop ---
    println!("Starting simulation loop ({} actions)...", num_actions);
    let mut current_timestamp = 0u64;
    state.change_timestamp(current_timestamp);

    for action_i in 0..num_actions {
        // 1. Advance Time
        let time_increase = rng.random_range(1..=max_simulation_time_increase_seconds);
        current_timestamp += time_increase;
        state.change_timestamp(current_timestamp);
        state.update_markets(&OWNER_ADDRESS, markets.clone());

        println!(
            "\n--- Action {} (Time: {}) ---",
            action_i + 1,
            current_timestamp
        );

        // Optionally update markets periodically
        if action_i % 10 == 0 {
            let mut markets = MultiValueEncoded::new();
            markets.push(EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN));
            println!("Updating markets...");
            state.update_markets(&OWNER_ADDRESS, markets);
        }

        // 2. Choose User
        let user_index = rng.random_range(0..all_users.len());
        let user_addr = all_users[user_index].clone();
        let is_borrower = borrowers.contains(&user_addr);

        // 3. Choose Action
        let action_type = rng.random_range(0..100); // Probability distribution

        // 4/5. Select Amount & Perform Action
        let max_amount = 100_000u64; // Example max op amount

        if is_borrower {
            // Borrower Actions: Supply (30%), Borrow (30%), Repay (20%), Withdraw (20%)
            if action_type < 30 {
                // Supply
                let amount = BigUint::from(rng.random_range(1_000..=max_amount));
                if user_nonces.contains(&user_addr) {
                    let nonce = user_nonces.get(&user_addr);
                    println!(
                        "User {:?} (Borrower, Nonce {}) tries Supply: {}",
                        user_addr,
                        nonce,
                        amount.to_display()
                    );
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
                // Borrow
                if user_nonces.contains(&user_addr) {
                    let nonce = user_nonces.get(&user_addr);
                    let total_borrow = state.get_total_borrow_in_egld(nonce);
                    let total_collateral = state.get_total_collateral_in_egld(nonce);
                    let total_collateral_liquidation = state.get_ltv_collateral_in_egld(nonce);
                    let available_borrow =
                        total_collateral_liquidation.clone() - total_borrow.clone();
                    if available_borrow.into_raw_units() > &BigUint::zero() {
                        println!(
                            "User {:?} (Borrower, Nonce {}) tries Borrow: {}, total_borrow: {}, total_collateral: {}, total_ltv_collateral: {}",
                            user_addr, nonce, available_borrow, total_borrow, total_collateral, total_collateral_liquidation
                        );
                        // Note: No explicit borrow limit check here for simplicity, relies on contract checks
                        state.borrow_asset(
                            &user_addr,
                            EGLD_TOKEN,
                            available_borrow.into_raw_units().clone() / BigUint::from(WAD),
                            nonce,
                            EGLD_DECIMALS,
                        );
                    } else {
                        println!(
                            "User {:?} (Borrower, Nonce {}) has no collateral to borrow",
                            user_addr, nonce
                        );
                    }
                } else {
                    println!(
                        "User {:?} (Borrower) cannot Borrow yet (no nonce)",
                        user_addr
                    );
                }
            } else if action_type < 80 {
                // Repay
                if user_nonces.contains(&user_addr) {
                    let nonce = user_nonces.get(&user_addr);
                    let current_borrow = state.get_total_borrow_in_egld(nonce);
                    if current_borrow.into_raw_units() > &BigUint::zero() {
                        // Repay random amount up to full debt (+1 to sometimes trigger full repay)
                        println!(
                            "User {:?} (Borrower, Nonce {}) tries Repay: {} (Current Debt: {})",
                            user_addr, nonce, current_borrow, current_borrow
                        );
                        state.repay_asset_deno(
                            &user_addr,
                            &EGLD_TOKEN,
                            current_borrow.into_raw_units().clone(),
                            nonce,
                        );
                    } else {
                        println!(
                            "User {:?} (Borrower, Nonce {}) has no debt to Repay",
                            user_addr, nonce
                        );
                    }
                } else {
                    println!(
                        "User {:?} (Borrower) cannot Repay yet (no nonce)",
                        user_addr
                    );
                }
            } else {
                // Withdraw
                if user_nonces.contains(&user_addr) {
                    let nonce = user_nonces.get(&user_addr);
                    let current_supply = state.get_total_collateral_in_egld(nonce);
                    let total_borrow = state.get_total_borrow_in_egld(nonce);
                    if current_supply.into_raw_units() > &BigUint::zero()
                        && total_borrow.into_raw_units() == &BigUint::zero()
                    {
                        let max_withdraw = current_supply.clone();
                        // let amount_to_withdraw = rng.random_range(1..=max_withdraw);
                        println!("User {:?} (Borrower, Nonce {}) tries Withdraw: {} (Current Supply: {})", user_addr, nonce, max_withdraw, current_supply);
                        // Note: No explicit health factor check here for simplicity
                        state.withdraw_asset_den(
                            &user_addr,
                            EGLD_TOKEN,
                            max_withdraw.into_raw_units().clone() - BigUint::from(1u64),
                            nonce,
                        );
                        user_nonces.remove(&user_addr);
                    } else {
                        println!(
                            "User {:?} (Borrower, Nonce {}) has nothing supplied to Withdraw",
                            user_addr, nonce
                        );
                    }
                } else {
                    println!(
                        "User {:?} (Borrower) cannot Withdraw yet (no nonce)",
                        user_addr
                    );
                }
            }
        } else {
            // Supplier Actions: Supply (70%), Withdraw (30%)
            if action_type < 70 {
                // Supply
                let amount = BigUint::from(rng.random_range(1_000..=max_amount));
                if user_nonces.contains(&user_addr) {
                    let nonce = user_nonces.get(&user_addr);
                    println!(
                        "User {:?} (Supplier, Nonce {}) tries Supply: {}",
                        user_addr,
                        nonce,
                        amount.to_display()
                    );
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
                // Withdraw
                if user_nonces.contains(&user_addr) {
                    let nonce = user_nonces.get(&user_addr);
                    let current_supply = state.get_total_collateral_in_egld(nonce);
                    if current_supply.into_raw_units() > &BigUint::zero() {
                        // Withdraw random amount up to full supply (+1)
                        println!("User {:?} (Supplier, Nonce {}) tries Withdraw: {} (Current Supply: {})", user_addr, nonce, current_supply, current_supply);
                        state.withdraw_asset_den(
                            &user_addr,
                            EGLD_TOKEN,
                            current_supply.into_raw_units().clone(),
                            nonce,
                        );
                        user_nonces.remove(&user_addr);
                    } else {
                        println!(
                            "User {:?} (Supplier, Nonce {}) has nothing supplied to Withdraw",
                            user_addr, nonce
                        );
                    }
                } else {
                    println!(
                        "User {:?} (Supplier) cannot Withdraw yet (no nonce)",
                        user_addr
                    );
                }
            }
        }
        // Small delay for readability if needed
        // std::thread::sleep(std::time::Duration::from_millis(10));
    }

    // --- Final Settlement ---
    println!("\n--- Final Settlement ---");
    let final_timestamp = current_timestamp + SECONDS_PER_DAY; // Advance time one more day
    state.change_timestamp(final_timestamp);
    state.update_markets(&OWNER_ADDRESS, markets.clone());
    println!("Final market update at time {}", final_timestamp);

    for user_addr in &all_users {
        if user_nonces.contains(&user_addr) {
            let nonce = user_nonces.get(&user_addr);
            println!("Settling user {:?} (Nonce {})", user_addr, nonce);

            // Repay all debt if borrower
            if borrowers.contains(user_addr) {
                let final_borrow = state.get_total_borrow_in_egld(nonce);
                if final_borrow.into_raw_units() > &BigUint::zero() {
                    println!("Repaying final debt: {}", final_borrow);
                    state.repay_asset_deno(
                        user_addr,
                        &EGLD_TOKEN,
                        final_borrow.into_raw_units().clone(),
                        nonce,
                    ); // Repay full amount + 1 wei
                }
            }

            // Withdraw all supply
            let final_supply = state.get_total_collateral_in_egld(nonce);
            if final_supply.into_raw_units() > &BigUint::zero() {
                println!("Withdrawing final supply: {}", final_supply);
                // Important: This might panic if HF doesn't allow withdrawal after repay,
                // which would indicate a potential issue elsewhere or require more complex handling.
                state.withdraw_asset_den(
                    user_addr,
                    EGLD_TOKEN,
                    final_supply.into_raw_units().clone(),
                    nonce,
                ); // Withdraw full amount + 1 wei

                user_nonces.remove(&user_addr);
            }
        }
    }

    // --- Final Checks ---
    println!("\n--- Final Checks ---");
    let protocol_revenue = state.get_market_revenue(state.egld_market.clone());
    println!("Final Protocol Revenue: {:?}", protocol_revenue);
    let reserves = state.get_market_reserves(state.egld_market.clone());
    println!("Final Reserves: {:?}", reserves);

    // Allow for tiny dust difference
    let dust_threshold = ManagedDecimal::from_raw_units(BigUint::from(1000u64), WAD_PRECISION); // Allow up to 10 wei difference
    let revenue_raw = protocol_revenue;
    let reserves_raw = reserves;

    if revenue_raw > reserves_raw {
        let diff = revenue_raw - reserves_raw;
        println!("Dust (Revenue > Reserves): {:?}", diff);
        assert!(diff <= dust_threshold);
    } else {
        let diff = reserves_raw - revenue_raw;
        println!("Dust (Reserves >= Revenue): {:?}", diff);
        assert!(diff <= dust_threshold);
    }

    //  println!("Final Total Supplied: {:?}", total_supplied);
    //  println!("Final Total Borrowed: {:?}", total_borrowed);
    //  assert_eq!(total_supplied.into_raw_units(), BigUint::zero());
    //  assert_eq!(total_borrowed.into_raw_units(), BigUint::zero());
}
