use common_structs::{AccountAttributes, AccountPosition, PriceFeedShort};

use crate::{
    contexts::base::StorageCache, helpers, oracle, proxy_pool, storage, utils, validation,
};

use super::{account, vault};

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
    + vault::PositionVaultModule
{
    /// Processes a withdrawal from a deposit position.
    /// Handles vault or market withdrawals with validations.
    ///
    /// # Arguments
    /// - `account_nonce`: Position NFT nonce.
    /// - `withdraw_payment`: Withdrawal payment details.
    /// - `caller`: Withdrawer's address.
    /// - `is_liquidation`: Liquidation flag.
    /// - `liquidation_fee`: Optional fee for liquidation.
    /// - `storage_cache`: Mutable storage cache.
    /// - `position_attributes`: NFT attributes.
    /// - `is_swap`: Swap operation flag.
    ///
    /// # Returns
    /// - Updated deposit position.
    fn process_withdrawal(
        &self,
        account_nonce: u64,
        withdraw_payment: EgldOrEsdtTokenPayment,
        caller: &ManagedAddress,
        is_liquidation: bool,
        liquidation_fee: Option<ManagedDecimal<Self::Api, NumDecimals>>,
        storage_cache: &mut StorageCache<Self>,
        position_attributes: &AccountAttributes,
        is_swap: bool,
    ) -> AccountPosition<Self::Api> {
        let (token_id, _, requested_amount) = withdraw_payment.into_tuple();
        let price_feed = self.get_token_price(&token_id, storage_cache);
        let pool_address = storage_cache.get_cached_pool_address(&token_id);

        let mut deposit_position = self.get_deposit_position(account_nonce, &token_id);
        let withdraw_amount = self.calculate_withdraw_amount(&deposit_position, requested_amount);

        let updated_position = if position_attributes.is_vault() {
            self.process_vault_withdrawal(
                &token_id,
                &withdraw_amount,
                is_liquidation,
                liquidation_fee,
                caller,
                pool_address,
                &price_feed,
                &mut deposit_position,
                is_swap,
            )
        } else {
            self.process_market_withdrawal(
                pool_address,
                caller,
                &withdraw_amount,
                &mut deposit_position,
                is_liquidation,
                liquidation_fee,
                &price_feed,
            )
        };

        self.emit_withdrawal_event(
            &withdraw_amount,
            &updated_position,
            &price_feed,
            caller,
            position_attributes,
        );
        self.update_deposit_position_storage(account_nonce, &token_id, &updated_position);

        updated_position
    }

    /// Processes a vault withdrawal.
    /// Updates storage and transfers assets for vault positions.
    ///
    /// # Arguments
    /// - `token_id`: Token identifier.
    /// - `amount`: Withdrawal amount.
    /// - `is_liquidation`: Liquidation flag.
    /// - `liquidation_fee_opt`: Optional liquidation fee.
    /// - `caller`: Receiver's address.
    /// - `pool_address`: Pool address.
    /// - `price_feed`: Price data for the token.
    /// - `deposit_position`: Mutable deposit position.
    /// - `is_swap`: Swap operation flag.
    ///
    /// # Returns
    /// - Updated deposit position.
    fn process_vault_withdrawal(
        &self,
        token_id: &EgldOrEsdtTokenIdentifier,
        amount: &ManagedDecimal<Self::Api, NumDecimals>,
        is_liquidation: bool,
        liquidation_fee_opt: Option<ManagedDecimal<Self::Api, NumDecimals>>,
        caller: &ManagedAddress,
        pool_address: ManagedAddress,
        price_feed: &PriceFeedShort<Self::Api>,
        deposit_position: &mut AccountPosition<Self::Api>,
        is_swap: bool,
    ) -> AccountPosition<Self::Api> {
        self.update_vault_supplied_amount(token_id, amount, false);
        deposit_position.principal_amount -= amount;

        if is_liquidation && liquidation_fee_opt.is_some() {
            let liquidation_fee = liquidation_fee_opt.unwrap();
            let amount_after_fee = amount.clone() - liquidation_fee.clone();
            self.transfer_withdrawn_assets(caller, token_id, &amount_after_fee, is_swap);
            self.transfer_liquidation_fee(pool_address, token_id, &liquidation_fee, price_feed);
        } else {
            self.transfer_withdrawn_assets(caller, token_id, amount, is_swap);
        }

        deposit_position.clone()
    }

    /// Executes a market withdrawal via the liquidity pool.
    ///
    /// # Arguments
    /// - `pool_address`: Pool address.
    /// - `caller`: Withdrawer's address.
    /// - `amount`: Withdrawal amount.
    /// - `deposit_position`: Mutable deposit position.
    /// - `is_liquidation`: Liquidation flag.
    /// - `liquidation_fee`: Optional fee.
    /// - `price_feed`: Price data for the token.
    ///
    /// # Returns
    /// - Updated deposit position.
    fn process_market_withdrawal(
        &self,
        pool_address: ManagedAddress,
        caller: &ManagedAddress,
        amount: &ManagedDecimal<Self::Api, NumDecimals>,
        deposit_position: &mut AccountPosition<Self::Api>,
        is_liquidation: bool,
        liquidation_fee: Option<ManagedDecimal<Self::Api, NumDecimals>>,
        price_feed: &PriceFeedShort<Self::Api>,
    ) -> AccountPosition<Self::Api> {
        self.tx()
            .to(pool_address)
            .typed(proxy_pool::LiquidityPoolProxy)
            .withdraw(
                caller,
                amount.clone(),
                deposit_position.clone(),
                is_liquidation,
                liquidation_fee,
                price_feed.price.clone(),
            )
            .returns(ReturnsResult)
            .sync_call()
    }

    /// Manages the position NFT after withdrawal.
    /// Burns NFT if position is fully closed, otherwise transfers it.
    ///
    /// # Arguments
    /// - `amount`: NFT token amount.
    /// - `token_nonce`: NFT nonce.
    /// - `token_identifier`: NFT identifier.
    /// - `caller`: Withdrawer's address.
    fn manage_account_after_withdrawal(
        &self,
        account_payment: &EsdtTokenPayment<Self::Api>,
        caller: &ManagedAddress,
    ) {
        let deposit_positions_count = self.deposit_positions(account_payment.token_nonce).len();
        let borrow_positions_count = self.borrow_positions(account_payment.token_nonce).len();

        // Burn NFT if position is fully closed
        if deposit_positions_count == 0 && borrow_positions_count == 0 {
            self.account_token()
                .nft_burn(account_payment.token_nonce, &BigUint::from(1u64));
            self.account_positions()
                .swap_remove(&account_payment.token_nonce);
            self.account_attributes(account_payment.token_nonce).clear();
        } else {
            self.tx().to(caller).payment(account_payment).transfer();
        }
    }

    /// Retrieves a deposit position for a token.
    ///
    /// # Arguments
    /// - `account_nonce`: Position NFT nonce.
    /// - `token_id`: Token identifier.
    ///
    /// # Returns
    /// - Deposit position.
    fn get_deposit_position(
        &self,
        account_nonce: u64,
        token_id: &EgldOrEsdtTokenIdentifier,
    ) -> AccountPosition<Self::Api> {
        let maybe_deposit_position = self.deposit_positions(account_nonce).get(&token_id);
        require!(
            maybe_deposit_position.is_some(),
            "Token {} is not available for this account",
            token_id
        );
        maybe_deposit_position.unwrap()
    }

    /// Calculates the actual withdrawal amount.
    /// Caps withdrawal at available balance.
    ///
    /// # Arguments
    /// - `deposit_position`: Current deposit position.
    /// - `requested_amount`: Requested withdrawal amount.
    ///
    /// # Returns
    /// - Actual withdrawal amount in decimal format.
    fn calculate_withdraw_amount(
        &self,
        deposit_position: &AccountPosition<Self::Api>,
        requested_amount: BigUint,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let requested_amount_dec = deposit_position.make_amount_decimal(requested_amount);
        let total_amount = deposit_position.get_total_amount();
        if requested_amount_dec > total_amount {
            total_amount
        } else {
            requested_amount_dec
        }
    }

    /// Transfers withdrawn assets to the caller.
    /// Skips transfer if part of a swap operation.
    ///
    /// # Arguments
    /// - `caller`: Receiver's address.
    /// - `token_id`: Token identifier.
    /// - `amount`: Transfer amount.
    /// - `is_swap`: Swap operation flag.
    fn transfer_withdrawn_assets(
        &self,
        caller: &ManagedAddress,
        token_id: &EgldOrEsdtTokenIdentifier,
        amount: &ManagedDecimal<Self::Api, NumDecimals>,
        is_swap: bool,
    ) {
        if !is_swap {
            self.tx()
                .to(caller)
                .egld_or_single_esdt(token_id, 0, amount.into_raw_units())
                .transfer();
        }
    }

    /// Transfers liquidation fee to the liquidity pool.
    /// Adds fee as protocol revenue.
    ///
    /// # Arguments
    /// - `pool_address`: Pool address.
    /// - `token_id`: Token identifier.
    /// - `fee`: Fee amount.
    /// - `price_feed`: Price data for the token.
    fn transfer_liquidation_fee(
        &self,
        pool_address: ManagedAddress,
        token_id: &EgldOrEsdtTokenIdentifier,
        fee: &ManagedDecimal<Self::Api, NumDecimals>,
        price_feed: &PriceFeedShort<Self::Api>,
    ) {
        self.tx()
            .to(pool_address)
            .typed(proxy_pool::LiquidityPoolProxy)
            .add_protocol_revenue(price_feed.price.clone())
            .egld_or_single_esdt(token_id, 0, fee.into_raw_units())
            .returns(ReturnsResult)
            .sync_call();
    }

    /// Emits an event for a withdrawal operation.
    /// Logs withdrawal details for transparency.
    ///
    /// # Arguments
    /// - `amount`: Withdrawn amount.
    /// - `position`: Updated position.
    /// - `price_feed`: Price data for the token.
    /// - `caller`: Withdrawer's address.
    /// - `position_attributes`: NFT attributes.
    fn emit_withdrawal_event(
        &self,
        amount: &ManagedDecimal<Self::Api, NumDecimals>,
        position: &AccountPosition<Self::Api>,
        price_feed: &PriceFeedShort<Self::Api>,
        caller: &ManagedAddress,
        position_attributes: &AccountAttributes,
    ) {
        self.update_position_event(
            amount,
            position,
            OptionalValue::Some(price_feed.price.clone()),
            OptionalValue::Some(caller),
            OptionalValue::Some(position_attributes),
        );
    }

    /// Updates or removes a deposit position in storage.
    /// Reflects withdrawal changes in storage.
    ///
    /// # Arguments
    /// - `account_nonce`: Position NFT nonce.
    /// - `token_id`: Token identifier.
    /// - `position`: Updated deposit position.
    fn update_deposit_position_storage(
        &self,
        account_nonce: u64,
        token_id: &EgldOrEsdtTokenIdentifier,
        position: &AccountPosition<Self::Api>,
    ) {
        let mut deposit_positions = self.deposit_positions(account_nonce);
        if position.can_remove() {
            deposit_positions.remove(token_id);
        } else {
            deposit_positions.insert(token_id.clone(), position.clone());
        }
    }
}
