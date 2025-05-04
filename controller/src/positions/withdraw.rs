use common_structs::{AccountAttributes, AccountPosition, PriceFeedShort};

use crate::{cache::Cache, helpers, oracle, proxy_pool, storage, utils, validation};

use super::{account, update, vault};

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

#[multiversx_sc::module]
pub trait PositionWithdrawModule:
    storage::Storage
    + validation::ValidationModule
    + oracle::OracleModule
    + common_events::EventsModule
    + utils::LendingUtilsModule
    + helpers::math::MathsModule
    + account::PositionAccountModule
    + common_math::SharedMathModule
    + vault::PositionVaultModule
    + update::PositionUpdateModule
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
    /// - `cache`: Mutable storage cache.
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
        cache: &mut Cache<Self>,
        position_attributes: &AccountAttributes<Self::Api>,
        is_swap: bool,
    ) -> AccountPosition<Self::Api> {
        let (token_id, _, requested_amount) = withdraw_payment.into_tuple();
        let feed = self.get_token_price(&token_id, cache);
        let pool_address = cache.get_cached_pool_address(&token_id);

        let mut deposit_position = self.get_deposit_position(account_nonce, &token_id);
        let amount = deposit_position.make_amount_decimal(&requested_amount);

        if position_attributes.is_vault() {
            self.process_vault_withdrawal(
                &token_id,
                &deposit_position.cap_amount(amount.clone()),
                is_liquidation,
                liquidation_fee,
                caller,
                pool_address,
                &feed,
                &mut deposit_position,
                is_swap,
            );
        } else {
            // The amount cap happens in the liquidity pool to account for the interest accrued after sync
            self.process_market_withdrawal(
                pool_address,
                caller,
                &amount,
                &mut deposit_position,
                is_liquidation,
                liquidation_fee,
                &feed,
            );
        };

        self.emit_position_update_event(
            &amount,
            &deposit_position,
            feed.price.clone(),
            caller,
            position_attributes,
        );

        self.update_or_remove_position(account_nonce, &deposit_position);

        deposit_position
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
    /// - `feed`: Price data for the token.
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
        feed: &PriceFeedShort<Self::Api>,
        deposit_position: &mut AccountPosition<Self::Api>,
        is_swap: bool,
    ) {
        self.update_vault_supplied_amount(token_id, amount, false);

        deposit_position.principal_amount -= amount;

        if is_liquidation && liquidation_fee_opt.is_some() {
            let liquidation_fee = unsafe { liquidation_fee_opt.unwrap_unchecked() };
            let amount_after_fee = amount.clone() - liquidation_fee.clone();
            self.transfer_withdrawn_assets(caller, token_id, &amount_after_fee, is_swap);
            self.transfer_liquidation_fee(pool_address, token_id, &liquidation_fee, feed);
        } else {
            self.transfer_withdrawn_assets(caller, token_id, amount, is_swap);
        }
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
    /// - `feed`: Price data for the token.
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
        feed: &PriceFeedShort<Self::Api>,
    ) {
        *deposit_position = self
            .tx()
            .to(pool_address)
            .typed(proxy_pool::LiquidityPoolProxy)
            .withdraw(
                caller,
                amount.clone(),
                deposit_position.clone(),
                is_liquidation,
                liquidation_fee,
                feed.price.clone(),
            )
            .returns(ReturnsResult)
            .sync_call();
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
        let opt_deposit_position = self.deposit_positions(account_nonce).get(token_id);
        require!(
            opt_deposit_position.is_some(),
            "Token {} is not available for this account",
            token_id
        );
        opt_deposit_position.unwrap()
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
    /// - `feed`: Price data for the token.
    fn transfer_liquidation_fee(
        &self,
        pool_address: ManagedAddress,
        token_id: &EgldOrEsdtTokenIdentifier,
        fee: &ManagedDecimal<Self::Api, NumDecimals>,
        feed: &PriceFeedShort<Self::Api>,
    ) {
        self.tx()
            .to(pool_address)
            .typed(proxy_pool::LiquidityPoolProxy)
            .add_protocol_revenue(feed.price.clone())
            .egld_or_single_esdt(token_id, 0, fee.into_raw_units())
            .returns(ReturnsResult)
            .sync_call();
    }
}
