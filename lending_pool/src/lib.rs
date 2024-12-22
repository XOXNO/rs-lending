#![no_std]

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

pub mod config;
pub mod contexts;
pub mod errors;
pub mod factory;
pub mod math;
pub mod oracle;
pub mod position;
pub mod proxies;
pub mod router;
pub mod storage;
pub mod utils;
pub mod validation;
pub mod views;

pub use common_structs::*;
use contexts::base::StorageCache;
pub use errors::*;
pub use proxies::*;

#[multiversx_sc::contract]
pub trait LendingPool:
    factory::FactoryModule
    + router::RouterModule
    + config::ConfigModule
    + common_events::EventsModule
    + storage::LendingStorageModule
    + oracle::OracleModule
    + position::PositionModule
    + validation::ValidationModule
    + utils::LendingUtilsModule
    + math::LendingMathModule
    + views::ViewsModule
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
        lp_template_address: ManagedAddress,
        aggregator: ManagedAddress,
        safe_view_address: ManagedAddress,
        accumulator_address: ManagedAddress,
    ) {
        self.liq_pool_template_address().set(&lp_template_address);
        self.price_aggregator_address().set(&aggregator);
        self.safe_price_view().set(&safe_view_address);
        self.accumulator_address().set(&accumulator_address);
    }

    #[upgrade]
    fn upgrade(&self) {}

    /// Supplies collateral to the lending pool
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
    /// # Flow
    /// 1. Validates payments and extracts tokens
    /// 2. Gets/creates position and NFT attributes, returns NFT to owner
    /// 3. Validates e-mode constraints
    /// 4. Validates vault consistency
    /// 5. Validates isolated collateral rules
    /// 6. Checks supply caps
    /// 7. Updates position
    /// 8. Emits event
    ///
    /// # Example
    /// ```
    /// Supply 100 EGLD as collateral
    /// supply(false, OptionalValue::None)
    /// ```
    #[payable("*")]
    #[endpoint(supply)]
    fn supply(&self, is_vault: bool, e_mode_category: OptionalValue<u8>) {
        let payments = self.get_multi_payments();
        let initial_caller = self.blockchain().get_caller();

        // 1. Validate payments and extract tokens
        let (collateral_payment, account_token) =
            self.validate_supply_payment(&initial_caller, &payments);

        // 2. Get asset info and validate it can be used as collateral
        let mut asset_info = self
            .asset_config(&collateral_payment.token_identifier)
            .get();

        // 3. Get or create position and NFT attributes
        let (account_nonce, nft_attributes) = self.get_or_create_supply_position(
            &initial_caller,
            asset_info.is_isolated,
            is_vault,
            e_mode_category.clone(),
            account_token.map(|t| t.token_nonce),
        );

        // 4. Validate e-mode constraints first
        let category = self.validate_e_mode_constraints(
            &collateral_payment.token_identifier,
            &asset_info,
            &nft_attributes,
        );

        // 5. Update asset config if NFT has active e-mode
        self.update_asset_config_for_e_mode(
            &mut asset_info,
            nft_attributes.e_mode_category,
            &collateral_payment.token_identifier,
            category,
        );

        require!(
            asset_info.can_be_collateral,
            ERROR_ASSET_NOT_SUPPORTED_AS_COLLATERAL
        );

        self.validate_vault_consistency(&nft_attributes, is_vault);

        // 6. Validate isolated collateral
        self.validate_isolated_collateral(
            account_nonce,
            &collateral_payment.token_identifier,
            &asset_info,
            &nft_attributes,
        );

        // 7. Check supply caps
        self.check_supply_cap(
            &asset_info,
            &collateral_payment.amount,
            &collateral_payment.token_identifier,
            is_vault,
        );

        let mut storage_cache = StorageCache::new(self);
        let asset_data_feed =
            self.get_token_price_data(&collateral_payment.token_identifier, &mut storage_cache);

        // 8. Update position and get updated state
        let updated_position = self.update_supply_position(
            account_nonce,
            &collateral_payment,
            &asset_info,
            is_vault,
            &asset_data_feed,
        );

        // 9. Emit event
        self.update_position_event(
            &collateral_payment.amount,
            &updated_position,
            OptionalValue::Some(asset_data_feed.price),
            OptionalValue::Some(initial_caller),
            OptionalValue::Some(nft_attributes),
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
    /// # Example
    /// ```
    /// // Withdraw 50 EGLD using account NFT
    /// ESDTTransfer {
    ///   token: "LEND-123456", // Account NFT
    ///   nonce: 1,
    ///   amount: 1
    /// }
    /// withdraw("EGLD-123456", 50_000_000_000_000_000_000) // 50 EGLD
    /// ```
    #[payable("*")]
    #[endpoint(withdraw)]
    fn withdraw(&self, withdraw_token_id: &EgldOrEsdtTokenIdentifier, amount: &BigUint) {
        let account_token = self.call_value().single_esdt();
        let initial_caller = self.blockchain().get_caller();

        // 1. Validate payment and parameters
        self.validate_withdraw_payment(
            &account_token.token_identifier,
            withdraw_token_id,
            amount,
            &initial_caller,
        );

        let attributes =
            self.get_account_attributes(account_token.token_nonce, &account_token.token_identifier);

        // 2. Process withdrawal

        let mut storage_cache = StorageCache::new(self);
        storage_cache.allow_unsafe_price = false;
        self.internal_withdraw(
            account_token.token_nonce,
            withdraw_token_id,
            amount.clone(),
            &initial_caller,
            false, // not liquidation
            &BigUint::zero(),
            &mut storage_cache,
            OptionalValue::Some(attributes),
            Option::None,
        );

        // 3. Validate health factor after withdrawal
        self.validate_withdraw_health_factor(
            account_token.token_nonce,
            false,
            &mut storage_cache,
            None,
        );

        // 4. Handle NFT (burn or return)
        self.handle_nft_after_withdraw(account_token, &initial_caller);
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
    /// # Flow
    /// 1. Validates payment and parameters
    /// 2. Gets NFT attributes and asset config
    /// 3. Updates positions with latest interest
    /// 4. Validates borrowing constraints
    /// 5. Checks borrow cap
    /// 6. Validates collateral sufficiency
    /// 7. Processes borrow
    /// 8. Emits event
    /// 9. Returns NFT
    ///
    /// # Example
    /// ```
    /// // Borrow 1000 USDC using account NFT
    /// ESDTTransfer {
    ///   token: "LEND-123456", // Account NFT
    ///   nonce: 1,
    ///   amount: 1
    /// }
    /// borrow("USDC-123456", 1_000_000_000) // 1000 USDC
    /// ```
    #[payable("*")]
    #[endpoint(borrow)]
    fn borrow(&self, asset_to_borrow: EgldOrEsdtTokenIdentifier, amount: BigUint) {
        let nft_token = self.call_value().single_esdt();
        let initial_caller = self.blockchain().get_caller();

        // 1. Validate payment and parameters
        self.validate_borrow_payment(&nft_token, &asset_to_borrow, &amount, &initial_caller);

        // 2. Get NFT attributes and asset config
        let nft_attributes =
            self.get_account_attributes(nft_token.token_nonce, &nft_token.token_identifier);
        let mut asset_config = self.asset_config(&asset_to_borrow).get();

        // 3. Update positions with latest interest

        let mut storage_cache = StorageCache::new(self);

        storage_cache.allow_unsafe_price = false;

        let collaterals =
            self.update_collateral_with_interest(nft_token.token_nonce, &mut storage_cache, false);
        let borrows =
            self.update_borrows_with_debt(nft_token.token_nonce, &mut storage_cache, false);

        // 4. Validate borrowing constraints
        self.validate_borrow_asset(&asset_config, &asset_to_borrow, &nft_attributes, &borrows);

        // 5. Update asset config if in e-mode
        self.update_asset_config_for_e_mode(
            &mut asset_config,
            nft_attributes.e_mode_category,
            &asset_to_borrow,
            None,
        );

        require!(asset_config.can_be_borrowed, ERROR_ASSET_NOT_BORROWABLE);

        // 6. Check borrow cap
        self.check_borrow_cap(&asset_config, &amount, &asset_to_borrow);

        // 7. Validate collateral and get amounts in egld
        let (amount_to_borrow_in_egld, asset_data_feed) = self.validate_and_get_borrow_amounts(
            &asset_to_borrow,
            &amount,
            &collaterals,
            &borrows,
            &mut storage_cache,
        );

        let usd_amount = self.get_token_amount_in_dollars_raw(
            &amount_to_borrow_in_egld,
            &storage_cache.egld_price_feed,
        );
        // 8. Process borrow
        let updated_position = self.handle_borrow_position(
            nft_token.token_nonce,
            &asset_to_borrow,
            &amount,
            &usd_amount,
            &initial_caller,
            &asset_config,
            &nft_attributes,
            &collaterals,
            &asset_data_feed,
        );

        // 9. Emit event
        self.update_position_event(
            &amount,
            &updated_position,
            OptionalValue::Some(asset_data_feed.price),
            OptionalValue::Some(initial_caller.clone()),
            OptionalValue::Some(nft_attributes),
        );

        // 10. Return NFT to owner
        self.tx().to(&initial_caller).esdt(nft_token).transfer();
    }

    /// Repays borrowed assets
    ///
    /// # Arguments
    /// * `account_nonce` - NFT nonce of the account position to repay
    ///
    /// # Payment
    /// Accepts EGLD or single ESDT payment of the debt token
    ///
    /// # Flow
    /// 1. Updates positions with latest interest
    /// 2. Validates payment and parameters
    /// 3. Processes repayment
    ///
    /// # Example
    /// ```
    /// // Repay 500 USDC for account position
    /// ESDTTransfer {
    ///   token: "USDC-123456",
    ///   amount: 500_000_000 // 500 USDC
    /// }
    /// repay(1) // Account NFT nonce
    /// ```
    #[payable("*")]
    #[endpoint(repay)]
    fn repay(&self, account_nonce: u64) {
        let (repay_token_id, repay_amount) = self.call_value().egld_or_single_fungible_esdt();
        let initial_caller = self.blockchain().get_caller();

        // 1. Update positions with latest interest
        let mut storage_cache = StorageCache::new(self);

        // 2. Validate payment and parameters
        self.validate_repay_payment(&repay_token_id, &repay_amount, account_nonce);
        let price_data = self.get_token_price_data(&repay_token_id, &mut storage_cache);

        // 3. Process repay
        self.internal_repay(
            account_nonce,
            &repay_token_id,
            &repay_amount,
            &initial_caller,
            self.get_token_amount_in_egld_raw(&repay_amount, &price_data),
            &price_data,
            &mut storage_cache,
        );
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
    /// # Example
    /// ```
    /// // Liquidate position by repaying 1000 USDC debt
    /// ESDTTransfer {
    ///   token: "USDC-123456",
    ///   amount: 1_000_000_000 // 1000 USDC
    /// }
    /// liquidate(1, "EGLD-123456") // Get EGLD collateral
    /// ```
    #[payable("*")]
    #[endpoint(liquidate)]
    fn liquidate(
        &self,
        liquidatee_account_nonce: u64,
        collateral_to_receive: &EgldOrEsdtTokenIdentifier,
        min_amount_to_receive: OptionalValue<BigUint>,
    ) {
        let payment = self.call_value().egld_or_single_fungible_esdt();
        let initial_caller = self.blockchain().get_caller();

        let debt_payment = EgldOrEsdtTokenPayment::new(payment.0, 0, payment.1);
        // 1. Basic validations
        self.lending_account_in_the_market(liquidatee_account_nonce);
        self.validate_liquidation_payment(&debt_payment, collateral_to_receive, &initial_caller);

        // 2. Process liquidation
        self.process_liquidation(
            liquidatee_account_nonce,
            debt_payment,
            collateral_to_receive,
            min_amount_to_receive,
            &initial_caller,
        );
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
    /// # Flow
    /// 1. Validates flash loan is enabled for asset
    /// 2. Validates contract is on same shard
    /// 3. Executes flash loan through pool:
    ///    - Transfers tokens to contract
    ///    - Calls specified endpoint
    ///    - Verifies repayment with fee
    ///
    /// # Example
    /// ```
    /// flashLoan(
    ///   "USDC-123456", // Token
    ///   1_000_000_000, // 1000 USDC
    ///   "erd1...",     // Contract address
    ///   "execute",     // Endpoint
    ///   []            // No arguments
    /// ```
    ///
    #[payable("*")]
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

        let destination_shard_id = self.blockchain().get_shard_of_address(contract_address);
        let current_shard_id = self
            .blockchain()
            .get_shard_of_address(&self.blockchain().get_sc_address());

        require!(
            destination_shard_id == current_shard_id,
            ERROR_INVALID_SHARD
        );

        self.require_amount_greater_than_zero(amount);

        require!(
            !self.blockchain().is_builtin_function(&endpoint) && !endpoint.is_empty(),
            ERROR_INVALID_ENDPOINT
        );

        let mut storage_cache = StorageCache::new(self);
        let asset_data = self.get_token_price_data(borrowed_token, &mut storage_cache);

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
                &asset_data.price,
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
        self.lending_account_in_the_market(account_nonce);

        let mut storage_cache = StorageCache::new(self);
        let borrow_positions =
            self.update_borrows_with_debt(account_nonce, &mut storage_cache, true);
        let deposit_positions =
            self.update_collateral_with_interest(account_nonce, &mut storage_cache, true);
        (deposit_positions, borrow_positions).into()
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
        let nft_token = self.call_value().single_esdt();
        self.lending_account_in_the_market(nft_token.token_nonce);
        self.account_token()
            .require_same_token(&nft_token.token_identifier);

        let mut storage_cache = StorageCache::new(self);
        let deposit_positions = self.deposit_positions(nft_token.token_nonce);
        let borrow_positions = self.borrow_positions(nft_token.token_nonce);

        for mut bp in borrow_positions.values() {
            if bp.is_vault {
                bp.is_vault = false;
                self.update_position_event(
                    &BigUint::zero(),
                    &bp,
                    OptionalValue::None,
                    OptionalValue::None,
                    OptionalValue::None,
                );
                // Update storage with the latest position
                self.borrow_positions(nft_token.token_nonce)
                    .insert(bp.token_id.clone(), bp);
            }
        }

        for mut dp in deposit_positions.values() {
            if dp.is_vault {
                dp.is_vault = false;
                let pool_address = self.get_pool_address(&dp.token_id);

                let asset_data_feed = self.get_token_price_data(&dp.token_id, &mut storage_cache);
                let old_amount = dp.amount.clone();
                let last_value = self.vault_supplied_amount(&dp.token_id).update(|am| {
                    *am -= &old_amount;
                    am.clone()
                });

                self.update_vault_supplied_amount_event(&dp.token_id, last_value);
                // Reset the amount to 0 to avoid double increase from the market shared pool
                // It also avoid giving the user interest of the funds that are being moved to the market shared pool
                dp.amount = BigUint::zero();
                dp = self
                    .tx()
                    .to(pool_address)
                    .typed(proxy_pool::LiquidityPoolProxy)
                    .supply(dp.clone(), &asset_data_feed.price)
                    .payment(EgldOrEsdtTokenPayment::new(dp.token_id, 0, old_amount))
                    .returns(ReturnsResult)
                    .sync_call();

                // Update storage with the latest position
                self.deposit_positions(nft_token.token_nonce)
                    .insert(dp.token_id.clone(), dp.clone());

                self.update_position_event(
                    &BigUint::zero(),
                    &dp,
                    OptionalValue::None,
                    OptionalValue::None,
                    OptionalValue::None,
                );
            }
        }

        self.tx()
            .to(self.blockchain().get_caller())
            .esdt(nft_token)
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
        let nft_token = self.call_value().single_esdt();
        self.lending_account_in_the_market(nft_token.token_nonce);
        self.account_token()
            .require_same_token(&nft_token.token_identifier);

        let mut storage_cache = StorageCache::new(self);
        let deposit_positions = self.deposit_positions(nft_token.token_nonce);
        let borrow_positions = self.borrow_positions(nft_token.token_nonce);
        let controller_sc = self.blockchain().get_sc_address();

        for mut bp in borrow_positions.values() {
            if !bp.is_vault {
                bp.is_vault = true;
                self.update_position_event(
                    &BigUint::zero(),
                    &bp,
                    OptionalValue::None,
                    OptionalValue::None,
                    OptionalValue::None,
                );
                // Update storage with the latest position
                self.borrow_positions(nft_token.token_nonce)
                    .insert(bp.token_id.clone(), bp.clone());
            }
        }

        for mut dp in deposit_positions.values() {
            let asset_address = self.get_pool_address(&dp.token_id);
            if !dp.is_vault {
                let feed = self.get_token_price_data(&dp.token_id, &mut storage_cache);
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
                        BigUint::zero(),
                        &feed.price,
                    )
                    .returns(ReturnsResult)
                    .sync_call();

                require!(
                    dp.get_total_amount() == BigUint::zero(),
                    ERROR_ENABLE_VAULT_MODE_FAILED
                );

                dp.is_vault = true;
                dp.amount = total_amount_with_interest.clone();

                let last_value = self.vault_supplied_amount(&dp.token_id).update(|am| {
                    *am += &total_amount_with_interest;
                    am.clone()
                });

                self.update_vault_supplied_amount_event(&dp.token_id, last_value);
                self.deposit_positions(nft_token.token_nonce)
                    .insert(dp.token_id.clone(), dp.clone());

                self.update_position_event(
                    &BigUint::zero(),
                    &dp,
                    OptionalValue::None,
                    OptionalValue::None,
                    OptionalValue::None,
                );
            }
        }
        self.tx()
            .to(self.blockchain().get_caller())
            .esdt(nft_token)
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
            self.lending_account_in_the_market(account_nonce);
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
                        Some(BigUint::from(20u64)),
                    );
                }

                self.update_position_event(
                    &BigUint::zero(),
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
            let asset_price = self.get_token_price_data(&asset, &mut storage_cache);
            self.tx()
                .to(pool_address)
                .typed(proxy_pool::LiquidityPoolProxy)
                .update_indexes(&asset_price.price)
                .sync_call();
        }
    }
}
