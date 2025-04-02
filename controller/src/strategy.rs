multiversx_sc::imports!();

use common_errors::{
    ERROR_ASSETS_ARE_THE_SAME, ERROR_INITIAL_COLLATERAL_OVER_FINAL_COLLATERAL,
    ERROR_INVALID_PAYMENTS, ERROR_SWAP_DEBT_NOT_SUPPORTED,
};
use common_events::AccountAttributes;

use crate::{
    cache::Cache,
    helpers::{self, strategies::ArdaSwapArgs},
    oracle, positions,
    proxy_ashswap::{AggregatorStep, TokenAmount},
    storage, utils, validation, ERROR_SWAP_COLLATERAL_NOT_SUPPORTED,
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
    + positions::update::PositionUpdateModule
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
        _mode: ManagedBuffer,
        steps: OptionalValue<ManagedVec<AggregatorStep<Self::Api>>>,
        limits: OptionalValue<ManagedVec<TokenAmount<Self::Api>>>,
    ) {
        let mut cache = Cache::new(self);
        require!(collateral_token != debt_token, ERROR_ASSETS_ARE_THE_SAME);
        // Get payments, account, caller and attributes
        let (payments, maybe_account, caller, maybe_attributes) =
            self.validate_supply_payment(false);

        require!(payments.len() == 1, ERROR_INVALID_PAYMENTS);

        let existing_position = maybe_account.is_some();

        let initial_payment = payments.get(0);
        self.validate_payment(&initial_payment);

        let collateral_config = cache.get_cached_asset_info(collateral_token);

        let (account_nonce, nft_attributes) = self.get_or_create_account(
            &caller,
            collateral_config.is_isolated(),
            false,
            OptionalValue::Some(e_mode_category),
            maybe_account,
            maybe_attributes,
            if collateral_config.is_isolated() {
                Some(collateral_token.clone())
            } else {
                None
            },
        );

        let collateral_oracle = cache.get_cached_oracle(collateral_token);

        let collateral_price_feed = self.get_token_price(collateral_token, &mut cache);
        let debt_price_feed = self.get_token_price(debt_token, &mut cache);
        let debt_oracle = cache.get_cached_oracle(debt_token);
        // Check if payment token matches debt token - potential optimization path
        let is_payment_same_as_debt = initial_payment.token_identifier == *debt_token;
        let is_payment_as_collateral = initial_payment.token_identifier == *collateral_token;
        let mut total_collateral_needed_to_buy = self.to_decimal(
            final_collateral_amount,
            collateral_price_feed.asset_decimals,
        );
        let mut collateral_to_be_supplied =
            self.to_decimal(BigUint::zero(), collateral_price_feed.asset_decimals);
        let mut debt_to_be_swapped =
            self.to_decimal(BigUint::zero(), debt_price_feed.asset_decimals);

        if is_payment_as_collateral {
            let collateral_received = self.to_decimal(
                initial_payment.amount.clone(),
                collateral_price_feed.asset_decimals,
            );

            total_collateral_needed_to_buy -= &collateral_received;
            collateral_to_be_supplied += &collateral_received
        } else if is_payment_same_as_debt {
            let debt_amount_received = self.to_decimal(
                initial_payment.amount.clone(),
                debt_price_feed.asset_decimals,
            );
            debt_to_be_swapped += &debt_amount_received;
        } else {
            //Swap token
            let payment_oracle = cache.get_cached_oracle(&initial_payment.token_identifier);
            let received = self.convert_token_from_to(
                &collateral_oracle,
                collateral_token,
                &initial_payment.token_identifier,
                &initial_payment.amount,
                &payment_oracle,
                &caller,
                steps.clone().into_option(),
                limits.clone().into_option(),
            );
            let collateral_received =
                self.to_decimal(received.amount, collateral_price_feed.asset_decimals);

            total_collateral_needed_to_buy -= &collateral_received;
            collateral_to_be_supplied += &collateral_received;
        }

        if existing_position {
            self.sync_deposit_positions_interest(
                account_nonce,
                &mut cache,
                true,
                &nft_attributes,
                false,
            );

            let supply_position = self.get_or_create_deposit_position(
                account_nonce,
                &collateral_config,
                collateral_token,
            );

            require!(
                supply_position.get_total_amount() < total_collateral_needed_to_buy,
                ERROR_INITIAL_COLLATERAL_OVER_FINAL_COLLATERAL,
            );

            total_collateral_needed_to_buy -= &supply_position.get_total_amount();
        }

        let egld_equivalent_of_collateral = self.get_token_egld_value(
            &total_collateral_needed_to_buy,
            &collateral_price_feed.price,
        );
        let equivalent_debt_of_collateral_needed =
            self.convert_egld_to_tokens(&egld_equivalent_of_collateral, &debt_price_feed);
        // In case we have debt to be swapped, we deduct it from the equivalent debt of collateral needed since was paid by the user in the initial payment
        require!(
            debt_to_be_swapped < equivalent_debt_of_collateral_needed,
            ERROR_INITIAL_COLLATERAL_OVER_FINAL_COLLATERAL
        );
        let debt_token_to_flash = equivalent_debt_of_collateral_needed.clone() - debt_to_be_swapped;
        let mut debt_config = cache.get_cached_asset_info(debt_token);

        self.handle_create_borrow_strategy(
            account_nonce,
            debt_token,
            debt_token_to_flash.into_raw_units(),
            &mut debt_config,
            &caller,
            &nft_attributes,
            &mut cache,
        );

        let mut final_collateral = self.convert_token_from_to(
            &collateral_oracle,
            collateral_token,
            debt_token,
            &equivalent_debt_of_collateral_needed.into_raw_units(),
            &debt_oracle,
            &caller,
            steps.into_option(),
            limits.into_option(),
        );

        final_collateral.amount += collateral_to_be_supplied.into_raw_units();

        self.process_deposit(
            &caller,
            account_nonce,
            nft_attributes,
            &ManagedVec::from_single_item(final_collateral),
            &mut cache,
        );

        // Validate health factor after looping was created to verify integrity of healthy
        self.validate_is_healthy(account_nonce, &mut cache, None);
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

        self.handle_create_borrow_strategy(
            account.token_nonce,
            new_debt_token,
            new_debt_amount_raw,
            &mut debt_config,
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
        steps: OptionalValue<ArdaSwapArgs<Self::Api>>,
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

        let received = self.common_swap_collateral_arda(
            current_collateral,
            &from_amount,
            new_collateral,
            steps,
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

    // #[payable]
    // #[allow_multiple_var_args]
    // #[endpoint(swapCollateral)]
    // fn swap_collateral(
    //     &self,
    //     current_collateral: &EgldOrEsdtTokenIdentifier,
    //     from_amount: BigUint,
    //     new_collateral: &EgldOrEsdtTokenIdentifier,
    //     steps: OptionalValue<ManagedVec<AggregatorStep<Self::Api>>>,
    //     limits: OptionalValue<ManagedVec<TokenAmount<Self::Api>>>,
    // ) {
    //     let mut cache = Cache::new(self);
    //     let (mut payments, maybe_account, caller, maybe_attributes) =
    //         self.validate_supply_payment(true);
    //     let account = maybe_account.unwrap();
    //     let account_attributes = maybe_attributes.unwrap();

    //     require!(
    //         !account_attributes.is_isolated(),
    //         ERROR_SWAP_COLLATERAL_NOT_SUPPORTED
    //     );

    //     let asset_info = cache.get_cached_asset_info(new_collateral);

    //     require!(
    //         !asset_info.is_isolated(),
    //         ERROR_SWAP_COLLATERAL_NOT_SUPPORTED
    //     );

    //     let received = self.common_swap_collateral(
    //         current_collateral,
    //         &from_amount,
    //         new_collateral,
    //         steps,
    //         limits,
    //         account.token_nonce,
    //         &caller,
    //         &account_attributes,
    //         &mut cache,
    //     );

    //     payments.push(received);

    //     self.process_deposit(
    //         &caller,
    //         account.token_nonce,
    //         account_attributes,
    //         &payments,
    //         &mut cache,
    //     );

    //     // Make sure that after the swap the position is not becoming eligible for liquidation due to slippage
    //     self.validate_is_healthy(account.token_nonce, &mut cache, None);
    // }

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

    fn common_swap_collateral_arda(
        &self,
        from_token: &EgldOrEsdtTokenIdentifier,
        from_amount: &BigUint,
        to_token: &EgldOrEsdtTokenIdentifier,
        steps: OptionalValue<ArdaSwapArgs<Self::Api>>,
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

        let received = self.convert_token_from_to_arda(
            &to_token,
            &from_token,
            &amount_to_swap.into_raw_units(),
            &caller,
            steps.into_option(),
        );

        received
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
