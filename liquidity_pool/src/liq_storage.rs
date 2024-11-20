multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use common_structs::PoolParams;

#[multiversx_sc::module]
pub trait StorageModule {
    #[view(getPoolAsset)]
    #[storage_mapper("pool_asset")]
    fn pool_asset(&self) -> SingleValueMapper<TokenIdentifier>;

    #[view(getReserves)]
    #[storage_mapper("reserves")]
    fn reserves(&self) -> SingleValueMapper<BigUint>;

    #[view(getSuppliedAmount)]
    #[storage_mapper("supplied_amount")]
    fn supplied_amount(&self) -> SingleValueMapper<BigUint>;

    #[view(getRewardsReserves)]
    #[storage_mapper("rewards_reserves")]
    fn rewards_reserves(&self) -> SingleValueMapper<BigUint>;

    #[view(getTotalBorrow)]
    #[storage_mapper("borrowed_amount")]
    fn borrowed_amount(&self) -> SingleValueMapper<BigUint>;

    #[view(getPoolParams)]
    #[storage_mapper("pool_params")]
    fn pool_params(&self) -> SingleValueMapper<PoolParams<Self::Api>>;

    #[view(getBorrowIndex)]
    #[storage_mapper("borrow_index")]
    fn borrow_index(&self) -> SingleValueMapper<BigUint>;

    #[view(getSupplyIndex)]
    #[storage_mapper("supply_index")]
    fn supply_index(&self) -> SingleValueMapper<BigUint>;

    #[view(borrowIndexLastUpdateRound)]
    #[storage_mapper("borrow_index_last_update_round")]
    fn borrow_index_last_update_round(&self) -> SingleValueMapper<u64>;
}
