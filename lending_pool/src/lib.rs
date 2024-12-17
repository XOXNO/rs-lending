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
        self.validate_e_mode_constraints(
            &collateral_payment.token_identifier,
            &asset_info,
            &nft_attributes,
            e_mode_category,
        );

        // 5. Update asset config if NFT has active e-mode
        self.update_asset_config_for_e_mode(
            &mut asset_info,
            nft_attributes.e_mode_category,
            &collateral_payment.token_identifier,
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
            &collateral_payment.token_identifier,
            &collateral_payment.amount,
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
        self.internal_withdraw(
            account_token.token_nonce,
            withdraw_token_id,
            amount.clone(),
            &initial_caller,
            false, // not liquidation
            &BigUint::zero(),
            &mut storage_cache,
            OptionalValue::Some(attributes),
        );

        // 3. Validate health factor after withdrawal
        self.validate_withdraw_health_factor(account_token.token_nonce, false, &mut storage_cache);

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
            &asset_config,
            &nft_attributes,
        );

        // 8. Process borrow
        let updated_position = self.handle_borrow_position(
            nft_token.token_nonce,
            &asset_to_borrow,
            &amount,
            &self.get_token_amount_in_dollars_raw(
                &amount_to_borrow_in_egld,
                &storage_cache.egld_price_feed,
            ),
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
        self.update_borrows_with_debt(account_nonce, &mut storage_cache, false);

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
    fn update_indexes(&self, token_id: EgldOrEsdtTokenIdentifier) {
        let pool_address = self.get_pool_address(&token_id);

        let mut storage_cache = StorageCache::new(self);
        let asset_price = self.get_token_price_data(&token_id, &mut storage_cache);
        self.tx()
            .to(pool_address)
            .typed(proxy_pool::LiquidityPoolProxy)
            .update_indexes(&asset_price.price)
            .sync_call();
    }
}
