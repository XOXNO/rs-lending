use common_constants::BASE_NFT_URI;
use common_events::PositionMode;
use common_structs::AccountAttributes;

use crate::storage;
use common_errors::{
    ERROR_ACCOUNT_ATTRIBUTES_MISMATCH, ERROR_ACCOUNT_NOT_IN_THE_MARKET, ERROR_ADDRESS_IS_ZERO,
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
        mode: PositionMode,
        e_mode_category: OptionalValue<u8>,
        isolated_token: Option<EgldOrEsdtTokenIdentifier>,
    ) -> (EsdtTokenPayment, AccountAttributes<Self::Api>) {
        let e_mode_category_id = if is_isolated {
            0
        } else {
            e_mode_category.into_option().unwrap_or(0)
        };

        let isolated_token = if is_isolated {
            ManagedOption::from(isolated_token)
        } else {
            ManagedOption::none()
        };

        let attributes = AccountAttributes {
            is_isolated_position: is_isolated,
            e_mode_category_id,
            mode,
            isolated_token,
        };

        let map_last_nonce = self.last_account_nonce();
        let last_account_nonce = map_last_nonce.get();
        let next_nonce = last_account_nonce + 1;

        let nft_nonce = self.send().esdt_nft_create(
            self.account_token().get_token_id_ref(),
            &BigUint::from(1u64),
            &sc_format!("Lending Account #{}", next_nonce),
            &BigUint::zero(),
            &ManagedBuffer::new(),
            &attributes,
            &ManagedVec::from_single_item(sc_format!("{}/{}", BASE_NFT_URI, next_nonce)),
        );

        map_last_nonce.set(nft_nonce);

        let nft_token_payment = EsdtTokenPayment::new(
            self.account_token().get_token_id(),
            nft_nonce,
            BigUint::from(1u64),
        );

        self.tx().to(caller).payment(&nft_token_payment).transfer();

        let _ = self.account_positions().insert(nft_nonce);
        self.account_attributes(nft_nonce).set(attributes.clone());

        (nft_token_payment, attributes)
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
        mode: PositionMode,
        e_mode_category: OptionalValue<u8>,
        opt_account: Option<EsdtTokenPayment<Self::Api>>,
        opt_attributes: Option<AccountAttributes<Self::Api>>,
        opt_isolated_token: Option<EgldOrEsdtTokenIdentifier>,
    ) -> (u64, AccountAttributes<Self::Api>) {
        match opt_account {
            Some(account) => (account.token_nonce, opt_attributes.unwrap()),
            None => {
                let (payment, account_attributes) = self.create_account_nft(
                    caller,
                    is_isolated,
                    mode,
                    e_mode_category,
                    opt_isolated_token,
                );
                (payment.token_nonce, account_attributes)
            },
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
    fn nft_attributes(
        &self,
        account_payment: &EsdtTokenPayment<Self::Api>,
    ) -> AccountAttributes<Self::Api> {
        let data = self.blockchain().get_esdt_token_data(
            &self.blockchain().get_sc_address(),
            &account_payment.token_identifier,
            account_payment.token_nonce,
        );

        data.decode_attributes()
    }

    /// Ensures an account nonce is active in the market.
    /// Prevents operations on uninitialized accounts.
    ///
    /// # Arguments
    /// - `nonce`: Account nonce to verify.
    #[inline]
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
    #[inline]
    fn validate_account(
        &self,
        return_account: bool,
    ) -> (
        EsdtTokenPayment<Self::Api>,
        ManagedAddress,
        AccountAttributes<Self::Api>,
    ) {
        let account_payment = self.call_value().single_esdt().clone();
        self.require_active_account(account_payment.token_nonce);
        self.account_token()
            .require_same_token(&account_payment.token_identifier);

        let caller = self.blockchain().get_caller();

        let account_attributes = self.nft_attributes(&account_payment);
        let stored_attributes = self.account_attributes(account_payment.token_nonce).get();

        require!(
            account_attributes == stored_attributes,
            ERROR_ACCOUNT_ATTRIBUTES_MISMATCH
        );

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
    #[inline]
    fn require_non_zero_address(&self, address: &ManagedAddress) {
        require!(!address.is_zero(), ERROR_ADDRESS_IS_ZERO);
    }
}
