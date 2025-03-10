use controller::{ERROR_POSITION_SHOULD_BE_VAULT, ERROR_SUPPLY_CAP, RAY_PRECISION, WAD_PRECISION};
use multiversx_sc::types::{
    ConstDecimals, EgldOrEsdtTokenIdentifier, ManagedDecimal, MultiValueEncoded,
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

// Vault Position Tests
#[test]
fn test_basic_vault_supply_and_borrow() {
    let mut state = LendingPoolTestState::new();
    let vault = TestAddress::new("vault");
    let user = TestAddress::new("user");

    // Setup accounts
    setup_accounts(&mut state, vault, user);

    // Test vault supply
    state.supply_asset(
        &vault,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        true, // is_vault = true
    );

    // Verify vault position
    let vault_supplied = state.get_vault_supplied_amount(EGLD_TOKEN);
    assert_eq!(
        vault_supplied,
        ManagedDecimal::<StaticApi, ConstDecimals<EGLD_DECIMALS>>::from(BigUint::from(100u64))
    );

    // Test normal user supply and borrow against vault liquidity
    state.supply_asset(
        &user,
        USDC_TOKEN,
        BigUint::from(5000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.borrow_asset(&vault, USDC_TOKEN, BigUint::from(50u64), 1, USDC_DECIMALS);

    // Verify amounts
    let borrowed = state.get_borrow_amount_for_token(1, USDC_TOKEN);
    let collateral = state.get_collateral_amount_for_token(1, EGLD_TOKEN);

    assert!(borrowed > ManagedDecimal::from_raw_units(BigUint::zero(), USDC_DECIMALS));
    assert!(collateral > ManagedDecimal::from_raw_units(BigUint::zero(), EGLD_DECIMALS));
}

#[test]
fn test_vault_supply_and_withdraw() {
    let mut state = LendingPoolTestState::new();
    let vault = TestAddress::new("vault");
    let user = TestAddress::new("user");

    // Setup accounts
    setup_accounts(&mut state, vault, user);

    // Supply as vault
    state.supply_asset(
        &vault,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        true,
    );

    let vault_amount = state.get_vault_supplied_amount(EGLD_TOKEN);
    let expected_value =
        ManagedDecimal::<StaticApi, ConstDecimals<EGLD_DECIMALS>>::from(BigUint::from(100u64));
    assert_eq!(vault_amount, expected_value);

    state
        .world
        .check_account(state.lending_sc.clone())
        .esdt_balance(EGLD_TOKEN, expected_value.into_raw_units());

    // Withdraw half
    state.withdraw_asset(&vault, EGLD_TOKEN, BigUint::from(50u64), 1, EGLD_DECIMALS);

    let after_withdraw_supplied = state.get_vault_supplied_amount(EGLD_TOKEN);
    let expected_value =
        ManagedDecimal::<StaticApi, ConstDecimals<EGLD_DECIMALS>>::from(BigUint::from(50u64));
    assert_eq!(after_withdraw_supplied, expected_value);

    state
        .world
        .check_account(state.lending_sc.clone())
        .esdt_balance(EGLD_TOKEN, expected_value.into_raw_units());
}

#[test]
fn test_vault_supply_cap() {
    let mut state = LendingPoolTestState::new();
    let vault = TestAddress::new("vault");
    let user = TestAddress::new("user");

    // Setup accounts
    setup_accounts(&mut state, vault, user);

    // Supply as vault up to cap
    state.supply_asset(
        &vault,
        CAPPED_TOKEN,
        BigUint::from(1u64),
        CAPPED_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        true,
    );

    state.supply_asset(
        &vault,
        CAPPED_TOKEN,
        BigUint::from(1u64),
        CAPPED_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Try to supply more than cap
    state.supply_asset_error(
        &vault,
        CAPPED_TOKEN,
        BigUint::from(149u64),
        CAPPED_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::None,
        true,
        ERROR_SUPPLY_CAP,
    );
}

#[test]
fn test_vault_liquidation() {
    let mut state = LendingPoolTestState::new();
    let vault = TestAddress::new("vault");
    let user = TestAddress::new("user");
    let liquidator = TestAddress::new("liquidator");

    // Setup accounts
    setup_accounts(&mut state, vault, user);
    state.world.account(liquidator).nonce(1).esdt_balance(
        USDC_TOKEN,
        BigUint::from(20000u64) * BigUint::from(10u64).pow(USDC_DECIMALS as u32),
    );

    // Supply as vault
    state.supply_asset(
        &vault,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        true,
    );

    // User supplies collateral and borrows
    state.supply_asset(
        &user,
        USDC_TOKEN,
        BigUint::from(4000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.borrow_asset(&vault, USDC_TOKEN, BigUint::from(2000u64), 1, USDC_DECIMALS);

    // Advance time and update interest
    state.world.current_block().block_timestamp(535000000u64);
    state.update_borrows_with_debt(&vault, 1);
    let health = state.get_account_health_factor(1);
    println!("health: {}", health);
    // Attempt liquidation
    state.liquidate_account(
        &liquidator,
        &USDC_TOKEN,
        BigUint::from(2000u64),
        1,
        USDC_DECIMALS,
    );

    let health = state.get_account_health_factor(1);
    println!("health: {}", health);

    // Verify vault supplied amount was reduced
    let vault_supplied = state.get_vault_supplied_amount(EGLD_TOKEN);
    let debt = state.get_total_borrow_in_egld(1);
    println!("vault_supplied: {:?}", vault_supplied);
    println!("debt: {:?}", debt);
    assert!(
        vault_supplied
            < ManagedDecimal::<StaticApi, ConstDecimals<EGLD_DECIMALS>>::from(BigUint::from(
                100u64
            ))
    );
}

#[test]
fn test_mixed_vault_and_normal_supply() {
    let mut state = LendingPoolTestState::new();
    let vault = TestAddress::new("vault");
    let user = TestAddress::new("user");

    // Setup accounts
    setup_accounts(&mut state, vault, user);

    // Supply as vault
    state.supply_asset(
        &vault,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        true,
    );

    // Supply as normal user
    state.supply_asset(
        &user,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Verify amounts
    let vault_supplied = state.get_vault_supplied_amount(EGLD_TOKEN);
    let total_supplied = state.get_collateral_amount_for_token(2, EGLD_TOKEN);

    assert_eq!(
        vault_supplied,
        ManagedDecimal::<StaticApi, ConstDecimals<EGLD_DECIMALS>>::from(BigUint::from(100u64))
    );
    assert_eq!(
        total_supplied,
        ManagedDecimal::<StaticApi, ConstDecimals<EGLD_DECIMALS>>::from(BigUint::from(100u64))
    );
    // Verify utilization rate includes both supplies
    let utilization = state.get_market_utilization(state.egld_market.clone());
    println!("Market utilization with mixed supplies: {:?}", utilization);
    assert_eq!(
        utilization,
        ManagedDecimal::from_raw_units(BigUint::zero(), WAD_PRECISION)
    );

    state.supply_asset(
        &user,
        USDC_TOKEN,
        BigUint::from(6000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );
    state.borrow_asset(&user, EGLD_TOKEN, BigUint::from(100u64), 3, EGLD_DECIMALS);
    let utilization = state.get_market_utilization(state.egld_market.clone());
    println!("Market utilization with mixed supplies: {:?}", utilization);
    assert_eq!(
        utilization.into_raw_units(),
        &(BigUint::from(1u64) * BigUint::from(10u64).pow(RAY_PRECISION as u32))
    );
}

#[test]
fn test_vault_multiple_positions() {
    let mut state = LendingPoolTestState::new();
    let vault = TestAddress::new("vault");
    let user = TestAddress::new("user");

    // Setup accounts
    setup_accounts(&mut state, vault, user);

    // Supply first position as vault
    state.supply_asset(
        &vault,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        true,
    );

    // Supply second position as vault
    state.supply_asset(
        &vault,
        USDC_TOKEN,
        BigUint::from(5000u64),
        USDC_DECIMALS,
        OptionalValue::Some(1),
        OptionalValue::None,
        true,
    );

    // Verify both positions
    let egld_supplied = state.get_vault_supplied_amount(EGLD_TOKEN);
    let usdc_supplied = state.get_vault_supplied_amount(USDC_TOKEN);

    assert_eq!(
        egld_supplied,
        ManagedDecimal::<StaticApi, ConstDecimals<EGLD_DECIMALS>>::from(BigUint::from(100u64))
    );
    assert_eq!(
        usdc_supplied,
        ManagedDecimal::<StaticApi, ConstDecimals<USDC_DECIMALS>>::from(BigUint::from(5000u64))
    );
}

#[test]
fn test_enable_vault_no_interest_no_borrows() {
    let mut state = LendingPoolTestState::new();
    let vault = TestAddress::new("vault");

    let user: TestAddress<'_> = TestAddress::new("user");
    // Setup accounts
    setup_accounts(&mut state, vault, user);

    // Supply first position as vault
    state.supply_asset(
        &vault,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        true,
    );
    let egld_supplied = state.get_vault_supplied_amount(EGLD_TOKEN);
    assert_eq!(
        egld_supplied,
        ManagedDecimal::<StaticApi, ConstDecimals<EGLD_DECIMALS>>::from(BigUint::from(100u64))
    );
    state.disable_vault(&vault, 1);
    let egld_supplied = state.get_vault_supplied_amount(EGLD_TOKEN);
    assert_eq!(
        egld_supplied,
        ManagedDecimal::from_raw_units(BigUint::zero(), EGLD_DECIMALS)
    );
}

#[test]
fn test_enable_disable_vault_with_borrows_and_interest() {
    let mut state = LendingPoolTestState::new();
    let vault = TestAddress::new("vault");

    let user: TestAddress<'_> = TestAddress::new("user");
    // Setup accounts
    setup_accounts(&mut state, vault, user);

    // Supply first position as vault
    state.supply_asset(
        &vault,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        true,
    );

    state.supply_asset(
        &user,
        USDC_TOKEN,
        BigUint::from(5000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.supply_asset(
        &user,
        EGLD_TOKEN,
        BigUint::from(50u64),
        EGLD_DECIMALS,
        OptionalValue::Some(2),
        OptionalValue::None,
        false,
    );

    state.borrow_asset(&user, EGLD_TOKEN, BigUint::from(10u64), 2, EGLD_DECIMALS);
    state.borrow_asset(&vault, USDC_TOKEN, BigUint::from(1000u64), 1, USDC_DECIMALS);
    let egld_supplied = state.get_vault_supplied_amount(EGLD_TOKEN);
    assert_eq!(
        egld_supplied,
        ManagedDecimal::<StaticApi, ConstDecimals<EGLD_DECIMALS>>::from(BigUint::from(100u64))
    );

    state.disable_vault(&vault, 1);
    let egld_supplied = state.get_vault_supplied_amount(EGLD_TOKEN);
    assert_eq!(
        egld_supplied,
        ManagedDecimal::from_raw_units(BigUint::zero(), EGLD_DECIMALS)
    );
    state.world.current_block().block_timestamp(535000u64);
    state.enable_vault(&vault, 1);

    let egld_supplied = state.get_vault_supplied_amount(EGLD_TOKEN);
    let mut markets = MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN));
    state.update_markets(&vault, markets);
    state.update_account_positions(&vault, 1);
    assert!(
        egld_supplied
            > ManagedDecimal::<StaticApi, ConstDecimals<EGLD_DECIMALS>>::from(BigUint::from(
                100u64
            )),
    );
}

#[test]
fn test_disable_enable_vault_with_borrows_and_interest() {
    let mut state = LendingPoolTestState::new();
    let vault = TestAddress::new("vault");

    let user: TestAddress<'_> = TestAddress::new("user");
    // Setup accounts
    setup_accounts(&mut state, vault, user);

    // Supply first position as vault
    state.supply_asset(
        &vault,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.supply_asset(
        &user,
        USDC_TOKEN,
        BigUint::from(5000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.supply_asset(
        &user,
        EGLD_TOKEN,
        BigUint::from(50u64),
        EGLD_DECIMALS,
        OptionalValue::Some(2),
        OptionalValue::None,
        false,
    );

    state.borrow_asset(&user, EGLD_TOKEN, BigUint::from(10u64), 2, EGLD_DECIMALS);
    state.borrow_asset(&vault, USDC_TOKEN, BigUint::from(1000u64), 1, USDC_DECIMALS);
    state.world.current_block().block_timestamp(530000u64);
    let egld_supplied = state.get_vault_supplied_amount(EGLD_TOKEN);
    assert!(
        egld_supplied
            == ManagedDecimal::<StaticApi, ConstDecimals<EGLD_DECIMALS>>::from(BigUint::from(0u64))
    );
    let collateral = state.get_collateral_amount_for_token(1, EGLD_TOKEN);
    assert!(
        collateral
            >= ManagedDecimal::<StaticApi, ConstDecimals<EGLD_DECIMALS>>::from(BigUint::from(
                100u64
            ))
    );
    state.enable_vault(&vault, 1);
    let egld_supplied = state.get_vault_supplied_amount(EGLD_TOKEN);
    assert!(
        egld_supplied
            > ManagedDecimal::<StaticApi, ConstDecimals<EGLD_DECIMALS>>::from(BigUint::from(
                100u64
            )),
    );
    state.world.current_block().block_timestamp(535000u64);
    state.disable_vault(&vault, 1);

    let egld_supplied = state.get_vault_supplied_amount(EGLD_TOKEN);
    let mut markets = MultiValueEncoded::new();
    markets.push(EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN));
    state.update_markets(&vault, markets);
    state.update_account_positions(&vault, 1);
    assert!(
        egld_supplied
            == ManagedDecimal::<StaticApi, ConstDecimals<EGLD_DECIMALS>>::from(BigUint::from(0u64))
    );
}

#[test]
fn test_vault_supply_with_normal_position_error() {
    let mut state = LendingPoolTestState::new();
    let vault = TestAddress::new("vault");
    let user = TestAddress::new("user");

    // Setup accounts
    setup_accounts(&mut state, vault, user);

    // First create normal position
    state.supply_asset(
        &vault,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false, // normal position
    );

    // Try to supply as vault with same NFT
    state.supply_asset_error(
        &vault,
        USDC_TOKEN,
        BigUint::from(5000u64),
        USDC_DECIMALS,
        OptionalValue::Some(1), // same NFT
        OptionalValue::None,
        true, // try as vault
        ERROR_POSITION_SHOULD_BE_VAULT,
    );
}
