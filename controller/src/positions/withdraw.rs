use common_errors::ERROR_WITHDRAW_TOKEN_RECEIVED;
use common_structs::{AccountAttributes, AccountPosition, AccountPositionType, PriceFeedShort};

use crate::{cache::Cache, helpers, oracle, proxy_pool, storage, utils, validation};

use super::{account, update};

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

#[multiversx_sc::module]
pub trait PositionWithdrawModule:
    storage::Storage
    + validation::ValidationModule
    + oracle::OracleModule
    + common_events::EventsModule
    + utils::LendingUtilsModule
    + helpers::MathsModule
    + account::PositionAccountModule
    + common_math::SharedMathModule
    + common_rates::InterestRates
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
        amount: ManagedDecimal<Self::Api, NumDecimals>,
        caller: &ManagedAddress,
        is_liquidation: bool,
        liquidation_fee: Option<ManagedDecimal<Self::Api, NumDecimals>>,
        cache: &mut Cache<Self>,
        position_attributes: &AccountAttributes<Self::Api>,
        deposit_position: &mut AccountPosition<Self::Api>,
        feed: &PriceFeedShort<Self::Api>,
    ) -> EgldOrEsdtTokenPayment<Self::Api> {
        let pool_address = cache.get_cached_pool_address(&deposit_position.asset_id);

        // The amount cap happens in the liquidity pool to account for the interest accrued after sync
        let payment = self.process_market_withdrawal(
            pool_address,
            caller,
            &amount,
            deposit_position,
            is_liquidation,
            liquidation_fee,
            feed,
        );

        self.emit_position_update_event(
            &amount,
            deposit_position,
            feed.price.clone(),
            caller,
            position_attributes,
        );

        self.update_or_remove_position(account_nonce, deposit_position);

        payment
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
    ) -> EgldOrEsdtTokenPayment<Self::Api> {
        let (position_updated, back_transfers) = self
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
            .returns(ReturnsBackTransfers)
            .sync_call();

        *deposit_position = position_updated;

        let mut payment =
            EgldOrEsdtTokenPayment::new(deposit_position.asset_id.clone(), 0, BigUint::zero());

        for esdt in back_transfers.esdt_payments {
            if esdt.token_identifier == deposit_position.asset_id {
                payment.amount += esdt.amount;
            }
        }
        if back_transfers.total_egld_amount > 0 {
            require!(
                deposit_position.asset_id.is_egld(),
                ERROR_WITHDRAW_TOKEN_RECEIVED
            );
            payment.amount += back_transfers.total_egld_amount;
        }

        payment
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
        let deposit_positions_count = self
            .positions(account_payment.token_nonce, AccountPositionType::Deposit)
            .len();
        let borrow_positions_count = self
            .positions(account_payment.token_nonce, AccountPositionType::Borrow)
            .len();

        // Burn NFT if position is fully closed
        if deposit_positions_count == 0 && borrow_positions_count == 0 {
            self.account()
                .nft_burn(account_payment.token_nonce, &BigUint::from(1u64));
            self.accounts().swap_remove(&account_payment.token_nonce);
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
        let opt_deposit_position = self
            .positions(account_nonce, AccountPositionType::Deposit)
            .get(token_id);
        require!(
            opt_deposit_position.is_some(),
            "Token {} is not available for this account",
            token_id
        );
        opt_deposit_position.unwrap()
    }
}
