#![no_std]

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

pub mod config;
pub mod errors;
pub mod events;
pub mod factory;
mod math;
pub mod proxy_pool;
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
    + events::EventsModule
    + common_checks::ChecksModule
    + common_tokens::AccountTokenModule
    + storage::LendingStorageModule
    + utils::LendingUtilsModule
    + math::LendingMathModule
    + price_aggregator_proxy::PriceAggregatorModule
    + multiversx_sc_modules::default_issue_callbacks::DefaultIssueCallbacksModule
{
    #[init]
    fn init(&self, lp_template_address: ManagedAddress) {
        self.liq_pool_template_address().set(&lp_template_address);
    }

    #[endpoint(enterMarket)]
    fn enter_market(&self) -> EsdtTokenPayment {
        let caller = self.blockchain().get_caller();
        let nft_account_amount = BigUint::from(1u64);

        let nft_token_payment =
            self.account_token()
                .nft_create_and_send(&caller, nft_account_amount, &Empty);

        self.account_positions()
            .insert(nft_token_payment.token_nonce);

        self.new_account_event(&caller, nft_token_payment.token_nonce);
        nft_token_payment
    }

    #[payable("*")]
    #[endpoint(addCollateral)]
    fn add_collateral(&self) {
        let [nft_account_token, collateral_payment] = self.call_value().multi_esdt();
        let (nft_account_token_id, nft_account_nonce, nft_account_amount) =
            nft_account_token.into_tuple();
        let (collateral_token_id, _, collateral_amount) = collateral_payment.clone().into_tuple();
        let pool_address = self.get_pool_address(&collateral_token_id);
        let initial_caller = self.blockchain().get_caller();

        self.require_asset_supported(&collateral_token_id);
        self.lending_account_in_the_market(nft_account_nonce);
        self.lending_account_token_valid(nft_account_token_id.clone());
        self.require_amount_greater_than_zero(&collateral_amount);
        self.require_non_zero_address(&initial_caller);

        let initial_or_new_deposit_position = self.get_existing_or_new_deposit_position_for_token(
            nft_account_nonce,
            collateral_token_id.clone(),
        );

        let return_deposit_position = self
            .tx()
            .to(pool_address)
            .typed(proxy_pool::LiquidityPoolProxy)
            .add_collateral(initial_or_new_deposit_position)
            .esdt(collateral_payment)
            .returns(ReturnsResult)
            .sync_call();

        self.update_deposit_position_event(
            nft_account_nonce,
            &collateral_amount, // Representing the amount of collateral added
            &return_deposit_position,
            &initial_caller,
        );

        self.deposit_positions(nft_account_nonce)
            .insert(collateral_token_id, return_deposit_position);

        // Return NFT to owner
        self.send().direct_esdt(
            &initial_caller,
            &nft_account_token_id,
            nft_account_nonce,
            &nft_account_amount,
        );
    }

    #[payable("*")]
    #[endpoint(removeCollateral)]
    fn remove_collateral(&self, withdraw_token_id: TokenIdentifier, amount: BigUint) {
        let (nft_account_token_id, nft_account_nonce, nft_account_amount) =
            self.call_value().single_esdt().into_tuple();
        let initial_caller = self.blockchain().get_caller();
        let pool_address = self.get_pool_address(&withdraw_token_id);

        self.require_asset_supported(&withdraw_token_id);
        self.lending_account_in_the_market(nft_account_nonce);
        self.lending_account_token_valid(nft_account_token_id.clone());
        self.require_amount_greater_than_zero(&amount);
        self.require_non_zero_address(&initial_caller);
        require!(
            amount
                > self.get_collateral_amount_for_token(
                    nft_account_nonce,
                    nft_account_token_id.clone()
                ),
            ERROR_INSUFFICIENT_DEPOSIT
        );

        let mut dep_pos_map = self.deposit_positions(nft_account_nonce);
        match dep_pos_map.get(&withdraw_token_id) {
            Some(dp) => {
                let deposit_position: DepositPosition<<Self as ContractBase>::Api> = self
                    .tx()
                    .to(pool_address)
                    .typed(proxy_pool::LiquidityPoolProxy)
                    .remove_collateral(&initial_caller, &amount, dp)
                    .execute_on_dest_context();

                self.update_deposit_position_event(
                    nft_account_nonce,
                    &amount, // Representing the amount of collateral removed
                    &deposit_position,
                    &initial_caller,
                );

                if deposit_position.amount == 0 {
                    dep_pos_map.remove(&withdraw_token_id);
                } else {
                    dep_pos_map.insert(withdraw_token_id, deposit_position);
                }
            }
            None => panic!(
                "Tokens {} are not available for this account", // maybe was liquidated already
                withdraw_token_id
            ),
        };
        // Return NFT to owner
        self.send().direct_esdt(
            &initial_caller,
            &nft_account_token_id,
            nft_account_nonce,
            &nft_account_amount,
        );
    }

    #[payable("*")]
    #[endpoint]
    fn borrow(&self, asset_to_borrow: TokenIdentifier, amount: BigUint) {
        let (nft_account_token_id, nft_account_nonce, nft_account_amount) =
            self.call_value().single_esdt().into_tuple();
        let initial_caller = self.blockchain().get_caller();
        let borrow_token_pool_address = self.get_pool_address(&asset_to_borrow);

        self.require_asset_supported(&asset_to_borrow);
        self.lending_account_in_the_market(nft_account_nonce);
        self.lending_account_token_valid(nft_account_token_id.clone());
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

        let borrow_position: BorrowPosition<Self::Api> = self
            .tx()
            .to(borrow_token_pool_address)
            .typed(proxy_pool::LiquidityPoolProxy)
            .borrow(&initial_caller, &amount, initial_borrow_position)
            .execute_on_dest_context();

        self.update_borrow_position_event(
            nft_account_nonce,
            &amount, // Representing the amount of borrowed tokens
            &borrow_position,
            &initial_caller,
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
    #[endpoint]
    fn repay(&self) {
        let [nft_account_token, payment_repay] = self.call_value().multi_esdt();
        let (nft_account_token_id, nft_account_nonce, nft_account_amount) =
            nft_account_token.into_tuple();
        let (repay_token_id, repay_nonce, repay_amount) = payment_repay.into_tuple();
        let initial_caller = self.blockchain().get_caller();
        let asset_address = self.get_pool_address(&repay_token_id);

        self.lending_account_in_the_market(nft_account_nonce);
        self.lending_account_token_valid(nft_account_token_id.clone());
        self.require_asset_supported(&repay_token_id);
        self.require_amount_greater_than_zero(&repay_amount);
        self.require_non_zero_address(&initial_caller);

        match self
            .borrow_positions(nft_account_nonce)
            .get(&repay_token_id)
        {
            Some(bp) => {
                let borrow_position: BorrowPosition<Self::Api> = self
                    .tx()
                    .to(asset_address)
                    .typed(proxy_pool::LiquidityPoolProxy)
                    .repay(&initial_caller, bp)
                    .with_esdt_transfer((repay_token_id.clone(), repay_nonce, repay_amount.clone()))
                    .execute_on_dest_context();

                self.update_borrow_position_event(
                    nft_account_nonce,
                    &repay_amount, // Representing the amount of repayed tokens
                    &borrow_position,
                    &initial_caller,
                );

                // Update BorrowPosition
                self.borrow_positions(nft_account_nonce)
                    .remove(&repay_token_id);
                if borrow_position.amount != 0 {
                    self.borrow_positions(nft_account_nonce)
                        .insert(repay_token_id, borrow_position);
                }
            }
            None => panic!(
                "Borrowed tokens {} are not available for this account",
                repay_token_id
            ),
        };
        // Return NFT to owner
        self.send().direct_esdt(
            &initial_caller,
            &nft_account_token_id,
            nft_account_nonce,
            &nft_account_amount,
        );
    }

    #[payable("*")]
    #[endpoint(liquidate)]
    fn liquidate(
        &self,
        liquidatee_account_nonce: u64,
        liquidation_threshold: BigUint,
        token_to_liquidate: TokenIdentifier,
    ) {
        let (liquidator_asset_token_id, liquidator_asset_amount) =
            self.call_value().single_fungible_esdt();
        let bp = BigUint::from(BP);

        let initial_caller = self.blockchain().get_caller();

        // Liquidatee is in the market; Liquidator doesn't have to be in the Lending Protocol
        self.lending_account_in_the_market(liquidatee_account_nonce);
        self.require_asset_supported(&liquidator_asset_token_id);
        self.require_amount_greater_than_zero(&liquidator_asset_amount);
        self.require_non_zero_address(&initial_caller);
        require!(
            token_to_liquidate == liquidator_asset_token_id,
            ERROR_TOKEN_MISMATCH
        );

        require!(
            liquidation_threshold <= MAX_THRESHOLD,
            MAX_THRESHOLD_ERROR_MSG
        );

        let liq_bonus = self.get_liquidation_bonus_non_zero(&liquidator_asset_token_id);
        let total_collateral_in_dollars =
            self.get_weighted_collateral_in_dollars(liquidatee_account_nonce);
        let borrowed_value_in_dollars = self.get_total_borrow_in_dollars(liquidatee_account_nonce);

        let health_factor = self.compute_health_factor(
            &total_collateral_in_dollars,
            &borrowed_value_in_dollars,
            &liquidation_threshold,
        );
        require!(health_factor < BP, ERROR_HEALTH_FACTOR);

        let liquidator_asset_data = self.get_token_price_data(&liquidator_asset_token_id);
        let liquidator_asset_value_in_dollars =
            liquidator_asset_amount.clone() * liquidator_asset_data.price;

        let amount_needed_for_liquidation = borrowed_value_in_dollars * liquidation_threshold / &bp;
        require!(
            liquidator_asset_value_in_dollars >= amount_needed_for_liquidation,
            ERROR_INSUFFICIENT_LIQUIDATION
        );

        // amount_liquidated (1 + liq_bonus)
        let amount_to_return_to_liquidator_in_dollars =
            (liquidator_asset_amount * (&bp + &liq_bonus)) / bp;

        // Go through all DepositPositions and send amount_to_return_in_dollars to Liquidator
        let amount_to_send = self.compute_amount_in_tokens(
            liquidatee_account_nonce,
            token_to_liquidate.clone(),
            amount_to_return_to_liquidator_in_dollars,
        );

        let asset_address = self.get_pool_address(&token_to_liquidate);

        let _: IgnoreValue = self
            .tx()
            .to(asset_address)
            .typed(proxy_pool::LiquidityPoolProxy)
            .send_tokens(&initial_caller, &amount_to_send)
            .execute_on_dest_context();
    }

    #[endpoint(updateCollateralWithInterest)]
    fn update_collateral_with_interest(&self, account_position: u64) {
        let deposit_positions = self.deposit_positions(account_position);

        for dp in deposit_positions.values() {
            let asset_address = self.get_pool_address(&dp.token_id);
            let _: IgnoreValue = self
                .tx()
                .to(asset_address)
                .typed(proxy_pool::LiquidityPoolProxy)
                .update_collateral_with_interest(dp)
                .execute_on_dest_context();
        }
    }

    #[endpoint(updateBorrowsWithDebt)]
    fn update_borrows_with_debt(&self, account_position: u64) {
        let borrow_positions = self.borrow_positions(account_position);

        for bp in borrow_positions.values() {
            let asset_address = self.get_pool_address(&bp.token_id);
            let _: IgnoreValue = self
                .tx()
                .to(asset_address)
                .typed(proxy_pool::LiquidityPoolProxy)
                .update_borrows_with_debt(bp)
                .execute_on_dest_context();
        }
    }
}
