#![no_std]

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

pub mod config;
pub mod contexts;
pub mod factory;
pub mod helpers;
pub mod multiply;
pub mod oracle;
pub mod positions;
pub mod proxies;
pub mod router;
pub mod storage;
pub mod utils;
pub mod validation;
pub mod views;

use crate::contexts::base::StorageCache;
pub use common_structs::*;
pub use common_errors::*;
pub use proxies::*;

#[multiversx_sc::contract]
pub trait LendingPool:
    factory::FactoryModule
    + positions::account::PositionAccountModule
    + positions::deposit::PositionDepositModule
    + positions::withdraw::PositionWithdrawModule
    + positions::borrow::PositionBorrowModule
    + positions::repay::PositionRepayModule
    + positions::liquidation::PositionLiquidationModule
    + positions::update::PositionUpdateModule
    + positions::emode::EModeModule
    + router::RouterModule
    + config::ConfigModule
    + common_events::EventsModule
    + storage::LendingStorageModule
    + oracle::OracleModule
    + validation::ValidationModule
    + utils::LendingUtilsModule
    + views::ViewsModule
    + multiply::MultiplyModule
    + helpers::math::MathsModule
    + common_math::SharedMathModule
    + helpers::strategies::StrategiesModule
    + multiversx_sc_modules::default_issue_callbacks::DefaultIssueCallbacksModule
{
    /// Initializes the lending pool contract
    ///
    /// # Arguments
    /// * `lp_template_address` - Address of the liquidity pool template contract
    /// * `aggregator` - Address of the price aggregator contract
    /// * `safe_view_address` - Address of the safe price view contract
    #[init]
    fn init(
        &self,
        lp_template_address: &ManagedAddress,
        aggregator: &ManagedAddress,
        safe_view_address: &ManagedAddress,
        accumulator_address: &ManagedAddress,
        wegld_address: &ManagedAddress,
        ash_swap: &ManagedAddress,
    ) {
        self.liq_pool_template_address().set(lp_template_address);
        self.price_aggregator_address().set(aggregator);
        self.safe_price_view().set(safe_view_address);
        self.accumulator_address().set(accumulator_address);
        self.wegld_wrapper().set(wegld_address);
        self.aggregator().set(ash_swap);
    }

    #[upgrade]
    fn upgrade(&self) {}

    /// Supplies collaterals to the lending pool
    ///
    /// # Arguments
    /// * `is_vault` - Whether this supply is for a vault position
    /// * `e_mode_category` - Optional e-mode category to use
    ///
    /// # Payment
    /// Accepts 1-2 ESDT payments:
    /// - Optional account NFT (if user has an existing position)
    /// - Collateral token to supply
    ///
    /// ```
    #[payable("*")]
    #[endpoint(supply)]
    fn supply(&self, is_vault: bool, e_mode_category: OptionalValue<u8>) {
        let mut storage_cache = StorageCache::new(self);
        let payments = self.call_value().all_transfers();
        let caller = self.blockchain().get_caller();

        // 1. Validate payments and extract tokens
        let (collaterals, account_token) = self.validate_supply_payment(&caller, &payments);

        let first_collateral = collaterals.get(0);

        self.validate_payment(&first_collateral);
        let first_asset_info =
            storage_cache.get_cached_asset_info(&first_collateral.token_identifier);

        // 3. Get or create position and NFT attributes
        let (account_nonce, account_attributes) = self.get_or_create_position(
            &caller,
            first_asset_info.is_isolated,
            is_vault,
            e_mode_category.clone(),
            account_token.clone(),
        );

        self.validate_vault_consistency(&account_attributes, is_vault);

        self.process_deposit(
            &caller,
            account_nonce,
            account_attributes,
            &collaterals,
            &mut storage_cache,
        );
    }

    /// Withdraws collateral from the lending pool
    ///
    /// # Arguments
    /// * `withdraw_token_id` - Token identifier to withdraw
    /// * `amount` - Amount to withdraw
    ///
    /// # Payment
    /// Requires account NFT payment
    ///
    /// # Flow
    /// 1. Validates payment and parameters
    /// 2. Processes withdrawal
    /// 3. Validates health factor after withdrawal
    /// 4. Handles NFT (burns or returns)
    ///
    /// ```
    #[payable("*")]
    #[endpoint(withdraw)]
    fn withdraw(&self, collaterals: MultiValueEncoded<EgldOrEsdtTokenPayment<Self::Api>>) {
        let account = self.call_value().single_esdt();
        let caller = self.blockchain().get_caller();
        // 1. Validate global pool_params
        self.account_token()
            .require_same_token(&account.token_identifier);
        self.require_non_zero_address(&caller);

        let attributes = self.nft_attributes(account.token_nonce, &account.token_identifier);
        self.require_active_account(account.token_nonce);

        let mut storage_cache = StorageCache::new(self);
        storage_cache.allow_unsafe_price = false;

        for collateral in collaterals {
            // 2. Validate payment and parameters
            self.validate_payment(&collateral);

            // 3. Process withdrawal
            self.internal_withdraw(
                account.token_nonce,
                collateral,
                &caller,
                false, // not liquidation
                None,  // No fees
                &mut storage_cache,
                &attributes,
                false,
            );
        }

        // 4. Validate health factor after withdrawal
        self.validate_withdraw_health_factor(account.token_nonce, false, &mut storage_cache, None);

        // 5. Handle NFT (burn or return)
        self.handle_nft_after_withdraw(
            &account.amount,
            account.token_nonce,
            &account.token_identifier,
            &caller,
        );
    }

    /// Borrows an asset from the lending pool
    ///
    /// # Arguments
    /// * `asset_to_borrow` - Token identifier to borrow
    /// * `amount` - Amount to borrow
    ///
    /// # Payment
    /// Requires account NFT payment
    ///
    /// ```
    #[payable("*")]
    #[endpoint(borrow)]
    fn borrow(&self, borrowed_tokens: MultiValueEncoded<EgldOrEsdtTokenPayment<Self::Api>>) {
        let account = self.call_value().single_esdt();
        let caller = self.blockchain().get_caller();

        let mut storage_cache = StorageCache::new(self);

        storage_cache.allow_unsafe_price = false;

        // 1. Update positions with latest interest
        let collaterals = self.update_interest(account.token_nonce, &mut storage_cache, false);

        let (_, _, ltv_collateral) = self.sum_collaterals(&collaterals, &mut storage_cache);

        let is_bulk_borrow = borrowed_tokens.len() > 1;

        let (mut borrows, mut borrow_index_mapper) = self.update_debt(
            account.token_nonce,
            &mut storage_cache,
            false,
            is_bulk_borrow,
        );

        // 2. Get NFT attributes
        let attributes = self.nft_attributes(account.token_nonce, &account.token_identifier);

        self.validate_borrow_account(&account, &caller);

        let e_mode = self.validate_e_mode_exists(attributes.e_mode_category);
        self.validate_not_deprecated_e_mode(&e_mode);

        for borrowed_token in borrowed_tokens {
            // 3. Validate asset supported
            self.require_asset_supported(&borrowed_token.token_identifier);
            self.require_amount_greater_than_zero(&borrowed_token.amount);

            // 4. Get asset configs
            let mut asset_config =
                storage_cache.get_cached_asset_info(&borrowed_token.token_identifier);

            // 5. Validate borrowing constraints
            self.validate_borrow_asset(
                &asset_config,
                &borrowed_token.token_identifier,
                &attributes,
                &borrows,
                &mut storage_cache,
            );

            let asset_emode_config = self.validate_token_of_emode(
                attributes.e_mode_category,
                &borrowed_token.token_identifier,
            );

            self.validate_e_mode_not_isolated(&asset_config, attributes.e_mode_category);

            // 5. Update asset config if NFT has active e-mode
            self.update_asset_config_with_e_mode(&mut asset_config, &e_mode, asset_emode_config);

            require!(asset_config.can_be_borrowed, ERROR_ASSET_NOT_BORROWABLE);

            // 7. Validate collateral and get amounts in egld
            let (borrow_amount_in_usd, feed, bororw_amount_dec) = self
                .validate_and_get_borrow_amounts(
                    &ltv_collateral,
                    &borrowed_token.token_identifier,
                    &borrowed_token.amount,
                    &borrows,
                    &mut storage_cache,
                );

            // 8. Check borrow cap
            self.validate_borrow_cap(
                &asset_config,
                &bororw_amount_dec,
                &borrowed_token.token_identifier,
            );

            // 9. Process borrow
            let updated_position = self.handle_borrow_position(
                account.token_nonce,
                &borrowed_token.token_identifier,
                bororw_amount_dec.clone(),
                borrow_amount_in_usd,
                &caller,
                &asset_config,
                &attributes,
                &collaterals,
                &feed,
                &mut storage_cache,
            );

            // 10. Emit event
            self.update_position_event(
                &bororw_amount_dec,
                &updated_position,
                OptionalValue::Some(feed.price),
                OptionalValue::Some(&caller),
                OptionalValue::Some(&attributes),
            );

            // In case of bulk borrows we need to update the borrows array position that is used to check the eligibility of LTV vs total borrow.
            if is_bulk_borrow {
                let existing_borrow = borrow_index_mapper.contains(&updated_position.token_id);
                if existing_borrow {
                    let safe_index = borrow_index_mapper.get(&updated_position.token_id);
                    let index = safe_index - 1;
                    let token_id = &borrows.get(index).token_id.clone();
                    require!(
                        token_id == &updated_position.token_id,
                        ERROR_INVALID_BULK_BORROW_TICKER
                    );
                    let _ = borrows.set(index, updated_position);
                } else {
                    let safe_index = &borrows.len() + 1;
                    borrow_index_mapper.put(&updated_position.token_id, &safe_index);
                    borrows.push(updated_position);
                }
            };
        }

        // 12. Return NFT to owner
        self.tx().to(&caller).esdt(account.clone()).transfer();
    }

    /// Repays borrowed assets
    ///
    /// # Arguments
    /// * `account_nonce` - NFT nonce of the account position to repay
    /// ```
    #[payable("*")]
    #[endpoint(repay)]
    fn repay(&self, account_nonce: u64) {
        let payments = self.call_value().all_transfers();
        let caller = self.blockchain().get_caller();

        // 1. Update positions with latest interest
        let mut storage_cache = StorageCache::new(self);

        // 2. Validate payment and parameters
        self.require_active_account(account_nonce);
        let attributes = self.account_attributes(account_nonce).get();
        for payment in payments.iter() {
            self.validate_payment(&payment);
            let feed = self.get_token_price(&payment.token_identifier, &mut storage_cache);
            let payment_dec =
                ManagedDecimal::from_raw_units(payment.amount.clone(), feed.decimals as usize);
            // 3. Process repay
            self.internal_repay(
                account_nonce,
                &payment.token_identifier,
                &payment_dec,
                &caller,
                self.get_token_amount_in_egld_raw(&payment_dec, &feed.price),
                &feed,
                &mut storage_cache,
                &attributes,
            );
        }
    }

    /// Liquidates an unhealthy position
    ///
    /// # Arguments
    /// * `liquidatee_account_nonce` - NFT nonce of the account to liquidate
    /// * `collateral_to_receive` - Token identifier of collateral to receive
    ///
    /// # Payment
    /// Accepts EGLD or single ESDT payment of the debt token
    ///
    /// # Flow
    /// 1. Validates payment and parameters
    /// 2. Processes liquidation:
    ///    - Calculates liquidation amounts
    ///    - Updates positions
    ///    - Transfers tokens
    ///    - Emits events
    ///
    /// ```
    #[payable("*")]
    #[endpoint(liquidate)]
    fn liquidate(&self, liquidatee_account_nonce: u64) {
        let payments = self.call_value().all_transfers();
        let caller = self.blockchain().get_caller();

        // 1. Basic validations
        self.require_active_account(liquidatee_account_nonce);
        self.validate_liquidation_payments(&payments, &caller);

        // 2. Process liquidation
        self.process_liquidation(liquidatee_account_nonce, &payments, &caller);
    }

    /// Executes a flash loan
    ///
    /// # Arguments
    /// * `borrowed_token` - Token identifier to borrow
    /// * `amount` - Amount to borrow
    /// * `contract_address` - Address of contract to receive funds
    /// * `endpoint` - Endpoint to call on receiving contract
    /// * `arguments` - Arguments to pass to endpoint
    ///
    ///
    #[endpoint(flashLoan)]
    fn flash_loan(
        &self,
        borrowed_token: &EgldOrEsdtTokenIdentifier,
        amount: BigUint,
        contract_address: &ManagedAddress,
        endpoint: ManagedBuffer<Self::Api>,
        arguments: ManagedArgBuffer<Self::Api>,
    ) {
        let asset_info = self.asset_config(&borrowed_token).get();
        require!(asset_info.flashloan_enabled, ERROR_FLASHLOAN_NOT_ENABLED);

        let pool_address = self.get_pool_address(borrowed_token);

        let destination_shard_id = self.blockchain().get_shard_of_address(contract_address);
        let current_shard_id = self
            .blockchain()
            .get_shard_of_address(&self.blockchain().get_sc_address());

        require!(
            destination_shard_id == current_shard_id,
            ERROR_INVALID_SHARD
        );

        self.require_amount_greater_than_zero(&amount);

        require!(
            !self.blockchain().is_builtin_function(&endpoint) && !endpoint.is_empty(),
            ERROR_INVALID_ENDPOINT
        );

        let mut storage_cache = StorageCache::new(self);
        let feed = self.get_token_price(borrowed_token, &mut storage_cache);

        self.tx()
            .to(pool_address)
            .typed(proxy_pool::LiquidityPoolProxy)
            .flash_loan(
                borrowed_token,
                ManagedDecimal::from_raw_units(amount, feed.decimals as usize),
                contract_address,
                endpoint,
                arguments,
                asset_info.flash_loan_fee.clone(),
                feed.price.clone(),
            )
            .returns(ReturnsResult)
            .sync_call();
    }

    /// Synchronizes account positions with latest interest rates
    ///
    /// # Arguments
    /// * `account_nonce` - NFT nonce of the account to sync
    ///
    /// # Returns
    /// * `MultiValue2<ManagedVec<AccountPosition>, ManagedVec<AccountPosition>>`
    ///   - Updated deposit positions
    ///   - Updated borrow positions
    ///
    /// # Flow
    /// 1. Validates account exists
    /// 2. Updates borrow positions with accumulated interest
    /// 3. Updates deposit positions with accumulated interest
    ///
    /// # Example
    /// ```
    /// let (deposits, borrows) = updateAccountPositions(1);
    /// // deposits and borrows contain updated position data
    /// ```
    #[endpoint(updateAccountPositions)]
    fn update_account_positions(
        &self,
        account_nonce: u64,
    ) -> MultiValue2<ManagedVec<AccountPosition<Self::Api>>, ManagedVec<AccountPosition<Self::Api>>>
    {
        self.require_active_account(account_nonce);

        let mut storage_cache = StorageCache::new(self);
        let deposits = self.update_interest(account_nonce, &mut storage_cache, true);
        let (borrows, _) = self.update_debt(account_nonce, &mut storage_cache, true, false);

        (deposits, borrows).into()
    }

    /// Disables vault for a given account that has vault enabled
    /// # Arguments
    /// * `account_nonce` - NFT nonce of the account to disable vault for
    ///
    /// # Flow
    /// 1. Validates account exists
    /// 2. Validates account token
    /// 3. Iterates over borrow positions
    /// 4. Disables vault for each borrow position
    /// 5. Iterates over deposit positions
    /// 6. Disables vault for each deposit position and move funds to the market shared pool
    /// 7. Emits event for each position
    ///
    /// ```
    #[payable("*")]
    #[endpoint(disableVault)]
    fn disable_vault(&self) {
        let account = self.call_value().single_esdt();
        self.require_active_account(account.token_nonce);
        self.account_token()
            .require_same_token(&account.token_identifier);
        let mut nft_attributes =
            self.nft_attributes(account.token_nonce, &account.token_identifier);
        require!(nft_attributes.is_vault, ERROR_VAULT_ALREADY_DISABLED);

        let mut storage_cache = StorageCache::new(self);
        let deposit_positions = self.deposit_positions(account.token_nonce);
        let borrow_positions = self.borrow_positions(account.token_nonce);

        for mut bp in borrow_positions.values() {
            bp.is_vault = false;

            self.update_position_event(
                &bp.zero_decimal(),
                &bp,
                OptionalValue::None,
                OptionalValue::None,
                OptionalValue::None,
            );
            // Update storage with the latest position
            self.borrow_positions(account.token_nonce)
                .insert(bp.token_id.clone(), bp);
        }

        for mut dp in deposit_positions.values() {
            let pool_address = storage_cache.get_cached_pool_address(&dp.token_id);

            let feed = self.get_token_price(&dp.token_id, &mut storage_cache);
            let old_amount = dp.amount.clone();
            let last_value = self.vault_supplied_amount(&dp.token_id).update(|am| {
                *am -= &old_amount;
                am.clone()
            });

            self.update_vault_supplied_amount_event(&dp.token_id, last_value);
            // Reset the amount to 0 to avoid double increase from the market shared pool
            // It also avoid giving the user interest of the funds that are being moved to the market shared pool
            dp.amount = dp.zero_decimal();

            dp = self
                .tx()
                .to(pool_address)
                .typed(proxy_pool::LiquidityPoolProxy)
                .supply(dp.clone(), feed.price.clone())
                .egld_or_single_esdt(&dp.token_id, 0, old_amount.into_raw_units())
                .returns(ReturnsResult)
                .sync_call();

            dp.is_vault = false;

            self.update_position_event(
                &dp.zero_decimal(),
                &dp,
                OptionalValue::None,
                OptionalValue::None,
                OptionalValue::None,
            );
            // Update storage with the latest position
            self.deposit_positions(account.token_nonce)
                .insert(dp.token_id.clone(), dp);
        }

        nft_attributes.is_vault = false;

        self.account_token()
            .nft_update_attributes(account.token_nonce, &nft_attributes);

        self.account_attributes(account.token_nonce)
            .set(&nft_attributes);

        self.tx()
            .to(self.blockchain().get_caller())
            .esdt(account.clone())
            .transfer();
    }

    /// Enables vault for a given account that has vault disabled
    /// # Arguments
    /// * `account_nonce` - NFT nonce of the account to enable vault for
    ///
    /// # Flow
    /// 1. Validates account exists
    /// 2. Validates account token
    /// 3. Iterates over borrow positions
    /// 4. Enables vault for each borrow position
    /// 5. Iterates over deposit positions
    /// 6. Enables vault for each deposit position and move funds from shared pool to the controller vault
    /// 7. Emits event for each position
    ///
    #[payable("*")]
    #[endpoint(enableVault)]
    fn enable_vault(&self) {
        let account = self.call_value().single_esdt();
        self.require_active_account(account.token_nonce);
        self.account_token()
            .require_same_token(&account.token_identifier);

        let mut nft_attributes =
            self.nft_attributes(account.token_nonce, &account.token_identifier);
        require!(!nft_attributes.is_vault, ERROR_VAULT_ALREADY_ENABLED);

        let mut storage_cache = StorageCache::new(self);
        let deposit_positions = self.deposit_positions(account.token_nonce);
        let borrow_positions = self.borrow_positions(account.token_nonce);
        let controller_sc = self.blockchain().get_sc_address();

        for mut bp in borrow_positions.values() {
            bp.is_vault = true;
            self.update_position_event(
                &ManagedDecimal::from_raw_units(BigUint::zero(), 0usize),
                &bp,
                OptionalValue::None,
                OptionalValue::None,
                OptionalValue::None,
            );
            // Update storage with the latest position
            self.borrow_positions(account.token_nonce)
                .insert(bp.token_id.clone(), bp.clone());
        }

        for mut dp in deposit_positions.values() {
            let asset_address = self.get_pool_address(&dp.token_id);

            let feed = self.get_token_price(&dp.token_id, &mut storage_cache);
            self.update_position(
                &asset_address,
                &mut dp,
                OptionalValue::Some(feed.price.clone()),
            );

            let total_amount_with_interest = dp.get_total_amount();

            dp = self
                .tx()
                .to(&asset_address)
                .typed(proxy_pool::LiquidityPoolProxy)
                .withdraw(
                    &controller_sc,
                    total_amount_with_interest.clone(),
                    dp,
                    false,
                    None,
                    feed.price.clone(),
                )
                .returns(ReturnsResult)
                .sync_call();

            require!(dp.can_remove(), ERROR_ENABLE_VAULT_MODE_FAILED);

            dp.is_vault = true;
            dp.amount = total_amount_with_interest.clone();

            let last_value = self.vault_supplied_amount(&dp.token_id).update(|am| {
                *am += &total_amount_with_interest;
                am.clone()
            });

            self.update_vault_supplied_amount_event(&dp.token_id, last_value);
            self.deposit_positions(account.token_nonce)
                .insert(dp.token_id.clone(), dp.clone());

            self.update_position_event(
                &dp.zero_decimal(),
                &dp,
                OptionalValue::None,
                OptionalValue::None,
                OptionalValue::None,
            );
        }

        nft_attributes.is_vault = true;

        self.account_token()
            .nft_update_attributes(account.token_nonce, &nft_attributes);

        self.account_attributes(account.token_nonce)
            .set(&nft_attributes);

        self.tx()
            .to(self.blockchain().get_caller())
            .esdt(account.clone())
            .transfer();
    }

    /// Updates the LTV for a given asset and account positions
    ///
    /// # Arguments
    /// * `token_id` - Token identifier to update LTV for
    /// * `account_nonces` - Nonces of the accounts to update LTV for
    ///
    /// # Flow
    /// 1. Validates asset is supported
    /// 2. Iterates over account positions
    /// 3. Updates LTV if necessary
    /// 4. Emits event if LTV is updated
    #[endpoint(updatePositionThreshold)]
    fn update_position_threshold(
        &self,
        token_id: EgldOrEsdtTokenIdentifier,
        is_ltv: bool,
        account_nonces: MultiValueEncoded<u64>,
    ) {
        self.require_asset_supported(&token_id);

        let asset_config = self.asset_config(&token_id).get();
        let mut storage_cache = StorageCache::new(self);
        for account_nonce in account_nonces {
            self.require_active_account(account_nonce);
            let mut deposit_positions = self.deposit_positions(account_nonce);
            let dp_option = deposit_positions.get(&token_id);

            require!(dp_option.is_some(), ERROR_POSITION_NOT_FOUND);

            let nft_attributes = self.account_attributes(account_nonce).get();

            let threshold = if nft_attributes.e_mode_category > 0 {
                let e_mode_category = self.e_mode_category().get(&nft_attributes.e_mode_category);

                if is_ltv {
                    e_mode_category.unwrap().ltv.clone()
                } else {
                    e_mode_category.unwrap().liquidation_threshold.clone()
                }
            } else {
                if is_ltv {
                    asset_config.ltv.clone()
                } else {
                    asset_config.liquidation_threshold.clone()
                }
            };

            let mut dp = dp_option.unwrap();

            let current_threshold = if is_ltv {
                &dp.entry_ltv
            } else {
                &dp.entry_liquidation_threshold
            };

            if current_threshold != &threshold {
                if is_ltv {
                    dp.entry_ltv = threshold;
                } else {
                    dp.entry_liquidation_threshold = threshold;
                }

                deposit_positions.insert(dp.token_id.clone(), dp.clone());

                if !is_ltv {
                    self.validate_withdraw_health_factor(
                        account_nonce,
                        false,
                        &mut storage_cache,
                        Some(ManagedDecimal::from_raw_units(BigUint::from(20u64), 0usize)),
                    );
                }

                self.update_position_event(
                    &ManagedDecimal::from_raw_units(BigUint::zero(), 0usize),
                    &dp,
                    OptionalValue::None,
                    OptionalValue::None,
                    OptionalValue::None,
                );
            }
        }
    }

    /// Updates interest rate indexes for a given asset
    ///
    /// # Arguments
    /// * `token_id` - Token identifier to update indexes for
    ///
    /// # Flow
    /// 1. Gets pool address for token
    /// 2. Gets current asset price
    /// 3. Calls pool to update indexes with current price
    #[endpoint(updateIndexes)]
    fn update_indexes(&self, assets: MultiValueEncoded<EgldOrEsdtTokenIdentifier>) {
        let mut storage_cache = StorageCache::new(self);
        for asset in assets {
            let pool_address = self.get_pool_address(&asset);
            let asset_price = self.get_token_price(&asset, &mut storage_cache);
            self.tx()
                .to(pool_address)
                .typed(proxy_pool::LiquidityPoolProxy)
                .update_indexes(asset_price.price.clone())
                .sync_call();
        }
    }
}
