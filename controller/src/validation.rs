multiversx_sc::imports!();

use common_errors::ERROR_INVALID_SHARD;

use crate::{
    helpers, oracle, storage, utils, ERROR_AMOUNT_MUST_BE_GREATER_THAN_ZERO,
    ERROR_ASSET_NOT_SUPPORTED,
};

#[multiversx_sc::module]
pub trait ValidationModule:
    storage::Storage
    + utils::LendingUtilsModule
    + common_events::EventsModule
    + oracle::OracleModule
    + helpers::math::MathsModule
    + common_math::SharedMathModule
{
    /// Validates a payment for operations like repayments or deposits.
    /// Ensures the asset is supported and the amount is valid.
    ///
    /// # Arguments
    /// - `payment`: The payment to validate (token identifier and amount).
    ///
    /// # Errors
    /// - `ERROR_ASSET_NOT_SUPPORTED`: If the asset has no liquidity pool.
    /// - `ERROR_AMOUNT_MUST_BE_GREATER_THAN_ZERO`: If the amount is zero or negative.
    fn validate_payment(&self, payment: &EgldOrEsdtTokenPayment<Self::Api>) {
        let _ = self.require_asset_supported(&payment.token_identifier);
        self.require_amount_greater_than_zero(&payment.amount);
    }

    /// Ensures an asset is supported by verifying its liquidity pool exists.
    ///
    /// # Arguments
    /// - `asset`: Token identifier (EGLD or ESDT) to check.
    ///
    /// # Returns
    /// - `ManagedAddress`: Pool address if the asset is supported.
    ///
    /// # Errors
    /// - `ERROR_ASSET_NOT_SUPPORTED`: If no pool exists for the asset.
    fn require_asset_supported(&self, asset: &EgldOrEsdtTokenIdentifier) -> ManagedAddress {
        let map = self.pools_map(asset);
        require!(!map.is_empty(), ERROR_ASSET_NOT_SUPPORTED);

        map.get()
    }

    /// Ensures an amount is greater than zero.
    /// Prevents zero-value operations like deposits or borrows.
    ///
    /// # Arguments
    /// - `amount`: The amount to validate as a `BigUint`.
    ///
    /// # Errors
    /// - `ERROR_AMOUNT_MUST_BE_GREATER_THAN_ZERO`: If the amount is zero or negative.
    fn require_amount_greater_than_zero(&self, amount: &BigUint) {
        require!(
            amount > &BigUint::zero(),
            ERROR_AMOUNT_MUST_BE_GREATER_THAN_ZERO
        );
    }

    // --- Helper Functions ---

    /// Validates shard compatibility for flash loans.
    fn validate_flash_loan_shard(&self, contract_address: &ManagedAddress) {
        let destination_shard_id = self.blockchain().get_shard_of_address(contract_address);
        let current_shard_id = self
            .blockchain()
            .get_shard_of_address(&self.blockchain().get_sc_address());

        require!(
            destination_shard_id == current_shard_id,
            ERROR_INVALID_SHARD
        );
    }
}
