#![no_std]

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

pub mod config;
pub mod errors;
pub mod factory;
pub mod math;
pub mod oracle;
pub mod proxy_pool;
pub mod proxy_price_aggregator;
pub mod router;
pub mod storage;
pub mod utils;
pub mod views;

pub use common_structs::*;
pub use common_tokens::*;
pub use errors::*;

#[multiversx_sc::contract]
pub trait LendingPool:
    factory::FactoryModule
    + router::RouterModule
    + config::ConfigModule
    + common_events::EventsModule
    + common_checks::ChecksModule
    + common_tokens::AccountTokenModule
    + storage::LendingStorageModule
    + oracle::OracleModule
    + utils::LendingUtilsModule
    + math::LendingMathModule
    + views::ViewsModule
    + multiversx_sc_modules::default_issue_callbacks::DefaultIssueCallbacksModule
{
    #[init]
    fn init(&self, lp_template_address: ManagedAddress, aggregator: ManagedAddress) {
        self.liq_pool_template_address().set(&lp_template_address);
        self.price_aggregator_address().set(&aggregator);
    }

    #[upgrade]
    fn upgrade(&self) {}

    fn enter(
        &self,
        caller: &ManagedAddress,
        is_isolated: bool,
        e_mode_category: OptionalValue<u8>,
    ) -> (EsdtTokenPayment, NftAccountAttributes) {
        let amount = BigUint::from(1u64);
        let attributes = &NftAccountAttributes {
            is_isolated,
            e_mode_category: if is_isolated {
                0
            } else {
                e_mode_category.into_option().unwrap_or(0)
            },
        };
        let nft_token_payment = self
            .account_token()
            .nft_create_and_send::<NftAccountAttributes>(caller, amount, attributes);

        self.account_positions()
            .insert(nft_token_payment.token_nonce);

        (nft_token_payment, attributes.clone())
    }

    #[payable("*")]
    #[endpoint(supply)]
    fn supply(&self, e_mode_category: OptionalValue<u8>) {
        let payments = self.get_multi_payments();
        let payments_len = payments.len();

        require!(
            payments_len == 2 || payments_len == 1,
            ERROR_INVALID_NUMBER_OF_ESDT_TRANSFERS
        );

        let account_nonce;
        let collateral_payment = payments.get(payments_len - 1).clone();
        let initial_caller = self.blockchain().get_caller();

        let mut asset_info = self
            .asset_config(&collateral_payment.token_identifier)
            .get();

        let nft_attributes;

        if payments_len == 2 {
            let account_token = payments.get(0);
            account_nonce = account_token.token_nonce;
            let token_identifier = account_token.token_identifier.into_esdt_option().unwrap();
            self.lending_account_in_the_market(account_nonce);
            self.lending_account_token_valid(&token_identifier);

            let data = self.blockchain().get_esdt_token_data(
                &self.blockchain().get_sc_address(),
                &token_identifier,
                account_nonce,
            );
            nft_attributes = data.decode_attributes::<NftAccountAttributes>();
            // Return NFT to owner
            self.send().direct_esdt(
                &initial_caller,
                &token_identifier,
                account_nonce,
                &account_token.amount,
            );
        } else {
            let (account_token, attributes) = self.enter(
                &initial_caller,
                asset_info.is_isolated,
                e_mode_category.clone(),
            );
            nft_attributes = attributes;
            account_nonce = account_token.token_nonce;
        }

        require!(
            !(asset_info.is_isolated && nft_attributes.e_mode_category != 0),
            ERROR_CANNOT_USE_EMODE_WITH_ISOLATED_ASSETS
        );

        if asset_info.is_isolated || nft_attributes.is_isolated {
            self.validate_isolated_collateral(account_nonce, &collateral_payment.token_identifier);
        }

        if (!asset_info.is_isolated && asset_info.is_e_mode_enabled && e_mode_category.is_some())
            || (!nft_attributes.is_isolated && nft_attributes.e_mode_category != 0)
        {
            let e_mode_category_id = e_mode_category.clone().into_option().unwrap();

            require!(
                self.asset_e_modes(&collateral_payment.token_identifier)
                    .contains(&e_mode_category_id),
                ERROR_EMODE_CATEGORY_NOT_FOUND
            );

            let category_data = self.e_mode_category().get(&e_mode_category_id).unwrap();

            let asset_emode_config = self
                .e_mode_assets(e_mode_category_id)
                .get(&collateral_payment.token_identifier)
                .unwrap();

            asset_info.can_be_collateral = asset_emode_config.can_be_collateral;
            asset_info.can_be_borrowed = asset_emode_config.can_be_borrowed;
            asset_info.ltv = category_data.ltv;
            asset_info.liquidation_threshold = category_data.liquidation_threshold;
            asset_info.liquidation_bonus = category_data.liquidation_bonus;
        }

        require!(
            asset_info.can_be_collateral,
            ERROR_ASSET_NOT_SUPPORTED_AS_COLLATERAL
        );

        self.check_supply_cap(
            &asset_info,
            &collateral_payment.amount,
            &collateral_payment.token_identifier,
        );

        let pool_address = self.get_pool_address(&collateral_payment.token_identifier);

        self.require_asset_supported(&collateral_payment.token_identifier);
        self.require_amount_greater_than_zero(&collateral_payment.amount);
        self.require_non_zero_address(&initial_caller);

        let deposit_position = self.get_existing_or_new_deposit_position_for_token(
            account_nonce,
            &asset_info,
            &collateral_payment.token_identifier,
        );

        let updated_deposit_position = self
            .tx()
            .to(pool_address)
            .typed(proxy_pool::LiquidityPoolProxy)
            .supply(deposit_position)
            .payment(EgldOrEsdtTokenPayment::new(
                collateral_payment.token_identifier.clone(),
                collateral_payment.token_nonce,
                collateral_payment.amount.clone(),
            ))
            .returns(ReturnsResult)
            .sync_call();

        self.update_position_event(
            &collateral_payment.amount,
            &updated_deposit_position,
            OptionalValue::Some(initial_caller),
            OptionalValue::Some(nft_attributes),
        );

        self.deposit_positions(account_nonce).insert(
            collateral_payment.token_identifier,
            updated_deposit_position,
        );
    }

    #[payable("*")]
    #[endpoint(withdraw)]
    fn withdraw(&self, withdraw_token_id: &EgldOrEsdtTokenIdentifier, amount: &BigUint) {
        let account_token = self.call_value().single_esdt();
        let initial_caller = self.blockchain().get_caller();

        self.lending_account_token_valid(&account_token.token_identifier);
        self.require_non_zero_address(&initial_caller);

        let attributes =
            self.get_account_attributes(account_token.token_nonce, &account_token.token_identifier);

        self.internal_withdraw(
            account_token.token_nonce,
            withdraw_token_id,
            amount.clone(),
            &initial_caller,
            false,
            &BigUint::from(0u64),
            OptionalValue::Some(attributes),
        );

        let dep_pos_map = self.deposit_positions(account_token.token_nonce).len();
        let borrow_pos_map = self.borrow_positions(account_token.token_nonce).len();
        if dep_pos_map == 0 && borrow_pos_map == 0 {
            self.account_token()
                .nft_burn(account_token.token_nonce, &account_token.amount);
            self.account_positions()
                .swap_remove(&account_token.token_nonce);
        } else {
            // Return NFT to owner
            self.tx()
                .to(&initial_caller)
                .esdt(account_token)
                .sync_call();
        }
    }

    fn internal_withdraw(
        &self,
        account_nonce: u64,
        withdraw_token_id: &EgldOrEsdtTokenIdentifier,
        mut amount: BigUint,
        initial_caller: &ManagedAddress,
        is_liquidation: bool,
        liquidation_fee: &BigUint,
        attributes: OptionalValue<NftAccountAttributes>,
    ) {
        let pool_address = self.get_pool_address(withdraw_token_id);

        self.require_asset_supported(withdraw_token_id);
        self.lending_account_in_the_market(account_nonce);
        self.require_amount_greater_than_zero(&amount);

        let mut dep_pos_map = self.deposit_positions(account_nonce);
        let dp_opt = dep_pos_map.get(withdraw_token_id);
        require!(
            dp_opt.is_some(),
            "Token {} is not available for this account",
            withdraw_token_id
        );
        let dp = dp_opt.unwrap();

        if amount > dp.get_total_amount() {
            amount = dp.get_total_amount();
        }

        let deposit_position = self
            .tx()
            .to(pool_address)
            .typed(proxy_pool::LiquidityPoolProxy)
            .withdraw(initial_caller, &amount, dp, is_liquidation, liquidation_fee)
            .returns(ReturnsResult)
            .sync_call();

        self.update_position_event(
            &amount, // Representing the amount of collateral removed
            &deposit_position,
            OptionalValue::Some(initial_caller.clone()),
            attributes,
        );

        if deposit_position.amount == 0 {
            dep_pos_map.remove(withdraw_token_id);
        } else {
            dep_pos_map.insert(withdraw_token_id.clone(), deposit_position);
        }

        let collateral_in_dollars = self.get_liquidation_collateral_available(account_nonce);

        let borrowed_dollars = self.get_total_borrow_in_dollars(account_nonce);

        let health_factor = self.compute_health_factor(&collateral_in_dollars, &borrowed_dollars);

        // Make sure the health factor is greater than 100% when is a normal withdraw, to prevent self liquidations risks
        // For liquidations, we allow health factor to go below 100% as maybe the liquidation was not enough to cover the debt and make it healthy again
        // A next liquidation can happen again until the health factor is above 100%

        require!(
            health_factor >= BP || is_liquidation,
            ERROR_HEALTH_FACTOR_WITHDRAW
        );
    }

    #[payable("*")]
    #[endpoint(borrow)]
    fn borrow(&self, asset_to_borrow: EgldOrEsdtTokenIdentifier, amount: BigUint) {
        let (nft_account_token_id, nft_account_nonce, nft_account_amount) =
            self.call_value().single_esdt().into_tuple();
        let initial_caller = self.blockchain().get_caller();
        let borrow_token_pool_address = self.get_pool_address(&asset_to_borrow);

        self.require_asset_supported(&asset_to_borrow);
        self.lending_account_in_the_market(nft_account_nonce);
        self.lending_account_token_valid(&nft_account_token_id);
        self.require_amount_greater_than_zero(&amount);
        self.require_non_zero_address(&initial_caller);

        let account_attributes =
            self.get_account_attributes(nft_account_nonce, &nft_account_token_id);

        let mut asset_config = self.asset_config(&asset_to_borrow).get();

        if account_attributes.is_isolated {
            require!(
                asset_config.can_borrow_in_isolation,
                ERROR_ASSET_NOT_BORROWABLE_IN_ISOLATION
            );
        }

        if account_attributes.e_mode_category != 0 {
            require!(
                self.asset_e_modes(&asset_to_borrow)
                    .contains(&account_attributes.e_mode_category),
                ERROR_EMODE_CATEGORY_NOT_FOUND
            );

            let category_data = self
                .e_mode_category()
                .get(&account_attributes.e_mode_category)
                .unwrap();

            let asset_emode_config = self
                .e_mode_assets(account_attributes.e_mode_category)
                .get(&asset_to_borrow)
                .unwrap();

            asset_config.can_be_borrowed = asset_emode_config.can_be_borrowed;
            asset_config.can_be_collateral = asset_emode_config.can_be_collateral;
            asset_config.ltv = category_data.ltv;
            asset_config.liquidation_threshold = category_data.liquidation_threshold;
            asset_config.liquidation_bonus = category_data.liquidation_bonus;
        }

        require!(asset_config.can_be_borrowed, ERROR_ASSET_NOT_BORROWABLE);

        self.check_borrow_cap(&asset_config, &amount, &asset_to_borrow);

        let collateral_positions = self.update_collateral_with_interest(nft_account_nonce);
        let borrow_positions = self.update_borrows_with_debt(nft_account_nonce);

        let amount_to_borrow_in_dollars =
            self.get_token_amount_in_dollars(&asset_to_borrow, &amount);

        // Siloed mode works in combination with e-mode categories
        if asset_config.is_siloed {
            require!(
                borrow_positions.len() <= 1,
                ERROR_ASSET_NOT_BORROWABLE_IN_SILOED
            );
        }

        if borrow_positions.len() == 1 {
            let final_first_borrow_position = borrow_positions.get(0).clone();
            let asset_config_borrowed = self
                .asset_config(&final_first_borrow_position.token_id)
                .get();
            if asset_config_borrowed.is_siloed || asset_config.is_siloed {
                require!(
                    asset_to_borrow == final_first_borrow_position.token_id,
                    ERROR_ASSET_NOT_BORROWABLE_IN_SILOED
                );
            }
        }

        let get_ltv_collateral_in_dollars =
            self.get_ltv_collateral_in_dollars_vec(&collateral_positions);

        let borrowed_amount_in_dollars = self.get_total_borrow_in_dollars_vec(&borrow_positions);

        require!(
            get_ltv_collateral_in_dollars
                > (borrowed_amount_in_dollars + &amount_to_borrow_in_dollars),
            ERROR_INSUFFICIENT_COLLATERAL
        );

        let borrow_position = self.get_existing_or_new_borrow_position_for_token(
            nft_account_nonce,
            &asset_config,
            asset_to_borrow.clone(),
        );

        let ret_borrow_position = self
            .tx()
            .to(borrow_token_pool_address.clone())
            .typed(proxy_pool::LiquidityPoolProxy)
            .borrow(&initial_caller, &amount, borrow_position)
            .returns(ReturnsResult)
            .sync_call();

        if account_attributes.is_isolated {
            self.validate_isolated_debt_ceiling(
                &asset_config,
                &asset_to_borrow,
                &amount_to_borrow_in_dollars,
            );
            self.update_isolated_debt_usd(
                &asset_to_borrow,
                &amount_to_borrow_in_dollars,
                true, // is_increase
            );
        }

        self.update_position_event(
            &amount, // Representing the amount of borrowed tokens
            &ret_borrow_position,
            OptionalValue::Some(initial_caller.clone()),
            OptionalValue::Some(account_attributes),
        );

        self.borrow_positions(nft_account_nonce)
            .insert(asset_to_borrow, ret_borrow_position);

        // Return NFT account to owner
        self.send().direct_esdt(
            &initial_caller,
            &nft_account_token_id,
            nft_account_nonce,
            &nft_account_amount,
        );
    }

    #[payable("*")]
    #[endpoint(repay)]
    fn repay(&self, account_nonce: u64) {
        let (repay_token_id, repay_amount) = self.call_value().egld_or_single_fungible_esdt();
        let initial_caller = self.blockchain().get_caller();
        self.update_borrows_with_debt(account_nonce);

        self.internal_repay(
            account_nonce,
            &repay_token_id,
            &repay_amount,
            &initial_caller,
            None,
        );
    }

    fn internal_repay(
        &self,
        account_nonce: u64,
        repay_token_id: &EgldOrEsdtTokenIdentifier,
        repay_amount: &BigUint,
        initial_caller: &ManagedAddress,
        repay_amount_in_usd: Option<BigUint>,
    ) {
        let asset_address = self.get_pool_address(repay_token_id);

        self.lending_account_in_the_market(account_nonce);
        self.require_asset_supported(repay_token_id);
        self.require_amount_greater_than_zero(repay_amount);

        let mut map = self.borrow_positions(account_nonce);
        let bp_opt = map.get(repay_token_id);

        require!(
            bp_opt.is_some(),
            "Borrowed token {} are not available for this account",
            repay_token_id
        );

        let collaterals_map = self.deposit_positions(account_nonce);
        if collaterals_map.len() == 1 {
            // Impossible to have 0 collateral when we repay a position
            let (collateral_token_id, _) = collaterals_map.iter().next().unwrap();

            let asset_config = self.asset_config(&collateral_token_id).get();

            // Check if collateral is an isolated asset
            if asset_config.is_isolated {
                let amount_to_repay_in_dollars = if repay_amount_in_usd.is_none() {
                    self.get_token_amount_in_dollars(&repay_token_id, &repay_amount)
                } else {
                    repay_amount_in_usd.unwrap()
                };
                // In repay function
                self.update_isolated_debt_usd(
                    &collateral_token_id,
                    &amount_to_repay_in_dollars,
                    false, // is_decrease
                );
            }
        };

        let bp = bp_opt.unwrap();

        let borrow_position = self
            .tx()
            .to(asset_address)
            .typed(proxy_pool::LiquidityPoolProxy)
            .repay(initial_caller, bp)
            .egld_or_single_esdt(repay_token_id, 0, repay_amount)
            .returns(ReturnsResult)
            .sync_call();

        self.update_position_event(
            repay_amount, // Representing the amount of repayed tokens
            &borrow_position,
            OptionalValue::Some(initial_caller.clone()),
            OptionalValue::None,
        );

        // Update BorrowPosition
        map.remove(repay_token_id);
        if borrow_position.amount != 0 {
            map.insert(repay_token_id.clone(), borrow_position);
        }
    }

    #[payable("*")]
    #[endpoint(liquidate)]
    fn liquidate(
        &self,
        liquidatee_account_nonce: u64,
        collateral_to_receive: &EgldOrEsdtTokenIdentifier,
    ) {
        let debt_payment = self.call_value().egld_or_single_fungible_esdt();
        let initial_caller = self.blockchain().get_caller();
        let bp = BigUint::from(BP);

        // Basic validations
        self.lending_account_in_the_market(liquidatee_account_nonce);
        self.require_asset_supported(&debt_payment.0);
        self.require_asset_supported(collateral_to_receive);
        self.require_amount_greater_than_zero(&debt_payment.1);
        self.require_non_zero_address(&initial_caller);

        // Update positions with latest interest
        let collateral_positions = self.update_collateral_with_interest(liquidatee_account_nonce);
        let borrow_positions = self.update_borrows_with_debt(liquidatee_account_nonce);

        // Calculate health factor
        let collateral_in_dollars =
            self.get_liquidation_collateral_in_dollars_vec(&collateral_positions);
        let borrowed_dollars = self.get_total_borrow_in_dollars_vec(&borrow_positions);
        let health_factor = self.compute_health_factor(&collateral_in_dollars, &borrowed_dollars);

        require!(health_factor < BP, ERROR_HEALTH_FACTOR);

        let debt_token_price_data = self.get_token_price_data(&debt_payment.0);
        let collateral_token_price_data = self.get_token_price_data(collateral_to_receive);

        // Calculate liquidation amount using Dutch auction mechanism
        let liquidation_amount_usd = self.calculate_single_asset_liquidation_amount(
            &health_factor,
            &borrowed_dollars,
            collateral_to_receive,
            &collateral_token_price_data,
            liquidatee_account_nonce,
        );

        // Convert liquidator's payment to USD
        let debt_payment_in_usd =
            self.get_token_amount_in_dollars_raw(&debt_payment.1, &debt_token_price_data);

        // Ensure liquidator is paying enough
        require!(
            debt_payment_in_usd >= liquidation_amount_usd,
            ERROR_INSUFFICIENT_LIQUIDATION
        );

        // Calculate actual payment to use (handle excess)
        let (payment_to_use, excess_amount) = if debt_payment_in_usd > liquidation_amount_usd {
            let excess_in_usd = &debt_payment_in_usd - &liquidation_amount_usd;
            let excess_in_tokens =
                self.get_usd_amount_in_tokens_raw(&excess_in_usd, &debt_token_price_data);
            (
                debt_payment.1.clone() - &excess_in_tokens,
                Some(excess_in_tokens),
            )
        } else {
            (debt_payment.1.clone(), None)
        };

        // Return excess if any
        if let Some(excess) = excess_amount {
            self.tx()
                .to(&initial_caller)
                .payment(EgldOrEsdtTokenPayment::new(
                    debt_payment.0.clone(),
                    0,
                    excess.clone(),
                ))
                .transfer();
        }

        let asset_config = self.asset_config(collateral_to_receive).get();
        // Calculate collateral to receive with bonus
        let bonus_rate = self
            .calculate_dynamic_liquidation_bonus(&health_factor, asset_config.liquidation_bonus);

        let collateral_to_receive_in_usd = self
            .get_token_amount_in_dollars_raw(&payment_to_use, &debt_token_price_data)
            * (bp.clone() + bonus_rate)
            / bp;

        // Convert USD value to collateral token amount
        let collateral_amount = self.compute_amount_in_tokens(
            liquidatee_account_nonce,
            collateral_to_receive,
            collateral_to_receive_in_usd,
            &collateral_token_price_data,
        );

        // Repay debt
        self.internal_repay(
            liquidatee_account_nonce,
            &debt_payment.0,
            &payment_to_use,
            &initial_caller,
            Some(debt_payment_in_usd),
        );

        // Calculate and transfer collateral with protocol fee
        let liquidation_fee =
            self.calculate_dynamic_protocol_fee(&health_factor, asset_config.liquidation_base_fee);

        self.internal_withdraw(
            liquidatee_account_nonce,
            collateral_to_receive,
            collateral_amount,
            &initial_caller,
            true,
            &liquidation_fee,
            OptionalValue::None,
        );
    }

    #[endpoint(flashLoan)]
    fn flash_loan(
        &self,
        borrowed_token: &EgldOrEsdtTokenIdentifier,
        amount: &BigUint,
        contract_address: &ManagedAddress,
        endpoint: ManagedBuffer<Self::Api>,
        arguments: ManagedArgBuffer<Self::Api>,
    ) {
        let asset_info = self.asset_config(&borrowed_token).get();
        require!(asset_info.flashloan_enabled, ERROR_FLASHLOAN_NOT_ENABLED);

        let pool_address = self.get_pool_address(borrowed_token);

        let shard_id = self.blockchain().get_shard_of_address(contract_address);
        let current_shard_id = self
            .blockchain()
            .get_shard_of_address(&self.blockchain().get_sc_address());

        require!(shard_id == current_shard_id, ERROR_INVALID_SHARD);

        self.tx()
            .to(pool_address)
            .typed(proxy_pool::LiquidityPoolProxy)
            .flash_loan(
                borrowed_token,
                amount,
                contract_address,
                endpoint,
                arguments,
                &asset_info.flash_loan_fee,
            )
            .returns(ReturnsResult)
            .sync_call();
    }

    #[endpoint(updatePositionInterest)]
    fn update_collateral_with_interest(
        &self,
        account_position: u64,
    ) -> ManagedVec<AccountPosition<Self::Api>> {
        let deposit_positions = self.deposit_positions(account_position);
        let mut positions: ManagedVec<Self::Api, AccountPosition<Self::Api>> = ManagedVec::new();
        for dp in deposit_positions.values() {
            let asset_address = self.get_pool_address(&dp.token_id);
            let latest_position = self
                .tx()
                .to(asset_address)
                .typed(proxy_pool::LiquidityPoolProxy)
                .update_collateral_with_interest(dp.clone())
                .returns(ReturnsResult)
                .sync_call();

            positions.push(latest_position.clone());
            self.deposit_positions(account_position)
                .insert(dp.token_id, latest_position);
        }
        positions
    }

    #[endpoint(updatePositionDebt)]
    fn update_borrows_with_debt(
        &self,
        account_position: u64,
    ) -> ManagedVec<AccountPosition<Self::Api>> {
        let borrow_positions = self.borrow_positions(account_position);
        let mut positions: ManagedVec<Self::Api, AccountPosition<Self::Api>> = ManagedVec::new();

        for bp in borrow_positions.values() {
            let asset_address = self.get_pool_address(&bp.token_id);
            let latest_position = self
                .tx()
                .to(asset_address)
                .typed(proxy_pool::LiquidityPoolProxy)
                .update_borrows_with_debt(bp.clone())
                .returns(ReturnsResult)
                .sync_call();

            positions.push(latest_position.clone());
            self.borrow_positions(account_position)
                .insert(bp.token_id, latest_position);
        }
        positions
    }
}
