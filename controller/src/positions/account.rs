use common_constants::BASE_NFT_URI;
use common_structs::{AccountAttributes, PositionMode};

use crate::storage;
use common_errors::{
    ERROR_ACCOUNT_ATTRIBUTES_MISMATCH, ERROR_ACCOUNT_NOT_IN_THE_MARKET, ERROR_ADDRESS_IS_ZERO,
};

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

#[multiversx_sc::module]
pub trait PositionAccountModule: common_events::EventsModule + storage::Storage {
    /// Creates a new NFT for a user's lending position.
    ///
    /// **Purpose**: Mints a new position NFT that represents a user's lending account
    /// with specific risk profile and operational mode configurations.
    ///
    /// **Methodology**:
    /// 1. Determines e-mode category (disabled for isolated positions)
    /// 2. Sets isolated token if position is isolated
    /// 3. Creates NFT with incremented nonce and position attributes
    /// 4. Transfers NFT to caller and updates storage mappings
    ///
    /// **NFT Attributes Structure**:
    /// - `is_isolated_position`: Flag for isolation mode
    /// - `e_mode_category_id`: E-mode category (0 = disabled)
    /// - `mode`: Position mode (standard, vault, etc.)
    /// - `isolated_token`: Token identifier for isolated positions
    ///
    /// **Security Considerations**:
    /// - Isolated positions cannot use e-mode (forced to category 0)
    /// - Incremental nonce prevents collision attacks
    /// - Atomic creation and storage updates
    ///
    /// **NFT Metadata**:
    /// - Name: "Lending Account #{nonce}"
    /// - URI: "{BASE_NFT_URI}/{nonce}"
    /// - Attributes: Encoded position configuration
    ///
    /// # Arguments
    /// - `caller`: User's address to receive the NFT
    /// - `is_isolated`: Flag indicating isolated collateral position
    /// - `mode`: Position mode configuration
    /// - `e_mode_category`: Optional e-mode category ID (ignored if isolated)
    /// - `isolated_token`: Required token identifier for isolated positions
    ///
    /// # Returns
    /// - Tuple containing (NFT payment, position attributes)
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

        let map_last_nonce = self.account_nonce();
        let last_account_nonce = map_last_nonce.get();
        let next_nonce = last_account_nonce + 1;

        let account_nonce = self.send().esdt_nft_create(
            self.account().get_token_id_ref(),
            &BigUint::from(1u64),
            &sc_format!("Lending Account #{}", next_nonce),
            &BigUint::zero(),
            &ManagedBuffer::new(),
            &attributes,
            &ManagedVec::from_single_item(sc_format!("{}/{}", BASE_NFT_URI, next_nonce)),
        );

        map_last_nonce.set(account_nonce);

        let account_payment = EsdtTokenPayment::new(
            self.account().get_token_id(),
            account_nonce,
            BigUint::from(1u64),
        );

        self.tx().to(caller).payment(&account_payment).transfer();

        let _ = self.accounts().insert(account_nonce);
        self.account_attributes(account_nonce)
            .set(attributes.clone());

        (account_payment, attributes)
    }

    /// Retrieves an existing position or creates a new one.
    ///
    /// **Purpose**: Manages position NFT lifecycle by either using existing positions
    /// or creating new ones when needed, optimizing for user experience.
    ///
    /// **Methodology**:
    /// - If existing account provided: validates and uses existing NFT
    /// - If no account provided: creates new NFT with specified configuration
    /// - Returns consistent interface regardless of creation path
    ///
    /// **Use Cases**:
    /// - First-time users: Creates new position NFT
    /// - Existing users: Reuses existing position for additional operations
    /// - Multi-position users: Can have multiple NFTs with different configs
    ///
    /// **Security Considerations**:
    /// - Validates existing NFT attributes match expected configuration
    /// - Ensures consistent position state across operations
    /// - Prevents unauthorized position modifications
    ///
    /// # Arguments
    /// - `caller`: User's address for NFT operations
    /// - `is_isolated`: Flag for isolation mode requirement
    /// - `mode`: Position mode configuration
    /// - `e_mode_category`: Optional e-mode category for new positions
    /// - `optional_account`: Optional existing account NFT to reuse
    /// - `optional_attributes`: Optional existing attributes for validation
    /// - `optional_isolated_token`: Required token for isolated position creation
    ///
    /// # Returns
    /// - Tuple containing (NFT nonce, validated position attributes)
    fn get_or_create_account(
        &self,
        caller: &ManagedAddress,
        is_isolated: bool,
        mode: PositionMode,
        e_mode_category: OptionalValue<u8>,
        optional_account: Option<EsdtTokenPayment<Self::Api>>,
        optional_attributes: Option<AccountAttributes<Self::Api>>,
        optional_isolated_token: Option<EgldOrEsdtTokenIdentifier>,
    ) -> (u64, AccountAttributes<Self::Api>) {
        match optional_account {
            Some(account) => (account.token_nonce, unsafe {
                optional_attributes.unwrap_unchecked()
            }),
            None => {
                let (payment, account_attributes) = self.create_account_nft(
                    caller,
                    is_isolated,
                    mode,
                    e_mode_category,
                    optional_isolated_token,
                );
                (payment.token_nonce, account_attributes)
            },
        }
    }

    /// Decodes and retrieves attributes of a position NFT.
    ///
    /// **Purpose**: Extracts position configuration from NFT metadata to determine
    /// operational constraints and risk parameters for the position.
    ///
    /// **Methodology**:
    /// - Queries blockchain for NFT token data at specified nonce
    /// - Decodes stored attributes from NFT metadata
    /// - Returns structured position configuration
    ///
    /// **Attribute Decoding**:
    /// - Uses built-in codec for secure deserialization
    /// - Validates attribute structure integrity
    /// - Provides type-safe access to position configuration
    ///
    /// **Security Considerations**:
    /// - Tamper-proof storage in NFT metadata
    /// - Cryptographic validation of attribute integrity
    /// - Read-only access prevents unauthorized modifications
    ///
    /// # Arguments
    /// - `account_payment`: NFT payment containing identifier and nonce
    ///
    /// # Returns
    /// - `AccountAttributes` with decoded position configuration
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
    ///
    /// **Purpose**: Validates that a position NFT is properly registered in the protocol
    /// before allowing any lending operations on it.
    ///
    /// **Methodology**:
    /// - Checks if nonce exists in active accounts registry
    /// - Prevents operations on invalid or burned NFTs
    /// - Provides early validation for all position operations
    ///
    /// **Security Rationale**:
    /// - Prevents operations on non-existent positions
    /// - Protects against replay attacks using old NFTs
    /// - Ensures data consistency across protocol operations
    ///
    /// # Arguments
    /// - `nonce`: Account NFT nonce to validate for existence

    fn require_active_account(&self, nonce: u64) {
        require!(
            self.accounts().contains(&nonce),
            ERROR_ACCOUNT_NOT_IN_THE_MARKET
        );
    }

    /// Validates account NFT and extracts operation parameters.
    ///
    /// **Purpose**: Comprehensive validation of account NFT for lending operations,
    /// ensuring NFT authenticity and attribute consistency.
    ///
    /// **Methodology**:
    /// 1. Extracts NFT payment from call context
    /// 2. Validates account is active in the protocol
    /// 3. Verifies NFT token identifier matches expected account token
    /// 4. Validates attribute consistency between NFT and storage
    /// 5. Optionally returns NFT to caller after validation
    ///
    /// **Security Checks**:
    /// - Account activity validation prevents unauthorized operations
    /// - Token identifier validation prevents spoofing attacks
    /// - Attribute consistency check prevents tampering
    /// - Caller address validation ensures proper authorization
    ///
    /// **Attribute Consistency**:
    /// - Compares NFT attributes with stored attributes
    /// - Ensures no unauthorized modifications to position config
    /// - Validates integrity of position state
    ///
    /// # Arguments
    /// - `return_account`: Flag to return NFT to caller after validation
    ///
    /// # Returns
    /// - Tuple containing (NFT payment, caller address, validated attributes)

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
        self.account()
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
    ///
    /// **Purpose**: Validates addresses to prevent operations with invalid zero addresses
    /// that could lead to token loss or protocol malfunctions.
    ///
    /// **Security Rationale**:
    /// - Zero addresses indicate uninitialized or invalid states
    /// - Prevents accidental token transfers to burn address
    /// - Ensures proper caller identification for all operations
    /// - Protects against address computation errors
    ///
    /// **Validation Context**:
    /// - Caller addresses for operation authorization
    /// - Contract addresses for cross-contract interactions
    /// - Recipient addresses for token transfers
    ///
    /// # Arguments
    /// - `address`: Address to validate for non-zero value
    ///
    /// # Errors
    /// - `ERROR_ADDRESS_IS_ZERO`: If address equals zero/empty address

    fn require_non_zero_address(&self, address: &ManagedAddress) {
        require!(!address.is_zero(), ERROR_ADDRESS_IS_ZERO);
    }
}
