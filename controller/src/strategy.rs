multiversx_sc::imports!();

use common_errors::{
    ERROR_ASSETS_ARE_THE_SAME, ERROR_INVALID_PAYMENTS, ERROR_INVALID_POSITION_MODE,
    ERROR_MULTIPLY_REQUIRE_EXTRA_STEPS, ERROR_SWAP_DEBT_NOT_SUPPORTED,
};
use common_structs::{AccountAttributes, AccountPositionType, PositionMode};

use crate::{
    cache::Cache, helpers, oracle, positions, storage, utils, validation,
    ERROR_SWAP_COLLATERAL_NOT_SUPPORTED,
};

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
{
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
        steps_payment: OptionalValue<ManagedArgBuffer<Self::Api>>,
    ) {
        let mut cache = Cache::new(self);
        cache.allow_unsafe_price = false;
        self.reentrancy_guard(cache.flash_loan_ongoing);
        require!(collateral_token != debt_token, ERROR_ASSETS_ARE_THE_SAME);
        // Get payments, account, caller and attributes
        let (payments, opt_account, caller, opt_attributes) =
            self.validate_supply_payment(false, true, OptionalValue::None);

        let collateral_config = cache.get_cached_asset_info(collateral_token);
        let mut debt_config = cache.get_cached_asset_info(debt_token);

        let collateral_price_feed = self.get_token_price(collateral_token, &mut cache);
        let debt_price_feed = self.get_token_price(debt_token, &mut cache);

        let mut collateral_to_be_supplied =
            self.to_decimal(BigUint::zero(), collateral_price_feed.asset_decimals);
        let mut debt_to_be_swapped =
            self.to_decimal(debt_to_flash_loan.clone(), debt_price_feed.asset_decimals);

        if opt_account.is_none() {
            // Check if payment token matches debt token - potential optimization path
            require!(payments.len() == 1, ERROR_INVALID_PAYMENTS);
            let initial_payment = payments.get(0);
            self.validate_payment(&initial_payment);

            let is_payment_same_as_debt = initial_payment.token_identifier == *debt_token;
            let is_payment_as_collateral = initial_payment.token_identifier == *collateral_token;
            if is_payment_as_collateral {
                let collateral_received = self.to_decimal(
                    initial_payment.amount.clone(),
                    collateral_price_feed.asset_decimals,
                );

                collateral_to_be_supplied += &collateral_received
            } else if is_payment_same_as_debt {
                let debt_amount_received = self.to_decimal(
                    initial_payment.amount.clone(),
                    debt_price_feed.asset_decimals,
                );
                debt_to_be_swapped += &debt_amount_received;
            } else {
                //Swap token
                require!(steps_payment.is_some(), ERROR_MULTIPLY_REQUIRE_EXTRA_STEPS);
                self.convert_token_from_to(
                    collateral_token,
                    &initial_payment.token_identifier,
                    &initial_payment.amount,
                    &caller,
                    steps_payment.into_option().unwrap(),
                );
                // let collateral_received =
                //     self.to_decimal(received.amount, collateral_price_feed.asset_decimals);

                // TODO: Once Bernard mainnet protocol is live, we can uncomment this line
                // collateral_to_be_supplied += &collateral_received;
            }
        } else {
            require!(payments.is_empty(), ERROR_INVALID_PAYMENTS);
        }

        let (account_nonce, nft_attributes) = self.get_or_create_account(
            &caller,
            collateral_config.is_isolated(),
            mode,
            OptionalValue::Some(e_mode_category),
            opt_account,
            opt_attributes,
            if collateral_config.is_isolated() {
                Some(collateral_token.clone())
            } else {
                None
            },
        );

        require!(
            nft_attributes.mode != PositionMode::Normal
                && nft_attributes.mode != PositionMode::None,
            ERROR_INVALID_POSITION_MODE
        );

        self.handle_create_borrow_strategy(
            account_nonce,
            debt_token,
            &debt_to_flash_loan,
            &mut debt_config,
            &caller,
            &nft_attributes,
            &mut cache,
        );

        let mut final_collateral = self.convert_token_from_to(
            collateral_token,
            debt_token,
            debt_to_be_swapped.into_raw_units(),
            &caller,
            steps,
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
    /// * `existing_debt_token` - The existing debt token
    /// * `new_debt_amount_raw` - The new debt token amount
    /// * `new_debt_token` - The new debt token
    /// * `steps` - Optional swap steps for token conversion
    #[payable]
    #[endpoint(swapDebt)]
    fn swap_debt(
        &self,
        existing_debt_token: &EgldOrEsdtTokenIdentifier,
        new_debt_amount_raw: &BigUint,
        new_debt_token: &EgldOrEsdtTokenIdentifier,
        steps: ManagedArgBuffer<Self::Api>,
    ) {
        require!(
            existing_debt_token != new_debt_token,
            ERROR_SWAP_DEBT_NOT_SUPPORTED
        );

        let mut cache = Cache::new(self);
        cache.allow_unsafe_price = false;
        self.reentrancy_guard(cache.flash_loan_ongoing);
        // Get payments, account, caller and attributes
        let (mut payments, opt_account, caller, opt_attributes) =
            self.validate_supply_payment(true, true, OptionalValue::None);

        let account = opt_account.unwrap();

        let account_attributes = opt_attributes.unwrap();
        let mut debt_config = cache.get_cached_asset_info(new_debt_token);
        let existing_debt_config = cache.get_cached_asset_info(existing_debt_token);

        // Siloed borrowing is not supported for swap debt if one of the tokens is siloed we reject the operation
        require!(
            !existing_debt_config.is_siloed_borrowing() && !debt_config.is_siloed_borrowing(),
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
            existing_debt_token,
            new_debt_token,
            new_debt_amount_raw,
            &caller,
            steps,
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
    #[endpoint(swapCollateral)]
    fn swap_collateral(
        &self,
        current_collateral: &EgldOrEsdtTokenIdentifier,
        from_amount: BigUint,
        new_collateral: &EgldOrEsdtTokenIdentifier,
        steps: ManagedArgBuffer<Self::Api>,
    ) {
        let mut cache = Cache::new(self);
        cache.allow_unsafe_price = false;
        self.reentrancy_guard(cache.flash_loan_ongoing);
        let (mut payments, opt_account, caller, opt_attributes) =
            self.validate_supply_payment(true, true, OptionalValue::None);

        let account = opt_account.unwrap();
        let account_attributes = opt_attributes.unwrap();

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
            from_amount,
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

    /// Repays debt using collateral assets
    ///
    /// # Arguments
    /// * `from_token` - The collateral token to use for repayment
    /// * `from_amount` - Amount of collateral to use
    /// * `to_token` - The debt token to repay
    /// * `close_position` - A flag to refund all collaterals when the full debt is fully repaid and burn the position NFT
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
    #[endpoint(repayDebtWithCollateral)]
    fn repay_debt_with_collateral(
        &self,
        from_token: &EgldOrEsdtTokenIdentifier,
        from_amount: BigUint,
        to_token: &EgldOrEsdtTokenIdentifier,
        close_position: bool,
        steps: OptionalValue<ManagedArgBuffer<Self::Api>>,
    ) {
        let mut cache = Cache::new(self);
        cache.allow_unsafe_price = false;
        self.reentrancy_guard(cache.flash_loan_ongoing);
        let (mut payments, opt_account, caller, opt_attributes) =
            self.validate_supply_payment(true, false, OptionalValue::None);
        let account = opt_account.unwrap();
        let account_attributes = opt_attributes.unwrap();

        let received = self.common_swap_collateral(
            from_token,
            from_amount,
            to_token,
            steps.into_option().unwrap_or(ManagedArgBuffer::new()),
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
        let has_no_debt = self
            .positions(account.token_nonce, AccountPositionType::Borrow)
            .is_empty();
        if close_position && has_no_debt {
            for mut deposit_position in self
                .positions(account.token_nonce, AccountPositionType::Deposit)
                .values()
            {
                let feed = self.get_token_price(&deposit_position.asset_id, &mut cache);
                let amount = self.get_total_amount(&deposit_position, &feed, &mut cache);

                let _ = self.process_withdrawal(
                    account.token_nonce,
                    amount,
                    &caller,
                    false,
                    None,
                    &mut cache,
                    &account_attributes,
                    &mut deposit_position,
                    &feed,
                );
            }
        }
        self.manage_account_after_withdrawal(&account, &caller);
    }

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
        let controller = self.blockchain().get_sc_address();

        let mut deposit_position = self.get_deposit_position(account_nonce, from_token);
        let feed = self.get_token_price(&deposit_position.asset_id, cache);
        let amount = deposit_position.make_amount_decimal(&from_amount, feed.asset_decimals);
        let withdraw_payment = self.process_withdrawal(
            account_nonce,
            amount,
            &controller,
            false,
            None,
            cache,
            account_attributes,
            &mut deposit_position,
            &feed,
        );

        self.convert_token_from_to(
            to_token,
            from_token,
            &withdraw_payment.amount,
            caller,
            steps,
        )
    }

    fn convert_token_from_to(
        &self,
        to_token: &EgldOrEsdtTokenIdentifier,
        from_token: &EgldOrEsdtTokenIdentifier,
        from_amount: &BigUint,
        caller: &ManagedAddress,
        args: ManagedArgBuffer<Self::Api>,
    ) -> EgldOrEsdtTokenPayment {
        if to_token == from_token {
            return EgldOrEsdtTokenPayment::new(to_token.clone(), 0, from_amount.clone());
        }

        self.swap_tokens(to_token, from_token, from_amount, caller, args)
    }

    fn swap_tokens(
        self,
        wanted_token: &EgldOrEsdtTokenIdentifier,
        from_token: &EgldOrEsdtTokenIdentifier,
        from_amount: &BigUint,
        caller: &ManagedAddress,
        args: ManagedArgBuffer<Self::Api>,
    ) -> EgldOrEsdtTokenPayment {
        let back_transfers = self
            .tx()
            .to(self.swap_router().get())
            .raw_call(ManagedBuffer::new_from_bytes(b"swap"))
            .arguments_raw(args)
            .egld_or_single_esdt(from_token, 0, from_amount)
            .returns(ReturnsBackTransfersReset)
            .sync_call();

        let mut payments: ManagedVec<EgldOrEsdtTokenPayment<Self::Api>> = ManagedVec::new();

        if back_transfers.total_egld_amount > 0 {
            payments.push(EgldOrEsdtTokenPayment::new(
                EgldOrEsdtTokenIdentifier::egld(),
                0,
                back_transfers.total_egld_amount,
            ));
        }

        for esdt in back_transfers.esdt_payments.iter() {
            payments.push(EgldOrEsdtTokenPayment::new(
                EgldOrEsdtTokenIdentifier::esdt(esdt.token_identifier.clone()),
                esdt.token_nonce,
                esdt.amount.clone(),
            ));
        }

        let mut wanted_result =
            EgldOrEsdtTokenPayment::new(wanted_token.clone(), 0, BigUint::from(0u32));

        let mut refunds = ManagedVec::new();

        for payment in payments {
            if payment.token_identifier == *wanted_token && payment.token_nonce == 0 {
                wanted_result.amount += &payment.amount;
            } else {
                refunds.push(payment.clone());
            }
        }

        if !refunds.is_empty() {
            self.tx()
                .to(caller)
                .payment(refunds)
                .transfer_if_not_empty();
        }

        wanted_result
    }
}
