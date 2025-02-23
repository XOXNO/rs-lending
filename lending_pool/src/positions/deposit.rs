use common_constants::TOTAL_SUPPLY_AMOUNT_STORAGE_KEY;
use common_events::{
    AccountPosition, AccountPositionType, AssetConfig, NftAccountAttributes, PriceFeedShort,
};
use multiversx_sc::storage::StorageKey;

use crate::{
    contexts::base::StorageCache, helpers, oracle, positions, proxy_pool, storage, utils,
    validation, ERROR_ASSET_NOT_SUPPORTED_AS_COLLATERAL, ERROR_INVALID_NUMBER_OF_ESDT_TRANSFERS,
    ERROR_MIX_ISOLATED_COLLATERAL, ERROR_SUPPLY_CAP,
};

use super::account;

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

#[multiversx_sc::module]
pub trait PositionDepositModule:
    storage::LendingStorageModule
    + validation::ValidationModule
    + oracle::OracleModule
    + common_events::EventsModule
    + utils::LendingUtilsModule
    + helpers::math::MathsModule
    + account::PositionAccountModule
    + positions::emode::EModeModule
    + common_math::SharedMathModule
{
    fn internal_supply(
        &self,
        caller: &ManagedAddress,
        account_nonce: u64,
        attributes: NftAccountAttributes,
        collaterals: &ManagedVec<EgldOrEsdtTokenPayment>,
        storage_cache: &mut StorageCache<Self>,
    ) {
        let e_mode = self.validate_e_mode_exists(attributes.get_emode_id());
        self.validate_not_depracated_e_mode(&e_mode);

        for collateral in collaterals {
            self.validate_payment(&collateral);
            let mut asset_info = storage_cache.get_cached_asset_info(&collateral.token_identifier);

            let asset_emode_config = self
                .validate_token_of_emode(attributes.get_emode_id(), &collateral.token_identifier);

            self.validate_e_mode_not_isolated(&asset_info, attributes.get_emode_id());

            self.update_asset_config_for_e_mode(&mut asset_info, &e_mode, asset_emode_config);

            require!(
                asset_info.can_supply(),
                ERROR_ASSET_NOT_SUPPORTED_AS_COLLATERAL
            );

            self.validate_isolated_collateral(
                account_nonce,
                &collateral.token_identifier,
                &asset_info,
                &attributes,
            );

            self.validate_supply_cap(
                &asset_info,
                &collateral,
                attributes.is_vault(),
                storage_cache,
            );

            self.update_supply_position(
                account_nonce,
                &collateral,
                &asset_info,
                &caller,
                &attributes,
                storage_cache,
            );
        }
    }

    /// Retrieves existing deposit position or creates new one
    ///
    /// # Arguments
    /// * `account_nonce` - NFT nonce of the account position
    /// * `asset_info` - Configuration of the asset being deposited
    /// * `token_id` - Token identifier of the deposit
    /// * `is_vault` - Whether this is a vault position
    ///
    /// # Returns
    /// * `AccountPosition` - The existing or new deposit position
    ///
    /// If a position exists for the token, returns it.
    /// Otherwise creates a new position with zero balance and default parameters.
    fn get_or_create_deposit_position(
        &self,
        account_nonce: u64,
        asset_info: &AssetConfig<Self::Api>,
        token_id: &EgldOrEsdtTokenIdentifier,
        is_vault: bool,
    ) -> AccountPosition<Self::Api> {
        let mut positions = self.deposit_positions(account_nonce);

        if let Some(position) = positions.get(token_id) {
            positions.remove(token_id);
            position
        } else {
            let data = self.token_oracle(token_id).get();
            AccountPosition::new(
                AccountPositionType::Deposit,
                token_id.clone(),
                ManagedDecimal::from_raw_units(BigUint::zero(), data.decimals as usize),
                ManagedDecimal::from_raw_units(BigUint::zero(), data.decimals as usize),
                account_nonce,
                self.blockchain().get_block_timestamp(),
                self.ray(),
                asset_info.liquidation_threshold.clone(),
                asset_info.liquidation_base_bonus.clone(),
                asset_info.liquidation_max_fee.clone(),
                asset_info.ltv.clone(),
                is_vault,
            )
        }
    }

    /// Updates supply position with new deposit
    ///
    /// # Arguments
    /// * `account_nonce` - NFT nonce of the account position
    /// * `token_id` - Token identifier of the deposit
    /// * `amount` - Amount being deposited
    /// * `asset_info` - Configuration of the asset
    /// * `is_vault` - Whether this is a vault position
    ///
    /// # Returns
    /// * `AccountPosition` - The updated position after deposit
    ///
    /// For vault positions, directly updates storage.
    /// For market positions, calls liquidity pool to handle deposit.
    /// Updates position storage and returns updated position.
    fn update_supply_position(
        &self,
        account_nonce: u64,
        collateral: &EgldOrEsdtTokenPayment<Self::Api>,
        asset_info: &AssetConfig<Self::Api>,
        caller: &ManagedAddress,
        attributes: &NftAccountAttributes,
        storage_cache: &mut StorageCache<Self>,
    ) -> AccountPosition<Self::Api> {
        let feed = self.get_token_price(&collateral.token_identifier, storage_cache);
        let mut position = self.get_or_create_deposit_position(
            account_nonce,
            asset_info,
            &collateral.token_identifier,
            attributes.is_vault(),
        );

        // Auto upgrade values when changed on demand
        if position.entry_ltv != asset_info.ltv {
            position.entry_ltv = asset_info.ltv.clone();
        }

        if position.entry_liquidation_bonus != asset_info.liquidation_base_bonus {
            position.entry_liquidation_bonus = asset_info.liquidation_base_bonus.clone();
        }

        if position.entry_liquidation_fees != asset_info.liquidation_max_fee {
            position.entry_liquidation_fees = asset_info.liquidation_max_fee.clone();
        }
        let amount_decimal = position.make_amount_decimal(collateral.amount.clone());
        if attributes.is_vault() {
            self.increase_vault_position(
                &mut position,
                &amount_decimal,
                &collateral.token_identifier,
            );
        } else {
            self.update_market_position(
                &mut position,
                &collateral.amount,
                &collateral.token_identifier,
                &feed,
            );
        }

        self.update_position_event(
            &amount_decimal,
            &position,
            OptionalValue::Some(feed.price.clone()),
            OptionalValue::Some(caller),
            OptionalValue::Some(attributes),
        );

        // Update storage with the latest position
        self.deposit_positions(account_nonce)
            .insert(collateral.token_identifier.clone(), position.clone());

        position
    }

    /// Updates market position through liquidity pool
    ///
    /// # Arguments
    /// * `position` - Current position to update
    /// * `amount` - Amount being deposited
    /// * `token_id` - Token identifier
    ///
    /// Calls liquidity pool to handle deposit, update interest indices,
    /// and return updated position. Used for non-vault positions.
    fn update_market_position(
        &self,
        position: &mut AccountPosition<Self::Api>,
        amount: &BigUint,
        token_id: &EgldOrEsdtTokenIdentifier,
        feed: &PriceFeedShort<Self::Api>,
    ) {
        let pool_address = self.get_pool_address(token_id);

        *position = self
            .tx()
            .to(pool_address)
            .typed(proxy_pool::LiquidityPoolProxy)
            .supply(position.clone(), feed.price.clone())
            .egld_or_single_esdt(token_id, 0, amount)
            .returns(ReturnsResult)
            .sync_call();
    }

    /// Increase vault position directly in storage
    ///
    /// # Arguments
    /// * `position` - Current position to update
    /// * `amount` - Amount being deposited
    /// * `token_id` - Token identifier
    ///
    /// Increase vault supplied amount in storage and position balance.
    /// Used for vault positions that don't accrue interest.
    /// Emits event for tracking vault deposits.
    fn increase_vault_position(
        &self,
        position: &mut AccountPosition<Self::Api>,
        amount: &ManagedDecimal<Self::Api, NumDecimals>,
        token_id: &EgldOrEsdtTokenIdentifier,
    ) {
        let last_value = self.vault_supplied_amount(token_id).update(|am| {
            *am += amount;
            am.clone()
        });

        self.update_vault_supplied_amount_event(token_id, last_value.clone());
        position.amount += amount;
    }

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
        Option<EsdtTokenPayment<Self::Api>>,
    ) {
        require!(payments.len() >= 1, ERROR_INVALID_NUMBER_OF_ESDT_TRANSFERS);

        // Validate the collateral payment token
        self.require_non_zero_address(caller);

        let account = payments.get(0);

        if self.account_token().get_token_id() == account.token_identifier {
            require!(payments.len() >= 2, ERROR_INVALID_NUMBER_OF_ESDT_TRANSFERS);
            self.require_active_account(account.token_nonce);

            (
                payments.slice(1, payments.len()).unwrap(),
                Some(account.clone().unwrap_esdt()),
            )
        } else {
            (payments.clone(), None)
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

    fn get_total_supply(
        &self,
        pair_address: ManagedAddress,
    ) -> SingleValueMapper<ManagedDecimal<Self::Api, NumDecimals>, ManagedAddress> {
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
    fn validate_supply_cap(
        &self,
        asset_info: &AssetConfig<Self::Api>,
        collateral: &EgldOrEsdtTokenPayment,
        is_vault: bool,
        storage_cache: &mut StorageCache<Self>,
    ) {
        // Only check supply cap if
        if asset_info.supply_cap.is_some() {
            let pool_address = storage_cache.get_cached_pool_address(&collateral.token_identifier);
            let mut total_supplied = self.get_total_supply(pool_address).get();

            if is_vault {
                let vault_supplied_amount = self
                    .vault_supplied_amount(&collateral.token_identifier)
                    .get();
                total_supplied += vault_supplied_amount;
            }
            require!(
                total_supplied.into_raw_units() + &collateral.amount
                    <= asset_info.supply_cap.clone().unwrap(),
                ERROR_SUPPLY_CAP
            );
        }
    }
}
