use common_constants::{RAY, TOTAL_BORROWED_AMOUNT_STORAGE_KEY};
use common_events::{
    AccountPosition, AccountPositionType, AssetConfig, NftAccountAttributes, PriceFeedShort,
    RAY_PRECISION,
};
use multiversx_sc::storage::StorageKey;

use crate::{
    contexts::base::StorageCache, helpers, oracle, proxy_pool, storage, utils, validation,
    ERROR_ASSET_NOT_BORROWABLE_IN_ISOLATION, ERROR_ASSET_NOT_BORROWABLE_IN_SILOED,
    ERROR_BORROW_CAP, ERROR_DEBT_CEILING_REACHED, ERROR_INSUFFICIENT_COLLATERAL,
};

use super::account;

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

#[multiversx_sc::module]
pub trait PositionBorrowModule:
    storage::LendingStorageModule
    + validation::ValidationModule
    + oracle::OracleModule
    + common_events::EventsModule
    + utils::LendingUtilsModule
    + helpers::math::MathsModule
    + account::PositionAccountModule
    + common_math::SharedMathModule
{
    /// Processes borrow operation
    ///
    /// # Arguments
    /// * `account_nonce` - NFT nonce of the account position
    /// * `asset_to_borrow` - Token to borrow
    /// * `amount` - Amount to borrow
    /// * `amount_in_usd` - USD value of borrow
    /// * `caller` - Address initiating borrow
    /// * `asset_config` - Asset configuration
    /// * `account` - Position NFT attributes
    /// * `collaterals` - Current collateral positions
    /// * `feed` - Price data for the asset being borrowed
    ///
    /// # Returns
    /// * `AccountPosition` - Updated borrow position
    ///
    /// Creates or updates borrow position through liquidity pool.
    /// Handles isolated mode debt ceiling checks.
    /// Updates storage with new position.
    fn handle_borrow_position(
        &self,
        account_nonce: u64,
        asset_to_borrow: &EgldOrEsdtTokenIdentifier,
        amount: ManagedDecimal<Self::Api, NumDecimals>,
        amount_in_usd: ManagedDecimal<Self::Api, NumDecimals>,
        caller: &ManagedAddress,
        asset_config: &AssetConfig<Self::Api>,
        account: &NftAccountAttributes,
        collaterals: &ManagedVec<AccountPosition<Self::Api>>,
        feed: &PriceFeedShort<Self::Api>,
    ) -> AccountPosition<Self::Api> {
        let pool_address = self.get_pool_address(asset_to_borrow);

        // Get or create borrow position
        let mut borrow_position = self.get_or_create_borrow_position(
            account_nonce,
            asset_config,
            asset_to_borrow,
            account.is_vault,
        );

        // Execute borrow
        borrow_position = self
            .tx()
            .to(pool_address)
            .typed(proxy_pool::LiquidityPoolProxy)
            .borrow(caller, amount, borrow_position, feed.price.clone())
            .returns(ReturnsResult)
            .sync_call();

        // Handle isolated mode debt ceiling
        if account.is_isolated {
            let collateral_token_id = &collaterals.get(0).token_id;
            let collateral_config = self.asset_config(collateral_token_id).get();

            self.validate_isolated_debt_ceiling(
                &collateral_config,
                collateral_token_id,
                amount_in_usd.clone(),
            );
            self.update_isolated_debt_usd(
                collateral_token_id,
                amount_in_usd,
                true, // is_increase
            );
        }

        // Update storage
        self.borrow_positions(account_nonce)
            .insert(asset_to_borrow.clone(), borrow_position.clone());

        borrow_position
    }

    /// Gets existing borrow position or creates new one
    ///
    /// # Arguments
    /// * `account_nonce` - NFT nonce of the account position
    /// * `asset_info` - Asset configuration for the borrowed token
    /// * `token_id` - Token identifier of the borrowed asset
    /// * `is_vault` - Whether this is a vault position
    ///
    /// # Returns
    /// * `AccountPosition` - The existing or new borrow position
    ///
    /// If a borrow position exists for the token, returns it.
    /// Otherwise creates a new position with zero balance and default parameters.
    /// Used in both normal borrowing and liquidation flows.
    fn get_or_create_borrow_position(
        &self,
        account_nonce: u64,
        asset_info: &AssetConfig<Self::Api>,
        token_id: &EgldOrEsdtTokenIdentifier,
        is_vault: bool,
    ) -> AccountPosition<Self::Api> {
        let mut borrow_positions = self.borrow_positions(account_nonce);

        if let Some(position) = borrow_positions.get(token_id) {
            borrow_positions.remove(token_id);
            position
        } else {
            let price_data = self.token_oracle(token_id).get();
            AccountPosition::new(
                AccountPositionType::Borrow,
                token_id.clone(),
                ManagedDecimal::from_raw_units(BigUint::zero(), price_data.decimals as usize),
                ManagedDecimal::from_raw_units(BigUint::zero(), price_data.decimals as usize),
                account_nonce,
                self.blockchain().get_block_timestamp(),
                ManagedDecimal::from_raw_units(BigUint::from(RAY), RAY_PRECISION),
                asset_info.liquidation_threshold.clone(),
                asset_info.liquidation_base_bonus.clone(),
                asset_info.liquidation_max_fee.clone(),
                asset_info.ltv.clone(),
                is_vault,
            )
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
        amount: &ManagedDecimal<Self::Api, NumDecimals>,
        asset: &EgldOrEsdtTokenIdentifier,
    ) {
        if asset_config.borrow_cap.is_some() {
            let pool = self.pools_map(asset).get();
            let borrow_cap = asset_config.borrow_cap.clone().unwrap();
            let total_borrow = self.get_total_borrow(pool).get();

            require!(
                total_borrow.clone() + amount.clone()
                    <= ManagedDecimal::from_raw_units(borrow_cap, total_borrow.scale()),
                ERROR_BORROW_CAP
            );
        }
    }

    fn get_total_borrow(
        &self,
        pair_address: ManagedAddress,
    ) -> SingleValueMapper<ManagedDecimal<Self::Api, NumDecimals>, ManagedAddress> {
        SingleValueMapper::<_, _, ManagedAddress>::new_from_address(
            pair_address,
            StorageKey::new(TOTAL_BORROWED_AMOUNT_STORAGE_KEY),
        )
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
        ltv_collateral_in_egld: &ManagedDecimal<Self::Api, NumDecimals>,
        borrowed_amount_in_egld: &ManagedDecimal<Self::Api, NumDecimals>,
        amount_to_borrow_in_egld: &ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        require!(
            ltv_collateral_in_egld
                >= &(borrowed_amount_in_egld.clone() + amount_to_borrow_in_egld.clone()),
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
        ltv_collateral: &ManagedDecimal<Self::Api, NumDecimals>,
        asset_to_borrow: &EgldOrEsdtTokenIdentifier,
        amount_raw: &BigUint,
        borrow_positions: &ManagedVec<AccountPosition<Self::Api>>,
        storage_cache: &mut StorageCache<Self>,
    ) -> (
        ManagedDecimal<Self::Api, NumDecimals>,
        PriceFeedShort<Self::Api>,
        ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        let asset_data_feed = self.get_token_price(asset_to_borrow, storage_cache);
        let amount =
            ManagedDecimal::from_raw_units(amount_raw.clone(), asset_data_feed.decimals as usize);

        let egld_amount = self.get_token_amount_in_egld_raw(&amount, &asset_data_feed.price);

        let egld_total_borrowed = self.sum_borrows(borrow_positions, storage_cache);

        self.validate_borrow_collateral(ltv_collateral, &egld_total_borrowed, &egld_amount);

        let amount_in_usd = self
            .get_token_amount_in_dollars_raw(&egld_amount, &storage_cache.egld_price_feed);

        (amount_in_usd, asset_data_feed, amount)
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
        amount_to_borrow_in_dollars: ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        let current_debt = self.isolated_asset_debt_usd(token_id).get();
        let total_debt = current_debt + amount_to_borrow_in_dollars;

        require!(
            total_debt <= asset_config.debt_ceiling_usd,
            ERROR_DEBT_CEILING_REACHED
        );
    }
}
