multiversx_sc::imports!();

use crate::{
    aggregator::{AggregatorStep, TokenAmount},
    contexts::base::StorageCache,
    helpers, oracle, positions, proxy_pool, storage, utils, validation, ERROR_ASSET_NOT_BORROWABLE,
    ERROR_ASSET_NOT_SUPPORTED_AS_COLLATERAL, ERROR_INVALID_NUMBER_OF_ESDT_TRANSFERS,
    ERROR_SWAP_COLLATERAL_NOT_SUPPORTED,
};

#[multiversx_sc::module]
pub trait MultiplyModule:
    storage::LendingStorageModule
    + oracle::OracleModule
    + validation::ValidationModule
    + utils::LendingUtilsModule
    + common_events::EventsModule
    + helpers::math::MathsModule
    + helpers::strategies::StrategiesModule
    + positions::account::PositionAccountModule
    + positions::deposit::PositionDepositModule
    + positions::borrow::PositionBorrowModule
    + positions::withdraw::PositionWithdrawModule
    + positions::repay::PositionRepayModule
    + positions::emode::EModeModule
    + common_math::SharedMathModule
{
    // e-mode 1
    // EGLD, xEGLD, xEGLD/EGLD LP
    // Send EGLD -> Stake for xEGLD -> Supply xEGLD (COLLATERAL) -> Borrow EGLD -> loop again
    // Send xEGLD -> Supply xEGLD (COLLATERAL) -> Borrow EGLD -> loop again
    #[payable]
    #[endpoint]
    #[allow_multiple_var_args]
    fn multiply(
        &self,
        leverage_raw: BigUint,
        e_mode_category: u8,
        collateral_token: &EgldOrEsdtTokenIdentifier,
        debt_token: &EgldOrEsdtTokenIdentifier,
        steps: OptionalValue<ManagedVec<AggregatorStep<Self::Api>>>,
        limits: OptionalValue<ManagedVec<TokenAmount<Self::Api>>>,
    ) {
        // let wad = self.wad();
        let payment = self.call_value().egld_or_single_esdt();
        let caller = self.blockchain().get_caller();
        let e_mode = self.validate_e_mode_exists(e_mode_category);
        self.validate_not_deprecated_e_mode(&e_mode);
        let leverage = self.to_decimal_wad(leverage_raw);
        let mut storage_cache = StorageCache::new(self);

        // let target = wad.clone() * self.to_decimal_wad(BigUint::from(2u32)) / 100 + wad.clone(); // 1.02
        // let reserves_factor = wad.clone() / 5; // 20%

        // let collateral_market_sc = self.require_asset_supported(collateral_token);
        let debt_market_sc = self.require_asset_supported(debt_token);

        let collateral_oracle = self.token_oracle(collateral_token).get();
        let debt_oracle = self.token_oracle(debt_token).get();
        let payment_oracle = self.token_oracle(&payment.token_identifier).get();

        let mut collateral_config = storage_cache.get_cached_asset_info(collateral_token);
        let mut debt_config = storage_cache.get_cached_asset_info(debt_token);

        let collateral_price_feed = self.get_token_price(collateral_token, &mut storage_cache);
        let debt_price_feed = self.get_token_price(debt_token, &mut storage_cache);

        // let max_l = self.calculate_max_leverage(
        //     &debt_payment.amount,
        //     &target,
        //     &e_mode,
        //     &debt_config,
        //     &self.get_total_reserves(debt_market_sc).get(),
        //     &reserves_factor,
        // );

        // require!(
        //     leverage <= &max_l,
        //     "The leverage is over the maximum allowed: {}!",
        //     max_l
        // );

        let (account, nft_attributes) =
            self.create_position_nft(&caller, false, false, OptionalValue::Some(e_mode_category));

        let e_mode_id = nft_attributes.e_mode_category;
        // 4. Validate e-mode constraints first
        let collateral_emode_config = self.validate_token_of_emode(e_mode_id, &collateral_token);
        let debt_emode_config = self.validate_token_of_emode(e_mode_id, &debt_token);

        self.validate_e_mode_not_isolated(&collateral_config, e_mode_id);
        self.validate_e_mode_not_isolated(&debt_config, e_mode_id);

        // 5. Update asset config if NFT has active e-mode
        self.update_asset_config_with_e_mode(
            &mut collateral_config,
            &e_mode,
            collateral_emode_config,
        );
        self.update_asset_config_with_e_mode(&mut debt_config, &e_mode, debt_emode_config);

        require!(
            collateral_config.can_be_collateral,
            ERROR_ASSET_NOT_SUPPORTED_AS_COLLATERAL
        );

        require!(debt_config.can_be_borrowed, ERROR_ASSET_NOT_BORROWABLE);

        let initial_collateral = self.process_payment_to_collateral(
            &payment,
            &payment_oracle,
            collateral_token,
            &collateral_oracle,
            steps.clone().into_option(),
            limits.clone().into_option(),
        );

        let initial_collateral_amount_dec = ManagedDecimal::from_raw_units(
            initial_collateral.amount.clone(),
            collateral_price_feed.decimals as usize,
        );

        let initial_egld_collateral = self.get_token_amount_in_egld_raw(
            &initial_collateral_amount_dec,
            &collateral_price_feed.price,
        );
        let final_strategy_collateral =
            initial_egld_collateral.clone() * leverage / storage_cache.wad_dec.clone();
        let required_collateral = final_strategy_collateral - initial_egld_collateral;

        let debt_amount_to_flash_loan =
            self.compute_egld_in_tokens(&required_collateral, &debt_price_feed);

        let flash_fee = debt_amount_to_flash_loan.clone() * debt_config.flash_loan_fee.clone()
            / storage_cache.bps_dec.clone();

        let total_borrowed = debt_amount_to_flash_loan.clone() + flash_fee.clone();

        self.validate_borrow_cap(&debt_config, &total_borrowed, debt_token);

        let (borrow_index, timestamp) = self
            .tx()
            .to(debt_market_sc)
            .typed(proxy_pool::LiquidityPoolProxy)
            .create_flash_strategy(
                debt_token,
                debt_amount_to_flash_loan.clone(),
                flash_fee.clone(),
                debt_price_feed.price.clone(),
            )
            .returns(ReturnsResult)
            .sync_call();

        let mut borrow_position = self.get_or_create_borrow_position(
            account.token_nonce,
            &debt_config,
            debt_token,
            false,
        );

        borrow_position.amount += &debt_amount_to_flash_loan;
        borrow_position.accumulated_interest += &flash_fee;
        borrow_position.index = borrow_index;
        borrow_position.timestamp = timestamp;

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
            steps.into_option(),
            limits.into_option(),
        );

        // sc_panic!(
        //     "Final collateral {}, Initial {}, Borrowed: {}",
        //     final_collateral.amount,
        //     initial_collateral.amount,
        //     flash_borrow.amount
        // );

        self.update_deposit_position(
            account.token_nonce,
            &final_collateral,
            &collateral_config,
            &caller,
            &nft_attributes,
            &mut storage_cache,
        );

        // 4. Validate health factor after looping was created to verify integrity of healthy
        self.validate_withdraw_health_factor(account.token_nonce, false, &mut storage_cache, None);
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
        let mut storage_cache = StorageCache::new(self);
        let caller = self.blockchain().get_caller();
        let controller = self.blockchain().get_sc_address();
        let payments = self.call_value().all_transfers();
        let (mut payments, maybe_account) = self.validate_supply_payment(&caller, &payments);

        require!(
            maybe_account.is_some(),
            ERROR_INVALID_NUMBER_OF_ESDT_TRANSFERS
        );

        let account = maybe_account.unwrap();

        let attributes = self.nft_attributes(account.token_nonce, &account.token_identifier);
        // Refund NFT
        self.tx().to(&caller).esdt(account.clone()).transfer();

        require!(
            !attributes.is_isolated(),
            ERROR_SWAP_COLLATERAL_NOT_SUPPORTED
        );

        let asset_info = storage_cache.get_cached_asset_info(to_token);

        require!(!asset_info.is_isolated, ERROR_SWAP_COLLATERAL_NOT_SUPPORTED);

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
            &self.get_pool_address(&deposit_position.token_id),
            &mut deposit_position,
            OptionalValue::Some(self.get_token_price(&from_token, &mut storage_cache).price),
        );

        let mut amount_to_swap = deposit_position.make_amount_decimal(from_amount);

        // Cap the withdrawal amount to the available balance with interest
        amount_to_swap = if amount_to_swap > deposit_position.get_total_amount() {
            deposit_position.get_total_amount()
        } else {
            amount_to_swap
        };

        self.internal_withdraw(
            account.token_nonce,
            EgldOrEsdtTokenPayment::new(
                from_token.clone(),
                0,
                amount_to_swap.into_raw_units().clone(),
            ),
            &controller,
            false,
            None,
            &mut storage_cache,
            &attributes,
            true,
        );

        let received = self.swap_tokens(
            &to_token,
            &from_token,
            &amount_to_swap.into_raw_units(),
            steps.into_option(),
            limits.into_option(),
        );

        payments.push(received);

        self.process_deposit(
            &caller,
            account.token_nonce,
            attributes,
            &payments,
            &mut storage_cache,
        );

        // Make sure that after the swap the position is not becoming eligible for liquidation due to slippage
        self.validate_withdraw_health_factor(account.token_nonce, false, &mut storage_cache, None);
    }

    #[payable]
    #[allow_multiple_var_args]
    #[endpoint(repayDebtWithCollateral)]
    fn repay_debt_with_collateral(
        &self,
        from_token: &EgldOrEsdtTokenIdentifier,
        from_amount: BigUint,
        debt_token: &EgldOrEsdtTokenIdentifier,
        steps: OptionalValue<ManagedVec<AggregatorStep<Self::Api>>>,
        limits: OptionalValue<ManagedVec<TokenAmount<Self::Api>>>,
    ) {
        let mut storage_cache = StorageCache::new(self);
        let caller = self.blockchain().get_caller();
        let controller = self.blockchain().get_sc_address();
        let payments = self.call_value().all_transfers();
        let (mut payments, maybe_account) = self.validate_supply_payment(&caller, &payments);

        require!(
            maybe_account.is_some(),
            ERROR_INVALID_NUMBER_OF_ESDT_TRANSFERS
        );

        let account = maybe_account.unwrap();

        let attributes = self.nft_attributes(account.token_nonce, &account.token_identifier);
        // Refund NFT
        self.tx().to(&caller).esdt(account.clone()).transfer();

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
            &storage_cache.get_cached_pool_address(&deposit_position.token_id),
            &mut deposit_position,
            OptionalValue::Some(self.get_token_price(&from_token, &mut storage_cache).price),
        );

        let mut amount_to_swap = deposit_position.make_amount_decimal(from_amount);

        // Cap the withdrawal amount to the available balance with interest
        amount_to_swap = if amount_to_swap > deposit_position.get_total_amount() {
            deposit_position.get_total_amount()
        } else {
            amount_to_swap
        };

        self.internal_withdraw(
            account.token_nonce,
            EgldOrEsdtTokenPayment::new(
                from_token.clone(),
                0,
                amount_to_swap.into_raw_units().clone(),
            ),
            &controller,
            false,
            None,
            &mut storage_cache,
            &attributes,
            true,
        );

        let received = self.swap_tokens(
            &debt_token,
            &from_token,
            &amount_to_swap.into_raw_units(),
            steps.into_option(),
            limits.into_option(),
        );

        payments.push(received);

        for payment in payments.iter() {
            self.validate_payment(&payment);
            let feed = self.get_token_price(&payment.token_identifier, &mut storage_cache);
            let payment_dec =
                ManagedDecimal::from_raw_units(payment.amount.clone(), feed.decimals as usize);
            // 3. Process repay
            self.internal_repay(
                account.token_nonce,
                &payment.token_identifier,
                &payment_dec,
                &caller,
                self.get_token_amount_in_egld_raw(&payment_dec, &feed.price),
                &feed,
                &mut storage_cache,
                &attributes,
            );
        }

        // Make sure that after the swap the position is not becoming eligible for liquidation due to slippage
        self.validate_withdraw_health_factor(account.token_nonce, false, &mut storage_cache, None);
    }
}
