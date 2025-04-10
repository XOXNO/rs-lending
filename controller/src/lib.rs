#![no_std]

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

pub mod cache;
pub mod config;
pub mod helpers;
pub mod oracle;
pub mod positions;
pub mod router;
pub mod storage;
pub mod strategy;
pub mod utils;
pub mod validation;
pub mod views;

use cache::Cache;
pub use common_errors::*;
pub use common_structs::*;
pub use common_proxies::*;

#[multiversx_sc::contract]
pub trait Controller:
    positions::account::PositionAccountModule
    + positions::supply::PositionDepositModule
    + positions::withdraw::PositionWithdrawModule
    + positions::borrow::PositionBorrowModule
    + positions::repay::PositionRepayModule
    + positions::liquidation::PositionLiquidationModule
    + positions::update::PositionUpdateModule
    + positions::emode::EModeModule
    + positions::vault::PositionVaultModule
    + router::RouterModule
    + config::ConfigModule
    + common_events::EventsModule
    + storage::Storage
    + oracle::OracleModule
    + validation::ValidationModule
    + utils::LendingUtilsModule
    + views::ViewsModule
    + strategy::SnapModule
    + helpers::math::MathsModule
    + common_math::SharedMathModule
    + helpers::swaps::SwapsModule
    + multiversx_sc_modules::default_issue_callbacks::DefaultIssueCallbacksModule
{
    /// Initializes the lending pool contract with required addresses.
    ///
    /// # Arguments
    /// - `lp_template_address`: Address of the liquidity pool template.
    /// - `price_aggregator_address`: Address of the price aggregator.
    /// - `safe_price_view_address`: Address for safe price views.
    /// - `accumulator_address`: Address for revenue accumulation.
    /// - `wegld_address`: Address for wrapped EGLD.
    /// - `ash_swap_address`: Address for AshSwap integration.
    #[init]
    fn init(
        &self,
        lp_template_address: &ManagedAddress,
        price_aggregator_address: &ManagedAddress,
        safe_price_view_address: &ManagedAddress,
        accumulator_address: &ManagedAddress,
        wegld_address: &ManagedAddress,
        ash_swap_address: &ManagedAddress,
    ) {
        self.liq_pool_template_address().set(lp_template_address);
        self.price_aggregator_address()
            .set(price_aggregator_address);
        self.safe_price_view().set(safe_price_view_address);
        self.accumulator_address().set(accumulator_address);
        self.wegld_wrapper().set(wegld_address);
        self.aggregator().set(ash_swap_address);
    }

    #[upgrade]
    fn upgrade(&self) {}

    /// Supplies collateral to the lending pool.
    ///
    /// # Arguments
    /// - `is_vault`: Indicates if the supply is for a vault position.
    /// - `e_mode_category`: Optional e-mode category for specialized parameters.
    ///
    /// # Payment
    /// - Accepts minimum 1 payment: optional account NFT and bulk collateral tokens.
    #[payable]
    #[endpoint(supply)]
    fn supply(&self, is_vault: bool, e_mode_category: OptionalValue<u8>) {
        let mut cache = Cache::new(self);

        // Validate and extract payment details
        let (collaterals, maybe_account, caller, maybe_attributes) =
            self.validate_supply_payment(false);

        require!(
            collaterals.len() >= 1,
            ERROR_INVALID_NUMBER_OF_ESDT_TRANSFERS
        );

        // At this point we know we have at least one collateral
        let first_collateral = collaterals.get(0);
        self.validate_payment(&first_collateral);

        let first_asset_info = cache.get_cached_asset_info(&first_collateral.token_identifier);

        // If the asset is isolated, we can only supply one collateral not a bulk
        require!(
            first_asset_info.is_isolated() && collaterals.len() == 1
                || !first_asset_info.is_isolated(),
            ERROR_BULK_SUPPLY_NOT_SUPPORTED
        );

        // Get or create account position
        let (account_nonce, account_attributes) = self.get_or_create_account(
            &caller,
            first_asset_info.is_isolated(),
            is_vault,
            e_mode_category,
            maybe_account,
            maybe_attributes,
            if first_asset_info.is_isolated() {
                Some(first_collateral.token_identifier.clone())
            } else {
                None
            },
        );

        self.validate_vault_consistency(&account_attributes, is_vault);

        // Process the deposit
        self.process_deposit(
            &caller,
            account_nonce,
            account_attributes,
            &collaterals,
            &mut cache,
        );
    }

    /// Withdraws collateral from the lending pool.
    ///
    /// # Arguments
    /// - `collaterals`: List of tokens and amounts to withdraw.
    ///
    /// # Payment
    /// - Requires account NFT payment.
    #[payable]
    #[endpoint(withdraw)]
    fn withdraw(&self, collaterals: MultiValueEncoded<EgldOrEsdtTokenPayment<Self::Api>>) {
        let (account_payment, caller, account_attributes) = self.validate_account(false);

        let mut cache = Cache::new(self);
        cache.allow_unsafe_price = false;

        // Process each withdrawal
        for collateral in collaterals {
            self.validate_payment(&collateral);
            self.process_withdrawal(
                account_payment.token_nonce,
                collateral,
                &caller,
                false,
                None,
                &mut cache,
                &account_attributes,
                false,
            );
        }

        // Prevent self-liquidation
        self.validate_is_healthy(account_payment.token_nonce, &mut cache, None);

        self.manage_account_after_withdrawal(&account_payment, &caller);
    }

    /// Borrows assets from the lending pool.
    ///
    /// # Arguments
    /// - `borrowed_tokens`: List of tokens and amounts to borrow.
    ///
    /// # Payment
    /// - Requires account NFT payment.
    #[payable]
    #[endpoint(borrow)]
    fn borrow(&self, borrowed_tokens: MultiValueEncoded<EgldOrEsdtTokenPayment<Self::Api>>) {
        let mut cache = Cache::new(self);
        cache.allow_unsafe_price = false;

        let (account_payment, caller, account_attributes) = self.validate_account(true);

        // Sync positions with interest
        let collaterals = self.sync_deposit_positions_interest(
            account_payment.token_nonce,
            &mut cache,
            false,
            &account_attributes,
            true,
        );

        let (_, _, ltv_collateral) = self.calculate_collateral_values(&collaterals, &mut cache);

        let is_bulk_borrow = borrowed_tokens.len() > 1;

        let (mut borrows, mut borrow_index_mapper) = self.sync_borrow_positions_interest(
            account_payment.token_nonce,
            &mut cache,
            false,
            is_bulk_borrow,
        );

        let e_mode = self.get_e_mode_category(account_attributes.get_emode_id());
        self.ensure_e_mode_not_deprecated(&e_mode);

        // Process each borrow
        for borrowed_token in borrowed_tokens {
            self.process_borrow(
                &mut cache,
                account_payment.token_nonce,
                &caller,
                &borrowed_token,
                &account_attributes,
                &e_mode,
                &mut borrows,
                &mut borrow_index_mapper,
                is_bulk_borrow,
                &ltv_collateral,
            );
        }
    }

    /// Repays borrowed assets for an account.
    ///
    /// # Arguments
    /// - `account_nonce`: NFT nonce of the account position.
    #[payable]
    #[endpoint(repay)]
    fn repay(&self, account_nonce: u64) {
        let mut cache = Cache::new(self);
        let payments = self.call_value().all_transfers();
        let caller = self.blockchain().get_caller();
        self.require_active_account(account_nonce);
        let account_attributes = self.account_attributes(account_nonce).get();

        for payment in payments.iter() {
            self.validate_payment(&payment);
            let feed = self.get_token_price(&payment.token_identifier, &mut cache);
            let payment_decimal = self.to_decimal(payment.amount.clone(), feed.asset_decimals);
            let egld_value = self.get_token_egld_value(&payment_decimal, &feed.price);

            self.process_repayment(
                account_nonce,
                &payment.token_identifier,
                &payment_decimal,
                &caller,
                egld_value,
                &feed,
                &mut cache,
                &account_attributes,
            );
        }
    }

    /// Liquidates an unhealthy position.
    ///
    /// # Arguments
    /// - `account_nonce`: NFT nonce of the account to liquidate.
    #[payable]
    #[endpoint(liquidate)]
    fn liquidate(&self, account_nonce: u64) {
        let payments = self.call_value().all_transfers();
        let caller = self.blockchain().get_caller();
        self.process_liquidation(account_nonce, &payments, &caller);
    }

    /// Executes a flash loan.
    ///
    /// # Arguments
    /// - `borrowed_asset_id`: Token identifier to borrow.
    /// - `amount`: Amount to borrow.
    /// - `contract_address`: Address of the contract to receive the loan.
    /// - `endpoint`: Endpoint to call on the receiving contract.
    /// - `arguments`: Arguments for the endpoint call.
    #[endpoint(flashLoan)]
    fn flash_loan(
        &self,
        borrowed_asset_id: &EgldOrEsdtTokenIdentifier,
        amount: BigUint,
        contract_address: &ManagedAddress,
        endpoint: ManagedBuffer<Self::Api>,
        arguments: ManagedArgBuffer<Self::Api>,
    ) {
        let mut cache = Cache::new(self);
        let asset_config = cache.get_cached_asset_info(borrowed_asset_id);
        require!(asset_config.can_flashloan(), ERROR_FLASHLOAN_NOT_ENABLED);

        let pool_address = self.get_pool_address(borrowed_asset_id);
        self.validate_flash_loan_shard(contract_address);
        self.require_amount_greater_than_zero(&amount);
        self.validate_flash_loan_endpoint(&endpoint);

        let feed = self.get_token_price(borrowed_asset_id, &mut cache);

        self.tx()
            .to(pool_address)
            .typed(proxy_pool::LiquidityPoolProxy)
            .flash_loan(
                borrowed_asset_id,
                self.to_decimal(amount, feed.asset_decimals),
                contract_address,
                endpoint,
                arguments,
                asset_config.flashloan_fee.clone(),
                feed.price.clone(),
            )
            .returns(ReturnsResult)
            .sync_call();
    }

    /// Updates account positions with the latest interest data.
    ///
    /// # Arguments
    /// - `account_nonce`: NFT nonce of the account to sync.
    ///
    /// # Returns
    /// - `MultiValue2<ManagedVec<AccountPosition>, ManagedVec<AccountPosition>>`: Updated deposit and borrow positions.
    #[endpoint(updateAccountPositions)]
    fn update_account_positions(
        &self,
        account_nonce: u64,
    ) -> MultiValue2<ManagedVec<AccountPosition<Self::Api>>, ManagedVec<AccountPosition<Self::Api>>>
    {
        self.require_active_account(account_nonce);
        let mut cache = Cache::new(self);
        let account_attributes = self.account_attributes(account_nonce).get();
        let deposits = self.sync_deposit_positions_interest(
            account_nonce,
            &mut cache,
            true,
            &account_attributes,
            true,
        );

        let (borrows, _) =
            self.sync_borrow_positions_interest(account_nonce, &mut cache, true, false);
        (deposits, borrows).into()
    }

    /// Disables vault mode for an account, moving funds to the market pool.
    #[payable]
    #[endpoint(toggleVault)]
    fn toggle_vault(&self, status: bool) {
        let (account_payment, caller, mut account_attributes) = self.validate_account(false);
        self.validate_vault_account(&account_attributes, !status);

        let mut cache = Cache::new(self);
        account_attributes.is_vault_position = status;

        self.process_vault_toggle(
            account_payment.token_nonce,
            status,
            &mut cache,
            &account_attributes,
            &caller,
        );

        self.update_account_attributes(account_payment.token_nonce, &account_attributes);
        self.tx().to(caller).payment(&account_payment).transfer();
    }

    /// Updates LTV or liquidation threshold for account positions of a specific asset.
    ///
    /// # Arguments
    /// - `asset_id`: Token identifier to update.
    /// - `is_ltv`: True to update LTV, false for liquidation threshold.
    /// - `account_nonces`: List of account nonces to update.
    #[endpoint(updateAccountThreshold)]
    fn update_account_threshold(
        &self,
        asset_id: EgldOrEsdtTokenIdentifier,
        has_risks: bool,
        account_nonces: MultiValueEncoded<u64>,
    ) {
        self.require_asset_supported(&asset_id);
        let mut cache = Cache::new(self);
        let mut asset_config = cache.get_cached_asset_info(&asset_id);

        for account_nonce in account_nonces {
            self.require_active_account(account_nonce);
            self.update_position_threshold(
                account_nonce,
                &asset_id,
                has_risks,
                &mut asset_config,
                &mut cache,
            );
        }
    }

    /// Updates interest rate indexes for specified assets.
    ///
    /// # Arguments
    /// - `assets`: List of token identifiers to update.
    #[endpoint(updateIndexes)]
    fn update_indexes(&self, assets: MultiValueEncoded<EgldOrEsdtTokenIdentifier>) {
        let mut cache = Cache::new(self);
        for asset_id in assets {
            self.update_asset_index(&asset_id, &mut cache);
        }
    }

    /// Cleans bad debt from an account.
    ///
    /// It seizes all remaining collateral + interest and adds all remaining debt as bad debt,
    /// then cleans isolated debt if any.
    /// In case of a vault, it toggles the account to non-vault to move funds to the shared liquidity pool.
    /// # Arguments
    /// - `account_nonce`: NFT nonce of the account to clean.
    #[endpoint(cleanBadDebt)]
    fn clean_bad_debt(&self, account_nonce: u64) {
        let mut cache = Cache::new(self);
        self.require_active_account(account_nonce);

        let account_attributes = self.account_attributes(account_nonce).get();

        let collaterals = self.sync_deposit_positions_interest(
            account_nonce,
            &mut cache,
            true,
            &account_attributes,
            true,
        );

        let (borrow_positions, _) =
            self.sync_borrow_positions_interest(account_nonce, &mut cache, true, false);

        let (_, total_collateral, _) = self.calculate_collateral_values(&collaterals, &mut cache);
        let total_borrow = self.calculate_total_borrow_in_egld(&borrow_positions, &mut cache);

        let can_clean_bad_debt =
            self.can_clean_bad_debt_positions(&mut cache, &total_borrow, &total_collateral);

        require!(can_clean_bad_debt, ERROR_CANNOT_CLEAN_BAD_DEBT);

        self.perform_bad_debt_cleanup(account_nonce, &mut cache);
    }
}
