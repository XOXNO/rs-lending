multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use crate::structs::{OracleStatus, TimestampedPrice, TokenPair};

#[multiversx_sc::module]
pub trait StorageModule {
    #[storage_mapper("pair_decimals")]
    fn pair_decimals(
        &self,
        from: &ManagedBuffer,
        to: &ManagedBuffer,
    ) -> SingleValueMapper<Option<u8>>;

    #[view]
    #[storage_mapper("submission_count")]
    fn submission_count(&self) -> SingleValueMapper<usize>;

    #[storage_mapper("oracle_status")]
    fn oracle_status(&self) -> MapMapper<ManagedAddress, OracleStatus>;

    #[storage_mapper("rounds")]
    fn rounds(
        &self,
        from: &ManagedBuffer,
        to: &ManagedBuffer,
    ) -> VecMapper<TimestampedPrice<Self::Api>>;

    #[storage_mapper("first_submission_timestamp")]
    fn first_submission_timestamp(
        &self,
        token_pair: &TokenPair<Self::Api>,
    ) -> SingleValueMapper<u64>;

    #[storage_mapper("last_submission_timestamp")]
    fn last_submission_timestamp(
        &self,
        token_pair: &TokenPair<Self::Api>,
    ) -> SingleValueMapper<u64>;

    #[storage_mapper("submissions")]
    fn submissions(
        &self,
    ) -> MapStorageMapper<TokenPair<Self::Api>, MapMapper<ManagedAddress, BigUint>>;
}
