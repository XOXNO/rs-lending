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
    + multiversx_sc_modules::pause::PauseModule
    + multiversx_sc_modules::default_issue_callbacks::DefaultIssueCallbacksModule
{
    /// Initializes the lending pool contract with required addresses.
    ///
    /// # Arguments
    /// - `lp_template_address`: Address of the liquidity pool template.
    /// - `price_aggregator_address`: Address of the price aggregator.
    /// - `safe_price_view_address`: Address for safe price views.
    /// - `accumulator_address`: Address for revenue accumulation.
    /// - `swap_router_address`: Address for Swap Router integration.
    #[init]
    fn init(
        &self,
        lp_template_address: &ManagedAddress,
        price_aggregator_address: &ManagedAddress,
        safe_price_view_address: &ManagedAddress,
        accumulator_address: &ManagedAddress,
        swap_router_address: &ManagedAddress,
    ) {
        self.liq_pool_template_address().set(lp_template_address);
        self.price_aggregator_address()
            .set(price_aggregator_address);
        self.safe_price_view().set(safe_price_view_address);
        self.accumulator_address().set(accumulator_address);
        self.swap_router().set(swap_router_address);

        // Initialize default position limits for gas optimization during liquidations
        self.position_limits().set(PositionLimits {
            max_borrow_positions: 10,
            max_supply_positions: 10,
        });

        self.unpause_endpoint();
    }

    /// Handles contract upgrade by pausing all operations.
    /// Ensures safe state during code updates to prevent inconsistencies.
    /// Contract remains paused until manually unpaused after upgrade.
    #[upgrade]
    fn upgrade(&self) {
        self.pause_endpoint();
    }

    /// Supplies collateral to the lending pool.
    ///
    /// # Arguments
    /// - `optional_account_nonce`: Optional existing account NFT nonce (use `Some(0)` to auto-create).
    /// - `e_mode_category`: Optional e-mode category for specialized parameters.
    ///
    /// # Payment
    /// - Accepts payments: optional account NFT (if present, it must be the first payment) and one or more collateral tokens.
    /// - Requires at least one collateral token payment after extracting the optional NFT.
    #[payable]
    #[allow_multiple_var_args]
    #[endpoint(supply)]
    fn supply(
        &self,
        optional_account_nonce: OptionalValue<u64>,
        e_mode_category: OptionalValue<u8>,
    ) {
        self.require_not_paused();
        let mut cache = Cache::new(self);
        self.reentrancy_guard(cache.flash_loan_ongoing);
        // Validate and extract payment details
        let (collaterals, optional_account, caller, optional_attributes) =
            self.validate_supply_payment(false, true, optional_account_nonce);

        require!(
            !collaterals.is_empty(),
            ERROR_INVALID_NUMBER_OF_ESDT_TRANSFERS
        );

        // At this point we know we have at least one collateral
        let first_collateral = collaterals.get(0);
        self.validate_payment(&first_collateral);

        let first_asset_info = cache.cached_asset_info(&first_collateral.token_identifier);

        // If the asset is isolated, we can only supply one collateral not a bulk
        if first_asset_info.is_isolated() {
            require!(collaterals.len() == 1, ERROR_BULK_SUPPLY_NOT_SUPPORTED);
        }

        // Get or create account position
        let optional_isolated_token = if first_asset_info.is_isolated() {
            Some(first_collateral.token_identifier.clone())
        } else {
            None
        };
        let (account_nonce, account_attributes) = self.get_or_create_account(
            &caller,
            first_asset_info.is_isolated(),
            PositionMode::Normal,
            e_mode_category,
            optional_account,
            optional_attributes,
            optional_isolated_token,
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
    /// Purpose: Transfers requested collateral amounts from the user's deposit
    /// positions back to the caller while keeping the account healthy.
    ///
    /// Methodology:
    /// 1. Validates account NFT and returns it after checks
    /// 2. For each asset: validates payment, syncs price/index, computes amount
    /// 3. Executes pool withdrawal and updates/removes deposit position
    /// 4. Validates health factor remains above minimum after all withdrawals
    ///
    /// Payment
    /// - Requires the account NFT as payment (first and only NFT).
    ///
    /// Arguments
    /// - `collaterals`: List of token identifiers and amounts to withdraw
    #[payable]
    #[endpoint(withdraw)]
    fn withdraw(&self, collaterals: MultiValueEncoded<EgldOrEsdtTokenPayment<Self::Api>>) {
        self.require_not_paused();
        let (account_payment, caller, account_attributes) = self.validate_account(false);

        let mut cache = Cache::new(self);
        self.reentrancy_guard(cache.flash_loan_ongoing);
        let borrow_positions =
            self.positions(account_payment.token_nonce, AccountPositionType::Borrow);

        cache.allow_unsafe_price = borrow_positions.is_empty();

        // Process each withdrawal
        for collateral in collaterals {
            self.validate_payment(&collateral);
            let mut deposit_position =
                self.deposit_position(account_payment.token_nonce, &collateral.token_identifier);
            let feed = self.token_price(&deposit_position.asset_id, &mut cache);
            let amount_wad =
                deposit_position.make_amount_decimal(&collateral.amount, feed.asset_decimals);

            let _ = self.process_withdrawal(
                account_payment.token_nonce,
                amount_wad,
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
    /// Purpose: Creates or scales borrow positions for the account, with
    /// validations on LTV, e-mode, borrowability and position limits.
    ///
    /// Methodology:
    /// 1. Validates account NFT and syncs indexes/prices
    /// 2. Computes LTV collateral value from current deposits
    /// 3. Validates bulk position limits for all requested borrows
    /// 4. For each token: validates borrowability, caps, LTV, updates position
    ///
    /// Payment
    /// - Requires the account NFT as payment.
    ///
    /// Arguments
    /// - `borrowed_tokens`: List of tokens and amounts to borrow
    #[payable]
    #[endpoint(borrow)]
    fn borrow(&self, borrowed_tokens: MultiValueEncoded<EgldOrEsdtTokenPayment<Self::Api>>) {
        self.require_not_paused();
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
            self.borrow_positions(account_nonce, is_bulk_borrow);

        let e_mode = self.e_mode_category(account_attributes.emode_id());
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
    /// Purpose: Decreases or clears debt positions for one or more assets.
    ///
    /// Methodology:
    /// 1. Validates account and caller
    /// 2. For each payment: validates asset/amount, converts to decimals and EGLD value
    /// 3. Calls process_repayment to update pool and position, tracking isolated debt
    ///
    /// Arguments
    /// - `account_nonce`: NFT nonce of the account
    #[payable]
    #[endpoint(repay)]
    fn repay(&self, account_nonce: u64) {
        self.require_not_paused();
        let mut cache = Cache::new(self);
        self.reentrancy_guard(cache.flash_loan_ongoing);
        let payments = self.call_value().all_transfers();
        self.require_active_account(account_nonce);

        let account_attributes = self.account_attributes(account_nonce).get();
        let caller = self.blockchain().get_caller();
        for payment_raw in payments.iter() {
            self.validate_payment(&payment_raw);

            let feed = self.token_price(&payment_raw.token_identifier, &mut cache);
            let amount_wad = self.to_decimal(payment_raw.amount.clone(), feed.asset_decimals);
            let egld_value_wad = self.token_egld_value(&amount_wad, &feed.price_wad);

            self.process_repayment(
                account_nonce,
                &payment_raw.token_identifier,
                &amount_wad,
                &caller,
                egld_value_wad,
                &feed,
                &mut cache,
                &account_attributes,
            );
        }
    }

    /// Liquidates an unhealthy position.
    ///
    /// Purpose: Repays eligible debt using liquidator payments and seizes
    /// collateral with protocol fee, following the liquidation algorithm.
    ///
    /// Methodology:
    /// 1. Validates payments and account state
    /// 2. Executes liquidation core to compute repayments and seized collateral
    /// 3. Refunds excess payments, processes repayments and transfers collateral
    ///
    /// Arguments
    /// - `account_nonce`: NFT nonce identifying the liquidated account
    #[payable]
    #[endpoint(liquidate)]
    fn liquidate(&self, account_nonce: u64) {
        self.require_not_paused();
        let payments = self.call_value().all_transfers();
        let caller = self.blockchain().get_caller();
        self.process_liquidation(account_nonce, &payments, &caller);
    }

    /// Executes a flash loan.
    ///
    /// Purpose: Borrows funds for a single transaction to a target contract,
    /// which must repay plus fee within the same call.
    ///
    /// Methodology:
    /// 1. Validates shard, endpoint, amount and that asset supports flashloans
    /// 2. Pushes caller as final argument and forwards funds to pool flash_loan
    /// 3. Enforces flash_loan_ongoing guard around the call
    ///
    /// Arguments
    /// - `borrowed_asset_id`: Token to borrow
    /// - `amount_raw`: Borrow amount in raw units
    /// - `contract_address`: Receiver contract of the loan
    /// - `endpoint`: Callback endpoint to invoke on receiver
    /// - `arguments`: Extra arguments passed to receiver endpoint
    #[endpoint(flashLoan)]
    fn flash_loan(
        &self,
        borrowed_asset_id: &EgldOrEsdtTokenIdentifier,
        amount_raw: BigUint,
        contract_address: &ManagedAddress,
        endpoint: ManagedBuffer<Self::Api>,
        mut arguments: ManagedArgBuffer<Self::Api>,
    ) {
        self.require_not_paused();
        let mut cache = Cache::new(self);
        let caller = self.blockchain().get_caller();
        self.reentrancy_guard(cache.flash_loan_ongoing);
        let asset_config = cache.cached_asset_info(borrowed_asset_id);
        require!(asset_config.can_flashloan(), ERROR_FLASHLOAN_NOT_ENABLED);

        let pool_address = cache.cached_pool_address(borrowed_asset_id);
        self.validate_flash_loan_shard(contract_address);
        self.require_amount_greater_than_zero(&amount_raw);
        self.validate_flash_loan_endpoint(&endpoint);

        let feed = self.token_price(borrowed_asset_id, &mut cache);
        self.flash_loan_ongoing().set(true);
        arguments.push_arg(caller);
        self.tx()
            .to(pool_address)
            .typed(proxy_pool::LiquidityPoolProxy)
            .flash_loan(
                borrowed_asset_id,
                self.to_decimal(amount_raw, feed.asset_decimals),
                contract_address,
                endpoint,
                arguments,
                asset_config.flashloan_fee_bps.clone(),
                feed.price_wad.clone(),
            )
            .returns(ReturnsResult)
            .sync_call();

        self.flash_loan_ongoing().set(false);
    }

    /// Updates account thresholds for a specific asset.
    ///
    /// Purpose: Applies updated asset risk parameters (LTV/liquidation)
    /// to each account’s deposit position of the given asset, validating
    /// health if changes are risky.
    ///
    /// Arguments
    /// - `asset_id`: Asset to update within accounts
    /// - `has_risks`: Whether the change affects liquidation threshold (requires HF check)
    /// - `account_nonces`: Accounts to update
    #[endpoint(updateAccountThreshold)]
    fn update_account_threshold(
        &self,
        asset_id: EgldOrEsdtTokenIdentifier,
        has_risks: bool,
        account_nonces: MultiValueEncoded<u64>,
    ) {
        self.require_not_paused();
        self.require_asset_supported(&asset_id);

        let mut cache = Cache::new(self);
        cache.allow_unsafe_price = false;
        self.reentrancy_guard(cache.flash_loan_ongoing);
        let asset_config = cache.cached_asset_info(&asset_id);
        let controller_sc = self.blockchain().get_sc_address();
        let price_feed = self.token_price(&asset_id, &mut cache);

        for account_nonce in account_nonces {
            // Clone base config for each account so e-mode overrides remain account-specific.
            let mut account_asset_config = asset_config.clone();
            self.update_position_threshold(
                account_nonce,
                &asset_id,
                has_risks,
                &mut account_asset_config,
                &controller_sc,
                &price_feed,
                &mut cache,
            );
        }
    }

    /// Updates interest rate indexes for specified assets.
    ///
    /// Purpose: Synchronizes supply/borrow indexes using current prices.
    ///
    /// Arguments
    /// - `assets`: Asset identifiers to update
    #[endpoint(updateIndexes)]
    fn update_indexes(&self, assets: MultiValueEncoded<EgldOrEsdtTokenIdentifier>) {
        self.require_not_paused();
        let mut cache = Cache::new(self);
        self.reentrancy_guard(cache.flash_loan_ongoing);
        for asset_id in assets {
            self.update_asset_index(&asset_id, &mut cache, false);
        }
    }

    /// Cleans bad debt from an account.
    ///
    /// Purpose: Seizes all remaining collateral and marks remaining debt
    /// as bad debt when account qualifies for bad debt cleanup.
    ///
    /// Methodology:
    /// 1. Validates account is eligible (insufficient collateral vs debt)
    /// 2. Performs cleanup and removes positions appropriately
    ///
    /// Arguments
    /// - `account_nonce`: NFT nonce of the account
    #[endpoint(cleanBadDebt)]
    fn clean_bad_debt(&self, account_nonce: u64) {
        self.require_not_paused();
        let mut cache = Cache::new(self);
        cache.allow_unsafe_price = false;
        self.reentrancy_guard(cache.flash_loan_ongoing);
        self.require_active_account(account_nonce);

        let collaterals = self
            .positions(account_nonce, AccountPositionType::Deposit)
            .values()
            .collect();

        let (borrow_positions, _) = self.borrow_positions(account_nonce, false);

        let (_, total_collateral, _) = self.calculate_collateral_values(&collaterals, &mut cache);
        let total_borrow = self.calculate_total_borrow_in_egld(&borrow_positions, &mut cache);

        let can_clean_bad_debt =
            self.can_clean_bad_debt_positions(&mut cache, &total_borrow, &total_collateral);

        require!(can_clean_bad_debt, ERROR_CANNOT_CLEAN_BAD_DEBT);

        self.perform_bad_debt_cleanup(account_nonce, &mut cache);
    }
}
