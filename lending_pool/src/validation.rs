multiversx_sc::imports!();
use common_constants::{
    BP, TOTAL_BORROWED_AMOUNT_STORAGE_KEY, TOTAL_RESERVES_AMOUNT_STORAGE_KEY,
    TOTAL_SUPPLY_AMOUNT_STORAGE_KEY,
};
use common_events::{
    AccountPosition, AssetConfig, EModeAssetConfig, EModeCategory, NftAccountAttributes,
};
use common_structs::PriceFeedShort;
use multiversx_sc::storage::StorageKey;

use crate::{
    contexts::base::StorageCache, helpers, oracle, storage, utils, ERROR_ACCOUNT_NOT_IN_THE_MARKET,
    ERROR_ADDRESS_IS_ZERO, ERROR_AMOUNT_MUST_BE_GREATER_THAN_ZERO,
    ERROR_ASSET_NOT_BORROWABLE_IN_ISOLATION, ERROR_ASSET_NOT_BORROWABLE_IN_SILOED,
    ERROR_ASSET_NOT_SUPPORTED, ERROR_BORROW_CAP, ERROR_CANNOT_USE_EMODE_WITH_ISOLATED_ASSETS,
    ERROR_DEBT_CEILING_REACHED, ERROR_EMODE_CATEGORY_DEPRECATED, ERROR_EMODE_CATEGORY_NOT_FOUND,
    ERROR_HEALTH_FACTOR, ERROR_HEALTH_FACTOR_BECOME_LOW, ERROR_HEALTH_FACTOR_WITHDRAW,
    ERROR_INSUFFICIENT_COLLATERAL, ERROR_INVALID_NUMBER_OF_ESDT_TRANSFERS,
    ERROR_MIX_ISOLATED_COLLATERAL, ERROR_POSITION_SHOULD_BE_VAULT, ERROR_SUPPLY_CAP,
};

#[multiversx_sc::module]
pub trait ValidationModule:
    storage::LendingStorageModule
    + utils::LendingUtilsModule
    + common_events::EventsModule
    + oracle::OracleModule
    + helpers::math::MathsModule
{
    /// Validates supply payment and handles NFT return
    ///
    /// # Arguments
    /// * `caller` - Address of the user supplying assets
    /// * `payments` - Vector of payments (can include NFT and collateral)
    ///
    /// # Returns
    /// * `(EgldOrEsdtTokenPayment, Option<EgldOrEsdtTokenPayment>)` - Tuple containing:
    ///   - Collateral payment
    ///   - Optional account NFT payment
    ///
    fn validate_supply_payment(
        &self,
        caller: &ManagedAddress,
        payments: &ManagedVec<EgldOrEsdtTokenPayment<Self::Api>>,
    ) -> (
        ManagedVec<EgldOrEsdtTokenPayment<Self::Api>>,
        Option<EgldOrEsdtTokenPayment<Self::Api>>,
    ) {
        require!(payments.len() >= 1, ERROR_INVALID_NUMBER_OF_ESDT_TRANSFERS);

        // Validate the collateral payment token
        self.require_non_zero_address(caller);

        let account = payments.get(0);

        if self.account_token().get_token_id() == account.token_identifier {
            require!(payments.len() >= 2, ERROR_INVALID_NUMBER_OF_ESDT_TRANSFERS);

            (
                payments.slice(1, payments.len()).unwrap(),
                Some(account.clone()),
            )
        } else {
            (payments.clone(), None)
        }
    }

    fn validate_not_depracated_e_mode(&self, e_mode_category: &Option<EModeCategory<Self::Api>>) {
        if let Some(category) = e_mode_category {
            require!(!category.is_deprecated, ERROR_EMODE_CATEGORY_DEPRECATED);
        }
    }

    fn validate_e_mode_not_isolated(&self, asset_info: &AssetConfig<Self::Api>, e_mode: u8) {
        require!(
            !(asset_info.is_isolated && e_mode != 0),
            ERROR_CANNOT_USE_EMODE_WITH_ISOLATED_ASSETS
        );
    }

    fn validate_token_of_emode(
        &self,
        e_mode: u8,
        token_id: &EgldOrEsdtTokenIdentifier,
    ) -> Option<EModeAssetConfig> {
        if e_mode == 0 {
            return None;
        }

        require!(
            self.asset_e_modes(token_id).contains(&e_mode),
            ERROR_EMODE_CATEGORY_NOT_FOUND
        );
        let e_mode_mapper = self.e_mode_assets(e_mode);
        // Validate asset has configuration for this e-mode
        require!(
            e_mode_mapper.contains_key(token_id),
            ERROR_EMODE_CATEGORY_NOT_FOUND
        );

        Some(e_mode_mapper.get(token_id).unwrap())
    }

    fn validate_e_mode_exists(&self, e_mode: u8) -> Option<EModeCategory<Self::Api>> {
        if e_mode == 0 {
            return None;
        }
        let e_mode_mapper = self.e_mode_category();
        require!(
            e_mode_mapper.contains_key(&e_mode),
            ERROR_EMODE_CATEGORY_NOT_FOUND
        );
        Some(e_mode_mapper.get(&e_mode).unwrap())
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

    /// Validates isolated collateral constraints
    ///
    /// # Arguments
    /// * `account_nonce` - NFT nonce of the account position
    /// * `token_id` - Token identifier being supplied
    /// * `asset_info` - Asset configuration
    /// * `nft_attributes` - Position NFT attributes
    ///
    /// For isolated positions, ensures:
    /// - Only one collateral type is allowed
    /// - New collateral matches existing isolated collateral
    fn validate_isolated_collateral(
        &self,
        account_nonce: u64,
        token_id: &EgldOrEsdtTokenIdentifier,
        asset_info: &AssetConfig<Self::Api>,
        nft_attributes: &NftAccountAttributes,
    ) {
        if !asset_info.is_isolated && !nft_attributes.is_isolated {
            return;
        }

        // Only validate if there are existing positions
        let deposit_positions = self.deposit_positions(account_nonce);
        if !deposit_positions.is_empty() {
            let (first_token_id, _) = deposit_positions.iter().next().unwrap();
            require!(&first_token_id == token_id, ERROR_MIX_ISOLATED_COLLATERAL);
        }
    }

    /// Validates that a new borrow doesn't exceed asset borrow cap
    ///
    /// # Arguments
    /// * `asset_config` - Asset configuration
    /// * `amount` - Amount to borrow
    /// * `asset` - Token identifier
    ///
    /// # Errors
    /// * `ERROR_BORROW_CAP` - If new borrow would exceed cap
    ///
    /// ```
    fn validate_borrow_cap(
        &self,
        asset_config: &AssetConfig<Self::Api>,
        amount: &BigUint,
        asset: &EgldOrEsdtTokenIdentifier,
    ) {
        if asset_config.borrow_cap.is_some() {
            let pool = self.pools_map(asset).get();
            let borrow_cap = asset_config.borrow_cap.clone().unwrap();
            let total_borrow = self.get_total_borrow(pool).get();

            require!(total_borrow + amount <= borrow_cap, ERROR_BORROW_CAP);
        }
    }

    fn get_total_borrow(
        &self,
        pair_address: ManagedAddress,
    ) -> SingleValueMapper<BigUint, ManagedAddress> {
        SingleValueMapper::<_, _, ManagedAddress>::new_from_address(
            pair_address,
            StorageKey::new(TOTAL_BORROWED_AMOUNT_STORAGE_KEY),
        )
    }

    fn get_total_supply(
        &self,
        pair_address: ManagedAddress,
    ) -> SingleValueMapper<BigUint, ManagedAddress> {
        SingleValueMapper::<_, _, ManagedAddress>::new_from_address(
            pair_address,
            StorageKey::new(TOTAL_SUPPLY_AMOUNT_STORAGE_KEY),
        )
    }

    fn get_total_reserves(
        &self,
        pair_address: ManagedAddress,
    ) -> SingleValueMapper<BigUint, ManagedAddress> {
        SingleValueMapper::<_, _, ManagedAddress>::new_from_address(
            pair_address,
            StorageKey::new(TOTAL_RESERVES_AMOUNT_STORAGE_KEY),
        )
    }

    /// Validates supply cap constraints
    ///
    /// # Arguments
    /// * `asset_info` - Asset configuration
    /// * `amount` - Amount being supplied
    /// * `token_id` - Token identifier
    /// * `is_vault` - Whether this is a vault operation
    ///
    /// If asset has a supply cap:
    /// - Checks total supplied amount including vaults
    /// - Ensures new supply won't exceed cap
    fn validate_supply_cap(
        &self,
        asset_info: &AssetConfig<Self::Api>,
        collateral: &EgldOrEsdtTokenPayment,
        is_vault: bool,
    ) {
        // Only check supply cap if
        if asset_info.supply_cap.is_some() {
            let pool_address = self.get_pool_address(&collateral.token_identifier);
            let mut total_supplied = self.get_total_supply(pool_address).get();

            if is_vault {
                let vault_supplied_amount = self
                    .vault_supplied_amount(&collateral.token_identifier)
                    .get();
                total_supplied += vault_supplied_amount;
            }
            require!(
                total_supplied + &collateral.amount <= asset_info.supply_cap.clone().unwrap(),
                ERROR_SUPPLY_CAP
            );
        }
    }

    /// Validates health factor after withdrawal
    ///
    /// # Arguments
    /// * `account_nonce` - NFT nonce of the account position
    /// * `is_liquidation` - Whether this is a liquidation withdrawal
    /// * `egld_price_feed` - Price feed for EGLD
    ///
    /// For normal withdrawals:
    /// - Calculates new health factor
    /// - Ensures it stays above 100%
    /// Skips check for liquidation withdrawals
    fn validate_withdraw_health_factor(
        &self,
        account_nonce: u64,
        is_liquidation: bool,
        storage_cache: &mut StorageCache<Self>,
        safety_factor: Option<BigUint>,
    ) {
        if !is_liquidation {
            let borrow_positions = self.borrow_positions(account_nonce);
            let len = borrow_positions.len();
            if len == 0 {
                return;
            }
            let deposit_positions = self.deposit_positions(account_nonce);
            let (liquidation_collateral, _, _) =
                self.sum_collaterals(&deposit_positions.values().collect(), storage_cache);
            let borrowed_egld =
                self.sum_borrows(&borrow_positions.values().collect(), storage_cache);
            let health_factor = self.compute_health_factor(&liquidation_collateral, &borrowed_egld);

            // Make sure the health factor is greater than 100% when is a normal withdraw
            let health_factor_with_safety_factor = if let Some(safety_factor_value) = safety_factor
            {
                &storage_cache.bp + &(&storage_cache.bp / &safety_factor_value)
            } else {
                storage_cache.bp.clone()
            };
            require!(
                health_factor >= health_factor_with_safety_factor,
                ERROR_HEALTH_FACTOR_WITHDRAW
            );
        }
    }

    /// Validates borrow payment parameters
    ///
    /// # Arguments
    /// * `nft_token` - NFT token payment
    /// * `asset_to_borrow` - Token to borrow
    /// * `amount` - Amount to borrow
    /// * `initial_caller` - Address initiating borrow
    ///
    /// Validates:
    /// - Asset is supported
    /// - Account exists in market
    /// - NFT token is valid
    /// - Amount is greater than zero
    /// - Caller address is valid
    fn validate_borrow_account(
        &self,
        nft_token: &EsdtTokenPayment<Self::Api>,
        initial_caller: &ManagedAddress,
    ) {
        self.require_active_account(nft_token.token_nonce);
        self.account_token()
            .require_same_token(&nft_token.token_identifier);
        self.require_non_zero_address(&initial_caller);
    }

    /// Validates borrowing constraints for an asset
    ///
    /// # Arguments
    /// * `asset_config` - Asset configuration
    /// * `asset_to_borrow` - Token to borrow
    /// * `nft_attributes` - Position NFT attributes
    /// * `borrow_positions` - Current borrow positions
    ///
    /// Validates:
    /// - Asset can be borrowed in isolation mode
    /// - Asset supports e-mode category if active
    /// - Siloed borrowing constraints
    /// - Multiple asset borrowing constraints
    fn validate_borrow_asset(
        &self,
        asset_config: &AssetConfig<Self::Api>,
        asset_to_borrow: &EgldOrEsdtTokenIdentifier,
        nft_attributes: &NftAccountAttributes,
        borrow_positions: &ManagedVec<AccountPosition<Self::Api>>,
    ) {
        // Check if borrowing is allowed in isolation mode
        if nft_attributes.is_isolated {
            require!(
                asset_config.can_borrow_in_isolation,
                ERROR_ASSET_NOT_BORROWABLE_IN_ISOLATION
            );
        }

        // Validate siloed borrowing constraints
        if asset_config.is_siloed {
            require!(
                borrow_positions.len() <= 1,
                ERROR_ASSET_NOT_BORROWABLE_IN_SILOED
            );
        }

        // Check if trying to borrow a different asset when there's a siloed position
        if borrow_positions.len() == 1 {
            let first_position = borrow_positions.get(0);
            let first_asset_config = self.asset_config(&first_position.token_id).get();

            // If either the existing position or new borrow is siloed, they must be the same asset
            if first_asset_config.is_siloed || asset_config.is_siloed {
                require!(
                    asset_to_borrow == &first_position.token_id,
                    ERROR_ASSET_NOT_BORROWABLE_IN_SILOED
                );
            }
        }
    }

    /// Validates collateral sufficiency for borrow
    ///
    /// # Arguments
    /// * `ltv_collateral_in_egld` - EGLD value of collateral weighted by LTV
    /// * `borrowed_amount_in_egld` - Current EGLD value of borrows
    /// * `amount_to_borrow_in_egld` - EGLD value of new borrow
    ///
    /// Ensures sufficient collateral value (weighted by LTV)
    /// to cover existing and new borrows
    fn validate_borrow_collateral(
        &self,
        ltv_collateral_in_egld: &BigUint,
        borrowed_amount_in_egld: &BigUint,
        amount_to_borrow_in_egld: &BigUint,
    ) {
        require!(
            ltv_collateral_in_egld >= &(borrowed_amount_in_egld + amount_to_borrow_in_egld),
            ERROR_INSUFFICIENT_COLLATERAL
        );
    }

    /// Validates and calculates borrow amounts
    ///
    /// # Arguments
    /// * `asset_to_borrow` - Token to borrow
    /// * `amount` - Amount to borrow
    /// * `collateral_positions` - Current collateral positions
    /// * `borrow_positions` - Current borrow positions
    ///
    /// # Returns
    /// * `(BigUint, PriceFeedShort)` - Tuple containing:
    ///   - USD value of borrow amount
    ///   - Price feed data for borrowed asset
    ///
    /// Calculates EGLD values and validates collateral sufficiency
    fn validate_and_get_borrow_amounts(
        &self,
        ltv_collateral: &BigUint,
        asset_to_borrow: &EgldOrEsdtTokenIdentifier,
        amount: &BigUint,
        borrow_positions: &ManagedVec<AccountPosition<Self::Api>>,
        storage_cache: &mut StorageCache<Self>,
    ) -> (BigUint, PriceFeedShort<Self::Api>) {
        let asset_data_feed = self.get_token_price(asset_to_borrow, storage_cache);
        let egld_amount = self.get_token_amount_in_egld_raw(amount, &asset_data_feed);

        let egld_total_borrowed = self.sum_borrows(borrow_positions, storage_cache);

        self.validate_borrow_collateral(&ltv_collateral, &egld_total_borrowed, &egld_amount);

        let amount_in_usd =
            self.get_token_amount_in_dollars_raw(&egld_amount, &storage_cache.egld_price_feed);

        (amount_in_usd, asset_data_feed)
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
    /// ```
    fn validate_isolated_debt_ceiling(
        &self,
        asset_config: &AssetConfig<Self::Api>,
        token_id: &EgldOrEsdtTokenIdentifier,
        amount_to_borrow_in_dollars: &BigUint,
    ) {
        let current_debt = self.isolated_asset_debt_usd(token_id).get();
        let total_debt = current_debt + amount_to_borrow_in_dollars;

        require!(
            total_debt <= asset_config.debt_ceiling_usd,
            ERROR_DEBT_CEILING_REACHED
        );
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

    /// Validates borrow position exists
    ///
    /// # Arguments
    /// * `account_nonce` - NFT nonce of the account position
    /// * `borrowed_token_id` - Token being repaid
    ///
    /// # Returns
    /// * `AccountPosition` - The validated borrow position
    ///
    /// Ensures borrow position exists for the token and returns it
    fn validate_borrow_position(
        &self,
        account_nonce: u64,
        borrowed_token_id: &EgldOrEsdtTokenIdentifier,
    ) -> AccountPosition<Self::Api> {
        let borrow_positions = self.borrow_positions(account_nonce);
        let bp_opt = borrow_positions.get(borrowed_token_id);

        require!(
            bp_opt.is_some(),
            "Borrowed token {} is not available for this account",
            borrowed_token_id
        );

        bp_opt.unwrap()
    }

    /// Validates and calculates repayment amounts
    ///
    /// # Arguments
    /// * `repay_amount` - Amount being repaid
    /// * `borrow_position` - Position being repaid
    /// * `debt_token_price_data` - Price data for debt token
    /// * `repay_amount_in_egld` - Optional EGLD value of repayment
    ///
    /// # Returns
    /// * `BigUint` - EGLD value of principal being repaid
    ///
    fn validate_and_get_repay_amounts(
        &self,
        borrow_position: &AccountPosition<Self::Api>,
        debt_token_price_data: &PriceFeedShort<Self::Api>,
        amount_to_repay_in_egld: &BigUint,
    ) -> BigUint {
        let interest_egld_amount = self.get_token_amount_in_egld_raw(
            &borrow_position.accumulated_interest,
            debt_token_price_data,
        );
        let total_principal_borrowed_egld_amount =
            self.get_token_amount_in_egld_raw(&borrow_position.amount, debt_token_price_data);

        let principal_egld_amount = if amount_to_repay_in_egld > &interest_egld_amount {
            (amount_to_repay_in_egld - &interest_egld_amount)
                .min(total_principal_borrowed_egld_amount)
        } else {
            BigUint::zero()
        };

        principal_egld_amount
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

    /// Validates liquidation health factor
    ///
    /// # Arguments
    /// * `collateral_in_egld` - EGLD value of collateral
    /// * `borrowed_egld` - EGLD value of borrows
    ///
    /// # Returns
    /// * `BigUint` - Current health factor
    ///
    /// Calculates health factor and ensures it's below liquidation threshold
    fn validate_can_liquidate(
        &self,
        collateral_in_egld: &BigUint,
        borrowed_egld: &BigUint,
    ) -> BigUint {
        let health_factor = self.compute_health_factor(collateral_in_egld, borrowed_egld);
        require!(health_factor < BigUint::from(BP), ERROR_HEALTH_FACTOR);
        health_factor
    }

    /// Validates liquidation health factor remains safe
    ///
    /// # Arguments
    /// * `collateral_in_egld` - EGLD value of collateral
    /// * `borrowed_egld` - EGLD value of borrows
    ///
    /// # Returns
    /// * `BigUint` - Current health factor
    ///
    /// Calculates health factor and ensures it's above liquidation threshold
    fn validate_remain_healthy(
        &self,
        collateral_in_egld: &BigUint,
        borrowed_egld: &BigUint,
    ) -> BigUint {
        let health_factor = self.compute_health_factor(collateral_in_egld, borrowed_egld);
        require!(
            health_factor >= BigUint::from(BP),
            ERROR_HEALTH_FACTOR_BECOME_LOW
        );
        health_factor
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
}
