use common_constants::BASE_NFT_URI;
use common_structs::AccountAttributes;

use crate::storage;
use common_errors::{
    ERROR_ACCOUNT_NOT_IN_THE_MARKET, ERROR_ADDRESS_IS_ZERO, ERROR_POSITION_SHOULD_BE_VAULT,
};

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

#[multiversx_sc::module]
pub trait PositionAccountModule: common_events::EventsModule + storage::Storage {
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

        let nft_token_payment = self.account_token().nft_create_named::<AccountAttributes>(
            BigUint::from(1u64),
            &ManagedBuffer::from("Lending Account"),
            &attributes,
        );

        let _ = self
            .tx()
            .typed(system_proxy::UserBuiltinProxy)
            .nft_add_multiple_uri(
                self.account_token().get_token_id_ref(),
                nft_token_payment.token_nonce,
                &ManagedVec::from_single_item(sc_format!(
                    "{}/{}",
                    BASE_NFT_URI,
                    nft_token_payment.token_nonce
                )),
            );

        self.account_token()
            .send_payment(caller, &nft_token_payment);

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
        maybe_attributes: Option<AccountAttributes>,
    ) -> (u64, AccountAttributes) {
        if let Some(account) = existing_account {
            (account.token_nonce, maybe_attributes.unwrap())
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

    /// Validates borrow operation parameters.
    /// Ensures account, asset, and caller are valid.
    ///
    /// # Arguments
    /// - `position_nft_payment`: NFT payment.
    /// - `initial_caller`: Borrower's address.
    fn validate_account(
        &self,
        return_account: bool,
    ) -> (
        EsdtTokenPayment<Self::Api>,
        ManagedAddress,
        AccountAttributes,
    ) {
        let account_payment = self.call_value().single_esdt().clone();
        let caller = self.blockchain().get_caller();
        self.require_active_account(account_payment.token_nonce);
        self.account_token()
            .require_same_token(&account_payment.token_identifier);
        self.require_non_zero_address(&caller);
        let account_attributes = self.nft_attributes(&account_payment);

        if return_account {
            // Transfer the account NFT back to the caller right after validation
            self.tx().to(&caller).payment(&account_payment).transfer();
        }

        (account_payment, caller, account_attributes)
    }

    /// Ensures an address is not the zero address.
    /// Validates caller or contract addresses to avoid invalid operations.
    ///
    /// # Arguments
    /// - `address`: The address to validate as a `ManagedAddress`.
    ///
    /// # Errors
    /// - `ERROR_ADDRESS_IS_ZERO`: If the address is zero.
    fn require_non_zero_address(&self, address: &ManagedAddress) {
        require!(!address.is_zero(), ERROR_ADDRESS_IS_ZERO);
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
