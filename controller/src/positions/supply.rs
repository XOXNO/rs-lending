use common_constants::TOTAL_SUPPLY_AMOUNT_STORAGE_KEY;
use common_structs::{
    AccountAttributes, AccountPosition, AccountPositionType, AssetConfig, PriceFeedShort,
};
use multiversx_sc::storage::StorageKey;

use crate::{cache::Cache, helpers, oracle, proxy_pool, storage, utils, validation};
use common_errors::{
    ERROR_ACCOUNT_ATTRIBUTES_MISMATCH, ERROR_ASSET_NOT_SUPPORTED_AS_COLLATERAL,
    ERROR_INVALID_NUMBER_OF_ESDT_TRANSFERS, ERROR_MIX_ISOLATED_COLLATERAL,
    ERROR_POSITION_NOT_FOUND, ERROR_SUPPLY_CAP,
};

use super::{account, emode, update, vault};

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

#[multiversx_sc::module]
pub trait PositionDepositModule:
    storage::Storage
    + validation::ValidationModule
    + oracle::OracleModule
    + common_events::EventsModule
    + utils::LendingUtilsModule
    + helpers::math::MathsModule
    + account::PositionAccountModule
    + emode::EModeModule
    + common_math::SharedMathModule
    + vault::PositionVaultModule
    + update::PositionUpdateModule
{
    /// Processes a deposit operation for a user's position.
    /// Handles validations, e-mode, and updates for deposits.
    ///
    /// # Arguments
    /// - `caller`: Depositor's address.
    /// - `account_nonce`: Position NFT nonce.
    /// - `position_attributes`: NFT attributes.
    /// - `deposit_payments`: Vector of deposit payments.
    /// - `cache`: Mutable storage cache.
    fn process_deposit(
        &self,
        caller: &ManagedAddress,
        account_nonce: u64,
        position_attributes: AccountAttributes<Self::Api>,
        deposit_payments: &ManagedVec<EgldOrEsdtTokenPayment>,
        cache: &mut Cache<Self>,
    ) {
        let e_mode = self.get_e_mode_category(position_attributes.get_emode_id());
        self.ensure_e_mode_not_deprecated(&e_mode);

        for deposit_payment in deposit_payments {
            self.validate_payment(&deposit_payment);

            let mut asset_info = cache.get_cached_asset_info(&deposit_payment.token_identifier);
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
                &deposit_payment.token_identifier,
                &asset_info,
                &position_attributes,
            );
            self.validate_supply_cap(&asset_info, &deposit_payment, cache);

            self.update_deposit_position(
                account_nonce,
                &deposit_payment,
                &asset_info,
                caller,
                &position_attributes,
                cache,
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
    ) -> AccountPosition<Self::Api> {
        self.deposit_positions(account_nonce)
            .get(token_id)
            .unwrap_or_else(|| {
                let data = self.token_oracle(token_id).get();
                AccountPosition::new(
                    AccountPositionType::Deposit,
                    token_id.clone(),
                    self.to_decimal(BigUint::zero(), data.price_decimals),
                    self.to_decimal(BigUint::zero(), data.price_decimals),
                    account_nonce,
                    self.blockchain().get_block_timestamp(),
                    self.ray(),
                    asset_info.liquidation_threshold.clone(),
                    asset_info.liquidation_bonus.clone(),
                    asset_info.liquidation_fees.clone(),
                    asset_info.loan_to_value.clone(),
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
    /// - `cache`: Mutable storage cache.
    ///
    /// # Returns
    /// - Updated deposit position.
    fn update_deposit_position(
        &self,
        account_nonce: u64,
        collateral: &EgldOrEsdtTokenPayment<Self::Api>,
        asset_info: &AssetConfig<Self::Api>,
        caller: &ManagedAddress,
        attributes: &AccountAttributes<Self::Api>,
        cache: &mut Cache<Self>,
    ) -> AccountPosition<Self::Api> {
        let feed = self.get_token_price(&collateral.token_identifier, cache);
        let mut position = self.get_or_create_deposit_position(
            account_nonce,
            asset_info,
            &collateral.token_identifier,
        );

        // Auto upgrade safe values when changed on demand
        if position.loan_to_value != asset_info.loan_to_value {
            position.loan_to_value = asset_info.loan_to_value.clone();
        }

        if position.liquidation_bonus != asset_info.liquidation_bonus {
            position.liquidation_bonus = asset_info.liquidation_bonus.clone();
        }

        if position.liquidation_fees != asset_info.liquidation_fees {
            position.liquidation_fees = asset_info.liquidation_fees.clone();
        }

        let amount_decimal = position.make_amount_decimal(&collateral.amount);
        if attributes.is_vault() {
            self.update_vault_supplied_amount(&collateral.token_identifier, &amount_decimal, true);
            position.principal_amount += &amount_decimal;
        } else {
            self.update_market_position(
                &mut position,
                &collateral.amount,
                &collateral.token_identifier,
                &feed,
                cache,
            );
        }

        self.emit_position_update_event(
            &amount_decimal,
            &position,
            feed.price.clone(),
            caller,
            attributes,
        );

        // Update storage with the latest position
        self.store_updated_position(account_nonce, &position);

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
        cache: &mut Cache<Self>,
    ) {
        *position = self
            .tx()
            .to(cache.get_cached_pool_address(token_id))
            .typed(proxy_pool::LiquidityPoolProxy)
            .supply(position.clone(), feed.price.clone())
            .egld_or_single_esdt(token_id, 0, amount)
            .returns(ReturnsResult)
            .sync_call();
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
        require_account_payment: bool,
        return_nft: bool,
    ) -> (
        ManagedVec<EgldOrEsdtTokenPayment<Self::Api>>,
        Option<EsdtTokenPayment<Self::Api>>,
        ManagedAddress,
        Option<AccountAttributes<Self::Api>>,
    ) {
        let caller = self.blockchain().get_caller();
        let payments = self.call_value().all_transfers();
        require!(payments.len() >= 1, ERROR_INVALID_NUMBER_OF_ESDT_TRANSFERS);

        self.require_non_zero_address(&caller);

        let first_payment = payments.get(0);

        if self.account_token().get_token_id() == first_payment.token_identifier {
            self.require_active_account(first_payment.token_nonce);

            let account_payment = first_payment.clone().unwrap_esdt();
            let account_attributes = self.nft_attributes(&account_payment);
            let stored_attributes = self.account_attributes(account_payment.token_nonce).get();

            require!(
                account_attributes == stored_attributes,
                ERROR_ACCOUNT_ATTRIBUTES_MISMATCH
            );

            if return_nft {
                // Refund NFT
                self.tx().to(&caller).payment(&account_payment).transfer();
            }

            (
                payments.slice(1, payments.len()).unwrap_or_default(),
                Some(account_payment),
                caller,
                Some(account_attributes),
            )
        } else {
            require!(
                !require_account_payment,
                ERROR_INVALID_NUMBER_OF_ESDT_TRANSFERS
            );

            (payments.clone(), None, caller, None)
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
        token_id: &EgldOrEsdtTokenIdentifier,
        asset_info: &AssetConfig<Self::Api>,
        position_attributes: &AccountAttributes<Self::Api>,
    ) {
        let is_isolated = asset_info.is_isolated() || position_attributes.is_isolated();
        if !is_isolated {
            return;
        }

        require!(
            asset_info.is_isolated() == position_attributes.is_isolated()
                && position_attributes.get_isolated_token() == *token_id,
            ERROR_MIX_ISOLATED_COLLATERAL
        );
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
    /// - `cache`: Mutable storage cache.
    fn validate_supply_cap(
        &self,
        asset_info: &AssetConfig<Self::Api>,
        deposit_payment: &EgldOrEsdtTokenPayment,
        cache: &mut Cache<Self>,
    ) {
        match &asset_info.supply_cap {
            Some(supply_cap) => {
                if supply_cap == &0 {
                    return;
                }

                let pool_address = cache.get_cached_pool_address(&deposit_payment.token_identifier);
                let mut total_supplied = self.get_total_supply(pool_address).get();

                let vault_supplied_amount = self
                    .vault_supplied_amount(&deposit_payment.token_identifier)
                    .get();
                total_supplied += vault_supplied_amount;

                require!(
                    total_supplied.into_raw_units() + &deposit_payment.amount <= *supply_cap,
                    ERROR_SUPPLY_CAP
                );
            },
            None => {
                // No supply cap set, do nothing
            },
        }
    }

    /// Updates position threshold (LTV or liquidation) for an account.
    fn update_position_threshold(
        &self,
        account_nonce: u64,
        asset_id: &EgldOrEsdtTokenIdentifier<Self::Api>,
        has_risks: bool,
        asset_config: &mut AssetConfig<Self::Api>,
        cache: &mut Cache<Self>,
    ) {
        self.require_active_account(account_nonce);
        let controller_sc = self.blockchain().get_sc_address();
        let deposit_positions = self.deposit_positions(account_nonce);
        let dp_option = deposit_positions.get(asset_id);
        require!(dp_option.is_some(), ERROR_POSITION_NOT_FOUND);

        let account_attributes = self.account_attributes(account_nonce).get();
        let e_mode_category = self.get_e_mode_category(account_attributes.get_emode_id());
        let asset_emode_config =
            self.get_token_e_mode_config(account_attributes.get_emode_id(), asset_id);
        self.apply_e_mode_to_asset_config(asset_config, &e_mode_category, asset_emode_config);

        let mut dp = dp_option.unwrap();

        if has_risks {
            if dp.liquidation_threshold != asset_config.liquidation_threshold {
                dp.liquidation_threshold = asset_config.liquidation_threshold.clone();
            }
        } else {
            if dp.loan_to_value != asset_config.loan_to_value {
                dp.loan_to_value = asset_config.loan_to_value.clone();
            }

            if dp.liquidation_bonus != asset_config.liquidation_bonus {
                dp.liquidation_bonus = asset_config.liquidation_bonus.clone();
            }

            if dp.liquidation_fees != asset_config.liquidation_fees {
                dp.liquidation_fees = asset_config.liquidation_fees.clone();
            }
        }

        self.store_updated_position(account_nonce, &dp);

        if has_risks {
            self.validate_is_healthy(
                account_nonce,
                cache,
                Some(self.to_decimal(BigUint::from(20u64), 0usize)),
            );
        }

        self.emit_position_update_event(
            &dp.zero_decimal(),
            &dp,
            self.get_token_price(asset_id, cache).price,
            &controller_sc,
            &account_attributes,
        );
    }
}
