multiversx_sc::imports!();

use common_constants::TOTAL_RESERVES_AMOUNT_STORAGE_KEY;
use multiversx_sc::storage::StorageKey;

use crate::{
    helpers, oracle, positions, storage, utils, ERROR_ADDRESS_IS_ZERO,
    ERROR_AMOUNT_MUST_BE_GREATER_THAN_ZERO, ERROR_ASSET_NOT_SUPPORTED,
};

#[multiversx_sc::module]
pub trait ValidationModule:
    storage::LendingStorageModule
    + utils::LendingUtilsModule
    + common_events::EventsModule
    + oracle::OracleModule
    + helpers::math::MathsModule
    + positions::account::PositionAccountModule
    + common_math::SharedMathModule
{
    fn get_total_reserves(
        &self,
        pair_address: ManagedAddress,
    ) -> SingleValueMapper<ManagedDecimal<Self::Api, NumDecimals>, ManagedAddress> {
        SingleValueMapper::<_, _, ManagedAddress>::new_from_address(
            pair_address,
            StorageKey::new(TOTAL_RESERVES_AMOUNT_STORAGE_KEY),
        )
    }

    /// Validates repay payment parameters
    ///
    /// # Arguments
    /// * `repay_token_id` - Token being repaid
    /// * `repay_amount` - Amount being repaid
    /// * `account_nonce` - NFT nonce of the account position
    ///
    /// Validates:
    /// - Account exists in market
    /// - Asset is supported
    /// - Amount is greater than zero
    fn validate_payment(&self, payment: &EgldOrEsdtTokenPayment<Self::Api>) {
        self.require_asset_supported(&payment.token_identifier);
        self.require_amount_greater_than_zero(&payment.amount);
    }

    /// Validates liquidation payment parameters
    ///
    /// # Arguments
    /// * `debt_payment` - Payment to cover debt
    /// * `initial_caller` - Address initiating liquidation
    ///
    /// Validates:
    /// - Both assets are supported
    /// - Payment amount is greater than zero
    /// - Caller address is valid
    fn validate_liquidation_payments(
        &self,
        debt_repayments: &ManagedVec<EgldOrEsdtTokenPayment<Self::Api>>,
        initial_caller: &ManagedAddress,
    ) {
        for debt_payment in debt_repayments {
            self.validate_payment(&debt_payment);
        }
        self.require_non_zero_address(initial_caller);
    }

    /// Validates that an asset is supported by the protocol
    ///
    /// # Arguments
    /// * `asset` - Token identifier to check
    ///
    /// # Errors
    /// * `ERROR_ASSET_NOT_SUPPORTED` - If asset has no liquidity pool
    fn require_asset_supported(&self, asset: &EgldOrEsdtTokenIdentifier) -> ManagedAddress {
        let map = self.pools_map(asset);
        require!(!map.is_empty(), ERROR_ASSET_NOT_SUPPORTED);
        map.get()
    }

    /// Validates that an amount is greater than zero
    ///
    /// # Arguments
    /// * `amount` - Amount to validate
    ///
    /// # Errors
    /// * `ERROR_AMOUNT_MUST_BE_GREATER_THAN_ZERO` - If amount is not greater than zero
    fn require_amount_greater_than_zero(&self, amount: &BigUint) {
        require!(amount > &BigUint::zero(), ERROR_AMOUNT_MUST_BE_GREATER_THAN_ZERO);
    }

    /// Validates that an address is not zero
    ///
    /// # Arguments
    /// * `address` - Address to validate
    ///
    /// # Errors
    /// * `ERROR_ADDRESS_IS_ZERO` - If address is zero
    fn require_non_zero_address(&self, address: &ManagedAddress) {
        require!(!address.is_zero(), ERROR_ADDRESS_IS_ZERO);
    }
}
