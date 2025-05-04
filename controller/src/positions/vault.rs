multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use crate::{helpers, oracle, storage, utils, validation};

use common_structs::AccountAttributes;

use super::{account, update};

#[multiversx_sc::module]
pub trait PositionVaultModule:
    storage::Storage
    + validation::ValidationModule
    + oracle::OracleModule
    + common_events::EventsModule
    + utils::LendingUtilsModule
    + helpers::math::MathsModule
    + common_math::SharedMathModule
    + account::PositionAccountModule
    + update::PositionUpdateModule
{
    /// Updates account attributes in storage and NFT.
    fn update_account_attributes(
        &self,
        account_nonce: u64,
        account_attributes: &AccountAttributes<Self::Api>,
    ) {
        self.account_token()
            .nft_update_attributes(account_nonce, account_attributes);
        self.account_attributes(account_nonce)
            .set(account_attributes);
    }
}
