multiversx_sc::imports!();

use common_errors::ERROR_SWAP_DEBT_NOT_SUPPORTED;

use crate::{
    aggregator::{AggregatorStep, TokenAmount},
    cache::Cache,
    helpers, oracle, positions, proxy_pool, storage, utils, validation, ERROR_ASSET_NOT_BORROWABLE,
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
        steps: OptionalValue<ManagedVec<AggregatorStep<Self::Api>>>,
        limits: OptionalValue<ManagedVec<TokenAmount<Self::Api>>>,
    ) {
        let payment = self.call_value().egld_or_single_esdt();
        let caller = self.blockchain().get_caller();
        let e_mode = self.get_e_mode_category(e_mode_category);
        self.ensure_e_mode_not_deprecated(&e_mode);
        let mut cache = Cache::new(self);

        let debt_market_sc = self.require_asset_supported(debt_token);

        let collateral_oracle = self.token_oracle(collateral_token).get();
        let debt_oracle = self.token_oracle(debt_token).get();
        let payment_oracle = self.token_oracle(&payment.token_identifier).get();

        let mut collateral_config = cache.get_cached_asset_info(collateral_token);
        let mut debt_config = cache.get_cached_asset_info(debt_token);

        let collateral_price_feed = self.get_token_price(collateral_token, &mut cache);
        let debt_price_feed = self.get_token_price(debt_token, &mut cache);

        let (account, nft_attributes) =
            self.create_account_nft(&caller, false, false, OptionalValue::Some(e_mode_category));

        let e_mode_id = nft_attributes.get_emode_id();
        // 4. Validate e-mode constraints first
        let collateral_emode_config = self.get_token_e_mode_config(e_mode_id, &collateral_token);
        let debt_emode_config = self.get_token_e_mode_config(e_mode_id, &debt_token);

        self.ensure_e_mode_compatible_with_asset(&collateral_config, e_mode_id);
        self.ensure_e_mode_compatible_with_asset(&debt_config, e_mode_id);

        // 5. Update asset config if NFT has active e-mode
        self.apply_e_mode_to_asset_config(&mut collateral_config, &e_mode, collateral_emode_config);
        self.apply_e_mode_to_asset_config(&mut debt_config, &e_mode, debt_emode_config);

        require!(
            collateral_config.can_supply(),
            ERROR_ASSET_NOT_SUPPORTED_AS_COLLATERAL
        );

        require!(debt_config.can_borrow(), ERROR_ASSET_NOT_BORROWABLE);

        let initial_collateral = self.process_payment_to_collateral(
            &payment,
            &payment_oracle,
            collateral_token,
            &collateral_oracle,
            &caller,
            steps.clone().into_option(),
            limits.clone().into_option(),
        );

        require!(
            initial_collateral.amount <= final_collateral_amount,
            "Initial collateral amount is less than the final collateral amount"
        );

        let initial_collateral_amount_dec = ManagedDecimal::from_raw_units(
            initial_collateral.amount.clone(),
            collateral_price_feed.asset_decimals,
        );
        let final_collateral_amount_dec = ManagedDecimal::from_raw_units(
            final_collateral_amount,
            collateral_price_feed.asset_decimals,
        );

        let initial_egld_collateral =
            self.get_token_egld_value(&initial_collateral_amount_dec, &collateral_price_feed.price);
        let final_egld_collateral =
            self.get_token_egld_value(&final_collateral_amount_dec, &collateral_price_feed.price);

        let required_collateral = final_egld_collateral - initial_egld_collateral;

        let debt_amount_to_flash_loan =
            self.convert_egld_to_tokens(&required_collateral, &debt_price_feed);

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

        // Convert the debt token to the LSD token
        let final_collateral = self.process_flash_loan_to_collateral(
            &debt_token,
            debt_amount_to_flash_loan.into_raw_units(),
            collateral_token,
            &initial_collateral.amount,
            &collateral_oracle,
            &debt_oracle,
            &caller,
            steps.into_option(),
            limits.into_option(),
        );

        self.update_deposit_position(
            account.token_nonce,
            &final_collateral,
            &collateral_config,
            &caller,
            &nft_attributes,
            &mut cache,
        );

        // 4. Validate health factor after looping was created to verify integrity of healthy
        self.validate_is_healthy(account.token_nonce, &mut cache, None);
    }

    #[payable]
    #[allow_multiple_var_args]
    #[endpoint(swapCollateral)]
    fn swap_collateral(
        &self,
        from_token: &EgldOrEsdtTokenIdentifier,
        from_amount: BigUint,
        to_token: &EgldOrEsdtTokenIdentifier,
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

        let asset_info = cache.get_cached_asset_info(to_token);

        require!(
            !asset_info.is_isolated(),
            ERROR_SWAP_COLLATERAL_NOT_SUPPORTED
        );

        // Retrieve deposit position for the given token
        let deposit_positions = self.deposit_positions(account.token_nonce);
        let maybe_deposit_position = deposit_positions.get(from_token);
        require!(
            maybe_deposit_position.is_some(),
            "Token {} is not available for this account",
            from_token
        );
        let mut deposit_position = maybe_deposit_position.unwrap();

        // Required to be in sync with the global index for accurate swaps to avoid extra interest during withdraw
        self.update_position(
            &self.get_pool_address(&deposit_position.asset_id),
            &mut deposit_position,
            OptionalValue::Some(self.get_token_price(&from_token, &mut cache).price),
        );

        self.update_deposit_position_storage(
            account.token_nonce,
            from_token,
            &mut deposit_position,
        );
        let mut amount_to_swap = deposit_position.make_amount_decimal(from_amount);

        // Cap the withdrawal amount to the available balance with interest
        amount_to_swap = if amount_to_swap > deposit_position.get_total_amount() {
            deposit_position.get_total_amount()
        } else {
            amount_to_swap
        };

        let controller = self.blockchain().get_sc_address();
        self.process_withdrawal(
            account.token_nonce,
            EgldOrEsdtTokenPayment::new(
                from_token.clone(),
                0,
                amount_to_swap.into_raw_units().clone(),
            ),
            &controller,
            false,
            None,
            &mut cache,
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

    #[payable]
    #[allow_multiple_var_args]
    #[endpoint(swapDebt)]
    fn swap_debt(
        &self,
        from_token: &EgldOrEsdtTokenIdentifier,
        from_amount: BigUint,
        to_token: &EgldOrEsdtTokenIdentifier,
        steps: OptionalValue<ManagedVec<AggregatorStep<Self::Api>>>,
        limits: OptionalValue<ManagedVec<TokenAmount<Self::Api>>>,
    ) {
        let mut cache = Cache::new(self);
        let (mut payments, maybe_account, caller, maybe_attributes) =
            self.validate_supply_payment(true);

        let account = maybe_account.unwrap();

        let account_attributes = maybe_attributes.unwrap();

        let asset_info = cache.get_cached_asset_info(to_token);

        if account_attributes.is_isolated() {
            require!(
                asset_info.can_borrow_in_isolation(),
                ERROR_SWAP_DEBT_NOT_SUPPORTED
            );
        }

        // Retrieve borrow position for the given token
        let borrow_positions = self.borrow_positions(account.token_nonce);
        let maybe_borrow_position = borrow_positions.get(from_token);

        require!(
            maybe_borrow_position.is_some(),
            "Token {} is not available for this account",
            from_token
        );

        let mut borrow_position = maybe_borrow_position.unwrap();

        // Required to be in sync with the global index for accurate swaps to avoid extra interest during withdraw
        self.update_position(
            &cache.get_cached_pool_address(&borrow_position.asset_id),
            &mut borrow_position,
            OptionalValue::Some(self.get_token_price(&from_token, &mut cache).price),
        );

        self.update_borrow_position_storage(account.token_nonce, from_token, &mut borrow_position);

        let mut amount_to_repay = borrow_position.make_amount_decimal(from_amount);

        amount_to_repay = if amount_to_repay > borrow_position.get_total_amount() {
            borrow_position.get_total_amount()
        } else {
            amount_to_repay
        };

        let from_debt_feed = self.get_token_price(from_token, &mut cache);
        let to_debt_feed = self.get_token_price(to_token, &mut cache);

        let from_debt_egld_amount =
            self.get_token_egld_value(&amount_to_repay, &from_debt_feed.price);

        let required_to_swap = self.convert_egld_to_tokens(&from_debt_egld_amount, &to_debt_feed);
        let debt_config = cache.get_cached_asset_info(to_token);
        let flash_fee = required_to_swap.clone() * debt_config.flashloan_fee.clone() / self.bps();

        let mut to_debt_borrow_position =
            self.get_or_create_borrow_position(account.token_nonce, &debt_config, to_token);

        let debt_market_sc = cache.get_cached_pool_address(&to_debt_borrow_position.asset_id);

        to_debt_borrow_position = self
            .tx()
            .to(debt_market_sc)
            .typed(proxy_pool::LiquidityPoolProxy)
            .create_strategy(
                to_debt_borrow_position,
                required_to_swap.clone(),
                flash_fee.clone(),
                to_debt_feed.price.clone(),
            )
            .returns(ReturnsResult)
            .sync_call();

        self.update_position_event(
            &required_to_swap,
            &to_debt_borrow_position,
            OptionalValue::Some(to_debt_feed.price),
            OptionalValue::Some(&caller),
            OptionalValue::Some(&account_attributes),
        );

        self.update_borrow_position_storage(
            account.token_nonce,
            to_token,
            &mut to_debt_borrow_position,
        );

        let received = self.swap_tokens(
            &from_token,
            &to_token,
            &required_to_swap.into_raw_units(),
            &caller,
            steps.into_option(),
            limits.into_option(),
        );

        payments.push(received);

        for payment in payments.iter() {
            self.validate_payment(&payment);
            let feed = self.get_token_price(&payment.token_identifier, &mut cache);
            let payment_decimal =
                ManagedDecimal::from_raw_units(payment.amount.clone(), feed.asset_decimals);

            self.process_repayment(
                account.token_nonce,
                &payment.token_identifier,
                &payment_decimal,
                &caller,
                self.get_token_egld_value(&payment_decimal, &feed.price),
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
    #[endpoint(repayDebtWithCollateral)]
    fn repay_debt_with_collateral(
        &self,
        from_token: &EgldOrEsdtTokenIdentifier,
        from_amount: BigUint,
        to_token: &EgldOrEsdtTokenIdentifier,
        steps: OptionalValue<ManagedVec<AggregatorStep<Self::Api>>>,
        limits: OptionalValue<ManagedVec<TokenAmount<Self::Api>>>,
    ) {
        let mut cache = Cache::new(self);
        let (mut payments, maybe_account, caller, maybe_attributes) =
            self.validate_supply_payment(true);
        let account = maybe_account.unwrap();
        let account_attributes = maybe_attributes.unwrap();

        // Retrieve deposit position for the given token
        let deposit_positions = self.deposit_positions(account.token_nonce);
        let maybe_deposit_position = deposit_positions.get(from_token);

        require!(
            maybe_deposit_position.is_some(),
            "Token {} is not available for this account",
            from_token
        );

        let mut deposit_position = maybe_deposit_position.unwrap();

        // Required to be in sync with the global index for accurate swaps to avoid extra interest during withdraw
        self.update_position(
            &cache.get_cached_pool_address(&deposit_position.asset_id),
            &mut deposit_position,
            OptionalValue::Some(self.get_token_price(&from_token, &mut cache).price),
        );

        self.update_deposit_position_storage(
            account.token_nonce,
            from_token,
            &mut deposit_position,
        );

        let mut amount_to_swap = deposit_position.make_amount_decimal(from_amount);

        // Cap the withdrawal amount to the available balance with interest
        amount_to_swap = if amount_to_swap > deposit_position.get_total_amount() {
            deposit_position.get_total_amount()
        } else {
            amount_to_swap
        };

        let controller = self.blockchain().get_sc_address();
        self.process_withdrawal(
            account.token_nonce,
            EgldOrEsdtTokenPayment::new(
                from_token.clone(),
                0,
                amount_to_swap.into_raw_units().clone(),
            ),
            &controller,
            false,
            None,
            &mut cache,
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

        payments.push(received);

        for payment in payments.iter() {
            self.validate_payment(&payment);
            let feed = self.get_token_price(&payment.token_identifier, &mut cache);
            let payment_dec =
                ManagedDecimal::from_raw_units(payment.amount.clone(), feed.asset_decimals);
            // 3. Process repay
            self.process_repayment(
                account.token_nonce,
                &payment.token_identifier,
                &payment_dec,
                &caller,
                self.get_token_egld_value(&payment_dec, &feed.price),
                &feed,
                &mut cache,
                &account_attributes,
            );
        }

        // Make sure that after the swap the position is not becoming eligible for liquidation due to slippage
        self.validate_is_healthy(account.token_nonce, &mut cache, None);
    }
}
