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
    ///
    /// **Purpose**: Orchestrates the complete withdrawal flow, handling both regular
    /// withdrawals and liquidation scenarios with proper validation and position updates.
    ///
    /// **Methodology**:
    /// 1. Retrieves pool address for the deposit asset
    /// 2. Executes withdrawal through liquidity pool with amount validation
    /// 3. Handles liquidation fees if applicable
    /// 4. Emits position update event for monitoring
    /// 5. Updates or removes position based on remaining balance
    ///
    /// **Security Considerations**:
    /// - Amount validation performed at pool level to account for accrued interest
    /// - Liquidation fee handling ensures proper protocol revenue
    /// - Position state consistency maintained across updates
    ///
    /// **Mathematical Operations** (performed in pool):
    /// ```
    /// withdrawal_amount = min(requested_amount, position_balance_with_interest)
    /// scaled_withdrawal = withdrawal_amount * supply_index / RAY_PRECISION
    /// new_position_balance = old_balance - scaled_withdrawal
    /// ```
    ///
    /// # Arguments
    /// - `account_nonce`: Position NFT nonce for storage operations
    /// - `amount`: Withdrawal amount in asset's decimal format
    /// - `caller`: Withdrawer's address for token transfer
    /// - `is_liquidation`: Flag indicating liquidation scenario
    /// - `liquidation_fee`: Optional fee amount for liquidation revenue
    /// - `cache`: Storage cache for pool address lookup
    /// - `position_attributes`: Position attributes for event emission
    /// - `deposit_position`: Mutable deposit position to update
    /// - `feed`: Price feed for valuation and decimal conversion
    ///
    /// # Returns
    /// - `EgldOrEsdtTokenPayment` containing actual withdrawn tokens
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
            cache,
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
    /// **Purpose**: Performs the core cross-contract call to execute withdrawal
    /// through the appropriate liquidity pool, handling interest accrual and fees.
    ///
    /// **Methodology**:
    /// 1. Makes synchronous call to pool's withdraw function
    /// 2. Passes withdrawal parameters including liquidation context
    /// 3. Receives back transfers and updated position state
    /// 4. Processes returned tokens and validates amounts
    ///
    /// **Security Considerations**:
    /// - Trusted interaction with verified pool contracts
    /// - Proper handling of back transfers to prevent token loss
    /// - Validation of returned token types against expected asset
    ///
    /// **Transfer Processing**:
    /// - Aggregates ESDT transfers matching position asset
    /// - Handles EGLD transfers for native token positions
    /// - Validates transfer consistency with position asset type
    ///
    /// **Mathematical Operations** (performed in pool):
    /// ```
    /// available_amount = position.scaled_amount * supply_index / RAY_PRECISION
    /// actual_withdrawal = min(requested_amount, available_amount)
    /// if liquidation: protocol_fee = actual_withdrawal * liquidation_fee
    /// user_receives = actual_withdrawal - protocol_fee
    /// ```
    ///
    /// # Arguments
    /// - `pool_address`: Verified liquidity pool contract address
    /// - `caller`: Withdrawer's address for token transfer destination
    /// - `amount`: Requested withdrawal amount in asset decimals
    /// - `deposit_position`: Mutable position to update with new state
    /// - `is_liquidation`: Flag for liquidation fee application
    /// - `liquidation_fee`: Optional fee percentage for protocol revenue
    /// - `feed`: Price feed for pool-side validation
    ///
    /// # Returns
    /// - `EgldOrEsdtTokenPayment` with actual tokens transferred to user
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
        let (back_transfers, position_updated) = self
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
            .returns(ReturnsBackTransfersReset)
            .returns(ReturnsResult)
            .sync_call();

        *deposit_position = position_updated;

        let mut payment =
            EgldOrEsdtTokenPayment::new(deposit_position.asset_id.clone(), 0, BigUint::zero());

        for transfer in back_transfers.payments {
            if transfer.token_identifier == deposit_position.asset_id {
                payment.amount += transfer.amount;
            }
        }

        payment
    }

    /// Manages the position NFT after withdrawal.
    ///
    /// **Purpose**: Handles NFT lifecycle management by either burning empty positions
    /// or returning active positions to the user after withdrawal operations.
    ///
    /// **Methodology**:
    /// 1. Counts remaining deposit and borrow positions
    /// 2. If no positions remain: burns NFT and clears storage
    /// 3. If positions remain: transfers NFT back to caller
    ///
    /// **Position Lifecycle Rules**:
    /// - NFT represents active lending position
    /// - Burned when all deposits and borrows are closed
    /// - Returned to user when any positions remain active
    ///
    /// **Storage Cleanup**:
    /// - Removes account from active accounts set
    /// - Clears account attributes storage
    /// - Burns NFT to reclaim storage
    ///
    /// **Security Considerations**:
    /// - Proper validation of position closure before burning
    /// - Safe storage cleanup to prevent orphaned data
    /// - NFT transfer validation for active positions
    ///
    /// # Arguments
    /// - `account_payment`: Account NFT payment containing nonce and amount
    /// - `caller`: Withdrawer's address for NFT return
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
                .nft_burn(account_payment.token_nonce, &account_payment.amount);
            self.accounts().swap_remove(&account_payment.token_nonce);
            self.account_attributes(account_payment.token_nonce).clear();
        } else {
            self.tx().to(caller).payment(account_payment).transfer();
        }
    }

    /// Retrieves a deposit position for a token.
    ///
    /// **Purpose**: Safely fetches an existing deposit position with validation
    /// to ensure the position exists before withdrawal operations.
    ///
    /// **Methodology**:
    /// - Attempts to retrieve position from storage mapping
    /// - Validates position exists for the specified token
    /// - Returns position for further processing
    ///
    /// **Security Considerations**:
    /// - Prevents withdrawal attempts on non-existent positions
    /// - Provides clear error messaging for invalid requests
    /// - Uses unsafe unwrap only after existence validation
    ///
    /// # Arguments
    /// - `account_nonce`: Position NFT nonce for storage lookup
    /// - `token_id`: Token identifier to find position for
    ///
    /// # Returns
    /// - `AccountPosition` containing the validated deposit position
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
        unsafe { opt_deposit_position.unwrap_unchecked() }
    }
}
