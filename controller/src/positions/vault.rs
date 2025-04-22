multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use crate::{cache::Cache, helpers, oracle, proxy_pool, storage, utils, validation};
use common_errors::*;
use common_structs::{AccountAttributes, AccountPosition, PriceFeedShort};

use super::account;

#[multiversx_sc::module]
pub trait PositionVaultModule:
    storage::Storage
    + validation::ValidationModule
    + oracle::OracleModule
    + common_events::EventsModule
    + utils::LendingUtilsModule
    + helpers::math::MathsModule
    + common_math::SharedMathModule
    + account::PositionAccountModule
{
    /// Validates vault account and checks vault state.
    fn validate_vault_account(
        &self,
        account_attributes: &AccountAttributes<Self::Api>,
        expect_vault: bool,
    ) {
        match expect_vault {
            true => require!(account_attributes.is_vault(), ERROR_VAULT_ALREADY_ENABLED),
            false => require!(!account_attributes.is_vault(), ERROR_VAULT_ALREADY_DISABLED),
        };
    }

    /// Processes enabling or disabling vault mode.
    fn process_vault_toggle(
        &self,
        account_nonce: u64,
        enable: bool,
        cache: &mut Cache<Self>,
        account_attributes: &AccountAttributes<Self::Api>,
        caller: &ManagedAddress<Self::Api>,
    ) {
        let deposit_positions = self.deposit_positions(account_nonce);

        for mut dp in deposit_positions.values() {
            let pool_address = cache.get_cached_pool_address(&dp.asset_id);
            let feed = self.get_token_price(&dp.asset_id, cache);
            if enable {
                self.enable_vault_position(
                    &mut dp,
                    pool_address,
                    &feed,
                    account_nonce,
                    &caller,
                    account_attributes,
                );
            } else {
                self.disable_vault_position(
                    &mut dp,
                    pool_address,
                    &feed,
                    account_nonce,
                    &caller,
                    account_attributes,
                );
            }
        }
    }

    /// Enables a vault position by moving funds from the market pool.
    fn enable_vault_position(
        &self,
        dp: &mut AccountPosition<Self::Api>,
        pool_address: ManagedAddress<Self::Api>,
        feed: &PriceFeedShort<Self::Api>,
        account_nonce: u64,
        caller: &ManagedAddress<Self::Api>,
        account_attributes: &AccountAttributes<Self::Api>,
    ) {
        let controller_sc = self.blockchain().get_sc_address();
        self.update_position(&pool_address, dp, OptionalValue::Some(feed.price.clone()));
        let total_amount_with_interest = dp.get_total_amount();

        *dp = self
            .tx()
            .to(&pool_address)
            .typed(proxy_pool::LiquidityPoolProxy)
            .withdraw(
                &controller_sc,
                total_amount_with_interest.clone(),
                dp.clone(),
                false,
                None,
                feed.price.clone(),
            )
            .returns(ReturnsResult)
            .sync_call();

        // Ensure the position can be removed, means no more deposits
        require!(dp.can_remove(), ERROR_ENABLE_VAULT_MODE_FAILED);

        // Re add the withdrawn amount to the principal amount only
        self.update_vault_supplied_amount(&dp.asset_id, &total_amount_with_interest, true);

        dp.principal_amount += total_amount_with_interest;

        self.deposit_positions(account_nonce)
            .insert(dp.asset_id.clone(), dp.clone());

        self.update_position_event(
            &dp.zero_decimal(),
            dp,
            OptionalValue::Some(feed.price.clone()),
            OptionalValue::Some(caller),
            OptionalValue::Some(account_attributes),
        );
    }

    /// Disables a vault position by moving funds to the market pool.
    fn disable_vault_position(
        &self,
        dp: &mut AccountPosition<Self::Api>,
        pool_address: ManagedAddress<Self::Api>,
        feed: &PriceFeedShort<Self::Api>,
        account_nonce: u64,
        caller: &ManagedAddress<Self::Api>,
        account_attributes: &AccountAttributes<Self::Api>,
    ) {
        let old_amount = dp.principal_amount.clone();
        self.update_vault_supplied_amount(&dp.asset_id, &old_amount, false);

        dp.principal_amount = dp.zero_decimal();

        *dp = self
            .tx()
            .to(pool_address)
            .typed(proxy_pool::LiquidityPoolProxy)
            .supply(dp.clone(), feed.price.clone())
            .egld_or_single_esdt(&dp.asset_id, 0, old_amount.into_raw_units())
            .returns(ReturnsResult)
            .sync_call();

        self.deposit_positions(account_nonce)
            .insert(dp.asset_id.clone(), dp.clone());

        self.update_position_event(
            &dp.zero_decimal(),
            dp,
            OptionalValue::Some(feed.price.clone()),
            OptionalValue::Some(caller),
            OptionalValue::Some(account_attributes),
        );
    }

    /// Updates account attributes in storage and NFT.
    fn update_account_attributes(
        &self,
        account_nonce: u64,
        account_attributes: &AccountAttributes<Self::Api>,
    ) {
        self.account_token()
            .nft_update_attributes(account_nonce, account_attributes);
        self.account_attributes(account_nonce)
            .set(account_attributes);
    }

    /// Updates the vault's supplied amount in storage.
    /// Adjusts for deposits or withdrawals.
    ///
    /// # Arguments
    /// - `token_id`: Token identifier.
    /// - `amount`: Adjustment amount.
    /// - `is_increase`: Increase (true) or decrease (false) flag.
    fn update_vault_supplied_amount(
        &self,
        token_id: &EgldOrEsdtTokenIdentifier,
        amount: &ManagedDecimal<Self::Api, NumDecimals>,
        is_increase: bool,
    ) {
        let last_value = self.vault_supplied_amount(token_id).update(|am| {
            if is_increase {
                *am += amount;
            } else {
                *am -= amount;
            }

            am.clone()
        });
        self.update_vault_supplied_amount_event(token_id, last_value);
    }
}
