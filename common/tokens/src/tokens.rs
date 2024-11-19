#![no_std]

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

#[multiversx_sc::module]
pub trait AccountTokenModule {
    #[view(getAccountToken)]
    #[storage_mapper("account_token")]
    fn account_token(&self) -> NonFungibleTokenMapper<Self::Api>;

    #[view(getAccountPositions)]
    #[storage_mapper("account_positions")]
    fn account_positions(&self) -> UnorderedSetMapper<u64>;

    fn lending_account_in_the_market(&self, nonce: u64) {
        require!(
            self.account_positions().contains(&nonce),
            "Account not in Lending Protocol!"
        );
    }

    fn lending_account_token_valid(&self, account_token_id: TokenIdentifier) {
        require!(
            account_token_id == self.account_token().get_token_id(),
            "Account token not valid!"
        );
    }
}
