use common_errors::*;

use multiversx_sc::types::{ManagedArgBuffer, ManagedBuffer};
use multiversx_sc_scenario::imports::{BigUint, OptionalValue, TestAddress};
pub mod constants;
pub mod proxys;
pub mod setup;
use constants::*;
use setup::*;
use std::ops::Mul;

#[test]
fn flash_loan_success_repayment() {
    let mut state = LendingPoolTestState::new();
    let supplier: TestAddress<'_> = TestAddress::new("supplier");
    let borrower: TestAddress<'_> = TestAddress::new("borrower");
    setup_accounts(&mut state, supplier, borrower);
    // Supply first position as ch
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.flash_loan(
        &OWNER_ADDRESS,
        &EGLD_TOKEN,
        BigUint::from(100u64).mul(BigUint::from(10u64).pow(EGLD_DECIMALS as u32)),
        state.flash_mock.clone(),
        ManagedBuffer::from("flash"),
        ManagedArgBuffer::new(),
    );
}

#[test]
fn flash_loan_no_repayment() {
    let mut state = LendingPoolTestState::new();
    let supplier: TestAddress<'_> = TestAddress::new("supplier");
    let borrower: TestAddress<'_> = TestAddress::new("borrower");
    setup_accounts(&mut state, supplier, borrower);
    // Supply first position as vault
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.flash_loan_error(
        &OWNER_ADDRESS,
        &EGLD_TOKEN,
        BigUint::from(100u64).mul(BigUint::from(10u64).pow(EGLD_DECIMALS as u32)),
        state.flash_mock.clone(),
        ManagedBuffer::from("flashNoRepay"),
        ManagedArgBuffer::new(),
        ERROR_INVALID_FLASHLOAN_REPAYMENT,
    );
}

#[test]
fn flash_loan_no_amount() {
    let mut state = LendingPoolTestState::new();
    let supplier: TestAddress<'_> = TestAddress::new("supplier");
    let borrower: TestAddress<'_> = TestAddress::new("borrower");
    setup_accounts(&mut state, supplier, borrower);

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

#[test]
fn flash_loan_repayment_some() {
    let mut state = LendingPoolTestState::new();
    let supplier: TestAddress<'_> = TestAddress::new("supplier");
    let borrower: TestAddress<'_> = TestAddress::new("borrower");
    setup_accounts(&mut state, supplier, borrower);
    // Supply first position as vault
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
        false,
    );

    state.flash_loan_error(
        &OWNER_ADDRESS,
        &EGLD_TOKEN,
        BigUint::from(100u64).mul(BigUint::from(10u64).pow(EGLD_DECIMALS as u32)),
        state.flash_mock.clone(),
        ManagedBuffer::from("flashRepaySome"),
        ManagedArgBuffer::new(),
        ERROR_INVALID_FLASHLOAN_REPAYMENT,
    );
}

#[test]
fn flash_loan_invalid_endpoint_empty() {
    let mut state = LendingPoolTestState::new();

    state.flash_loan_error(
        &OWNER_ADDRESS,
        &EGLD_TOKEN,
        BigUint::from(100u64).mul(BigUint::from(10u64).pow(EGLD_DECIMALS as u32)),
        state.flash_mock.clone(),
        ManagedBuffer::new(),
        ManagedArgBuffer::new(),
        ERROR_INVALID_ENDPOINT,
    );
}

#[test]
fn flash_loan_build_in_functions_throw() {
    let mut state = LendingPoolTestState::new();
    let endpoints = [
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
        // "SaveKeyValue",
        // "ESDTBurn",
        // "ESDTFreeze",
        // "ESDTUnFreeze",
        // "ESDTWipe",
        // "ESDTPause",
        // "ESDTUnPause",
        // "ESDTSetRole",
        // "ESDTUnSetRole",
        // "ESDTSetLimitedTransfer",
        // "ESDTUnSetLimitedTransfer",
        // "SetGuardian",
        // "GuardAccount",
        // "UnGuardAccount",
    ];

    for endpoint in endpoints.iter() {
        println!("endpoint: {:?}", endpoint);
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
