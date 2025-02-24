use common_constants::TOTAL_SUPPLY_AMOUNT_STORAGE_KEY;
use common_events::{
    AccountPosition, AccountPositionType, AssetConfig, NftAccountAttributes, PriceFeedShort,
};
use multiversx_sc::storage::StorageKey;

use crate::{
    contexts::base::StorageCache, helpers, oracle, positions, proxy_pool, storage, utils,
    validation,
};
use common_errors::{
    ERROR_ASSET_NOT_SUPPORTED_AS_COLLATERAL, ERROR_INVALID_NUMBER_OF_ESDT_TRANSFERS,
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
    /// Processes a deposit operation for a user's position.
    /// Handles validations, e-mode, and updates for deposits.
    ///
    /// # Arguments
    /// - `caller`: Depositor's address.
    /// - `account_nonce`: Position NFT nonce.
    /// - `position_attributes`: NFT attributes.
    /// - `deposit_payments`: Vector of deposit payments.
    /// - `storage_cache`: Mutable storage cache.
    fn process_deposit(
        &self,
        caller: &ManagedAddress,
        account_nonce: u64,
        position_attributes: NftAccountAttributes,
        deposit_payments: &ManagedVec<EgldOrEsdtTokenPayment>,
        storage_cache: &mut StorageCache<Self>,
    ) {
        let e_mode = self.get_e_mode_category(position_attributes.get_emode_id());
        self.ensure_e_mode_not_deprecated(&e_mode);

        for deposit_payment in deposit_payments {
            self.validate_payment(&deposit_payment);
            let mut asset_info =
                storage_cache.get_cached_asset_info(&deposit_payment.token_identifier);

            let asset_emode_config = self.get_token_e_mode_config(
                position_attributes.get_emode_id(),
                &deposit_payment.token_identifier,
            );

            self.ensure_e_mode_compatible_with_asset(
                &asset_info,
                position_attributes.get_emode_id(),
            );

            self.apply_e_mode_to_asset_config(&mut asset_info, &e_mode, asset_emode_config);

            require!(
                asset_info.can_supply(),
                ERROR_ASSET_NOT_SUPPORTED_AS_COLLATERAL
            );

            self.validate_isolated_collateral(
                account_nonce,
                &deposit_payment.token_identifier,
                &asset_info,
                &position_attributes,
            );

            self.validate_supply_cap(
                &asset_info,
                &deposit_payment,
                position_attributes.is_vault(),
                storage_cache,
            );

            self.update_deposit_position(
                account_nonce,
                &deposit_payment,
                &asset_info,
                caller,
                &position_attributes,
                storage_cache,
            );
        }
    }

    /// Retrieves or creates a deposit position for a token.
    /// Initializes new positions if none exist.
    ///
    /// # Arguments
    /// - `account_nonce`: Position NFT nonce.
    /// - `asset_info`: Deposited asset configuration.
    /// - `token_id`: Token identifier.
    /// - `is_vault`: Vault status flag.
    ///
    /// # Returns
    /// - Deposit position.
    fn get_or_create_deposit_position(
        &self,
        account_nonce: u64,
        asset_info: &AssetConfig<Self::Api>,
        token_id: &EgldOrEsdtTokenIdentifier,
        is_vault: bool,
    ) -> AccountPosition<Self::Api> {
        let positions = self.deposit_positions(account_nonce);

        positions.get(token_id).unwrap_or_else(|| {
            let data = self.token_oracle(token_id).get();
            AccountPosition::new(
                AccountPositionType::Deposit,
                token_id.clone(),
                ManagedDecimal::from_raw_units(BigUint::zero(), data.price_decimals),
                ManagedDecimal::from_raw_units(BigUint::zero(), data.price_decimals),
                account_nonce,
                self.blockchain().get_block_timestamp(),
                self.ray(),
                asset_info.liquidation_threshold.clone(),
                asset_info.liquidation_bonus.clone(),
                asset_info.liquidation_fees.clone(),
                asset_info.loan_to_value.clone(),
                is_vault,
            )
        })
    }

    /// Updates a deposit position with a new deposit amount.
    /// Handles vault or market logic accordingly.
    ///
    /// # Arguments
    /// - `account_nonce`: Position NFT nonce.
    /// - `collateral`: Deposit payment details.
    /// - `asset_info`: Asset configuration.
    /// - `caller`: Depositor's address.
    /// - `attributes`: NFT attributes.
    /// - `storage_cache`: Mutable storage cache.
    ///
    /// # Returns
    /// - Updated deposit position.
    fn update_deposit_position(
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
        if position.loan_to_value != asset_info.loan_to_value {
            position.loan_to_value = asset_info.loan_to_value.clone();
        }

        if position.liquidation_bonus != asset_info.liquidation_bonus {
            position.liquidation_bonus = asset_info.liquidation_bonus.clone();
        }

        if position.liquidation_fees != asset_info.liquidation_fees {
            position.liquidation_fees = asset_info.liquidation_fees.clone();
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

    /// Updates a market position via the liquidity pool.
    /// Handles non-vault deposit updates.
    ///
    /// # Arguments
    /// - `position`: Current deposit position.
    /// - `amount`: Deposit amount.
    /// - `token_id`: Token identifier.
    /// - `feed`: Price feed for the token.
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

    /// Increases a vault position directly in storage.
    /// Updates vault-specific deposit logic.
    ///
    /// # Arguments
    /// - `position`: Current deposit position.
    /// - `amount`: Deposit amount.
    /// - `token_id`: Token identifier.
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
        position.principal_amount += amount;
    }

    /// Validates deposit payments and handles NFT return.
    /// Separates collateral from NFT payments if present.
    ///
    /// # Arguments
    /// - `caller`: Depositor's address.
    /// - `payments`: Vector of payments (NFT and/or collateral).
    ///
    /// # Returns
    /// - Tuple of (collateral payments, optional NFT payment).
    fn validate_supply_payment(
        &self,
        caller: &ManagedAddress,
        payments: &ManagedVec<EgldOrEsdtTokenPayment<Self::Api>>,
    ) -> (
        ManagedVec<EgldOrEsdtTokenPayment<Self::Api>>,
        Option<EsdtTokenPayment<Self::Api>>,
    ) {
        require!(payments.len() >= 1, ERROR_INVALID_NUMBER_OF_ESDT_TRANSFERS);

        self.require_non_zero_address(caller);

        let first_payment = payments.get(0);

        if self.account_token().get_token_id() == first_payment.token_identifier {
            require!(payments.len() >= 2, ERROR_INVALID_NUMBER_OF_ESDT_TRANSFERS);
            self.require_active_account(first_payment.token_nonce);

            (
                payments.slice(1, payments.len()).unwrap(),
                Some(first_payment.clone().unwrap_esdt()),
            )
        } else {
            (payments.clone(), None)
        }
    }

    /// Ensures isolated collateral constraints are met.
    /// Prevents mixing of isolated collaterals.
    ///
    /// # Arguments
    /// - `account_nonce`: Position NFT nonce.
    /// - `token_id`: Deposited token identifier.
    /// - `asset_info`: Asset configuration.
    /// - `position_attributes`: NFT attributes.
    fn validate_isolated_collateral(
        &self,
        account_nonce: u64,
        token_id: &EgldOrEsdtTokenIdentifier,
        asset_info: &AssetConfig<Self::Api>,
        position_attributes: &NftAccountAttributes,
    ) {
        if !asset_info.is_isolated() && !position_attributes.is_isolated() {
            return;
        }

        let deposit_positions = self.deposit_positions(account_nonce);
        if !deposit_positions.is_empty() {
            let (first_token_id, _) = deposit_positions.iter().next().unwrap();
            require!(&first_token_id == token_id, ERROR_MIX_ISOLATED_COLLATERAL);
        }
    }

    /// Retrieves total supply amount from the liquidity pool.
    ///
    /// # Arguments
    /// - `pool_address`: Pool address.
    ///
    /// # Returns
    /// - `SingleValueMapper` with total supply amount.
    fn get_total_supply(
        &self,
        pool_address: ManagedAddress,
    ) -> SingleValueMapper<ManagedDecimal<Self::Api, NumDecimals>, ManagedAddress> {
        SingleValueMapper::<_, _, ManagedAddress>::new_from_address(
            pool_address,
            StorageKey::new(TOTAL_SUPPLY_AMOUNT_STORAGE_KEY),
        )
    }

    /// Ensures a deposit respects the asset's supply cap.
    ///
    /// # Arguments
    /// - `asset_info`: Asset configuration.
    /// - `deposit_payment`: Deposit payment details.
    /// - `is_vault`: Vault status flag.
    /// - `storage_cache`: Mutable storage cache.
    fn validate_supply_cap(
        &self,
        asset_info: &AssetConfig<Self::Api>,
        deposit_payment: &EgldOrEsdtTokenPayment,
        is_vault: bool,
        storage_cache: &mut StorageCache<Self>,
    ) {
        if let Some(supply_cap) = &asset_info.supply_cap {
            let pool_address =
                storage_cache.get_cached_pool_address(&deposit_payment.token_identifier);
            let mut total_supplied = self.get_total_supply(pool_address).get();

            if is_vault {
                let vault_supplied_amount = self
                    .vault_supplied_amount(&deposit_payment.token_identifier)
                    .get();
                total_supplied += vault_supplied_amount;
            }

            require!(
                total_supplied.into_raw_units() + &deposit_payment.amount <= *supply_cap,
                ERROR_SUPPLY_CAP
            );
        }
    }
}
