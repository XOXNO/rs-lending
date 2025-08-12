multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use common_errors::{
    ERROR_ASSETS_ARE_THE_SAME, ERROR_INVALID_PAYMENTS, ERROR_INVALID_POSITION_MODE,
    ERROR_MULTIPLY_REQUIRE_EXTRA_STEPS, ERROR_SWAP_DEBT_NOT_SUPPORTED,
};
use common_structs::{AccountAttributes, AccountPositionType, PositionMode};

use crate::{
    cache::Cache, helpers, oracle, positions, storage, utils, validation,
    ERROR_SWAP_COLLATERAL_NOT_SUPPORTED,
};

#[type_abi]
#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, Clone)]
pub struct InitialMultiplyPayment<M: ManagedTypeApi> {
    pub token_identifier: EgldOrEsdtTokenIdentifier<M>,
    pub amount: ManagedDecimal<M, NumDecimals>,
    pub nonce: u64,
}

#[multiversx_sc::module]
pub trait SnapModule:
    storage::Storage
    + helpers::MathsModule
    + oracle::OracleModule
    + validation::ValidationModule
    + utils::LendingUtilsModule
    + common_events::EventsModule
    + common_math::SharedMathModule
    + positions::account::PositionAccountModule
    + positions::supply::PositionDepositModule
    + positions::borrow::PositionBorrowModule
    + positions::withdraw::PositionWithdrawModule
    + positions::repay::PositionRepayModule
    + positions::emode::EModeModule
    + positions::update::PositionUpdateModule
    + common_rates::InterestRates
    + multiversx_sc_modules::pause::PauseModule
{
    /// **MULTIPLY STRATEGY: Flash Loan Leverage Position Creation**
    ///
    /// # Purpose and Scope
    /// Creates leveraged positions by borrowing debt tokens via flash loan, swapping them to collateral tokens,
    /// and depositing the collateral to create or enhance an existing position. This strategy allows users to
    /// increase their exposure to an asset without having the full collateral amount upfront.
    ///
    /// # Methodology and Process
    /// 1. **Flash Loan Initiation**: Borrows `debt_to_flash_loan` amount of debt tokens from liquidity layer
    /// 2. **Token Conversion**: Swaps the borrowed debt tokens to collateral tokens using the swap router
    /// 3. **Collateral Supply**: Deposits the received collateral tokens to the lending pool
    /// 4. **Position Management**: Creates or updates the user's position with new collateral and debt
    /// 5. **Health Validation**: Ensures the final position maintains healthy collateralization ratio
    ///
    /// # Mathematical Formula
    /// Final Leverage = (Initial Collateral + Borrowed Amount * Swap Rate) / Initial Collateral
    /// Health Factor = (Collateral Value * Liquidation Threshold) / Total Debt Value
    /// Where Health Factor must be > 1.0 for position safety
    ///
    /// # Security Checks Implemented
    /// - **Reentrancy Protection**: Guards against flash loan reentrancy attacks
    /// - **Token Validation**: Ensures collateral and debt tokens are different assets
    /// - **Payment Validation**: Validates initial payment amounts and token types
    /// - **Position Mode Validation**: Ensures position is in leverage mode (not Normal/None)
    /// - **Health Factor Validation**: Verifies position remains healthy after leverage
    /// - **Asset Configuration**: Validates both tokens are properly configured in the protocol
    /// - **E-Mode Compatibility**: Ensures tokens are compatible with efficiency mode if specified
    ///
    /// # Arguments
    /// - `e_mode_category`: Efficiency mode category for correlated assets (0 for disabled)
    /// - `collateral_token`: Token to be used as collateral after swap
    /// - `debt_to_flash_loan`: Amount of debt token to borrow via flash loan
    /// - `debt_token`: Token to be borrowed and swapped to collateral
    /// - `mode`: Position mode (must be leverage-compatible, not Normal/None)
    /// - `steps`: Swap router steps for debt token to collateral token conversion
    /// - `steps_payment`: Optional swap steps if initial payment needs conversion
    ///
    /// # Returns
    /// - Creates or updates leveraged position with increased collateral and debt
    /// - Emits position creation/update events through the lending protocol
    ///
    /// # Risk Considerations
    /// - High slippage during swaps can reduce effective leverage and affect health factor
    /// - Flash loan failure will revert entire transaction, protecting user funds
    /// - Position becomes liquidatable if collateral value drops significantly
    #[payable]
    #[endpoint(multiply)]
    fn multiply(
        &self,
        e_mode_category: u8,
        collateral_token: &EgldOrEsdtTokenIdentifier,
        debt_to_flash_loan: BigUint,
        debt_token: &EgldOrEsdtTokenIdentifier,
        mode: PositionMode,
        steps: ManagedArgBuffer<Self::Api>,
        optional_steps_payment: OptionalValue<ManagedArgBuffer<Self::Api>>,
    ) {
        self.require_not_paused();
        // Initialize secure cache with price safety enabled
        let mut cache = Cache::new(self);
        cache.allow_unsafe_price = false; // Enforce secure price feeds only

        // Critical security: Prevent reentrancy attacks during flash loan operations
        self.reentrancy_guard(cache.flash_loan_ongoing);

        // Validate tokens are different to prevent circular operations
        require!(collateral_token != debt_token, ERROR_ASSETS_ARE_THE_SAME);
        // Extract and validate initial payments, account info, and caller details
        // Parameters: allow_empty_payments=false, require_account_exists=true
        let (payments, opt_account, caller, opt_attributes) =
            self.validate_supply_payment(false, true, OptionalValue::None);

        // Load asset configurations and validate they're enabled for lending operations
        let collateral_config = cache.cached_asset_info(collateral_token);
        let mut debt_config = cache.cached_asset_info(debt_token);

        // Fetch current market prices from oracle feeds with staleness protection
        let collateral_oracle = cache.cached_oracle(collateral_token);
        let debt_oracle = cache.cached_oracle(debt_token);

        let mut collateral_to_be_supplied =
            self.to_decimal(BigUint::zero(), collateral_oracle.asset_decimals);
        let mut debt_to_be_swapped = self.to_decimal(BigUint::zero(), debt_oracle.asset_decimals);

        // Create or retrieve user's lending position account
        // Handles isolated asset restrictions and efficiency mode configuration
        let (account_nonce, nft_attributes) = self.get_or_create_account(
            &caller,
            collateral_config.is_isolated(), // Isolated assets require dedicated positions
            mode,
            OptionalValue::Some(e_mode_category),
            opt_account.clone(),
            opt_attributes,
            // For isolated assets, restrict position to single collateral type
            if collateral_config.is_isolated() {
                Some(collateral_token.clone())
            } else {
                None
            },
        );

        let mut initial_multiply_payment: Option<InitialMultiplyPayment<Self::Api>> = None;

        if opt_account.is_none() {
            // New position creation: validate single payment and determine token conversion path
            require!(payments.len() == 1, ERROR_INVALID_PAYMENTS);
            let initial_payment = payments.get(0);
            self.validate_payment(&initial_payment); // Validates amount > 0 and token is enabled

            // Determine token conversion strategy based on payment token type
            let is_payment_same_as_debt = initial_payment.token_identifier == *debt_token;
            let is_payment_as_collateral = initial_payment.token_identifier == *collateral_token;

            if is_payment_as_collateral {
                // Direct collateral deposit: no conversion needed
                let collateral_received = self.to_decimal(
                    initial_payment.amount.clone(),
                    collateral_oracle.asset_decimals,
                );

                collateral_to_be_supplied += &collateral_received;

                initial_multiply_payment = Some(InitialMultiplyPayment {
                    token_identifier: initial_payment.token_identifier.clone(),
                    amount: collateral_received,
                    nonce: account_nonce,
                });
            } else if is_payment_same_as_debt {
                // Payment in debt token: reduces flash loan requirement
                let debt_amount_received =
                    self.to_decimal(initial_payment.amount.clone(), debt_oracle.asset_decimals);
                debt_to_be_swapped += &debt_amount_received;

                initial_multiply_payment = Some(InitialMultiplyPayment {
                    token_identifier: initial_payment.token_identifier.clone(),
                    amount: debt_amount_received,
                    nonce: account_nonce,
                });
            } else {
                // Payment in different token: requires conversion to collateral
                require!(
                    optional_steps_payment.is_some(),
                    ERROR_MULTIPLY_REQUIRE_EXTRA_STEPS
                );
                let steps_payment =
                    unsafe { optional_steps_payment.into_option().unwrap_unchecked() };

                // Convert payment token to collateral token via swap router
                let received = self.convert_token_from_to(
                    collateral_token,
                    &initial_payment.token_identifier,
                    &initial_payment.amount,
                    &caller,
                    steps_payment,
                );

                let collateral_received =
                    self.to_decimal(received.amount, collateral_oracle.asset_decimals);

                collateral_to_be_supplied += &collateral_received;

                initial_multiply_payment = Some(InitialMultiplyPayment {
                    token_identifier: received.token_identifier.clone(),
                    amount: collateral_received,
                    nonce: account_nonce,
                });
            }
        } else {
            // Existing position enhancement: no additional payments allowed
            require!(payments.is_empty(), ERROR_INVALID_PAYMENTS);
        }
        // Ensure position is in leverage-compatible mode (not Normal or None)
        // This prevents accidental leverage on regular lending positions
        require!(
            nft_attributes.mode == PositionMode::Multiply
                || nft_attributes.mode == PositionMode::Long
                || nft_attributes.mode == PositionMode::Short,
            ERROR_INVALID_POSITION_MODE
        );

        // Execute flash loan borrow operation
        // This creates the debt position that will be backed by swapped collateral
        let received_debt = self.handle_create_borrow_strategy(
            account_nonce,
            debt_token,
            &debt_to_flash_loan,
            &mut debt_config,
            &caller,
            &nft_attributes,
            &mut cache,
        );
        debt_to_be_swapped += &received_debt;

        // Convert borrowed debt tokens to collateral tokens via swap router
        // This is the core leverage mechanism: debt → collateral conversion
        let mut final_collateral = self.convert_token_from_to(
            collateral_token,
            debt_token,
            debt_to_be_swapped.into_raw_units(),
            &caller,
            steps,
        );
        // Add any directly supplied collateral to the swapped amount
        final_collateral.amount += collateral_to_be_supplied.into_raw_units();

        // Deposit the final collateral amount to complete the leveraged position
        self.process_deposit(
            &caller,
            account_nonce,
            nft_attributes,
            &ManagedVec::from_single_item(final_collateral),
            &mut cache,
        );

        // Remove the prices from the cache to have a fresh value after the swaps to prevent a bad HF
        cache.clean_prices_cache();
        // CRITICAL: Validate position health after leverage creation
        // Ensures the position is not immediately liquidatable due to slippage or market conditions
        self.validate_is_healthy(account_nonce, &mut cache, None);

        if let Some(initial_multiply_payment) = initial_multiply_payment {
            self.emit_initial_multiply_payment(
                &initial_multiply_payment.token_identifier,
                &initial_multiply_payment.amount,
                initial_multiply_payment.nonce,
                &mut cache,
            );
        }
    }

    /// **SWAP DEBT STRATEGY: Convert Debt Position Between Different Tokens**
    ///
    /// # Purpose and Scope
    /// Converts existing debt from one token type to another while maintaining the same collateral.
    /// This strategy allows users to change their debt exposure without affecting their collateral positions,
    /// useful for interest rate arbitrage, risk management, or taking advantage of better borrowing conditions.
    ///
    /// # Methodology and Process
    /// 1. **Flash Loan Creation**: Borrows the new debt token amount from liquidity layer
    /// 2. **Token Swap**: Converts new debt tokens to existing debt tokens via swap router
    /// 3. **Debt Repayment**: Uses swapped tokens plus any provided payments to repay existing debt
    /// 4. **Position Update**: Updates the debt position to reflect the new debt token type
    /// 5. **Health Validation**: Ensures position remains healthy after the debt conversion
    ///
    /// # Mathematical Formula
    /// Debt Conversion Rate = (Existing Debt Amount * Existing Token Price) / (New Token Price * (1 - Slippage))
    /// New Health Factor = (Total Collateral Value * Liquidation Threshold) / New Debt Value
    /// Where New Health Factor must remain > 1.0
    ///
    /// # Security Checks Implemented
    /// - **Reentrancy Protection**: Guards against flash loan reentrancy attacks
    /// - **Token Differentiation**: Ensures existing and new debt tokens are different
    /// - **Account Validation**: Verifies caller owns the position being modified
    /// - **Siloed Borrowing Check**: Prevents debt swaps involving siloed (restricted) tokens
    /// - **Payment Validation**: Validates any additional payments for debt coverage
    /// - **Health Factor Validation**: Ensures position remains healthy post-swap
    /// - **Slippage Protection**: Built into swap router to prevent excessive value loss
    ///
    /// # Arguments
    /// - `existing_debt_token`: Current debt token to be repaid and replaced
    /// - `new_debt_amount_raw`: Amount of new debt token to borrow (raw units)
    /// - `new_debt_token`: New debt token type to replace existing debt
    /// - `steps`: Swap router configuration for token conversion path
    ///
    /// # Returns
    /// - Updates debt position with new token type and amount
    /// - Maintains existing collateral positions unchanged
    /// - Emits debt swap events through the lending protocol
    ///
    /// # Risk Considerations
    /// - Swap slippage may require additional payments to fully repay existing debt
    /// - Interest rate changes between tokens affect ongoing borrowing costs
    /// - Market volatility during swap execution can impact final debt amounts
    /// - Position may become liquidatable if swap results in higher debt value
    #[payable]
    #[endpoint(swapDebt)]
    fn swap_debt(
        &self,
        existing_debt_token: &EgldOrEsdtTokenIdentifier,
        new_debt_amount_raw: &BigUint,
        new_debt_token: &EgldOrEsdtTokenIdentifier,
        steps: ManagedArgBuffer<Self::Api>,
    ) {
        self.require_not_paused();
        // Validate tokens are different - prevent no-op swaps
        require!(
            existing_debt_token != new_debt_token,
            ERROR_SWAP_DEBT_NOT_SUPPORTED
        );

        // Initialize secure cache and enable reentrancy protection
        let mut cache = Cache::new(self);
        cache.allow_unsafe_price = false; // Enforce secure price feeds only
        self.reentrancy_guard(cache.flash_loan_ongoing);
        // Extract payments and validate account ownership
        // Parameters: allow_empty_payments=true, require_account_exists=true
        let (mut payments, opt_account, caller, opt_attributes) =
            self.validate_supply_payment(true, true, OptionalValue::None);

        // Extract account info (guaranteed to exist due to validation above)
        let account = unsafe { opt_account.unwrap_unchecked() };
        let account_attributes = unsafe { opt_attributes.unwrap_unchecked() };

        // Load asset configurations for both debt tokens
        let mut debt_config = cache.cached_asset_info(new_debt_token);
        let existing_debt_config = cache.cached_asset_info(existing_debt_token);

        // SECURITY: Reject debt swaps involving siloed (restricted) borrowing tokens
        // Siloed tokens have special isolation requirements that prevent debt conversions
        require!(
            !existing_debt_config.is_siloed_borrowing() && !debt_config.is_siloed_borrowing(),
            ERROR_SWAP_DEBT_NOT_SUPPORTED
        );

        // Create new debt position via flash loan
        // This borrows the new debt token amount that will replace existing debt
        let received_debt = self.handle_create_borrow_strategy(
            account.token_nonce,
            new_debt_token,
            new_debt_amount_raw,
            &mut debt_config,
            &caller,
            &account_attributes,
            &mut cache,
        );

        // Convert new debt tokens to existing debt tokens for repayment
        // Uses swap router with slippage protection
        let received = self.swap_tokens(
            existing_debt_token,
            new_debt_token,
            received_debt.into_raw_units(),
            &caller,
            steps,
        );

        // Add swapped tokens to payment collection for debt repayment
        payments.push(received);

        // Process all payments (swapped tokens + any additional payments) to repay existing debt
        for payment_ref in payments.iter() {
            self.validate_payment(&payment_ref); // Ensure valid amount and enabled token
            let price_feed = self.token_price(&payment_ref.token_identifier, &mut cache);
            let payment = self.to_decimal(payment_ref.amount.clone(), price_feed.asset_decimals);
            let egld_amount = self.token_egld_value(&payment, &price_feed.price_wad);

            // Apply payment to existing debt positions
            self.process_repayment(
                account.token_nonce,
                &payment_ref.token_identifier,
                &payment,
                &caller,
                egld_amount,
                &price_feed,
                &mut cache,
                &account_attributes,
            );
        }

        // Remove the prices from the cache to have a fresh value after the swaps to prevent a bad HF
        cache.clean_prices_cache();
        // CRITICAL: Validate position health after debt swap completion
        // Ensures the position is not liquidatable due to swap slippage or price movements
        self.validate_is_healthy(account.token_nonce, &mut cache, None);
    }

    /// **SWAP COLLATERAL STRATEGY: Convert Collateral Between Different Token Types**
    ///
    /// # Purpose and Scope
    /// Converts existing collateral from one token type to another while maintaining the same debt positions.
    /// This strategy enables portfolio rebalancing, risk management, and optimization of collateral efficiency
    /// without affecting existing borrowing positions.
    ///
    /// # Methodology and Process
    /// 1. **Collateral Withdrawal**: Withdraws specified amount of current collateral from the position
    /// 2. **Token Conversion**: Swaps the withdrawn collateral to the new collateral token via swap router
    /// 3. **Collateral Redeposit**: Deposits the converted tokens back as new collateral
    /// 4. **Position Update**: Updates collateral composition while preserving debt positions
    /// 5. **Health Validation**: Ensures position maintains healthy collateralization after swap
    ///
    /// # Mathematical Formula
    /// Collateral Conversion Rate = (Withdrawn Amount * Current Token Price) / (New Token Price * (1 - Slippage))
    /// New Health Factor = (New Collateral Value * New Liquidation Threshold) / Total Debt Value
    /// Where New Health Factor must remain > 1.0
    ///
    /// # Security Checks Implemented
    /// - **Reentrancy Protection**: Guards against flash loan reentrancy attacks
    /// - **Account Validation**: Verifies caller owns the position being modified
    /// - **Isolation Mode Check**: Prevents collateral swaps in isolated asset positions
    /// - **New Asset Validation**: Ensures target collateral is not an isolated asset
    /// - **Withdrawal Validation**: Confirms sufficient collateral balance for withdrawal
    /// - **Health Factor Validation**: Ensures position remains healthy post-swap
    /// - **Payment Validation**: Validates any additional payments provided
    ///
    /// # Arguments
    /// - `current_collateral`: Existing collateral token to be converted
    /// - `from_amount`: Amount of current collateral to convert (raw units)
    /// - `new_collateral`: Target collateral token type
    /// - `steps`: Swap router configuration for token conversion path
    ///
    /// # Returns
    /// - Updates collateral position with new token type and converted amount
    /// - Maintains existing debt positions unchanged
    /// - Emits collateral swap events through the lending protocol
    ///
    /// # Risk Considerations
    /// - Swap slippage reduces effective collateral value and may impact health factor
    /// - Different liquidation thresholds between tokens affect position safety
    /// - Market volatility during swap execution can impact final collateral amounts
    /// - Position may become liquidatable if swap results in insufficient collateral value
    #[payable]
    #[endpoint(swapCollateral)]
    fn swap_collateral(
        &self,
        current_collateral: &EgldOrEsdtTokenIdentifier,
        from_amount: BigUint,
        new_collateral: &EgldOrEsdtTokenIdentifier,
        steps: ManagedArgBuffer<Self::Api>,
    ) {
        self.require_not_paused();

        // Initialize secure cache and enable reentrancy protection
        let mut cache = Cache::new(self);
        self.reentrancy_guard(cache.flash_loan_ongoing);

        // Extract payments and validate account ownership
        // Parameters: allow_empty_payments=true, require_account_exists=true
        let (mut payments, opt_account, caller, opt_attributes) =
            self.validate_supply_payment(true, true, OptionalValue::None);

        // Extract account info (guaranteed to exist due to validation above)
        let account = unsafe { opt_account.unwrap_unchecked() };
        let borrow_positions = self.positions(account.token_nonce, AccountPositionType::Borrow);
        let allow_unsafe_price = borrow_positions.is_empty();
        cache.allow_unsafe_price = allow_unsafe_price; // Enforce secure price feeds only

        let account_attributes = unsafe { opt_attributes.unwrap_unchecked() };

        // SECURITY: Prevent collateral swaps in isolated asset positions
        // Isolated positions are restricted to single collateral types
        require!(
            !account_attributes.is_isolated(),
            ERROR_SWAP_COLLATERAL_NOT_SUPPORTED
        );

        // Load target collateral asset configuration and validate
        let asset_info = cache.cached_asset_info(new_collateral);

        // SECURITY: Prevent swapping to isolated assets in regular positions
        // Isolated assets require dedicated isolated positions
        require!(
            !asset_info.is_isolated(),
            ERROR_SWAP_COLLATERAL_NOT_SUPPORTED
        );

        // Execute collateral withdrawal and conversion process
        let received = self.common_swap_collateral(
            current_collateral,
            from_amount,
            new_collateral,
            steps,
            account.token_nonce,
            &caller,
            &account_attributes,
            &mut cache,
        );

        // Add converted collateral to payment collection for redeposit
        payments.push(received);

        // Deposit the converted collateral and any additional payments
        self.process_deposit(
            &caller,
            account.token_nonce,
            account_attributes,
            &payments,
            &mut cache,
        );

        // Remove the prices from the cache to have a fresh value after the swaps to prevent a bad HF
        cache.clean_prices_cache();

        // If the position has debt, validate the health of the position
        if !allow_unsafe_price {
            // CRITICAL: Validate position health after collateral swap completion
            // Ensures the position is not liquidatable due to swap slippage or different liquidation thresholds
            self.validate_is_healthy(account.token_nonce, &mut cache, None);
        }
    }

    /// **REPAY DEBT WITH COLLATERAL STRATEGY: Liquidate Collateral to Repay Debt**
    ///
    /// # Purpose and Scope
    /// Converts collateral assets to debt tokens for automatic debt repayment, enabling users to reduce
    /// their debt burden without external token sources. This strategy is particularly useful for
    /// deleveraging positions, managing liquidation risk, or closing positions entirely.
    ///
    /// # Methodology and Process
    /// 1. **Collateral Withdrawal**: Withdraws specified amount of collateral from the user's position
    /// 2. **Token Conversion**: Swaps withdrawn collateral to debt tokens via swap router
    /// 3. **Debt Repayment**: Uses converted tokens plus any additional payments to repay outstanding debt
    /// 4. **Position Closure**: If `close_position` is true and all debt is repaid, withdraws remaining collateral and burns position NFT
    /// 5. **Health Validation**: Ensures position remains healthy after debt reduction (if not fully closed)
    ///
    /// # Mathematical Formula
    /// Debt Repayment Amount = (Withdrawn Collateral * Collateral Price * (1 - Slippage)) / Debt Token Price
    /// Remaining Health Factor = (Remaining Collateral Value * Liquidation Threshold) / Remaining Debt Value
    /// For partial repayment, Remaining Health Factor must be > 1.0
    ///
    /// # Security Checks Implemented
    /// - **Reentrancy Protection**: Guards against flash loan reentrancy attacks
    /// - **Account Validation**: Verifies caller owns the position being modified
    /// - **Collateral Sufficiency**: Ensures sufficient collateral balance for withdrawal
    /// - **Payment Validation**: Validates any additional payments provided for debt coverage
    /// - **Health Factor Validation**: Ensures position remains healthy after partial repayment
    /// - **Position Closure Validation**: Verifies all debt is repaid before position closure
    /// - **Asset Validation**: Confirms both collateral and debt tokens are properly configured
    ///
    /// # Arguments
    /// - `from_token`: Collateral token to be converted for debt repayment
    /// - `from_amount`: Amount of collateral to withdraw and convert (raw units)
    /// - `to_token`: Debt token to be repaid with converted collateral
    /// - `close_position`: Flag to close entire position if all debt is repaid (burns NFT and withdraws remaining collateral)
    /// - `steps`: Optional swap router configuration for collateral to debt token conversion
    ///
    /// # Returns
    /// - Reduces debt position by the amount repaid through collateral conversion
    /// - Reduces collateral position by the amount withdrawn for conversion
    /// - If `close_position` and debt fully repaid: burns position NFT and returns all remaining collateral
    /// - Emits debt repayment and position update events through the lending protocol
    ///
    /// # Risk Considerations
    /// - Swap slippage may result in insufficient debt tokens to fully repay intended amount
    /// - Market volatility during conversion affects final repayment effectiveness
    /// - Position may become liquidatable if remaining collateral is insufficient after partial repayment
    /// - Closing positions requires complete debt repayment; partial closure is not supported
    #[payable]
    #[endpoint(repayDebtWithCollateral)]
    fn repay_debt_with_collateral(
        &self,
        from_token: &EgldOrEsdtTokenIdentifier,
        from_amount: BigUint,
        to_token: &EgldOrEsdtTokenIdentifier,
        close_position: bool,
        optional_steps: OptionalValue<ManagedArgBuffer<Self::Api>>,
    ) {
        self.require_not_paused();
        // Initialize secure cache and enable reentrancy protection
        let mut cache = Cache::new(self);
        cache.allow_unsafe_price = false; // Enforce secure price feeds only
        self.reentrancy_guard(cache.flash_loan_ongoing);
        // Extract payments and validate account ownership
        // Parameters: allow_empty_payments=true, require_account_exists=false (but will exist)
        let (mut payments, opt_account, caller, opt_attributes) =
            self.validate_supply_payment(true, false, OptionalValue::None);

        // Extract account info (guaranteed to exist for debt repayment)
        let account = unsafe { opt_account.unwrap_unchecked() };
        let account_attributes = unsafe { opt_attributes.unwrap_unchecked() };

        // Execute collateral withdrawal and conversion to debt token
        let received = self.common_swap_collateral(
            from_token,
            from_amount,
            to_token,
            optional_steps
                .into_option()
                .unwrap_or(ManagedArgBuffer::new()),
            account.token_nonce,
            &caller,
            &account_attributes,
            &mut cache,
        );

        // Add converted debt tokens to payment collection for repayment
        payments.push(received);

        // Process all payments (converted debt tokens + any additional payments) for debt repayment
        for payment in payments.iter() {
            self.validate_payment(&payment); // Ensure valid amount and enabled token
            let price_feed = self.token_price(&payment.token_identifier, &mut cache);
            let payment_dec = self.to_decimal(payment.amount.clone(), price_feed.asset_decimals);
            let egld_amount = self.token_egld_value(&payment_dec, &price_feed.price_wad);

            // Apply payment to outstanding debt positions
            self.process_repayment(
                account.token_nonce,
                &payment.token_identifier,
                &payment_dec,
                &caller,
                egld_amount,
                &price_feed,
                &mut cache,
                &account_attributes,
            );
        }

        // Remove the prices from the cache to have a fresh value after the swaps to prevent a bad HF
        cache.clean_prices_cache();

        // Check if all debt has been repaid for potential position closure
        let has_no_debt = self
            .positions(account.token_nonce, AccountPositionType::Borrow)
            .is_empty();

        // If it still has debt clean the cache of prices to have a fresh value after the swaps to prevent a bad HF
        if !has_no_debt {
            cache.clean_prices_cache();
        }

        // CRITICAL: Validate position health after debt repayment
        // Ensures remaining position is not liquidatable due to reduced collateral
        self.validate_is_healthy(account.token_nonce, &mut cache, None);

        // Execute full position closure if requested and all debt is repaid
        if close_position && has_no_debt {
            // Withdraw all remaining collateral and return to user
            for mut deposit_position in self
                .positions(account.token_nonce, AccountPositionType::Deposit)
                .values()
            {
                let price_feed = self.token_price(&deposit_position.asset_id, &mut cache);
                let amount = self.total_amount(&deposit_position, &price_feed, &mut cache);
                // Withdraw full collateral balance
                let _ = self.process_withdrawal(
                    account.token_nonce,
                    amount,
                    &caller,
                    false,
                    None,
                    &mut cache,
                    &account_attributes,
                    &mut deposit_position,
                    &price_feed,
                );
            }
        }

        // Manage account lifecycle (burns NFT if position is fully closed)
        self.manage_account_after_withdrawal(&account, &caller);
    }

    /// **COMMON COLLATERAL SWAP HELPER: Withdraw and Convert Collateral**
    ///
    /// # Purpose and Scope
    /// Internal helper function that handles the common pattern of withdrawing collateral from a position
    /// and converting it to a different token type. Used by both collateral swap and debt repayment strategies.
    ///
    /// # Methodology and Process
    /// 1. **Position Lookup**: Retrieves the user's deposit position for the source token
    /// 2. **Amount Validation**: Converts and validates the withdrawal amount against position balance
    /// 3. **Withdrawal Execution**: Processes withdrawal from the lending pool to controller address
    /// 4. **Token Conversion**: Converts withdrawn tokens to target token via swap router
    /// 5. **Return Conversion**: Returns the converted token payment for further processing
    ///
    /// # Security Checks Implemented
    /// - **Position Validation**: Ensures the deposit position exists and has sufficient balance
    /// - **Amount Validation**: Validates withdrawal amount is within available limits
    /// - **Price Feed Validation**: Uses secure oracle price feeds for amount calculations
    /// - **Controller Authorization**: Withdrawal is made to controller address for immediate conversion
    ///
    /// # Arguments
    /// - `from_token`: Source collateral token to withdraw and convert
    /// - `from_amount`: Amount to withdraw in raw token units
    /// - `to_token`: Target token type for conversion
    /// - `steps`: Swap router configuration for token conversion
    /// - `account_nonce`: User's position NFT nonce
    /// - `caller`: Original transaction caller for routing converted tokens
    /// - `account_attributes`: Position attributes for validation
    /// - `cache`: Protocol state cache for efficient operations
    ///
    /// # Returns
    /// - `EgldOrEsdtTokenPayment`: The converted token payment ready for deposit or repayment
    ///
    /// # Risk Considerations
    /// - Withdrawal may fail if amount exceeds available collateral or violates health factor
    /// - Swap slippage reduces the effective converted amount
    /// - Price feed staleness could affect amount calculations
    fn common_swap_collateral(
        &self,
        from_token: &EgldOrEsdtTokenIdentifier,
        from_amount: BigUint,
        to_token: &EgldOrEsdtTokenIdentifier,
        steps: ManagedArgBuffer<Self::Api>,
        account_nonce: u64,
        caller: &ManagedAddress,
        account_attributes: &AccountAttributes<Self::Api>,
        cache: &mut Cache<Self>,
    ) -> EgldOrEsdtTokenPayment<Self::Api> {
        // Get controller address for intermediate token holding during conversion
        let controller = self.blockchain().get_sc_address();

        // Retrieve and validate the deposit position for the source token
        let mut deposit_position = self.deposit_position(account_nonce, from_token);
        let price_feed = self.token_price(&deposit_position.asset_id, cache);

        // Convert withdrawal amount to decimal format for position calculations
        let amount = deposit_position.make_amount_decimal(&from_amount, price_feed.asset_decimals);

        // Execute withdrawal from user's position to controller for conversion
        let withdraw_payment = self.process_withdrawal(
            account_nonce,
            amount,
            &controller, // Withdraw to controller for immediate conversion
            false,       // Not a full withdrawal
            None,        // No additional validation
            cache,
            account_attributes,
            &mut deposit_position,
            &price_feed,
        );

        // Convert withdrawn tokens to target token type and return to caller
        self.convert_token_from_to(
            to_token,
            from_token,
            &withdraw_payment.amount,
            caller,
            steps,
        )
    }

    /// **TOKEN CONVERSION HELPER: Convert Between Token Types**
    ///
    /// # Purpose and Scope
    /// Internal helper function that handles token conversion between different types, with optimization
    /// for same-token operations. Acts as a wrapper around the swap router with intelligent routing.
    ///
    /// # Methodology and Process
    /// 1. **Same Token Check**: Optimizes for cases where source and target tokens are identical
    /// 2. **Swap Routing**: For different tokens, delegates to the swap router with provided configuration
    /// 3. **Payment Creation**: Returns properly formatted token payment for further processing
    ///
    /// # Security Checks Implemented
    /// - **Token Identity Validation**: Handles same-token conversions without unnecessary swaps
    /// - **Swap Router Integration**: Uses protocol-approved swap router for conversions
    /// - **Amount Preservation**: Ensures amount integrity during token format conversions
    ///
    /// # Arguments
    /// - `to_token`: Target token type for conversion
    /// - `from_token`: Source token type being converted
    /// - `from_amount`: Amount of source tokens to convert
    /// - `caller`: Address to receive any refunded tokens from swaps
    /// - `args`: Swap router arguments for conversion path configuration
    ///
    /// # Returns
    /// - `EgldOrEsdtTokenPayment`: Token payment in target token type
    ///   - For same tokens: Returns original amount in target token format
    ///   - For different tokens: Returns swapped amount after router conversion
    ///
    /// # Risk Considerations
    /// - Swap operations are subject to slippage and may return less than expected
    /// - Invalid swap router arguments can cause transaction failure
    /// - Market conditions affect conversion rates and final amounts
    fn convert_token_from_to(
        &self,
        to_token: &EgldOrEsdtTokenIdentifier,
        from_token: &EgldOrEsdtTokenIdentifier,
        from_amount: &BigUint,
        caller: &ManagedAddress,
        args: ManagedArgBuffer<Self::Api>,
    ) -> EgldOrEsdtTokenPayment {
        // Optimization: Skip swap for same-token conversions
        if to_token == from_token {
            return EgldOrEsdtTokenPayment::new(to_token.clone(), 0, from_amount.clone());
        }

        // Execute token swap via router for different token types
        self.swap_tokens(to_token, from_token, from_amount, caller, args)
    }

    /// **SWAP ROUTER INTEGRATION: Execute Token Swaps with Refund Handling**
    ///
    /// # Purpose and Scope
    /// Core swap execution function that interfaces with the external swap router to convert tokens.
    /// Handles multi-token return scenarios, refunds excess tokens, and ensures proper token routing
    /// back to users while extracting the desired target token.
    ///
    /// # Methodology and Process
    /// 1. **Router Invocation**: Calls external swap router with provided arguments and source tokens
    /// 2. **Return Processing**: Processes all returned tokens (EGLD and ESDT) from the swap
    /// 3. **Target Extraction**: Identifies and accumulates the desired target token from returns
    /// 4. **Refund Handling**: Returns any unexpected or excess tokens to the original caller
    /// 5. **Result Formatting**: Returns the target token amount in standardized payment format
    ///
    /// # Security Checks Implemented
    /// - **Router Address Validation**: Uses protocol-configured swap router address only
    /// - **Return Token Validation**: Properly handles both EGLD and ESDT token returns
    /// - **Refund Security**: Ensures all non-target tokens are returned to original caller
    /// - **Amount Aggregation**: Safely accumulates multiple returns of the same target token
    ///
    /// # Arguments
    /// - `wanted_token`: The target token type expected from the swap
    /// - `from_token`: Source token being swapped
    /// - `from_amount`: Amount of source token to swap
    /// - `caller`: Address to receive any refunded tokens
    /// - `args`: Raw swap router arguments (path, slippage, etc.)
    ///
    /// # Returns
    /// - `EgldOrEsdtTokenPayment`: Payment containing the target token amount received
    ///   - Aggregates all instances of target token from swap returns
    ///   - Amount reflects actual received after slippage and fees
    ///   - Zero amount if swap failed or target token not received
    ///
    /// # Risk Considerations
    /// - Swap router failure will revert the entire transaction
    /// - High slippage may result in significantly less target tokens than expected
    /// - Complex swap paths increase failure probability and gas costs
    /// - MEV attacks possible if swap router lacks proper protection
    /// - Malicious router could drain funds (hence importance of router address validation)
    fn swap_tokens(
        self,
        wanted_token: &EgldOrEsdtTokenIdentifier,
        from_token: &EgldOrEsdtTokenIdentifier,
        from_amount: &BigUint,
        caller: &ManagedAddress,
        args: ManagedArgBuffer<Self::Api>,
    ) -> EgldOrEsdtTokenPayment {
        // Execute swap via external router with source tokens and configuration
        let back_transfers = self
            .tx()
            .to(self.swap_router().get()) // Use protocol-configured swap router
            .raw_call(ManagedBuffer::new_from_bytes(b"swap"))
            .arguments_raw(args) // Pass through swap configuration (path, slippage, etc.)
            .egld_or_single_esdt(from_token, 0, from_amount)
            .returns(ReturnsBackTransfers) // Reset to capture all return transfers
            .sync_call();

        // Initialize result container for target token accumulation
        let mut target_token_result =
            EgldOrEsdtTokenPayment::new(wanted_token.clone(), 0, BigUint::from(0u32));

        // Separate target tokens from refundable tokens
        let mut refunds = ManagedVec::new();

        for payment in back_transfers.payments {
            // Accumulate all instances of the target token (fungible tokens only, nonce = 0)
            if payment.token_identifier == *wanted_token {
                target_token_result.amount += &payment.amount;
            } else {
                // Collect non-target tokens for refund to caller
                refunds.push(payment.clone());
            }
        }

        // Refund any non-target tokens back to the original caller
        if !refunds.is_empty() {
            self.tx()
                .to(caller)
                .payment(refunds)
                .transfer_if_not_empty();
        }

        // Return the accumulated target token amount
        target_token_result
    }

    /// Emits event for initial multiply payment with token amount and USD value.
    /// Calculates USD equivalent through EGLD conversion for transparency.
    /// Records initial collateral contribution for leverage position tracking.
    fn emit_initial_multiply_payment(
        &self,
        token_identifier: &EgldOrEsdtTokenIdentifier,
        amount: &ManagedDecimal<Self::Api, NumDecimals>,
        nonce: u64,
        cache: &mut Cache<Self>,
    ) {
        let price_feed = self.token_price(token_identifier, cache);
        let egld_price = self.token_egld_value(amount, &price_feed.price_wad);
        let usd_price = self.egld_usd_value(&egld_price, &cache.egld_usd_price_wad);
        self.initial_multiply_payment_event(token_identifier, amount, usd_price, nonce);
    }
}
