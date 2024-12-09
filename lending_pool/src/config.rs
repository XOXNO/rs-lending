multiversx_sc::imports!();

pub use common_tokens::*;
#[multiversx_sc::module]
pub trait ConfigModule: common_tokens::AccountTokenModule {
    #[only_owner]
    #[payable("EGLD")]
    #[endpoint(registerAccountToken)]
    fn register_account_token(&self, token_name: ManagedBuffer, ticker: ManagedBuffer) {
        let payment_amount = self.call_value().egld_value();
        self.account_token().issue_and_set_all_roles(
            EsdtTokenType::NonFungible,
            payment_amount.clone_value(),
            token_name,
            ticker,
            1,
            None,
        );
    }
}
