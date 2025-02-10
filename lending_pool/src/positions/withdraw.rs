use common_constants::BP;
use common_events::{
    AccountPosition, AccountPositionType, AssetConfig, NftAccountAttributes, PriceFeedShort,
};

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
        liquidation_fee: &BigUint,
        storage_cache: &mut StorageCache<Self>,
        attributes: &NftAccountAttributes,
    ) -> AccountPosition<Self::Api> {
        let (withdraw_token_id, _, mut amount) = collateral.into_tuple();
        let pool_address = self.get_pool_address(&withdraw_token_id);
        let mut dep_pos_map = self.deposit_positions(account_nonce);
        let dp_opt = dep_pos_map.get(&withdraw_token_id);

        require!(
            dp_opt.is_some(),
            "Token {} is not available for this account",
            withdraw_token_id
        );

        let mut dp = dp_opt.unwrap();
        // Cap withdraw amount to available balance
        if amount > dp.get_total_amount() {
            amount = dp.get_total_amount();
        }
        let asset_data = self.get_token_price(&withdraw_token_id, storage_cache);
        let position = if dp.is_vault {
            let last_value = self.vault_supplied_amount(&withdraw_token_id).update(|am| {
                *am -= &amount;
                am.clone()
            });

            self.update_vault_supplied_amount_event(&withdraw_token_id, last_value);

            dp.amount -= &amount;

            if is_liquidation {
                let liquidated_amount_after_fees = &(&amount - liquidation_fee);
                self.tx()
                    .to(caller)
                    .payment(EgldOrEsdtTokenPayment::new(
                        withdraw_token_id.clone(),
                        0,
                        liquidated_amount_after_fees.clone(),
                    ))
                    .transfer();

                self.tx()
                    .to(pool_address)
                    .typed(proxy_pool::LiquidityPoolProxy)
                    .add_external_protocol_revenue(&asset_data.price)
                    .egld_or_single_esdt(&withdraw_token_id, 0, liquidation_fee)
                    .returns(ReturnsResult)
                    .sync_call();
            } else {
                self.tx()
                    .to(caller)
                    .payment(EgldOrEsdtTokenPayment::new(
                        withdraw_token_id.clone(),
                        0,
                        amount.clone(),
                    ))
                    .transfer();
            };

            dp
        } else {
            self.tx()
                .to(pool_address)
                .typed(proxy_pool::LiquidityPoolProxy)
                .withdraw(
                    caller,
                    &amount,
                    dp,
                    is_liquidation,
                    liquidation_fee,
                    &asset_data.price,
                )
                .returns(ReturnsResult)
                .sync_call()
        };

        self.update_position_event(
            &amount,
            &position,
            OptionalValue::Some(asset_data.price),
            OptionalValue::Some(&caller),
            OptionalValue::Some(&attributes),
        );

        if position.get_total_amount().gt(&BigUint::zero()) {
            dep_pos_map.insert(withdraw_token_id.clone(), position.clone());
        } else {
            dep_pos_map.remove(&withdraw_token_id);
        }

        position
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
}
