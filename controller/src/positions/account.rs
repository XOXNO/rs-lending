use common_events::AccountAttributes;

use crate::storage;
use common_errors::{ERROR_ACCOUNT_NOT_IN_THE_MARKET, ERROR_POSITION_SHOULD_BE_VAULT};

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

#[multiversx_sc::module]
pub trait PositionAccountModule:
    common_events::EventsModule + storage::LendingStorageModule
{
    /// Creates a new NFT for a user's lending position.
    /// Tracks position type (isolated, vault, e-mode) via NFT attributes.
    ///
    /// # Arguments
    /// - `caller`: User's address.
    /// - `is_isolated`: Indicates an isolated position.
    /// - `is_vault`: Indicates a vault position.
    /// - `e_mode_category`: Optional e-mode category ID.
    ///
    /// # Returns
    /// - Tuple of (NFT payment, attributes).
    fn create_account_nft(
        &self,
        caller: &ManagedAddress,
        is_isolated: bool,
        is_vault: bool,
        e_mode_category: OptionalValue<u8>,
    ) -> (EsdtTokenPayment, AccountAttributes) {
        let attributes = AccountAttributes {
            is_isolated_position: is_isolated,
            e_mode_category_id: if is_isolated {
                0
            } else {
                e_mode_category.into_option().unwrap_or(0)
            },
            is_vault_position: is_vault,
        };
        let nft_token_payment = self
            .account_token()
            .nft_create_and_send::<AccountAttributes>(caller, BigUint::from(1u64), &attributes);

        self.account_positions()
            .insert(nft_token_payment.token_nonce);
        self.account_attributes(nft_token_payment.token_nonce)
            .set(attributes.clone());

        (nft_token_payment, attributes)
    }

    /// Validates that an existing position NFT is active.
    /// Prevents operations on invalid or inactive positions.
    ///
    /// # Arguments
    /// - `account`: NFT payment to check.
    fn validate_existing_account(&self, account: &EsdtTokenPayment<Self::Api>) {
        self.require_active_account(account.token_nonce);
        self.account_token()
            .require_same_token(&account.token_identifier);
    }

    /// Retrieves an existing position or creates a new one.
    /// Reuses existing NFTs or initializes new ones as needed.
    ///
    /// # Arguments
    /// - `caller`: User's address.
    /// - `is_isolated`: Indicates an isolated position.
    /// - `is_vault`: Indicates a vault position.
    /// - `e_mode_category`: Optional e-mode category.
    /// - `existing_position`: Optional existing NFT.
    ///
    /// # Returns
    /// - Tuple of (NFT nonce, attributes).
    fn get_or_create_account(
        &self,
        caller: &ManagedAddress,
        is_isolated: bool,
        is_vault: bool,
        e_mode_category: OptionalValue<u8>,
        existing_account: Option<EsdtTokenPayment<Self::Api>>,
    ) -> (u64, AccountAttributes) {
        if let Some(account) = existing_account {
            self.validate_existing_account(&account);
            let account_attributes = self.nft_attributes(&account);
            self.tx().to(caller).payment(&account).transfer();
            (account.token_nonce, account_attributes)
        } else {
            let (payment, account_attributes) =
                self.create_account_nft(caller, is_isolated, is_vault, e_mode_category);
            (payment.token_nonce, account_attributes)
        }
    }

    /// Decodes and retrieves attributes of a position NFT.
    /// Accesses on-chain position metadata.
    ///
    /// # Arguments
    /// - `account_nonce`: NFT nonce.
    /// - `token_id`: NFT identifier.
    ///
    /// # Returns
    /// - `AccountAttributes` containing position details.
    fn nft_attributes(&self, account_payment: &EsdtTokenPayment<Self::Api>) -> AccountAttributes {
        let data = self.blockchain().get_esdt_token_data(
            &self.blockchain().get_sc_address(),
            &account_payment.token_identifier,
            account_payment.token_nonce,
        );

        data.decode_attributes::<AccountAttributes>()
    }

    /// Ensures an account nonce is active in the market.
    /// Prevents operations on uninitialized accounts.
    ///
    /// # Arguments
    /// - `nonce`: Account nonce to verify.
    fn require_active_account(&self, nonce: u64) {
        require!(
            self.account_positions().contains(&nonce),
            ERROR_ACCOUNT_NOT_IN_THE_MARKET
        );
    }

    /// Validates consistency between position and operation vault status.
    /// Ensures correct interest accrual behavior.
    ///
    /// # Arguments
    /// - `account_attributes`: Account attributes.
    /// - `is_vault`: Operation vault flag.
    fn validate_vault_consistency(&self, account_attributes: &AccountAttributes, is_vault: bool) {
        if account_attributes.is_vault() || is_vault {
            require!(
                account_attributes.is_vault() == is_vault,
                ERROR_POSITION_SHOULD_BE_VAULT
            );
        }
    }
}
