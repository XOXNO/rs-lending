multiversx_sc::imports!();

use common_errors::{
    ERROR_INITIAL_COLLATERAL_OVER_FINAL_COLLATERAL, ERROR_MULTIPLY_STRATEGY_REQUIRES_FLASH_LOAN,
    ERROR_SWAP_DEBT_NOT_SUPPORTED,
};
use common_events::AccountAttributes;

use crate::{
    cache::Cache,
    helpers, oracle, positions,
    proxy_ashswap::{AggregatorStep, TokenAmount},
    proxy_pool, storage, utils, validation, ERROR_ASSET_NOT_BORROWABLE,
    ERROR_ASSET_NOT_SUPPORTED_AS_COLLATERAL, ERROR_SWAP_COLLATERAL_NOT_SUPPORTED,
};

#[multiversx_sc::module]
pub trait SnapModule:
    storage::Storage
    + helpers::math::MathsModule
    + oracle::OracleModule
    + validation::ValidationModule
    + utils::LendingUtilsModule
    + common_events::EventsModule
    + common_math::SharedMathModule
    + helpers::strategies::StrategiesModule
    + positions::account::PositionAccountModule
    + positions::supply::PositionDepositModule
    + positions::borrow::PositionBorrowModule
    + positions::withdraw::PositionWithdrawModule
    + positions::repay::PositionRepayModule
    + positions::vault::PositionVaultModule
    + positions::emode::EModeModule
{
    #[payable]
    #[endpoint]
    #[allow_multiple_var_args]
    fn multiply(
        &self,
        e_mode_category: u8,
        collateral_token: &EgldOrEsdtTokenIdentifier,
        final_collateral_amount: BigUint,
        debt_token: &EgldOrEsdtTokenIdentifier,
        mode: ManagedBuffer,
        steps: OptionalValue<ManagedVec<AggregatorStep<Self::Api>>>,
        limits: OptionalValue<ManagedVec<TokenAmount<Self::Api>>>,
    ) {
        let mut cache = Cache::new(self);
        let payment = self.call_value().egld_or_single_esdt();
        let caller = self.blockchain().get_caller();
        let e_mode = self.get_e_mode_category(e_mode_category);
        self.ensure_e_mode_not_deprecated(&e_mode);

        let debt_market_sc = self.require_asset_supported(debt_token);

        let collateral_oracle = self.token_oracle(collateral_token).get();
        let debt_oracle = self.token_oracle(debt_token).get();
        let payment_oracle = self.token_oracle(&payment.token_identifier).get();

        let mut collateral_config = cache.get_cached_asset_info(collateral_token);
        let mut debt_config = cache.get_cached_asset_info(debt_token);

        let collateral_price_feed = self.get_token_price(collateral_token, &mut cache);
        let debt_price_feed = self.get_token_price(debt_token, &mut cache);

        let (account, nft_attributes) = self.create_account_nft(
            &caller,
            collateral_config.is_isolated(),
            false,
            OptionalValue::Some(e_mode_category),
            Some(collateral_token.clone()),
        );

        let e_mode_id = nft_attributes.get_emode_id();
        // Validate e-mode constraints first
        let collateral_emode_config = self.get_token_e_mode_config(e_mode_id, &collateral_token);
        let debt_emode_config = self.get_token_e_mode_config(e_mode_id, &debt_token);

        self.ensure_e_mode_compatible_with_asset(&collateral_config, e_mode_id);
        self.ensure_e_mode_compatible_with_asset(&debt_config, e_mode_id);

        // Update asset config if NFT has active e-mode
        self.apply_e_mode_to_asset_config(&mut collateral_config, &e_mode, collateral_emode_config);
        self.apply_e_mode_to_asset_config(&mut debt_config, &e_mode, debt_emode_config);

        require!(
            collateral_config.can_supply(),
            ERROR_ASSET_NOT_SUPPORTED_AS_COLLATERAL
        );

        require!(debt_config.can_borrow(), ERROR_ASSET_NOT_BORROWABLE);

        // Check if payment token matches debt token - potential optimization path
        let is_payment_same_as_debt = payment.token_identifier == *debt_token;

        // Handle initial collateral conversion
        let initial_collateral = if is_payment_same_as_debt && collateral_token != debt_token {
            // Don't convert payment to collateral yet, we'll handle it with the flash loan together
            EgldOrEsdtTokenPayment::new(
                collateral_token.clone(),
                0,
                BigUint::zero(), // Temporarily zero, will be added later
            )
        } else {
            // Standard path - convert payment to collateral token
            self.process_payment_to_collateral(
                &payment,
                &payment_oracle,
                collateral_token,
                &collateral_oracle,
                &caller,
                steps.clone().into_option(),
                limits.clone().into_option(),
            )
        };

        require!(
            initial_collateral.amount <= final_collateral_amount || is_payment_same_as_debt,
            ERROR_INITIAL_COLLATERAL_OVER_FINAL_COLLATERAL
        );

        // Calculate how much additional collateral we need
        let initial_collateral_amount_dec =
            if is_payment_same_as_debt && collateral_token != debt_token {
                self.to_decimal(BigUint::zero(), collateral_price_feed.asset_decimals)
            } else {
                self.to_decimal(
                    initial_collateral.amount.clone(),
                    collateral_price_feed.asset_decimals,
                )
            };

        let final_collateral_amount_dec = self.to_decimal(
            final_collateral_amount,
            collateral_price_feed.asset_decimals,
        );

        let initial_egld_collateral =
            self.get_token_egld_value(&initial_collateral_amount_dec, &collateral_price_feed.price);
        let final_egld_collateral =
            self.get_token_egld_value(&final_collateral_amount_dec, &collateral_price_feed.price);

        let required_collateral = final_egld_collateral - initial_egld_collateral;

        // Convert the required collateral value to debt token amount
        let total_debt_token_needed =
            self.convert_egld_to_tokens(&required_collateral, &debt_price_feed);

        // Account for the initial payment if it's the same as the debt token
        let debt_amount_to_flash_loan = if is_payment_same_as_debt && collateral_token != debt_token
        {
            // Convert payment amount to decimal format for proper comparison
            let payment_amount_dec =
                self.to_decimal(payment.amount.clone(), debt_price_feed.asset_decimals);

            // If payment already exceeds what we need, we don't need to flash loan anything
            if payment_amount_dec >= total_debt_token_needed {
                self.to_decimal(BigUint::zero(), debt_price_feed.asset_decimals)
            } else {
                // Otherwise, flash loan only the difference needed
                total_debt_token_needed - payment_amount_dec
            }
        } else {
            // Standard path - we need to flash loan the full amount
            total_debt_token_needed
        };

        let flash_fee =
            debt_amount_to_flash_loan.clone() * debt_config.flashloan_fee.clone() / self.bps();

        self.validate_borrow_cap(
            &debt_config,
            &debt_amount_to_flash_loan,
            debt_token,
            &mut cache,
        );

        let mut borrow_position =
            self.get_or_create_borrow_position(account.token_nonce, &debt_config, debt_token);

        // Skip flash loan if amount is zero (payment was sufficient)
        let needs_flash_loan = debt_amount_to_flash_loan
            > self.to_decimal(BigUint::zero(), debt_price_feed.asset_decimals);

        // For multiply strategy, we always require a flash loan component
        require!(
            needs_flash_loan,
            ERROR_MULTIPLY_STRATEGY_REQUIRES_FLASH_LOAN
        );

        // Flash loan is always needed at this point
        borrow_position = self
            .tx()
            .to(debt_market_sc)
            .typed(proxy_pool::LiquidityPoolProxy)
            .create_strategy(
                borrow_position,
                debt_amount_to_flash_loan.clone(),
                flash_fee.clone(),
                debt_price_feed.price.clone(),
            )
            .returns(ReturnsResult)
            .sync_call();

        let mut borrow_positions = self.borrow_positions(account.token_nonce);

        self.update_position_event(
            &debt_amount_to_flash_loan,
            &borrow_position,
            OptionalValue::Some(debt_price_feed.price),
            OptionalValue::Some(&caller),
            OptionalValue::Some(&nft_attributes),
        );

        borrow_positions.insert(debt_token.clone(), borrow_position);

        // OPTIMIZATION POINT: Handle case where payment token is same as debt token
        let final_collateral = if is_payment_same_as_debt && collateral_token != debt_token {
            // Combine initial payment with flash loaned amount (if any) for a single conversion
            let combined_debt_amount = if needs_flash_loan {
                payment.amount.clone() + debt_amount_to_flash_loan.into_raw_units().clone()
            } else {
                // If we didn't need to flash loan, just use the payment amount
                payment.amount.clone()
            };

            // Convert the combined amount in one operation to reduce swaps
            self.convert_token_from_to(
                &collateral_oracle,
                collateral_token,
                debt_token,
                &combined_debt_amount,
                &debt_oracle,
                &caller,
                steps.into_option(),
                limits.into_option(),
            )
        } else {
            // Standard path - just convert flash loaned amount
            self.process_flash_loan_to_collateral(
                &debt_token,
                debt_amount_to_flash_loan.into_raw_units(),
                collateral_token,
                &initial_collateral.amount,
                &collateral_oracle,
                &debt_oracle,
                &caller,
                steps.into_option(),
                limits.into_option(),
            )
        };

        self.update_deposit_position(
            account.token_nonce,
            &final_collateral,
            &collateral_config,
            &caller,
            &nft_attributes,
            &mut cache,
        );

        // Validate health factor after looping was created to verify integrity of healthy
        self.validate_is_healthy(account.token_nonce, &mut cache, None);
    }

    /// Swaps debt token
    ///
    /// # Requirements
    /// * Account must have sufficient collateral
    /// * Position must remain healthy after operation
    /// * Tokens must be different
    ///
    /// # Arguments
    /// * `exisiting_debt_token` - The existing debt token
    /// * `new_debt_amount_raw` - The new debt token amount
    /// * `new_debt_token` - The new debt token
    /// * `steps` - Optional swap steps for token conversion
    #[payable]
    #[allow_multiple_var_args]
    #[endpoint(swapDebt)]
    fn swap_debt(
        &self,
        exisiting_debt_token: &EgldOrEsdtTokenIdentifier,
        new_debt_amount_raw: &BigUint,
        new_debt_token: &EgldOrEsdtTokenIdentifier,
        steps: OptionalValue<ManagedVec<AggregatorStep<Self::Api>>>,
        limits: OptionalValue<ManagedVec<TokenAmount<Self::Api>>>,
    ) {
        require!(
            exisiting_debt_token != new_debt_token,
            ERROR_SWAP_DEBT_NOT_SUPPORTED
        );

        let mut cache = Cache::new(self);
        // Get payments, account, caller and attributes
        let (mut payments, maybe_account, caller, maybe_attributes) =
            self.validate_supply_payment(true);

        let account = maybe_account.unwrap();

        let account_attributes = maybe_attributes.unwrap();
        let mut debt_config = cache.get_cached_asset_info(new_debt_token);
        let exisiting_debt_config = cache.get_cached_asset_info(exisiting_debt_token);

        // Siloed borrowing is not supported for swap debt if one of the tokens is siloed we reject the operation
        require!(
            !exisiting_debt_config.is_siloed_borrowing() && !debt_config.is_siloed_borrowing(),
            ERROR_SWAP_DEBT_NOT_SUPPORTED
        );

        // Validate e-mode constraints first
        let e_mode = self.get_e_mode_category(account_attributes.get_emode_id());
        self.ensure_e_mode_not_deprecated(&e_mode);

        // Apply e-mode configuration
        let asset_emode_config =
            self.get_token_e_mode_config(account_attributes.get_emode_id(), &new_debt_token);
        self.ensure_e_mode_compatible_with_asset(&debt_config, account_attributes.get_emode_id());
        self.apply_e_mode_to_asset_config(&mut debt_config, &e_mode, asset_emode_config);

        require!(debt_config.can_borrow(), ERROR_ASSET_NOT_BORROWABLE);

        if account_attributes.is_isolated() {
            require!(
                debt_config.can_borrow_in_isolation(),
                ERROR_SWAP_DEBT_NOT_SUPPORTED
            );
        }

        self.handle_create_borrow_strategy(
            account.token_nonce,
            new_debt_token,
            new_debt_amount_raw,
            &debt_config,
            &caller,
            &account_attributes,
            &mut cache,
        );

        let received = self.swap_tokens(
            &exisiting_debt_token,
            new_debt_token,
            new_debt_amount_raw,
            &caller,
            steps.into_option(),
            limits.into_option(),
        );

        payments.push(received);

        for payment_ref in payments.iter() {
            self.validate_payment(&payment_ref);
            let feed = self.get_token_price(&payment_ref.token_identifier, &mut cache);
            let payment = self.to_decimal(payment_ref.amount.clone(), feed.asset_decimals);
            let egld_amount = self.get_token_egld_value(&payment, &feed.price);

            self.process_repayment(
                account.token_nonce,
                &payment_ref.token_identifier,
                &payment,
                &caller,
                egld_amount,
                &feed,
                &mut cache,
                &account_attributes,
            );
        }

        // Make sure that after the swap the position is not becoming eligible for liquidation due to slippage
        self.validate_is_healthy(account.token_nonce, &mut cache, None);
    }

    #[payable]
    #[allow_multiple_var_args]
    #[endpoint(swapCollateral)]
    fn swap_collateral(
        &self,
        current_collateral: &EgldOrEsdtTokenIdentifier,
        from_amount: BigUint,
        new_collateral: &EgldOrEsdtTokenIdentifier,
        steps: OptionalValue<ManagedVec<AggregatorStep<Self::Api>>>,
        limits: OptionalValue<ManagedVec<TokenAmount<Self::Api>>>,
    ) {
        let mut cache = Cache::new(self);
        let (mut payments, maybe_account, caller, maybe_attributes) =
            self.validate_supply_payment(true);
        let account = maybe_account.unwrap();
        let account_attributes = maybe_attributes.unwrap();

        require!(
            !account_attributes.is_isolated(),
            ERROR_SWAP_COLLATERAL_NOT_SUPPORTED
        );

        let asset_info = cache.get_cached_asset_info(new_collateral);

        require!(
            !asset_info.is_isolated(),
            ERROR_SWAP_COLLATERAL_NOT_SUPPORTED
        );

        let received = self.common_swap_collateral(
            current_collateral,
            &from_amount,
            new_collateral,
            steps,
            limits,
            account.token_nonce,
            &caller,
            &account_attributes,
            &mut cache,
        );

        payments.push(received);

        self.process_deposit(
            &caller,
            account.token_nonce,
            account_attributes,
            &payments,
            &mut cache,
        );

        // Make sure that after the swap the position is not becoming eligible for liquidation due to slippage
        self.validate_is_healthy(account.token_nonce, &mut cache, None);
    }

    /// Repays debt using collateral assets
    ///
    /// # Arguments
    /// * `from_token` - The collateral token to use for repayment
    /// * `from_amount` - Amount of collateral to use
    /// * `to_token` - The debt token to repay
    /// * `steps` - Optional swap steps for token conversion
    /// * `limits` - Optional price limits for the swap
    ///
    /// # Requirements
    /// * Account must have sufficient collateral
    /// * Position must remain healthy after operation
    /// * Tokens must be different
    ///
    /// # Returns
    /// * Success if debt is repaid and position remains healthy
    /// * Error if any requirements are not met
    #[payable]
    #[allow_multiple_var_args]
    #[endpoint(repayDebtWithCollateral)]
    fn repay_debt_with_collateral(
        &self,
        from_token: &EgldOrEsdtTokenIdentifier,
        from_amount: &BigUint,
        to_token: &EgldOrEsdtTokenIdentifier,
        steps: OptionalValue<ManagedVec<AggregatorStep<Self::Api>>>,
        limits: OptionalValue<ManagedVec<TokenAmount<Self::Api>>>,
    ) {
        let mut cache = Cache::new(self);
        let (mut payments, maybe_account, caller, maybe_attributes) =
            self.validate_supply_payment(true);
        let account = maybe_account.unwrap();
        let account_attributes = maybe_attributes.unwrap();

        let received = self.common_swap_collateral(
            from_token,
            from_amount,
            to_token,
            steps,
            limits,
            account.token_nonce,
            &caller,
            &account_attributes,
            &mut cache,
        );

        payments.push(received);

        for payment in payments.iter() {
            self.validate_payment(&payment);
            let feed = self.get_token_price(&payment.token_identifier, &mut cache);
            let payment_dec = self.to_decimal(payment.amount.clone(), feed.asset_decimals);
            let egld_amount = self.get_token_egld_value(&payment_dec, &feed.price);
            // 3. Process repay
            self.process_repayment(
                account.token_nonce,
                &payment.token_identifier,
                &payment_dec,
                &caller,
                egld_amount,
                &feed,
                &mut cache,
                &account_attributes,
            );
        }

        // Make sure that after the swap the position is not becoming eligible for liquidation due to slippage
        self.validate_is_healthy(account.token_nonce, &mut cache, None);
    }

    fn common_swap_collateral(
        &self,
        from_token: &EgldOrEsdtTokenIdentifier,
        from_amount: &BigUint,
        to_token: &EgldOrEsdtTokenIdentifier,
        steps: OptionalValue<ManagedVec<AggregatorStep<Self::Api>>>,
        limits: OptionalValue<ManagedVec<TokenAmount<Self::Api>>>,
        account_nonce: u64,
        caller: &ManagedAddress,
        account_attributes: &AccountAttributes<Self::Api>,
        cache: &mut Cache<Self>,
    ) -> EgldOrEsdtTokenPayment<Self::Api> {
        // Retrieve deposit position for the given token
        let deposit_positions = self.deposit_positions(account_nonce);
        let maybe_deposit_position = deposit_positions.get(from_token);

        require!(
            maybe_deposit_position.is_some(),
            "Token {} is not available for this account",
            from_token
        );

        let mut deposit_position = maybe_deposit_position.unwrap();

        if !account_attributes.is_vault() {
            // Required to be in sync with the global index for accurate swaps to avoid extra interest during withdraw
            self.update_position(
                &cache.get_cached_pool_address(&deposit_position.asset_id),
                &mut deposit_position,
                OptionalValue::Some(self.get_token_price(&from_token, cache).price),
            );

            self.update_deposit_position_storage(account_nonce, from_token, &mut deposit_position);
        }

        let mut amount_to_swap = deposit_position.make_amount_decimal(from_amount);

        // Cap the withdrawal amount to the available balance with interest
        amount_to_swap = deposit_position.cap_amount(amount_to_swap);

        let controller = self.blockchain().get_sc_address();
        let withdraw_payment = EgldOrEsdtTokenPayment::new(
            from_token.clone(),
            0,
            amount_to_swap.into_raw_units().clone(),
        );

        self.process_withdrawal(
            account_nonce,
            withdraw_payment,
            &controller,
            false,
            None,
            cache,
            &account_attributes,
            true,
        );

        let from_oracle = self.token_oracle(from_token).get();
        let to_oracle = self.token_oracle(to_token).get();

        let received = self.convert_token_from_to(
            &to_oracle,
            &to_token,
            &from_token,
            &amount_to_swap.into_raw_units(),
            &from_oracle,
            &caller,
            steps.into_option(),
            limits.into_option(),
        );

        received
    }
}
