multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use crate::contexts::base::StorageCache;
use crate::{
    math, oracle, proxy_pool, storage, ERROR_ACCOUNT_NOT_IN_THE_MARKET, ERROR_ADDRESS_IS_ZERO,
    ERROR_AMOUNT_MUST_BE_GREATER_THAN_ZERO, ERROR_ASSET_NOT_SUPPORTED, ERROR_DEBT_CEILING_REACHED,
    ERROR_NO_POOL_FOUND, ERROR_UNEXPECTED_ANCHOR_TOLERANCES, ERROR_UNEXPECTED_FIRST_TOLERANCE,
    ERROR_UNEXPECTED_LAST_TOLERANCE,
};
use common_constants::{
    BP, EGLD_IDENTIFIER, MAX_FIRST_TOLERANCE, MAX_LAST_TOLERANCE, MIN_FIRST_TOLERANCE,
    MIN_LAST_TOLERANCE,
};
use common_structs::*;

#[multiversx_sc::module]
pub trait LendingUtilsModule:
    math::LendingMathModule
    + storage::LendingStorageModule
    + oracle::OracleModule
    + common_events::EventsModule
{
    /// Gets the liquidity pool address for a given asset
    ///
    /// # Arguments
    /// * `asset` - Token identifier of the asset
    ///
    /// # Returns
    /// * `ManagedAddress` - Address of the liquidity pool
    ///
    /// # Errors
    /// * `ERROR_NO_POOL_FOUND` - If no pool exists for the asset
    #[view(getPoolAddress)]
    fn get_pool_address(&self, asset: &EgldOrEsdtTokenIdentifier) -> ManagedAddress {
        let pool_address = self.pools_map(asset).get();

        require!(!pool_address.is_zero(), ERROR_NO_POOL_FOUND);

        pool_address
    }

    /// Calculates upper and lower bounds for a given tolerance
    ///
    /// # Arguments
    /// * `tolerance` - Tolerance value in basis points
    ///
    /// # Returns
    /// * `(BigUint, BigUint)` - Tuple containing:
    ///   - Upper bound (BP + tolerance)
    ///   - Lower bound (BP * BP / upper)
    ///
    /// # Example
    /// ```
    /// // For 5% tolerance (500 basis points)
    /// tolerance = 500
    /// upper = 10000 + 500 = 10500
    /// lower = 10000 * 10000 / 10500 ≈ 9524
    /// ```
    fn get_range(&self, tolerance: &BigUint) -> (BigUint, BigUint) {
        let bp = BigUint::from(BP);
        let upper = &bp + tolerance;
        let lower = &bp * &bp / &upper;

        (upper, lower)
    }

    /// Validates and calculates oracle price fluctuation tolerances
    ///
    /// # Arguments
    /// * `first_tolerance` - Initial tolerance for price deviation
    /// * `last_tolerance` - Maximum allowed tolerance
    ///
    /// # Returns
    /// * `OraclePriceFluctuation` - Struct containing upper/lower bounds for both tolerances
    ///
    /// # Errors
    /// * `ERROR_UNEXPECTED_FIRST_TOLERANCE` - If first tolerance is out of range
    /// * `ERROR_UNEXPECTED_LAST_TOLERANCE` - If last tolerance is out of range
    /// * `ERROR_UNEXPECTED_ANCHOR_TOLERANCES` - If last tolerance is less than first
    ///
    /// # Example
    /// ```
    /// // For 5% first tolerance and 10% last tolerance
    /// first_tolerance = 500 (5%)
    /// last_tolerance = 1000 (10%)
    ///
    /// Returns:
    /// OraclePriceFluctuation {
    ///   first_upper_ratio: 10500,  // 105%
    ///   first_lower_ratio: 9524,   // ~95.24%
    ///   last_upper_ratio: 11000,   // 110%
    ///   last_lower_ratio: 9091     // ~90.91%
    /// }
    /// ```
    fn get_anchor_tolerances(
        &self,
        first_tolerance: &BigUint,
        last_tolerance: &BigUint,
    ) -> OraclePriceFluctuation<Self::Api> {
        require!(
            first_tolerance >= &BigUint::from(MIN_FIRST_TOLERANCE)
                && first_tolerance <= &BigUint::from(MAX_FIRST_TOLERANCE),
            ERROR_UNEXPECTED_FIRST_TOLERANCE
        );

        require!(
            last_tolerance >= &BigUint::from(MIN_LAST_TOLERANCE)
                && last_tolerance <= &BigUint::from(MAX_LAST_TOLERANCE),
            ERROR_UNEXPECTED_LAST_TOLERANCE
        );

        require!(
            last_tolerance >= first_tolerance,
            ERROR_UNEXPECTED_ANCHOR_TOLERANCES
        );

        let (first_upper_ratio, first_lower_ratio) = self.get_range(first_tolerance);
        let (last_upper_ratio, last_lower_ratio) = self.get_range(last_tolerance);

        let tolerances = OraclePriceFluctuation {
            first_upper_ratio,
            first_lower_ratio,
            last_upper_ratio,
            last_lower_ratio,
        };

        tolerances
    }

    /// Calculates total weighted collateral value in EGLD for liquidation
    ///
    /// # Arguments
    /// * `positions` - Vector of account positions
    ///
    /// # Returns
    /// * `BigUint` - Total EGLD value weighted by liquidation thresholds
    ///
    /// # Example
    /// ```
    /// // Position 1: 100 EGLD, threshold 80%
    /// // Position 2: 1000 USDC, threshold 85%
    ///
    /// EGLD price = $100
    /// EGLD value = 100 * $100 * 0.80 = $8,000
    ///
    /// USDC price = $1
    /// USDC value = 1000 * $1 * 0.85 = $850
    ///
    /// Total = $8,850
    /// ```
    fn get_liquidation_collateral_in_egld_vec(
        &self,
        positions: &ManagedVec<AccountPosition<Self::Api>>,
        storage_cache: &mut StorageCache<Self>,
    ) -> BigUint {
        let mut weighted_collateral_in_egld = BigUint::zero();

        for dp in positions {
            weighted_collateral_in_egld +=
                self.get_token_amount_in_egld(&dp.token_id, &dp.get_total_amount(), storage_cache)
                    * &dp.entry_liquidation_threshold
                    / BigUint::from(BP);
        }

        weighted_collateral_in_egld
    }

    /// Calculates total weighted collateral value in EGLD for LTV
    ///
    /// # Arguments
    /// * `positions` - Vector of account positions
    ///
    /// # Returns
    /// * `BigUint` - Total EGLD value weighted by LTV ratios
    ///
    /// # Example
    /// ```
    /// // Position 1: 100 EGLD, LTV 75%
    /// // Position 2: 1000 USDC, LTV 80%
    ///
    /// EGLD price = $100
    /// EGLD value = 100 * $100 * 0.75 = $7,500
    ///
    /// USDC price = $1
    /// USDC value = 1000 * $1 * 0.80 = $800
    ///
    /// Total = $8,300
    /// ```
    fn get_ltv_collateral_in_egld_vec(
        &self,
        positions: &ManagedVec<AccountPosition<Self::Api>>,
        storage_cache: &mut StorageCache<Self>,
    ) -> BigUint {
        let mut weighted_collateral_in_egld = BigUint::zero();

        for dp in positions {
            let position_value_in_egld =
                self.get_token_amount_in_egld(&dp.token_id, &dp.get_total_amount(), storage_cache);

            weighted_collateral_in_egld +=
                &position_value_in_egld * &dp.entry_ltv / BigUint::from(BP);
        }

        weighted_collateral_in_egld
    }

    /// Calculates total borrow value in USD
    ///
    /// # Arguments
    /// * `positions` - Vector of account positions
    ///
    /// # Returns
    /// * `BigUint` - Total USD value of borrowed assets
    ///
    /// # Example
    /// ```
    /// // Position 1: 50 EGLD borrowed
    /// // Position 2: 500 USDC borrowed
    ///
    /// EGLD price = $100
    /// EGLD value = 50 * $100 = $5,000
    ///
    /// USDC price = $1
    /// USDC value = 500 * $1 = $500
    ///
    /// Total = $5,500
    /// ```
    fn get_total_borrow_in_egld_vec(
        &self,
        positions: &ManagedVec<AccountPosition<Self::Api>>,
        storage_cache: &mut StorageCache<Self>,
    ) -> BigUint {
        let mut total_borrow_in_egld = BigUint::zero();

        for bp in positions {
            total_borrow_in_egld +=
                self.get_token_amount_in_egld(&bp.token_id, &bp.get_total_amount(), storage_cache);
        }

        total_borrow_in_egld
    }

    /// Validates that a new borrow doesn't exceed isolated asset debt ceiling
    ///
    /// # Arguments
    /// * `asset_config` - Asset configuration
    /// * `token_id` - Token identifier
    /// * `amount_to_borrow_in_dollars` - USD value of new borrow
    ///
    /// # Errors
    /// * `ERROR_DEBT_CEILING_REACHED` - If new borrow would exceed debt ceiling
    ///
    /// # Example
    /// ```
    /// // Asset: USDC
    /// // Current debt: $800,000
    /// // Debt ceiling: $1,000,000
    /// // New borrow: $300,000
    /// // Total after: $1,100,000
    /// // Result: Error - ceiling exceeded
    /// ```
    fn validate_isolated_debt_ceiling(
        &self,
        asset_config: &AssetConfig<Self::Api>,
        token_id: &EgldOrEsdtTokenIdentifier,
        amount_to_borrow_in_dollars: &BigUint,
    ) {
        let current_debt = self.isolated_asset_debt_usd(token_id).get();

        let total_debt = current_debt.clone() + amount_to_borrow_in_dollars;

        require!(
            total_debt <= asset_config.debt_ceiling_usd,
            ERROR_DEBT_CEILING_REACHED
        );
    }

    /// Updates isolated asset debt tracking
    ///
    /// # Arguments
    /// * `token_id` - Token identifier
    /// * `amount_in_usd` - USD value to add/subtract
    /// * `is_increase` - Whether to increase or decrease debt
    ///
    /// # Flow
    /// 1. Skips if amount is zero
    /// 2. Updates debt tracking storage
    /// 3. Emits debt ceiling event
    ///
    /// # Example
    /// ```
    /// // Increase debt by $100,000
    /// update_isolated_debt_usd(
    ///   "USDC-123456",
    ///   BigUint::from(100_000),
    ///   true
    /// )
    /// ```
    fn update_isolated_debt_usd(
        &self,
        token_id: &EgldOrEsdtTokenIdentifier,
        amount_in_usd: &BigUint,
        is_increase: bool,
    ) {
        if amount_in_usd.eq(&BigUint::from(0u64)) {
            return;
        }

        let map = self.isolated_asset_debt_usd(token_id);

        if is_increase {
            map.update(|debt| *debt += amount_in_usd);
        } else {
            map.update(|debt| *debt -= amount_in_usd.min(&debt.clone()));
        }

        self.update_debt_ceiling_event(token_id, map.get());
    }

    /// Validates that an asset is supported by the protocol
    ///
    /// # Arguments
    /// * `asset` - Token identifier to check
    ///
    /// # Errors
    /// * `ERROR_ASSET_NOT_SUPPORTED` - If asset has no liquidity pool
    fn require_asset_supported(&self, asset: &EgldOrEsdtTokenIdentifier) {
        require!(!self.pools_map(asset).is_empty(), ERROR_ASSET_NOT_SUPPORTED);
    }

    /// Validates that an account is in the market
    ///
    /// # Arguments
    /// * `nonce` - Account nonce
    ///
    /// # Errors
    /// * `ERROR_ACCOUNT_NOT_IN_THE_MARKET` - If account is not in the market
    fn lending_account_in_the_market(&self, nonce: u64) {
        require!(
            self.account_positions().contains(&nonce),
            ERROR_ACCOUNT_NOT_IN_THE_MARKET
        );
    }

    /// Validates that an amount is greater than zero
    ///
    /// # Arguments
    /// * `amount` - Amount to validate
    ///
    /// # Errors
    /// * `ERROR_AMOUNT_MUST_BE_GREATER_THAN_ZERO` - If amount is not greater than zero
    fn require_amount_greater_than_zero(&self, amount: &BigUint) {
        require!(amount > &0, ERROR_AMOUNT_MUST_BE_GREATER_THAN_ZERO);
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

    /// Gets NFT attributes for an account position
    ///
    /// # Arguments
    /// * `account_nonce` - NFT nonce of the position
    /// * `token_id` - NFT token identifier
    ///
    /// # Returns
    /// * `NftAccountAttributes` - Decoded NFT attributes
    fn get_account_attributes(
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

    fn update_position(
        &self,
        asset_address: &ManagedAddress,
        position: &mut AccountPosition<Self::Api>,
        price: OptionalValue<BigUint>,
    ) {
        *position = self
            .tx()
            .to(asset_address)
            .typed(proxy_pool::LiquidityPoolProxy)
            .update_position_with_interest(position.clone(), price)
            .returns(ReturnsResult)
            .sync_call();
    }

    fn get_multi_payments(&self) -> ManagedVec<EgldOrEsdtTokenPaymentNew<Self::Api>> {
        let payments = self.call_value().all_esdt_transfers();

        let mut valid_payments = ManagedVec::new();
        for i in 0..payments.len() {
            let payment = payments.get(i);
            // EGLD sent as multi-esdt payment
            if payment.token_identifier.clone().into_managed_buffer()
                == ManagedBuffer::from(EGLD_IDENTIFIER)
                || payment.token_identifier.clone().into_managed_buffer()
                    == ManagedBuffer::from("EGLD")
            {
                valid_payments.push(EgldOrEsdtTokenPaymentNew {
                    token_identifier: EgldOrEsdtTokenIdentifier::egld(),
                    token_nonce: 0,
                    amount: payment.amount.clone(),
                });
            } else {
                valid_payments.push(EgldOrEsdtTokenPaymentNew {
                    token_identifier: EgldOrEsdtTokenIdentifier::esdt(
                        payment.token_identifier.clone(),
                    ),
                    token_nonce: payment.token_nonce,
                    amount: payment.amount.clone(),
                });
            }
        }

        valid_payments
    }
}
