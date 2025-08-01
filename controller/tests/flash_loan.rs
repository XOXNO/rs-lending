use common_errors::*;

use multiversx_sc::types::{ManagedArgBuffer, ManagedBuffer};
use multiversx_sc_scenario::imports::{BigUint, OptionalValue, TestAddress};
pub mod constants;
pub mod proxys;
pub mod setup;
use constants::*;
use setup::*;
use std::ops::Mul;

/// Tests successful flash loan execution with full repayment.
///
/// Covers:
/// - Controller::flashLoan endpoint functionality
/// - Flash loan callback execution
/// - Full repayment verification
/// - Pool balance restoration after flash loan
#[test]
fn flash_loan_full_repayment_success() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");
    setup_accounts(&mut state, supplier, borrower);

    // Supply liquidity to enable flash loan
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Execute flash loan with successful repayment
    state.flash_loan(
        &OWNER_ADDRESS,
        &EGLD_TOKEN,
        BigUint::from(100u64).mul(BigUint::from(10u64).pow(EGLD_DECIMALS as u32)),
        state.flash_mock.clone(),
        ManagedBuffer::from("flash"), // Endpoint that repays correctly
        ManagedArgBuffer::new(),
    );
}

/// Tests flash loan failure when borrower doesn't repay.
///
/// Covers:
/// - Controller::flashLoan error handling
/// - Repayment validation after callback
/// - ERROR_INVALID_FLASHLOAN_REPAYMENT error condition
/// - Transaction rollback on failed repayment
#[test]
fn flash_loan_no_repayment_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");
    setup_accounts(&mut state, supplier, borrower);

    // Supply liquidity to enable flash loan
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Attempt flash loan without repayment
    state.flash_loan_error(
        &OWNER_ADDRESS,
        &EGLD_TOKEN,
        BigUint::from(100u64).mul(BigUint::from(10u64).pow(EGLD_DECIMALS as u32)),
        state.flash_mock.clone(),
        ManagedBuffer::from("flashNoRepay"), // Endpoint that doesn't repay
        ManagedArgBuffer::new(),
        ERROR_INVALID_FLASHLOAN_REPAYMENT,
    );
}

/// Tests flash loan validation for zero amount.
///
/// Covers:
/// - Controller::flashLoan input validation
/// - Zero amount check
/// - ERROR_AMOUNT_MUST_BE_GREATER_THAN_ZERO error condition
#[test]
fn flash_loan_zero_amount_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");
    setup_accounts(&mut state, supplier, borrower);

    // Attempt flash loan with zero amount
    state.flash_loan_error(
        &OWNER_ADDRESS,
        &EGLD_TOKEN,
        BigUint::from(0u64),
        state.flash_mock.clone(),
        ManagedBuffer::from("flashNoRepay"),
        ManagedArgBuffer::new(),
        ERROR_AMOUNT_MUST_BE_GREATER_THAN_ZERO,
    );
}

/// Tests flash loan failure with partial repayment.
///
/// Covers:
/// - Controller::flashLoan partial repayment validation
/// - Exact repayment requirement
/// - ERROR_INVALID_FLASHLOAN_REPAYMENT for insufficient repayment
#[test]
fn flash_loan_partial_repayment_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");
    setup_accounts(&mut state, supplier, borrower);

    // Supply liquidity to enable flash loan
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    // Attempt flash loan with partial repayment
    state.flash_loan_error(
        &OWNER_ADDRESS,
        &EGLD_TOKEN,
        BigUint::from(100u64).mul(BigUint::from(10u64).pow(EGLD_DECIMALS as u32)),
        state.flash_mock.clone(),
        ManagedBuffer::from("flashRepaySome"), // Endpoint that repays partially
        ManagedArgBuffer::new(),
        ERROR_INVALID_FLASHLOAN_REPAYMENT,
    );
}

/// Tests flash loan failure with partial repayment.
///
/// Covers:
/// - Controller::flashLoan partial repayment validation
/// - Exact repayment requirement
/// - ERROR_INVALID_FLASHLOAN_REPAYMENT for insufficient repayment
#[test]
fn flash_loan_partial_repayment_wrong_token_error() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");
    setup_accounts(&mut state, supplier, borrower);

    // Supply liquidity to enable flash loan
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    let mut args = ManagedArgBuffer::new();
    args.push_arg(USDC_TOKEN);

    // Attempt flash loan with partial repayment
    state.flash_loan_error(
        &OWNER_ADDRESS,
        &EGLD_TOKEN,
        BigUint::from(100u64).mul(BigUint::from(10u64).pow(EGLD_DECIMALS as u32)),
        state.flash_mock.clone(),
        ManagedBuffer::from("flashRepaySomeWrongToken"), // Endpoint that repays partially
        args,
        ERROR_INVALID_FLASHLOAN_REPAYMENT,
    );
}

/// Tests flash loan validation for empty endpoint name.
///
/// Covers:
/// - Controller::flashLoan endpoint validation
/// - Empty endpoint rejection
/// - ERROR_INVALID_ENDPOINT error condition
#[test]
fn flash_loan_empty_endpoint_error() {
    let mut state = LendingPoolTestState::new();

    // Attempt flash loan with empty endpoint name
    state.flash_loan_error(
        &OWNER_ADDRESS,
        &EGLD_TOKEN,
        BigUint::from(100u64).mul(BigUint::from(10u64).pow(EGLD_DECIMALS as u32)),
        state.flash_mock.clone(),
        ManagedBuffer::new(), // Empty endpoint
        ManagedArgBuffer::new(),
        ERROR_INVALID_ENDPOINT,
    );
}

/// Tests flash loan protection against built-in function calls.
///
/// Covers:
/// - Controller::flashLoan security validation
/// - Built-in function blocking
/// - ERROR_INVALID_ENDPOINT for restricted endpoints
/// - Protection against system function abuse
#[test]
fn flash_loan_builtin_functions_blocked_error() {
    let mut state = LendingPoolTestState::new();

    // List of built-in functions that should be blocked
    let restricted_endpoints = [
        "ChangeOwnerAddress",
        "SetUserName",
        "ESDTTransfer",
        "ESDTLocalBurn",
        "ESDTLocalMint",
        "ESDTNFTTransfer",
        "ESDTNFTCreate",
        "ESDTNFTAddQuantity",
        "ESDTNFTBurn",
        "ESDTNFTAddURI",
        "ESDTNFTUpdateAttributes",
        "MultiESDTNFTTransfer",
    ];

    // Verify each restricted endpoint is blocked
    for endpoint in restricted_endpoints.iter() {
        state.flash_loan_error(
            &OWNER_ADDRESS,
            &EGLD_TOKEN,
            BigUint::from(100u64).mul(BigUint::from(10u64).pow(EGLD_DECIMALS as u32)),
            state.flash_mock.clone(),
            ManagedBuffer::from(*endpoint),
            ManagedArgBuffer::new(),
            ERROR_INVALID_ENDPOINT,
        );
    }
}
