multiversx_sc::imports!();

use common_structs::{BorrowPosition, DepositPosition};

#[multiversx_sc::module]
pub trait LendingStorageModule {
    #[view(getDepositPositions)]
    #[storage_mapper("deposit_positions")]
    fn deposit_positions(
        &self,
        owner_nonce: u64,
    ) -> MapMapper<TokenIdentifier, DepositPosition<Self::Api>>;

    #[view(getBorrowPositions)]
    #[storage_mapper("borrow_positions")]
    fn borrow_positions(
        &self,
        owner_nonce: u64,
    ) -> MapMapper<TokenIdentifier, BorrowPosition<Self::Api>>;

    #[storage_mapper("pools_map")]
    fn pools_map(&self, token_id: &TokenIdentifier) -> SingleValueMapper<ManagedAddress>;

    #[view(getPoolAllowed)]
    #[storage_mapper("pool_allowed")]
    fn pools_allowed(&self) -> UnorderedSetMapper<ManagedAddress>;

    #[view(getAssetLoanToValue)]
    #[storage_mapper("asset_loan_to_value")]
    fn asset_loan_to_value(&self, asset: &TokenIdentifier) -> SingleValueMapper<BigUint>;

    #[view(getAssetLiquidationBonus)]
    #[storage_mapper("asset_liquidation_bonus")]
    fn asset_liquidation_bonus(&self, asset: &TokenIdentifier) -> SingleValueMapper<BigUint>;
}
