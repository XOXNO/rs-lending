use multiversx_sc::derive_imports::*;
use multiversx_sc::imports::*;

#[type_abi]
#[derive(NestedEncode, NestedDecode, TopEncode, TopDecode, Clone)]
pub struct TokenPair<M: ManagedTypeApi> {
    pub from: ManagedBuffer<M>,
    pub to: ManagedBuffer<M>,
}

#[type_abi]
#[derive(NestedEncode, NestedDecode, TopEncode, TopDecode, Clone)]
pub struct PriceFeed<M: ManagedTypeApi> {
    pub round_id: u32,
    pub from: ManagedBuffer<M>,
    pub to: ManagedBuffer<M>,
    pub timestamp: u64,
    pub price: BigUint<M>,
    pub asset_decimals: u8,
}

#[type_abi]
#[derive(TopEncode, TopDecode, Debug, PartialEq, Eq)]
pub struct TimestampedPrice<M: ManagedTypeApi> {
    pub price: BigUint<M>,
    pub timestamp: u64,
    pub asset_decimals: u8,
    pub round: u32,
}

#[type_abi]
#[derive(NestedEncode, NestedDecode, TopEncode, TopDecode, Debug, PartialEq, Eq)]
pub struct OracleStatus {
    pub accepted_submissions: u64,
    pub total_submissions: u64,
}

#[type_abi]
#[derive(TopEncode)]
pub struct NewRoundEvent<M: ManagedTypeApi> {
    pub price: BigUint<M>,
    pub timestamp: u64,
    pub asset_decimals: u8,
    pub block: u64,
    pub epoch: u64,
}

#[type_abi]
#[derive(TopEncode)]
pub struct DiscardSubmissionEvent {
    pub submission_timestamp: u64,
    pub first_submission_timestamp: u64,
    pub has_caller_already_submitted: bool,
}
