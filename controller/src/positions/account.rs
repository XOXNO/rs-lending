use common_events::NftAccountAttributes;

use crate::storage;
use common_errors::{ERROR_ACCOUNT_NOT_IN_THE_MARKET, ERROR_POSITION_SHOULD_BE_VAULT};

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

#[multiversx_sc::module]
pub trait PositionAccountModule:
    common_events::EventsModule + storage::LendingStorageModule
{
    /// Creates a new account position NFT
    ///
    /// # Arguments
    /// * `caller` - Address of the user creating the position
    /// * `is_isolated` - Whether this is an isolated position (can only have one collateral)
    /// * `is_vault` - Whether this is a vault position (no interest accrual)
    /// * `e_mode_category` - Optional e-mode category for specialized LTV and liquidation parameters
    ///
    /// # Returns
    /// * `(EsdtTokenPayment, NftAccountAttributes)` - The created NFT and its attributes
    ///
    /// Creates and sends a new NFT to the caller representing their lending position.
    /// The NFT attributes store the position type (isolated/vault) and e-mode settings.
    fn create_position_nft(
        &self,
        caller: &ManagedAddress,
        is_isolated: bool,
        is_vault: bool,
        e_mode_category: OptionalValue<u8>,
    ) -> (EsdtTokenPayment, NftAccountAttributes) {
        let attributes = NftAccountAttributes {
            is_isolated,
            e_mode_category: if is_isolated {
                0
            } else {
                e_mode_category.into_option().unwrap_or(0)
            },
            is_vault,
        };
        let nft_token_payment = self
            .account_token()
            .nft_create_and_send::<NftAccountAttributes>(caller, BigUint::from(1u64), &attributes);

        self.account_positions()
            .insert(nft_token_payment.token_nonce);
        self.account_attributes(nft_token_payment.token_nonce)
            .set(attributes.clone());

        (nft_token_payment, attributes)
    }

    fn validate_existing_position(&self, account: &EsdtTokenPayment<Self::Api>) {
        self.require_active_account(account.token_nonce);
        self.account_token()
            .require_same_token(&account.token_identifier);
    }

    /// Gets or creates a supply position for a user
    ///
    /// # Arguments
    /// * `caller` - Address of the user supplying assets
    /// * `is_isolated` - Whether this is an isolated position
    /// * `is_vault` - Whether this is a vault position
    /// * `e_mode_category` - Optional e-mode category
    /// * `account_nonce` - Optional existing NFT nonce to use
    ///
    /// # Returns
    /// * `(u64, NftAccountAttributes)` - NFT nonce and its attributes
    ///
    /// If account_nonce is provided, validates and uses existing position.
    /// Otherwise creates a new position with specified parameters.
    fn get_or_create_position(
        &self,
        caller: &ManagedAddress,
        is_isolated: bool,
        is_vault: bool,
        e_mode_category: OptionalValue<u8>,
        existing_position: Option<EsdtTokenPayment<Self::Api>>,
    ) -> (u64, NftAccountAttributes) {
        if let Some(position) = existing_position {
            self.validate_existing_position(&position);
            let attributes = self.nft_attributes(position.token_nonce, &position.token_identifier);
            self.tx()
                .to(caller)
                .single_esdt(
                    &position.token_identifier,
                    position.token_nonce,
                    &BigUint::from(1u64),
                )
                .transfer();
            (position.token_nonce, attributes)
        } else {
            let (payment, attributes) =
                self.create_position_nft(caller, is_isolated, is_vault, e_mode_category);
            (payment.token_nonce, attributes)
        }
    }

    /// Gets NFT attributes for an account position
    ///
    /// # Arguments
    /// * `account_nonce` - NFT nonce of the position
    /// * `token_id` - NFT token identifier
    ///
    /// # Returns
    /// * `NftAccountAttributes` - Decoded NFT attributes
    fn nft_attributes(
        &self,
        account_nonce: u64,
        token_id: &TokenIdentifier<Self::Api>,
    ) -> NftAccountAttributes {
        let data = self.blockchain().get_esdt_token_data(
            &self.blockchain().get_sc_address(),
            token_id,
            account_nonce,
        );

        data.decode_attributes::<NftAccountAttributes>()
    }

    /// Validates that an account is in the market
    ///
    /// # Arguments
    /// * `nonce` - Account nonce
    ///
    /// # Errors
    /// * `ERROR_ACCOUNT_NOT_IN_THE_MARKET` - If account is not in the market
    fn require_active_account(&self, nonce: u64) {
        require!(
            self.account_positions().contains(&nonce),
            ERROR_ACCOUNT_NOT_IN_THE_MARKET
        );
    }

    /// Validates consistency between vault flags
    ///
    /// # Arguments
    /// * `nft_attributes` - Position NFT attributes
    /// * `is_vault` - Whether this operation is for a vault
    ///
    /// Ensures that if either the position or operation is vault-type,
    /// both must be vault-type to maintain consistency.
    fn validate_vault_consistency(&self, nft_attributes: &NftAccountAttributes, is_vault: bool) {
        if nft_attributes.is_vault || is_vault {
            require!(
                nft_attributes.is_vault == is_vault,
                ERROR_POSITION_SHOULD_BE_VAULT
            );
        }
    }
}
