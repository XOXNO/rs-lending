#![no_std]

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

pub const BP: u64 = 1_000_000_000; // Represents 100%
pub const MAX_THRESHOLD: u64 = BP / 2;

#[derive(TopEncode, TopDecode, TypeAbi)]
pub struct PoolParams<M: ManagedTypeApi> {
    pub r_max: BigUint<M>,
    pub r_base: BigUint<M>,
    pub r_slope1: BigUint<M>,
    pub r_slope2: BigUint<M>,
    pub u_optimal: BigUint<M>,
    pub reserve_factor: BigUint<M>,
}

#[derive(NestedEncode, NestedDecode, TopEncode, TopDecode, TypeAbi, Clone)]
pub enum AccountPositionType {
    None,
    Deposit,
    Borrow,
}

#[derive(NestedEncode, NestedDecode, TopEncode, TopDecode, TypeAbi, Clone)]
pub struct AccountPosition<M: ManagedTypeApi> {
    pub deposit_type: AccountPositionType,
    pub account_nonce: u64,
    pub token_id: TokenIdentifier<M>,
    pub amount: BigUint<M>,
    pub round: u64,
    pub initial_index: BigUint<M>,
}

impl<M: ManagedTypeApi> AccountPosition<M> {
    pub fn new(
        deposit_type: AccountPositionType,
        token_id: TokenIdentifier<M>,
        amount: BigUint<M>,
        account_nonce: u64,
        round: u64,
        initial_index: BigUint<M>,
    ) -> Self {
        AccountPosition {
            deposit_type,
            token_id,
            amount,
            account_nonce,
            round,
            initial_index,
        }
    }
}
