use common_structs::{AccountPosition, NftAccountAttributes, PriceFeedShort};

use crate::{
    contexts::base::StorageCache, helpers, oracle, proxy_pool, storage, utils, validation,
    ERROR_HEALTH_FACTOR_WITHDRAW,
};

use super::account;

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

#[multiversx_sc::module]
pub trait PositionWithdrawModule:
    storage::LendingStorageModule
    + validation::ValidationModule
    + oracle::OracleModule
    + common_events::EventsModule
    + utils::LendingUtilsModule
    + helpers::math::MathsModule
    + account::PositionAccountModule
    + common_math::SharedMathModule
{
    /// Processes withdrawal from a position
    ///
    /// # Arguments
    /// * `account_nonce` - NFT nonce of the account position
    /// * `withdraw_token_id` - Token to withdraw
    /// * `amount` - Amount to withdraw
    /// * `caller` - Address initiating withdrawal
    /// * `is_liquidation` - Whether this is a liquidation withdrawal
    /// * `liquidation_fee` - Protocol fee for liquidation
    /// * `attributes` - Optional NFT attributes
    ///
    /// # Returns
    /// * `AccountPosition` - Updated position after withdrawal
    ///
    /// Handles both normal withdrawals and liquidations.
    /// For vault positions, updates storage directly.
    /// For market positions, processes through liquidity pool.
    /// Handles protocol fees for liquidations.
    fn internal_withdraw(
        &self,
        account_nonce: u64,
        collateral: EgldOrEsdtTokenPayment,
        caller: &ManagedAddress,
        is_liquidation: bool,
        liquidation_fee: Option<ManagedDecimal<Self::Api, NumDecimals>>,
        storage_cache: &mut StorageCache<Self>,
        attributes: &NftAccountAttributes,
    ) -> AccountPosition<Self::Api> {
        // Unpack collateral details
        let (withdraw_token_id, _, requested_amount) = collateral.into_tuple();
        // Get price data for the collateral token
        let asset_data = self.get_token_price(&withdraw_token_id, storage_cache);
        let request_amount_dec =
            ManagedDecimal::from_raw_units(requested_amount, asset_data.decimals as usize);

        let pool_address = self.get_pool_address(&withdraw_token_id);

        // Retrieve deposit position for the given token
        let mut deposit_positions = self.deposit_positions(account_nonce);
        let maybe_deposit_position = deposit_positions.get(&withdraw_token_id);
        require!(
            maybe_deposit_position.is_some(),
            "Token {} is not available for this account",
            withdraw_token_id
        );
        let mut deposit_position = maybe_deposit_position.unwrap();

        // Cap the withdrawal amount to the available balance
        let amount = if request_amount_dec > deposit_position.get_total_amount() {
            deposit_position.get_total_amount()
        } else {
            request_amount_dec
        };

        // Process withdrawal differently based on deposit type
        let updated_position = if deposit_position.is_vault {
            self.process_vault_withdrawal(
                &withdraw_token_id,
                &amount,
                is_liquidation,
                liquidation_fee,
                caller,
                pool_address.clone(),
                &asset_data,
                &mut deposit_position,
            )
        } else {
            self.tx()
                .to(pool_address)
                .typed(proxy_pool::LiquidityPoolProxy)
                .withdraw(
                    caller,
                    amount.clone(),
                    deposit_position,
                    is_liquidation,
                    liquidation_fee,
                    asset_data.price.clone(),
                )
                .returns(ReturnsResult)
                .sync_call()
        };

        // Emit event and update deposit positions storage
        self.update_position_event(
            &amount,
            &updated_position,
            OptionalValue::Some(asset_data.price),
            OptionalValue::Some(caller),
            OptionalValue::Some(attributes),
        );

        if updated_position
            .get_total_amount()
            .gt(&ManagedDecimal::from_raw_units(
                BigUint::zero(),
                asset_data.decimals as usize,
            ))
        {
            deposit_positions.insert(withdraw_token_id.clone(), updated_position.clone());
        } else {
            deposit_positions.remove(&withdraw_token_id);
        }

        updated_position
    }

    /// Processes a withdrawal from a vault-type deposit position.
    ///
    /// This function updates the vault’s internal supplied amount, decreases the deposit position,
    /// sends a payment to the caller (subtracting liquidation fees if this is a liquidation) and, in the
    /// liquidation case, also transfers the liquidation fee to the pool as protocol revenue.
    ///
    /// # Arguments
    /// * `withdraw_token_id` - The token identifier being withdrawn.
    /// * `amount` - The amount to withdraw.
    /// * `is_liquidation` - True if this withdrawal is triggered by liquidation.
    /// * `liquidation_fee` - The fee to be deducted in a liquidation.
    /// * `caller` - The address receiving the withdrawn funds.
    /// * `pool_address` - The address of the liquidity pool for this token.
    /// * `asset_data` - Price feed data for the token.
    /// * `deposit_position` - The mutable deposit position for this token.
    fn process_vault_withdrawal(
        &self,
        withdraw_token_id: &EgldOrEsdtTokenIdentifier,
        amount: &ManagedDecimal<Self::Api, NumDecimals>,
        is_liquidation: bool,
        liquidation_fee_opt: Option<ManagedDecimal<Self::Api, NumDecimals>>,
        caller: &ManagedAddress,
        pool_address: ManagedAddress,
        asset_data: &PriceFeedShort<Self::Api>, // Adjust this type to your actual asset data type
        deposit_position: &mut AccountPosition<Self::Api>,
    ) -> AccountPosition<Self::Api> {
        // Update the vault’s stored supplied amount by subtracting the withdrawn amount.
        let last_value = self.vault_supplied_amount(withdraw_token_id).update(|am| {
            *am -= amount;
            am.clone()
        });

        // Emit an event for the updated vault supplied amount.
        self.update_vault_supplied_amount_event(withdraw_token_id, last_value);

        // Decrease the deposit position by the withdrawn amount.
        deposit_position.amount -= amount;

        if is_liquidation && liquidation_fee_opt.is_some() {
            // In a liquidation, deduct the liquidation fee from the withdrawn amount.
            let liquidation_fee = liquidation_fee_opt.unwrap();
            let liquidated_amount_after_fees = amount.clone() - liquidation_fee.clone();
            // Transfer the net amount to the caller.
            self.tx()
                .to(caller)
                .egld_or_single_esdt(
                    withdraw_token_id,
                    0,
                    liquidated_amount_after_fees.into_raw_units(),
                )
                .transfer();
            // Send the liquidation fee to the pool as protocol revenue.
            self.tx()
                .to(pool_address)
                .typed(proxy_pool::LiquidityPoolProxy)
                .add_external_protocol_revenue(asset_data.price.clone())
                .egld_or_single_esdt(withdraw_token_id, 0, liquidation_fee.into_raw_units())
                .returns(ReturnsResult)
                .sync_call();
        } else {
            // For a normal withdrawal, simply transfer the full amount to the caller.
            self.tx()
                .to(caller)
                .egld_or_single_esdt(withdraw_token_id, 0, amount.into_raw_units())
                .transfer();
        }

        // Return the updated deposit position.
        deposit_position.clone()
    }

    /// Handles NFT after withdrawal operation
    ///
    /// # Arguments
    /// * `account_token` - NFT token payment
    /// * `caller` - Address initiating withdrawal
    ///
    /// If no positions remain (no deposits or borrows),
    /// burns the NFT and removes from storage.
    /// Otherwise returns NFT to caller.
    fn handle_nft_after_withdraw(
        &self,
        amount: &BigUint,
        token_nonce: u64,
        token_identifier: &TokenIdentifier,
        caller: &ManagedAddress,
    ) {
        let dep_pos_map = self.deposit_positions(token_nonce).len();
        let borrow_pos_map = self.borrow_positions(token_nonce).len();

        if dep_pos_map == 0 && borrow_pos_map == 0 {
            self.account_token().nft_burn(token_nonce, amount);
            self.account_positions().swap_remove(&token_nonce);
            self.account_attributes(token_nonce).clear();
        } else {
            self.tx()
                .to(caller)
                .single_esdt(token_identifier, token_nonce, amount)
                .transfer();
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
        safety_factor: Option<ManagedDecimal<Self::Api, NumDecimals>>,
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
                storage_cache.wad_dec.clone()
                    + (storage_cache.wad_dec.clone() / safety_factor_value)
            } else {
                storage_cache.wad_dec.clone()
            };
            require!(
                health_factor >= health_factor_with_safety_factor,
                ERROR_HEALTH_FACTOR_WITHDRAW
            );
        }
    }
}
