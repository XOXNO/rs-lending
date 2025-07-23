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
pub use common_proxies::*;
pub use common_structs::*;

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
    + router::RouterModule
    + config::ConfigModule
    + common_events::EventsModule
    + storage::Storage
    + oracle::OracleModule
    + validation::ValidationModule
    + utils::LendingUtilsModule
    + views::ViewsModule
    + strategy::SnapModule
    + helpers::MathsModule
    + common_math::SharedMathModule
    + common_rates::InterestRates
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
    /// - `swap_router_address`: Address for Swap Router integration.
    #[init]
    fn init(
        &self,
        lp_template_address: &ManagedAddress,
        price_aggregator_address: &ManagedAddress,
        safe_price_view_address: &ManagedAddress,
        accumulator_address: &ManagedAddress,
        wegld_address: &ManagedAddress,
        swap_router_address: &ManagedAddress,
    ) {
        self.liq_pool_template_address().set(lp_template_address);
        self.price_aggregator_address()
            .set(price_aggregator_address);
        self.safe_price_view().set(safe_price_view_address);
        self.accumulator_address().set(accumulator_address);
        self.wegld_wrapper().set(wegld_address);
        self.swap_router().set(swap_router_address);

        // Initialize default position limits for gas optimization during liquidations
        self.position_limits().set(PositionLimits {
            max_borrow_positions: 10,
            max_supply_positions: 10,
        });
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
    /// - Accepts payments: optional account NFT (first payment if present) and one or more collateral tokens.
    /// - Requires at least one collateral token payment after NFT extraction.
    #[payable]
    #[allow_multiple_var_args]
    #[endpoint(supply)]
    fn supply(&self, opt_account_nonce: OptionalValue<u64>, e_mode_category: OptionalValue<u8>) {
        let mut cache = Cache::new(self);
        self.reentrancy_guard(cache.flash_loan_ongoing);
        // Validate and extract payment details
        let (collaterals, opt_account, caller, opt_attributes) =
            self.validate_supply_payment(false, true, opt_account_nonce);

        require!(
            !collaterals.is_empty(),
            ERROR_INVALID_NUMBER_OF_ESDT_TRANSFERS
        );

        // At this point we know we have at least one collateral
        let first_collateral = collaterals.get(0);
        self.validate_payment(&first_collateral);

        let first_asset_info = cache.get_cached_asset_info(&first_collateral.token_identifier);

        // If the asset is isolated, we can only supply one collateral not a bulk
        if first_asset_info.is_isolated() {
            require!(collaterals.len() == 1, ERROR_BULK_SUPPLY_NOT_SUPPORTED);
        }

        // Get or create account position
        let opt_isolated_token = if first_asset_info.is_isolated() {
            Some(first_collateral.token_identifier.clone())
        } else {
            None
        };
        let (account_nonce, account_attributes) = self.get_or_create_account(
            &caller,
            first_asset_info.is_isolated(),
            PositionMode::Normal,
            e_mode_category,
            opt_account,
            opt_attributes,
            opt_isolated_token,
        );

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
        self.reentrancy_guard(cache.flash_loan_ongoing);
        cache.allow_unsafe_price = false;

        // Process each withdrawal
        for collateral in collaterals {
            self.validate_payment(&collateral);
            let mut deposit_position = self
                .get_deposit_position(account_payment.token_nonce, &collateral.token_identifier);
            let feed = self.get_token_price(&deposit_position.asset_id, &mut cache);
            let amount =
                deposit_position.make_amount_decimal(&collateral.amount, feed.asset_decimals);

            let _ = self.process_withdrawal(
                account_payment.token_nonce,
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
        self.reentrancy_guard(cache.flash_loan_ongoing);
        cache.allow_unsafe_price = false;

        let (account_payment, caller, account_attributes) = self.validate_account(true);
        let (_, account_nonce, _) = account_payment.into_tuple();

        // Sync positions with interest
        let collaterals = self
            .positions(account_nonce, AccountPositionType::Deposit)
            .values()
            .collect();

        let (_, _, ltv_collateral) = self.calculate_collateral_values(&collaterals, &mut cache);

        let is_bulk_borrow = borrowed_tokens.len() > 1;
        let (mut borrows, mut borrow_index_mapper) =
            self.get_borrow_positions(account_nonce, is_bulk_borrow);

        let e_mode = self.get_e_mode_category(account_attributes.get_emode_id());
        self.ensure_e_mode_not_deprecated(&e_mode);

        // Validate position limits for all new borrow positions in this transaction
        let borrowed_tokens_vec = borrowed_tokens.to_vec();
        self.validate_bulk_position_limits(
            account_nonce,
            AccountPositionType::Borrow,
            &borrowed_tokens_vec,
        );

        // Process each borrow
        for borrowed_token in borrowed_tokens_vec {
            self.process_borrow(
                &mut cache,
                account_nonce,
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
        self.reentrancy_guard(cache.flash_loan_ongoing);
        let payments = self.call_value().all_transfers();
        self.require_active_account(account_nonce);

        let account_attributes = self.account_attributes(account_nonce).get();
        let caller = self.blockchain().get_caller();
        for payment_raw in payments.iter() {
            self.validate_payment(&payment_raw);

            let feed = self.get_token_price(&payment_raw.token_identifier, &mut cache);
            let amount = self.to_decimal(payment_raw.amount.clone(), feed.asset_decimals);
            let egld_value = self.get_token_egld_value(&amount, &feed.price);

            self.process_repayment(
                account_nonce,
                &payment_raw.token_identifier,
                &amount,
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
    /// - `amount`: Amount to borrow in raw token units (will be converted to decimal precision).
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
        mut arguments: ManagedArgBuffer<Self::Api>,
    ) {
        let mut cache = Cache::new(self);
        let caller = self.blockchain().get_caller();
        self.reentrancy_guard(cache.flash_loan_ongoing);
        let asset_config = cache.get_cached_asset_info(borrowed_asset_id);
        require!(asset_config.can_flashloan(), ERROR_FLASHLOAN_NOT_ENABLED);

        let pool_address = cache.get_cached_pool_address(borrowed_asset_id);
        self.validate_flash_loan_shard(contract_address);
        self.require_amount_greater_than_zero(&amount);
        self.validate_flash_loan_endpoint(&endpoint);

        let feed = self.get_token_price(borrowed_asset_id, &mut cache);
        self.flash_loan_ongoing().set(true);
        arguments.push_arg(caller);
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

        self.flash_loan_ongoing().set(false);
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
        self.reentrancy_guard(cache.flash_loan_ongoing);
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
        self.reentrancy_guard(cache.flash_loan_ongoing);
        for asset_id in assets {
            self.update_asset_index(&asset_id, &mut cache, false);
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
        self.reentrancy_guard(cache.flash_loan_ongoing);
        self.require_active_account(account_nonce);

        let collaterals = self
            .positions(account_nonce, AccountPositionType::Deposit)
            .values()
            .collect();

        let (borrow_positions, _) = self.get_borrow_positions(account_nonce, false);

        let (_, total_collateral, _) = self.calculate_collateral_values(&collaterals, &mut cache);
        let total_borrow = self.calculate_total_borrow_in_egld(&borrow_positions, &mut cache);

        let can_clean_bad_debt =
            self.can_clean_bad_debt_positions(&mut cache, &total_borrow, &total_collateral);

        require!(can_clean_bad_debt, ERROR_CANNOT_CLEAN_BAD_DEBT);

        self.perform_bad_debt_cleanup(account_nonce, &mut cache);
    }
}
