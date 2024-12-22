multiversx_sc::imports!();
use common_constants::{BP, TOTAL_BORROWED_AMOUNT_STORAGE_KEY, TOTAL_SUPPLY_AMOUNT_STORAGE_KEY};
use common_events::{
    AccountPosition, AssetConfig, EModeCategory, EgldOrEsdtTokenPaymentNew, NftAccountAttributes,
};
use common_structs::PriceFeedShort;
use multiversx_sc::storage::StorageKey;

use crate::{
    contexts::base::StorageCache, math, oracle, storage, utils, views,
    ERROR_ASSET_NOT_BORROWABLE_IN_ISOLATION, ERROR_ASSET_NOT_BORROWABLE_IN_SILOED,
    ERROR_BORROW_CAP, ERROR_CANNOT_USE_EMODE_WITH_ISOLATED_ASSETS, ERROR_EMODE_CATEGORY_NOT_FOUND,
    ERROR_HEALTH_FACTOR, ERROR_HEALTH_FACTOR_WITHDRAW, ERROR_INSUFFICIENT_COLLATERAL,
    ERROR_INVALID_NUMBER_OF_ESDT_TRANSFERS, ERROR_MIX_ISOLATED_COLLATERAL,
    ERROR_POSITION_SHOULD_BE_VAULT, ERROR_SUPPLY_CAP,
};

#[multiversx_sc::module]
pub trait ValidationModule:
    storage::LendingStorageModule
    + utils::LendingUtilsModule
    + common_events::EventsModule
    + math::LendingMathModule
    + oracle::OracleModule
    + views::ViewsModule
{
    /// Validates supply payment and handles NFT return
    ///
    /// # Arguments
    /// * `caller` - Address of the user supplying assets
    /// * `payments` - Vector of payments (can include NFT and collateral)
    ///
    /// # Returns
    /// * `(EgldOrEsdtTokenPaymentNew, Option<EgldOrEsdtTokenPaymentNew>)` - Tuple containing:
    ///   - Collateral payment
    ///   - Optional account NFT payment
    ///
    /// Validates:
    /// - Payment count (1 or 2 payments allowed)
    /// - Asset is supported
    /// - Amount is greater than zero
    /// - Caller is valid
    /// Returns NFT to owner if present
    fn validate_supply_payment(
        &self,
        caller: &ManagedAddress,
        payments: &ManagedVec<EgldOrEsdtTokenPaymentNew<Self::Api>>,
    ) -> (
        EgldOrEsdtTokenPaymentNew<Self::Api>,
        Option<EgldOrEsdtTokenPaymentNew<Self::Api>>,
    ) {
        require!(
            payments.len() == 2 || payments.len() == 1,
            ERROR_INVALID_NUMBER_OF_ESDT_TRANSFERS
        );

        let collateral_payment = payments.get(payments.len() - 1).clone();

        // Validate the collateral payment token
        self.require_asset_supported(&collateral_payment.token_identifier);
        self.require_amount_greater_than_zero(&collateral_payment.amount);
        self.require_non_zero_address(caller);

        let account_token = if payments.len() == 2 {
            let token = payments.get(0);
            let token_id = token.token_identifier.clone().unwrap_esdt();
            self.account_token().require_same_token(&token_id);

            Some(token)
        } else {
            None
        };

        (collateral_payment, account_token)
    }

    /// Validates e-mode constraints for a position
    ///
    /// # Arguments
    /// * `token_id` - Token identifier being supplied
    /// * `asset_info` - Asset configuration
    /// * `nft_attributes` - Position NFT attributes
    /// * `e_mode_category` - Optional e-mode category to use
    ///
    /// Validates:
    /// - Isolated assets cannot use e-mode
    /// - E-mode category exists if specified
    /// - Asset supports the e-mode category
    fn validate_e_mode_constraints(
        &self,
        token_id: &EgldOrEsdtTokenIdentifier,
        asset_info: &AssetConfig<Self::Api>,
        nft_attributes: &NftAccountAttributes,
    ) -> Option<EModeCategory<Self::Api>> {
        // 1. Validate isolated asset constraints
        require!(
            !(asset_info.is_isolated && nft_attributes.e_mode_category != 0),
            ERROR_CANNOT_USE_EMODE_WITH_ISOLATED_ASSETS
        );

        // 3. Validate e-mode if active
        if nft_attributes.e_mode_category != 0 {
            // Validate category exists
            require!(
                self.e_mode_category()
                    .contains_key(&nft_attributes.e_mode_category),
                ERROR_EMODE_CATEGORY_NOT_FOUND
            );

            let category = self
                .e_mode_category()
                .get(&nft_attributes.e_mode_category)
                .unwrap();

            if category.is_deprecated {
                return Some(category);
            }

            // Validate asset supports this e-mode
            require!(
                self.asset_e_modes(token_id)
                    .contains(&nft_attributes.e_mode_category),
                ERROR_EMODE_CATEGORY_NOT_FOUND
            );

            // Validate asset has configuration for this e-mode
            require!(
                self.e_mode_assets(nft_attributes.e_mode_category)
                    .contains_key(token_id),
                ERROR_EMODE_CATEGORY_NOT_FOUND
            );

            return Some(category);
        } else {
            None
        }
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
    /// # Example
    /// ```
    /// // Asset: EGLD
    /// // Current borrows: 900 EGLD
    /// // Borrow cap: 1000 EGLD
    /// // New borrow: 150 EGLD
    /// // Result: Error - cap exceeded
    /// ```
    fn check_borrow_cap(
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
    fn check_supply_cap(
        &self,
        asset_info: &AssetConfig<Self::Api>,
        amount: &BigUint,
        token_id: &EgldOrEsdtTokenIdentifier,
        is_vault: bool,
    ) {
        // Only check supply cap if
        if asset_info.supply_cap.is_some() {
            let pool_address = self.get_pool_address(token_id);
            let mut total_supplied = self.get_total_supply(pool_address).get();

            if is_vault {
                let vault_supplied_amount = self.vault_supplied_amount(token_id).get();
                total_supplied += vault_supplied_amount;
            }
            require!(
                total_supplied + amount <= asset_info.supply_cap.clone().unwrap(),
                ERROR_SUPPLY_CAP
            );
        }
    }

    /// Validates withdrawal payment parameters
    ///
    /// # Arguments
    /// * `account_token` - NFT token identifier
    /// * `withdraw_token_id` - Token to withdraw
    /// * `amount` - Amount to withdraw
    /// * `initial_caller` - Address initiating withdrawal
    ///
    /// Validates:
    /// - Account token is valid
    /// - Caller address is valid
    /// - Asset is supported
    /// - Amount is greater than zero
    fn validate_withdraw_payment(
        &self,
        account_token: &TokenIdentifier<Self::Api>,
        withdraw_token_id: &EgldOrEsdtTokenIdentifier,
        amount: &BigUint,
        initial_caller: &ManagedAddress,
    ) {
        self.account_token().require_same_token(account_token);
        self.require_non_zero_address(initial_caller);
        self.require_asset_supported(withdraw_token_id);
        self.require_amount_greater_than_zero(amount);
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
            let collateral_in_egld = self.get_liquidation_collateral_in_egld_vec(
                &deposit_positions.values().collect(),
                storage_cache,
            );
            let borrowed_egld = self
                .get_total_borrow_in_egld_vec(&borrow_positions.values().collect(), storage_cache);
            let health_factor = self.compute_health_factor(&collateral_in_egld, &borrowed_egld);

            // Make sure the health factor is greater than 100% when is a normal withdraw
            let health_factor_with_safety_factor = if let Some(safety_factor_value) = safety_factor
            {
                BigUint::from(BP) + (BigUint::from(BP) / safety_factor_value)
            } else {
                BigUint::from(BP)
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
    fn validate_borrow_payment(
        &self,
        nft_token: &EsdtTokenPayment<Self::Api>,
        asset_to_borrow: &EgldOrEsdtTokenIdentifier,
        amount: &BigUint,
        initial_caller: &ManagedAddress,
    ) {
        self.require_asset_supported(asset_to_borrow);
        self.lending_account_in_the_market(nft_token.token_nonce);
        self.account_token()
            .require_same_token(&nft_token.token_identifier);
        self.require_amount_greater_than_zero(amount);
        self.require_non_zero_address(initial_caller);
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

        // Handle e-mode validation
        if nft_attributes.e_mode_category != 0 {
            require!(
                self.asset_e_modes(asset_to_borrow)
                    .contains(&nft_attributes.e_mode_category),
                ERROR_EMODE_CATEGORY_NOT_FOUND
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
            ltv_collateral_in_egld > &(borrowed_amount_in_egld + amount_to_borrow_in_egld),
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
    ///   - EGLD value of borrow amount
    ///   - Price feed data for borrowed asset
    ///
    /// Calculates EGLD values and validates collateral sufficiency
    fn validate_and_get_borrow_amounts(
        &self,
        asset_to_borrow: &EgldOrEsdtTokenIdentifier,
        amount: &BigUint,
        collateral_positions: &ManagedVec<AccountPosition<Self::Api>>,
        borrow_positions: &ManagedVec<AccountPosition<Self::Api>>,
        storage_cache: &mut StorageCache<Self>,
    ) -> (BigUint, PriceFeedShort<Self::Api>) {
        let asset_data_feed = self.get_token_price_data(asset_to_borrow, storage_cache);
        let amount_to_borrow_in_egld = self.get_token_amount_in_egld_raw(amount, &asset_data_feed);
        let ltv_collateral_in_egld =
            self.get_ltv_collateral_in_egld_vec(collateral_positions, storage_cache);

        let borrowed_amount_in_egld =
            self.get_total_borrow_in_egld_vec(borrow_positions, storage_cache);

        self.validate_borrow_collateral(
            &ltv_collateral_in_egld,
            &borrowed_amount_in_egld,
            &amount_to_borrow_in_egld,
        );

        (amount_to_borrow_in_egld, asset_data_feed)
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
    fn validate_repay_payment(
        &self,
        repay_token_id: &EgldOrEsdtTokenIdentifier,
        repay_amount: &BigUint,
        account_nonce: u64,
    ) {
        self.lending_account_in_the_market(account_nonce);
        self.require_asset_supported(repay_token_id);
        self.require_amount_greater_than_zero(repay_amount);
    }

    /// Validates repay position exists
    ///
    /// # Arguments
    /// * `account_nonce` - NFT nonce of the account position
    /// * `repay_token_id` - Token being repaid
    ///
    /// # Returns
    /// * `AccountPosition` - The validated borrow position
    ///
    /// Ensures borrow position exists for the token and returns it
    fn validate_repay_position(
        &self,
        account_nonce: u64,
        repay_token_id: &EgldOrEsdtTokenIdentifier,
    ) -> AccountPosition<Self::Api> {
        let borrow_positions = self.borrow_positions(account_nonce);
        let bp_opt = borrow_positions.get(repay_token_id);

        require!(
            bp_opt.is_some(),
            "Borrowed token {} is not available for this account",
            repay_token_id
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
    /// Calculates:
    /// - EGLD value of repayment
    /// - Interest portion
    /// - Principal portion
    /// Handles partial repayments
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
            BigUint::from(0u64)
        };

        principal_egld_amount
    }

    /// Validates liquidation payment parameters
    ///
    /// # Arguments
    /// * `debt_payment` - Payment to cover debt
    /// * `collateral_to_receive` - Collateral token to receive
    /// * `initial_caller` - Address initiating liquidation
    ///
    /// Validates:
    /// - Both assets are supported
    /// - Payment amount is greater than zero
    /// - Caller address is valid
    fn validate_liquidation_payment(
        &self,
        debt_payment: &EgldOrEsdtTokenPayment<Self::Api>,
        collateral_to_receive: &EgldOrEsdtTokenIdentifier,
        initial_caller: &ManagedAddress,
    ) {
        self.require_asset_supported(&debt_payment.token_identifier);
        self.require_asset_supported(collateral_to_receive);
        self.require_amount_greater_than_zero(&debt_payment.amount);
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
    fn validate_liquidation_health_factor(
        &self,
        collateral_in_egld: &BigUint,
        borrowed_egld: &BigUint,
    ) -> BigUint {
        let health_factor = self.compute_health_factor(collateral_in_egld, borrowed_egld);
        require!(health_factor < BigUint::from(BP), ERROR_HEALTH_FACTOR);
        health_factor
    }
}
