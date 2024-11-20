#![no_std]

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

pub mod config;
pub mod errors;
pub mod factory;
pub mod math;
pub mod proxy_pool;
pub mod proxy_price_aggregator;
pub mod router;
pub mod storage;
pub mod utils;

pub use common_structs::*;
pub use common_tokens::*;
pub use errors::*;

use multiversx_sc::codec::Empty;

#[multiversx_sc::contract]
pub trait LendingPool:
    factory::FactoryModule
    + router::RouterModule
    + config::ConfigModule
    + common_events::EventsModule
    + common_checks::ChecksModule
    + common_tokens::AccountTokenModule
    + storage::LendingStorageModule
    + utils::LendingUtilsModule
    + math::LendingMathModule
    + multiversx_sc_modules::default_issue_callbacks::DefaultIssueCallbacksModule
{
    #[init]
    fn init(&self, lp_template_address: ManagedAddress, aggregator: ManagedAddress) {
        self.liq_pool_template_address().set(&lp_template_address);
        self.price_aggregator_address().set(&aggregator);
    }

    #[upgrade]
    fn upgrade(&self) {}

    #[endpoint(enter)]
    fn enter(&self) -> EsdtTokenPayment {
        let caller = self.blockchain().get_caller();
        let nft_account_amount = BigUint::from(1u64);

        let nft_token_payment =
            self.account_token()
                .nft_create_and_send(&caller, nft_account_amount, &Empty);

        self.account_positions()
            .insert(nft_token_payment.token_nonce);

        self.create_account_event(&caller, nft_token_payment.token_nonce);
        nft_token_payment
    }

    #[payable("*")]
    #[endpoint(supply)]
    fn supply(&self) {
        let [account_token, collateral_payment] = self.call_value().multi_esdt();
        let (account_token_id, account_nonce, account_amount) = account_token.into_tuple();
        let (collateral_token_id, _, collateral_amount) = collateral_payment.clone().into_tuple();
        let pool_address = self.get_pool_address(&collateral_token_id);
        let initial_caller = self.blockchain().get_caller();

        self.require_asset_supported(&collateral_token_id);
        self.lending_account_in_the_market(account_nonce);
        self.lending_account_token_valid(&account_token_id);
        self.require_amount_greater_than_zero(&collateral_amount);
        self.require_non_zero_address(&initial_caller);

        let initial_or_new_deposit_position = self.get_existing_or_new_deposit_position_for_token(
            account_nonce,
            collateral_token_id.clone(),
        );

        let return_deposit_position = self
            .tx()
            .to(pool_address)
            .typed(proxy_pool::LiquidityPoolProxy)
            .supply(initial_or_new_deposit_position)
            .esdt(collateral_payment)
            .returns(ReturnsResult)
            .sync_call();

        self.update_position_event(
            &collateral_amount, // Representing the amount of collateral added
            &return_deposit_position,
            Some(&initial_caller),
        );

        self.deposit_positions(account_nonce)
            .insert(collateral_token_id, return_deposit_position);

        // Return NFT to owner
        self.send().direct_esdt(
            &initial_caller,
            &account_token_id,
            account_nonce,
            &account_amount,
        );
    }

    #[payable("*")]
    #[endpoint(withdraw)]
    fn withdraw(&self, withdraw_token_id: &TokenIdentifier, amount: &BigUint) {
        let account_token = self.call_value().single_esdt();
        let initial_caller = self.blockchain().get_caller();

        self.lending_account_token_valid(&account_token.token_identifier);
        self.require_non_zero_address(&initial_caller);

        self.internal_withdraw(
            account_token.token_nonce,
            withdraw_token_id,
            amount,
            &initial_caller,
            false,
        );

        // Return NFT to owner
        self.tx()
            .to(&initial_caller)
            .esdt(account_token)
            .sync_call();
    }

    fn internal_withdraw(
        &self,
        account_nonce: u64,
        withdraw_token_id: &TokenIdentifier,
        amount: &BigUint,
        initial_caller: &ManagedAddress,
        is_liquidation: bool,
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
                    .withdraw(initial_caller, amount, dp, is_liquidation)
                    .returns(ReturnsResult)
                    .sync_call();

                self.update_position_event(
                    amount, // Representing the amount of collateral removed
                    &deposit_position,
                    Some(initial_caller),
                );

                if deposit_position.amount == 0 {
                    dep_pos_map.remove(withdraw_token_id);
                } else {
                    dep_pos_map.insert(withdraw_token_id.clone(), deposit_position);
                }
            }
            None => panic!(
                "Tokens {} are not available for this account", // maybe was liquidated already
                withdraw_token_id
            ),
        };
    }

    #[payable("*")]
    #[endpoint(borrow)]
    fn borrow(&self, asset_to_borrow: TokenIdentifier, amount: BigUint) {
        let (nft_account_token_id, nft_account_nonce, nft_account_amount) =
            self.call_value().single_esdt().into_tuple();
        let initial_caller = self.blockchain().get_caller();
        let borrow_token_pool_address = self.get_pool_address(&asset_to_borrow);

        self.require_asset_supported(&asset_to_borrow);
        self.lending_account_in_the_market(nft_account_nonce);
        self.lending_account_token_valid(&nft_account_token_id);
        self.require_amount_greater_than_zero(&amount);
        self.require_non_zero_address(&initial_caller);

        self.update_collateral_with_interest(nft_account_nonce);
        self.update_borrows_with_debt(nft_account_nonce);

        let get_weighted_collateral_in_dollars =
            self.get_total_collateral_in_dollars(nft_account_nonce);
        let borrowed_amount_in_dollars = self.get_total_borrow_in_dollars(nft_account_nonce);
        let amount_to_borrow_in_dollars =
            amount.clone() * self.get_token_price_data(&asset_to_borrow).price;

        require!(
            get_weighted_collateral_in_dollars
                > (borrowed_amount_in_dollars + amount_to_borrow_in_dollars),
            ERROR_INSUFFICIENT_COLLATERAL
        );

        let initial_borrow_position = self.get_existing_or_new_borrow_position_for_token(
            nft_account_nonce,
            asset_to_borrow.clone(),
        );

        let borrow_position = self
            .tx()
            .to(borrow_token_pool_address)
            .typed(proxy_pool::LiquidityPoolProxy)
            .borrow(&initial_caller, &amount, initial_borrow_position)
            .returns(ReturnsResult)
            .sync_call();

        self.update_position_event(
            &amount, // Representing the amount of borrowed tokens
            &borrow_position,
            Some(&initial_caller),
        );

        if borrow_position.amount == 0 {
            // Update BorrowPosition
            self.borrow_positions(nft_account_nonce)
                .remove(&asset_to_borrow);
        } else {
            // Update BorrowPosition if it's not empty
            self.borrow_positions(nft_account_nonce)
                .insert(asset_to_borrow, borrow_position);
        }

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
        let (repay_token_id, repay_amount) = self.call_value().single_fungible_esdt();
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
        repay_token_id: &TokenIdentifier,
        repay_amount: &BigUint,
        initial_caller: &ManagedAddress,
    ) {
        let asset_address = self.get_pool_address(repay_token_id);

        self.lending_account_in_the_market(account_nonce);
        self.require_asset_supported(repay_token_id);
        self.require_amount_greater_than_zero(repay_amount);
        let mut map = self.borrow_positions(account_nonce);
        match map.get(repay_token_id) {
            Some(bp) => {
                let borrow_position = self
                    .tx()
                    .to(asset_address)
                    .typed(proxy_pool::LiquidityPoolProxy)
                    .repay(initial_caller, bp)
                    .single_esdt(repay_token_id, 0, repay_amount)
                    .returns(ReturnsResult)
                    .sync_call();

                self.update_position_event(
                    repay_amount, // Representing the amount of repayed tokens
                    &borrow_position,
                    Some(&initial_caller),
                );

                // Update BorrowPosition
                map.remove(repay_token_id);
                if borrow_position.amount != 0 {
                    map.insert(repay_token_id.clone(), borrow_position);
                }
            }
            None => panic!(
                "Borrowed tokens {} are not available for this account",
                repay_token_id
            ),
        };
    }

    #[payable("*")]
    #[endpoint(liquidate)]
    fn liquidate(
        &self,
        liquidatee_account_nonce: u64,
        liquidation_threshold: BigUint,
        token_to_liquidate: &TokenIdentifier,
    ) {
        let liquidator_payment = self.call_value().single_fungible_esdt();
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

        require!(
            liquidation_threshold <= MAX_THRESHOLD,
            MAX_THRESHOLD_ERROR_MSG
        );

        // Make sure the collateral and borrows are updated with interest and debt before liquidation
        self.update_collateral_with_interest(liquidatee_account_nonce);
        self.update_borrows_with_debt(liquidatee_account_nonce);

        let liq_bonus = self.get_liquidation_bonus_non_zero(&liquidator_payment.0);

        let collateral_in_dollars =
            self.get_weighted_collateral_in_dollars(liquidatee_account_nonce);

        let borrowed_dollars = self.get_total_borrow_in_dollars(liquidatee_account_nonce);

        let health_factor = self.compute_health_factor(
            &collateral_in_dollars,
            &borrowed_dollars,
            &liquidation_threshold,
        );

        require!(health_factor < BP, ERROR_HEALTH_FACTOR);

        // Price in USD of the asset that the liquidator sends to us
        let liquidator_asset_data = self.get_token_price_data(&liquidator_payment.0);
        // Total USD value of the liquidator's asset
        let liquidator_asset_value_in_dollars =
            &liquidator_payment.1 * &liquidator_asset_data.price;

        let amount_needed_for_liquidation = &borrowed_dollars * &liquidation_threshold / &bp;

        require!(
            liquidator_asset_value_in_dollars >= amount_needed_for_liquidation,
            ERROR_INSUFFICIENT_LIQUIDATION
        );

        // amount_liquidated (1 + liq_bonus)
        let amount_to_return_to_liquidator_in_dollars =
            (&liquidator_payment.1 * &(&bp + &liq_bonus)) / bp;

        // Repay the liquidatee account debt with the liquidator's asset
        self.internal_repay(
            liquidatee_account_nonce,
            &liquidator_payment.0,
            &liquidator_payment.1,
            &initial_caller,
        );

        // Go through all DepositPositions and send amount_to_return_in_dollars to Liquidator
        // USDC return to liquidator
        let amount_to_send = self.compute_amount_in_tokens(
            liquidatee_account_nonce,
            token_to_liquidate,
            amount_to_return_to_liquidator_in_dollars,
        );

        // Remove collateral from the liquidatee account
        self.internal_withdraw(
            liquidatee_account_nonce,
            token_to_liquidate,
            &amount_to_send,
            &initial_caller,
            true,
        );
    }

    #[endpoint(updatePositionInterest)]
    fn update_collateral_with_interest(&self, account_position: u64) {
        let deposit_positions = self.deposit_positions(account_position);

        for dp in deposit_positions.values() {
            let asset_address = self.get_pool_address(&dp.token_id);
            let latest_position = self
                .tx()
                .to(asset_address)
                .typed(proxy_pool::LiquidityPoolProxy)
                .update_collateral_with_interest(dp.clone())
                .returns(ReturnsResult)
                .sync_call();

            self.deposit_positions(account_position)
                .insert(dp.token_id, latest_position);
        }
    }

    #[endpoint(updatePositionDebt)]
    fn update_borrows_with_debt(&self, account_position: u64) {
        let borrow_positions = self.borrow_positions(account_position);

        for bp in borrow_positions.values() {
            let asset_address = self.get_pool_address(&bp.token_id);
            let latest_position = self
                .tx()
                .to(asset_address)
                .typed(proxy_pool::LiquidityPoolProxy)
                .update_borrows_with_debt(bp.clone())
                .returns(ReturnsResult)
                .sync_call();

            self.borrow_positions(account_position)
                .insert(bp.token_id, latest_position);
        }
    }
}
