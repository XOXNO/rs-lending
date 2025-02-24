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
    /// Retrieves the total reserves for a liquidity pool.
    /// Provides liquidity data for reserve factor checks or availability.
    ///
    /// # Arguments
    /// - `pair_address`: Address of the liquidity pool.
    ///
    /// # Returns
    /// - `SingleValueMapper`: Total reserves in `ManagedDecimal` format, tied to the pool address.
    fn get_total_reserves(
        &self,
        pair_address: ManagedAddress,
    ) -> SingleValueMapper<ManagedDecimal<Self::Api, NumDecimals>, ManagedAddress> {
        SingleValueMapper::<_, _, ManagedAddress>::new_from_address(
            pair_address,
            StorageKey::new(TOTAL_RESERVES_AMOUNT_STORAGE_KEY),
        )
    }

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
        self.require_asset_supported(&payment.token_identifier);
        self.require_amount_greater_than_zero(&payment.amount);
    }

    /// Validates payments for liquidation operations.
    /// Ensures debt repayments are valid and the caller is authorized.
    ///
    /// # Arguments
    /// - `debt_repayments`: Vector of debt repayment payments.
    /// - `initial_caller`: Address initiating the liquidation.
    ///
    /// # Errors
    /// - Inherits errors from `validate_payment`.
    /// - `ERROR_ADDRESS_IS_ZERO`: If the caller address is zero.
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
}
