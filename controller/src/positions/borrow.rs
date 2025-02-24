use common_constants::TOTAL_BORROWED_AMOUNT_STORAGE_KEY;
use common_events::{
    AccountPosition, AccountPositionType, AssetConfig, NftAccountAttributes, PriceFeedShort,
};
use multiversx_sc::storage::StorageKey;

use crate::{
    contexts::base::StorageCache, helpers, oracle, proxy_pool, storage, utils, validation,
};
use common_errors::{
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
    fn handle_borrow_position(
        &self,
        account_nonce: u64,
        borrow_token_id: &EgldOrEsdtTokenIdentifier,
        amount: ManagedDecimal<Self::Api, NumDecimals>,
        amount_in_usd: ManagedDecimal<Self::Api, NumDecimals>,
        caller: &ManagedAddress,
        asset_config: &AssetConfig<Self::Api>,
        account: &NftAccountAttributes,
        collaterals: &ManagedVec<AccountPosition<Self::Api>>,
        feed: &PriceFeedShort<Self::Api>,
        storage_cache: &mut StorageCache<Self>,
    ) -> AccountPosition<Self::Api> {
        let pool_address = storage_cache.get_cached_pool_address(borrow_token_id);
        let mut borrow_position = self.get_or_create_borrow_position(
            account_nonce,
            asset_config,
            borrow_token_id,
            account.is_vault,
        );

        borrow_position = self.execute_borrow(
            pool_address,
            caller,
            amount,
            borrow_position,
            feed.price.clone(),
        );

        if account.is_isolated {
            self.handle_isolated_debt(collaterals, storage_cache, amount_in_usd.clone());
        }

        self.store_borrow_position(account_nonce, borrow_token_id, &borrow_position);
        borrow_position
    }

    fn execute_borrow(
        &self,
        pool_address: ManagedAddress,
        caller: &ManagedAddress,
        amount: ManagedDecimal<Self::Api, NumDecimals>,
        position: AccountPosition<Self::Api>,
        price: ManagedDecimal<Self::Api, NumDecimals>,
    ) -> AccountPosition<Self::Api> {
        self.tx()
            .to(pool_address)
            .typed(proxy_pool::LiquidityPoolProxy)
            .borrow(caller, amount, position, price)
            .returns(ReturnsResult)
            .sync_call()
    }

    fn store_borrow_position(
        &self,
        account_nonce: u64,
        borrow_token_id: &EgldOrEsdtTokenIdentifier,
        position: &AccountPosition<Self::Api>,
    ) {
        self.borrow_positions(account_nonce)
            .insert(borrow_token_id.clone(), position.clone());
    }

    fn handle_isolated_debt(
        &self,
        collaterals: &ManagedVec<AccountPosition<Self::Api>>,
        storage_cache: &mut StorageCache<Self>,
        amount_in_usd: ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        let collateral_token_id = &collaterals.get(0).token_id;
        let collateral_config = storage_cache.get_cached_asset_info(collateral_token_id);
        self.validate_isolated_debt_ceiling(
            &collateral_config,
            collateral_token_id,
            amount_in_usd.clone(),
        );
        self.update_isolated_debt_usd(collateral_token_id, amount_in_usd, true);
    }

    fn get_or_create_borrow_position(
        &self,
        account_nonce: u64,
        borrow_asset_config: &AssetConfig<Self::Api>,
        token_id: &EgldOrEsdtTokenIdentifier,
        is_vault: bool,
    ) -> AccountPosition<Self::Api> {
        let borrow_positions = self.borrow_positions(account_nonce);
        borrow_positions.get(token_id).unwrap_or_else(|| {
            let price_data = self.token_oracle(token_id).get();
            AccountPosition::new(
                AccountPositionType::Borrow,
                token_id.clone(),
                ManagedDecimal::from_raw_units(BigUint::zero(), price_data.decimals),
                ManagedDecimal::from_raw_units(BigUint::zero(), price_data.decimals),
                account_nonce,
                self.blockchain().get_block_timestamp(),
                self.ray(),
                borrow_asset_config.liquidation_threshold.clone(),
                borrow_asset_config.liquidation_base_bonus.clone(),
                borrow_asset_config.liquidation_max_fee.clone(),
                borrow_asset_config.ltv.clone(),
                is_vault,
            )
        })
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
        position_nft_payment: &EsdtTokenPayment<Self::Api>,
        initial_caller: &ManagedAddress,
    ) {
        self.require_active_account(position_nft_payment.token_nonce);
        self.account_token()
            .require_same_token(&position_nft_payment.token_identifier);
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
        if let Some(borrow_cap) = &asset_config.borrow_cap {
            let pool = self.pools_map(asset).get();
            let total_borrow = self.get_total_borrow(pool).get();

            require!(
                total_borrow.clone() + amount.clone()
                    <= ManagedDecimal::from_raw_units(borrow_cap.clone(), total_borrow.scale()),
                ERROR_BORROW_CAP
            );
        }
    }

    fn get_total_borrow(
        &self,
        pool_address: ManagedAddress,
    ) -> SingleValueMapper<ManagedDecimal<Self::Api, NumDecimals>, ManagedAddress> {
        SingleValueMapper::<_, _, ManagedAddress>::new_from_address(
            pool_address,
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
        ltv_base_amount: &ManagedDecimal<Self::Api, NumDecimals>,
        borrowed_amount: &ManagedDecimal<Self::Api, NumDecimals>,
        amount_to_borrow: &ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        require!(
            ltv_base_amount >= &(borrowed_amount.clone() + amount_to_borrow.clone()),
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
        ltv_base_amount: &ManagedDecimal<Self::Api, NumDecimals>,
        borrow_token_id: &EgldOrEsdtTokenIdentifier,
        amount_raw: &BigUint,
        borrow_positions: &ManagedVec<AccountPosition<Self::Api>>,
        storage_cache: &mut StorageCache<Self>,
    ) -> (
        ManagedDecimal<Self::Api, NumDecimals>,
        PriceFeedShort<Self::Api>,
        ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        let asset_data_feed = self.get_token_price(borrow_token_id, storage_cache);
        let amount =
            ManagedDecimal::from_raw_units(amount_raw.clone(), asset_data_feed.decimals as usize);

        let egld_amount = self.get_token_amount_in_egld_raw(&amount, &asset_data_feed.price);

        let egld_total_borrowed = self.sum_borrows(borrow_positions, storage_cache);

        self.validate_borrow_collateral(ltv_base_amount, &egld_total_borrowed, &egld_amount);

        let amount_in_usd =
            self.get_token_amount_in_dollars_raw(&egld_amount, &storage_cache.egld_price_feed);

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
        borrow_token_id: &EgldOrEsdtTokenIdentifier,
        nft_attributes: &NftAccountAttributes,
        borrow_positions: &ManagedVec<AccountPosition<Self::Api>>,
        storage_cache: &mut StorageCache<Self>,
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
            let first_asset_config = storage_cache.get_cached_asset_info(&first_position.token_id);

            // If either the existing position or new borrow is siloed, they must be the same asset
            if first_asset_config.is_siloed || asset_config.is_siloed {
                require!(
                    borrow_token_id == &first_position.token_id,
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
        borrow_token_id: &EgldOrEsdtTokenIdentifier,
    ) -> AccountPosition<Self::Api> {
        let borrow_positions = self.borrow_positions(account_nonce);
        let bp_opt = borrow_positions.get(borrow_token_id);

        require!(
            bp_opt.is_some(),
            "Borrowed token {} is not available for this account",
            borrow_token_id
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
