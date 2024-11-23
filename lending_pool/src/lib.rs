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

        require!(
            !(asset_info.is_isolated && !nft_attributes.is_isolated),
            ERROR_MIX_ISOLATED_COLLATERAL
        );

        if asset_info.is_isolated || nft_attributes.is_isolated {
            self.validate_isolated_collateral(account_nonce, &collateral_payment.token_identifier);
        }

        if (!asset_info.is_isolated && asset_info.is_e_mode_enabled && e_mode_category.is_some())
            || (!nft_attributes.is_isolated && nft_attributes.e_mode_category != 0)
        {
            let e_mode_category_id = e_mode_category.clone().into_option().unwrap();

            require!(
                !self
                    .asset_e_modes(&collateral_payment.token_identifier)
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
            Some(&initial_caller),
            Some(nft_attributes),
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
            amount,
            &initial_caller,
            false,
            &BigUint::from(0u64),
            Some(attributes),
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
        amount: &BigUint,
        initial_caller: &ManagedAddress,
        is_liquidation: bool,
        liquidation_fee: &BigUint,
        attributes: Option<NftAccountAttributes>,
    ) {
        let pool_address = self.get_pool_address(withdraw_token_id);

        self.require_asset_supported(withdraw_token_id);
        self.lending_account_in_the_market(account_nonce);
        self.require_amount_greater_than_zero(amount);

        let mut dep_pos_map = self.deposit_positions(account_nonce);
        match dep_pos_map.get(withdraw_token_id) {
            Some(dp) => {
                require!(amount > &dp.amount, ERROR_INSUFFICIENT_DEPOSIT);

                let deposit_position = self
                    .tx()
                    .to(pool_address)
                    .typed(proxy_pool::LiquidityPoolProxy)
                    .withdraw(initial_caller, amount, dp, is_liquidation, liquidation_fee)
                    .returns(ReturnsResult)
                    .sync_call();

                self.update_position_event(
                    amount, // Representing the amount of collateral removed
                    &deposit_position,
                    Some(initial_caller),
                    attributes,
                );

                if deposit_position.amount == 0 {
                    dep_pos_map.remove(withdraw_token_id);
                } else {
                    dep_pos_map.insert(withdraw_token_id.clone(), deposit_position);
                }
            }
            None => panic!(
                "Tokens {} are not available for this account", // maybe was liquidated already
                (withdraw_token_id.clone().into_name())
            ),
        };

        let collateral_in_dollars = self.get_liquidation_collateral_available(account_nonce);

        let borrowed_dollars = self.get_total_borrow_in_dollars(account_nonce);

        let health_factor = self.compute_health_factor(&collateral_in_dollars, &borrowed_dollars);

        // Make sure the health factor is greater than 100% when is a normal withdraw, to prevent self liquidations risks
        // For liquidations, we allow health factor to go below 100% as maybe the liquidation was not enough to cover the debt and make it healthy again
        // A next liquidation can happen again until the health factor is above 100%
        require!(
            health_factor >= BP && !is_liquidation,
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
                !self
                    .asset_e_modes(&asset_to_borrow)
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

            require!(
                asset_emode_config.can_be_borrowed,
                ERROR_EMODE_ASSET_NOT_SUPPORTED_AS_COLLATERAL
            );

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

        let price_data = self.get_token_price_data(&asset_to_borrow);

        // Siloed mode works in combination with e-mode categories
        if asset_config.is_siloed {
            require!(
                collateral_positions.len() <= 1,
                ERROR_ASSET_NOT_BORROWABLE_IN_SILOED
            );

            require!(
                borrow_positions.len() == 0
                    || (&borrow_positions.get(0).token_id == &asset_to_borrow),
                ERROR_ASSET_NOT_BORROWABLE_IN_SILOED
            );

            self.validate_isolated_debt_ceiling(
                &asset_config,
                &asset_to_borrow,
                &amount,
                &price_data,
            );
        }

        let get_ltv_collateral_in_dollars =
            self.get_ltv_collateral_in_dollars_vec(&collateral_positions);

        let borrowed_amount_in_dollars = self.get_total_borrow_in_dollars_vec(&borrow_positions);

        let amount_to_borrow_in_dollars = amount.clone() * price_data.price;

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
            .to(borrow_token_pool_address)
            .typed(proxy_pool::LiquidityPoolProxy)
            .borrow(&initial_caller, &amount, borrow_position)
            .returns(ReturnsResult)
            .sync_call();

        if account_attributes.is_isolated {
            self.update_isolated_debt_usd(
                &asset_to_borrow,
                &amount_to_borrow_in_dollars,
                true, // is_increase
            );
        }

        self.update_position_event(
            &amount, // Representing the amount of borrowed tokens
            &ret_borrow_position,
            Some(&initial_caller),
            Some(account_attributes),
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
        self.internal_repay(
            account_nonce,
            &repay_token_id,
            &repay_amount,
            &initial_caller,
        );
    }

    fn internal_repay(
        &self,
        account_nonce: u64,
        repay_token_id: &EgldOrEsdtTokenIdentifier,
        repay_amount: &BigUint,
        initial_caller: &ManagedAddress,
    ) {
        let asset_address = self.get_pool_address(repay_token_id);

        self.lending_account_in_the_market(account_nonce);
        self.require_asset_supported(repay_token_id);
        self.require_amount_greater_than_zero(repay_amount);
        let collaterals_map = self.deposit_positions(account_nonce);
        if collaterals_map.len() == 1 {
            // Impossible to have 0 collateral when we repay a position
            let (collateral_token_id, _) = collaterals_map.iter().next().unwrap();

            let asset_config = self.asset_config(&collateral_token_id).get();

            if asset_config.is_isolated {
                let price_data = self.get_token_price_data(&collateral_token_id);
                // In repay function
                self.update_isolated_debt_usd(
                    &collateral_token_id,
                    &(repay_amount * &price_data.price),
                    false, // is_decrease
                );
            }
        };

        let mut map = self.borrow_positions(account_nonce);
        match map.get(repay_token_id) {
            Some(bp) => {
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
                    Some(&initial_caller),
                    None,
                );

                // Update BorrowPosition
                map.remove(repay_token_id);
                if borrow_position.amount != 0 {
                    map.insert(repay_token_id.clone(), borrow_position);
                }
            }
            None => panic!(
                "Borrowed tokens {} are not available for this account",
                (repay_token_id.clone().into_name())
            ),
        };
    }

    #[payable("*")]
    #[endpoint(liquidate)]
    fn liquidate(
        &self,
        liquidatee_account_nonce: u64,
        token_to_liquidate: &EgldOrEsdtTokenIdentifier,
    ) {
        let liquidator_payment = self.call_value().egld_or_single_fungible_esdt();
        let bp = BigUint::from(BP);

        let initial_caller = self.blockchain().get_caller();

        // Liquidatee is in the market; Liquidator doesn't have to be in the Lending Protocol
        self.lending_account_in_the_market(liquidatee_account_nonce);
        self.require_asset_supported(&liquidator_payment.0);
        self.require_amount_greater_than_zero(&liquidator_payment.1);
        self.require_non_zero_address(&initial_caller);

        require!(
            token_to_liquidate == &liquidator_payment.0,
            ERROR_TOKEN_MISMATCH
        );

        // Make sure the collateral and borrows are updated with interest and debt before liquidation
        let collateral_positions = self.update_collateral_with_interest(liquidatee_account_nonce);
        let borrow_positions = self.update_borrows_with_debt(liquidatee_account_nonce);

        let collateral_in_dollars = self.get_ltv_collateral_in_dollars_vec(&collateral_positions);

        let borrowed_dollars = self.get_total_borrow_in_dollars_vec(&borrow_positions);

        let health_factor = self.compute_health_factor(&collateral_in_dollars, &borrowed_dollars);

        require!(health_factor < BP, ERROR_HEALTH_FACTOR);

        // Price in USD of the asset that the liquidator sends to us
        let liquidator_asset_data = self.get_token_price_data(&liquidator_payment.0);
        // Total USD value of the liquidator's asset
        let liquidator_asset_value_in_dollars =
            &liquidator_payment.1 * &liquidator_asset_data.price;

        let amount_needed_for_liquidation = self.calculate_single_asset_liquidation_amount(
            &health_factor,
            &borrowed_dollars,
            token_to_liquidate,
            &liquidator_asset_data,
            liquidatee_account_nonce,
        );

        require!(
            liquidator_asset_value_in_dollars >= amount_needed_for_liquidation,
            ERROR_INSUFFICIENT_LIQUIDATION
        );

        let asset_config = self.asset_config(&liquidator_payment.0).get();

        // Calculate dynamic liquidation bonus
        let liq_bonus = self
            .calculate_dynamic_liquidation_bonus(&health_factor, asset_config.liquidation_bonus);

        // Calculate how much of liquidator's payment we'll actually use
        let liquidator_payment_to_use =
            if liquidator_asset_value_in_dollars > amount_needed_for_liquidation {
                // Convert excess dollars back to token amount
                let excess_dollars =
                    &liquidator_asset_value_in_dollars - &amount_needed_for_liquidation;

                let excess_tokens = (&excess_dollars * BP / &liquidator_asset_data.price)
                    * BigUint::from(10u64).pow(liquidator_asset_data.decimals as u32)
                    / BP;

                // Return excess to liquidator
                self.tx()
                    .to(&initial_caller)
                    .payment(EgldOrEsdtTokenPayment::new(
                        liquidator_payment.0.clone(),
                        0,
                        excess_tokens.clone(),
                    ))
                    .transfer();

                // Use only what's needed
                liquidator_payment.1 - excess_tokens
            } else {
                liquidator_payment.1
            };

        // Calculate collateral to return with bonus
        let amount_to_return_to_liquidator_in_dollars =
            (&liquidator_payment_to_use * &liquidator_asset_data.price * &(&bp + &liq_bonus)) / &bp;

        // Repay the liquidatee account debt with the liquidator's asset
        self.internal_repay(
            liquidatee_account_nonce,
            &liquidator_payment.0,
            &liquidator_payment_to_use,
            &initial_caller,
        );

        // Go through all DepositPositions and send amount_to_return_in_dollars to Liquidator
        // USDC return to liquidator
        let amount_to_send = self.compute_amount_in_tokens(
            liquidatee_account_nonce,
            token_to_liquidate,
            amount_to_return_to_liquidator_in_dollars,
        );

        let liquidation_fee =
            self.calculate_dynamic_protocol_fee(&health_factor, asset_config.liquidation_base_fee);
        // Remove collateral from the liquidatee account
        self.internal_withdraw(
            liquidatee_account_nonce,
            token_to_liquidate,
            &amount_to_send,
            &initial_caller,
            true,
            &liquidation_fee,
            None,
        );
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
