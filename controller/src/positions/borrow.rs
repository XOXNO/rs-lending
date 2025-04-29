use common_constants::TOTAL_BORROWED_AMOUNT_STORAGE_KEY;
use common_structs::{
    AccountAttributes, AccountPosition, AccountPositionType, AssetConfig, EModeCategory,
    PriceFeedShort,
};
use multiversx_sc::storage::StorageKey;

use crate::{cache::Cache, helpers, oracle, proxy_pool, storage, utils, validation};
use common_errors::{
    ERROR_ASSET_NOT_BORROWABLE, ERROR_ASSET_NOT_BORROWABLE_IN_ISOLATION,
    ERROR_ASSET_NOT_BORROWABLE_IN_SILOED, ERROR_BORROW_CAP, ERROR_DEBT_CEILING_REACHED,
    ERROR_INSUFFICIENT_COLLATERAL,
};

use super::{account, emode, update};

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

#[multiversx_sc::module]
pub trait PositionBorrowModule:
    storage::Storage
    + validation::ValidationModule
    + oracle::OracleModule
    + common_events::EventsModule
    + utils::LendingUtilsModule
    + helpers::math::MathsModule
    + account::PositionAccountModule
    + update::PositionUpdateModule
    + common_math::SharedMathModule
    + emode::EModeModule
{
    fn handle_create_borrow_strategy(
        &self,
        account_nonce: u64,
        new_debt_token: &EgldOrEsdtTokenIdentifier,
        new_debt_amount_raw: &BigUint,
        debt_config: &mut AssetConfig<Self::Api>,
        caller: &ManagedAddress,
        account_attributes: &AccountAttributes<Self::Api>,
        cache: &mut Cache<Self>,
    ) -> AccountPosition<Self::Api> {
        self.require_asset_supported(new_debt_token);

        let e_mode = self.get_e_mode_category(account_attributes.e_mode_category_id);
        self.ensure_e_mode_not_deprecated(&e_mode);
        let e_mode_id = account_attributes.get_emode_id();
        // Validate e-mode constraints first
        let debt_emode_config = self.get_token_e_mode_config(e_mode_id, new_debt_token);
        self.ensure_e_mode_compatible_with_asset(debt_config, e_mode_id);
        // Update asset config if NFT has active e-mode
        self.apply_e_mode_to_asset_config(debt_config, &e_mode, debt_emode_config);
        require!(debt_config.can_borrow(), ERROR_ASSET_NOT_BORROWABLE);

        let (borrows, _) = self.sync_borrow_positions_interest(account_nonce, cache, false, false);

        let mut borrow_position =
            self.get_or_create_borrow_position(account_nonce, debt_config, new_debt_token);

        let new_debt_amount = borrow_position.make_amount_decimal(new_debt_amount_raw);

        self.validate_borrow_cap(debt_config, &new_debt_amount, new_debt_token, cache);

        let feed = self.get_token_price(new_debt_token, cache);
        self.handle_isolated_debt(cache, &new_debt_amount, account_attributes, &feed);

        let flash_fee = new_debt_amount.clone() * debt_config.flashloan_fee.clone() / self.bps();

        let pool_address = cache.get_cached_pool_address(&borrow_position.asset_id);

        self.validate_borrow_asset(
            debt_config,
            new_debt_token,
            account_attributes,
            &borrows,
            cache,
        );

        // Create the internal flash loan, taking the new debt amount and flash fee added as interest
        borrow_position = self
            .tx()
            .to(pool_address)
            .typed(proxy_pool::LiquidityPoolProxy)
            .create_strategy(
                borrow_position,
                new_debt_amount.clone(),
                flash_fee.clone(),
                feed.price.clone(),
            )
            .returns(ReturnsResult)
            .sync_call();

        self.update_position_event(
            &new_debt_amount,
            &borrow_position,
            OptionalValue::Some(feed.price),
            OptionalValue::Some(caller),
            OptionalValue::Some(account_attributes),
        );

        self.store_borrow_position(account_nonce, new_debt_token, &borrow_position);

        borrow_position
    }

    /// Manages a borrow operation, updating positions and handling isolated debt.
    /// Orchestrates borrowing logic with validations and storage updates.
    ///
    /// # Arguments
    /// - `account_nonce`: Position NFT nonce.
    /// - `borrow_token_id`: Token to borrow.
    /// - `amount`: Borrow amount.
    /// - `amount_in_usd`: USD value of the borrow.
    /// - `caller`: Borrower's address.
    /// - `asset_config`: Borrowed asset configuration.
    /// - `account`: NFT attributes.
    /// - `collaterals`: User's collateral positions.
    /// - `feed`: Price feed for the asset.
    /// - `cache`: Mutable storage cache.
    ///
    /// # Returns
    /// - Updated borrow position.
    fn handle_borrow_position(
        &self,
        account_nonce: u64,
        borrow_token_id: &EgldOrEsdtTokenIdentifier,
        amount: ManagedDecimal<Self::Api, NumDecimals>,
        caller: &ManagedAddress,
        asset_config: &AssetConfig<Self::Api>,
        account: &AccountAttributes<Self::Api>,
        feed: &PriceFeedShort<Self::Api>,
        cache: &mut Cache<Self>,
    ) -> AccountPosition<Self::Api> {
        let pool_address = cache.get_cached_pool_address(borrow_token_id);
        let mut borrow_position =
            self.get_or_create_borrow_position(account_nonce, asset_config, borrow_token_id);

        borrow_position = self.execute_borrow(
            pool_address,
            caller,
            amount.clone(),
            borrow_position,
            feed.price.clone(),
        );

        self.store_borrow_position(account_nonce, borrow_token_id, &borrow_position);

        self.update_position_event(
            &amount,
            &borrow_position,
            OptionalValue::Some(feed.price.clone()),
            OptionalValue::Some(caller),
            OptionalValue::Some(account),
        );

        borrow_position
    }

    /// Executes a borrow operation via the liquidity pool.
    /// Handles cross-contract interaction for borrowing.
    ///
    /// # Arguments
    /// - `pool_address`: Liquidity pool address.
    /// - `caller`: Borrower's address.
    /// - `amount`: Borrow amount.
    /// - `position`: Current borrow position.
    /// - `price`: Asset price.
    ///
    /// # Returns
    /// - Updated borrow position.
    fn execute_borrow(
        &self,
        pool_address: ManagedAddress,
        caller: &ManagedAddress,
        amount: ManagedDecimal<Self::Api, NumDecimals>,
        position: AccountPosition<Self::Api>,
        price: ManagedDecimal<Self::Api, NumDecimals>,
    ) -> AccountPosition<Self::Api> {
        self.tx()
            .to(pool_address)
            .typed(proxy_pool::LiquidityPoolProxy)
            .borrow(caller, amount, position, price)
            .returns(ReturnsResult)
            .sync_call()
    }

    /// Stores an updated borrow position in account storage.
    ///
    /// # Arguments
    /// - `account_nonce`: Position NFT nonce.
    /// - `borrow_token_id`: Borrowed token identifier.
    /// - `position`: Updated borrow position.
    fn store_borrow_position(
        &self,
        account_nonce: u64,
        borrow_token_id: &EgldOrEsdtTokenIdentifier,
        position: &AccountPosition<Self::Api>,
    ) {
        self.borrow_positions(account_nonce)
            .insert(borrow_token_id.clone(), position.clone());
    }

    /// Manages debt tracking for isolated positions.
    /// Validates and updates debt ceiling for isolated collateral.
    ///
    /// # Arguments
    /// - `collaterals`: User's collateral positions.
    /// - `cache`: Mutable storage cache.
    /// - `amount_in_usd`: USD value of the borrow.
    fn handle_isolated_debt(
        &self,
        cache: &mut Cache<Self>,
        amount: &ManagedDecimal<Self::Api, NumDecimals>,
        account_attributes: &AccountAttributes<Self::Api>,
        feed: &PriceFeedShort<Self::Api>,
    ) {
        if !account_attributes.is_isolated() {
            return;
        }
        let egld_amount = self.get_token_egld_value(amount, &feed.price);
        let amount_in_usd = self.get_egld_usd_value(&egld_amount, &cache.egld_usd_price);

        let isolated_token = account_attributes.get_isolated_token();
        let collateral_config = cache.get_cached_asset_info(&isolated_token);
        self.validate_isolated_debt_ceiling(
            &collateral_config,
            &isolated_token,
            amount_in_usd.clone(),
        );
        self.adjust_isolated_debt_usd(&isolated_token, amount_in_usd, true);
    }

    /// Retrieves or creates a borrow position for a token.
    /// Initializes new positions if none exist.
    ///
    /// # Arguments
    /// - `account_nonce`: Position NFT nonce.
    /// - `borrow_asset_config`: Borrowed asset configuration.
    /// - `token_id`: Token identifier.
    ///
    /// # Returns
    /// - Borrow position.
    fn get_or_create_borrow_position(
        &self,
        account_nonce: u64,
        borrow_asset_config: &AssetConfig<Self::Api>,
        token_id: &EgldOrEsdtTokenIdentifier,
    ) -> AccountPosition<Self::Api> {
        let borrow_positions = self.borrow_positions(account_nonce);
        borrow_positions.get(token_id).unwrap_or_else(|| {
            let price_data = self.token_oracle(token_id).get();
            AccountPosition::new(
                AccountPositionType::Borrow,
                token_id.clone(),
                self.to_decimal(BigUint::zero(), price_data.price_decimals),
                self.to_decimal(BigUint::zero(), price_data.price_decimals),
                account_nonce,
                self.blockchain().get_block_timestamp(),
                self.ray(),
                borrow_asset_config.liquidation_threshold.clone(),
                borrow_asset_config.liquidation_bonus.clone(),
                borrow_asset_config.liquidation_fees.clone(),
                borrow_asset_config.loan_to_value.clone(),
            )
        })
    }

    /// Ensures a new borrow stays within the asset's borrow cap.
    ///
    /// # Arguments
    /// - `asset_config`: Borrowed asset configuration.
    /// - `amount`: Borrow amount.
    /// - `asset`: Token identifier.
    fn validate_borrow_cap(
        &self,
        asset_config: &AssetConfig<Self::Api>,
        amount: &ManagedDecimal<Self::Api, NumDecimals>,
        asset: &EgldOrEsdtTokenIdentifier,
        cache: &mut Cache<Self>,
    ) {
        match &asset_config.borrow_cap {
            Some(borrow_cap) => {
                if borrow_cap == &BigUint::zero() {
                    return;
                }

                let pool = cache.get_cached_pool_address(asset);
                let total_borrow = self.get_total_borrow(pool).get();

                require!(
                    total_borrow.clone() + amount.clone()
                        <= self.to_decimal(borrow_cap.clone(), total_borrow.scale()),
                    ERROR_BORROW_CAP
                );
            },
            None => {
                // No borrow cap set, do nothing
            },
        }
    }

    /// Retrieves total borrow amount from the liquidity pool.
    ///
    /// # Arguments
    /// - `pool_address`: Pool address.
    ///
    /// # Returns
    /// - `SingleValueMapper` with total borrow amount.
    fn get_total_borrow(
        &self,
        pool_address: ManagedAddress,
    ) -> SingleValueMapper<ManagedDecimal<Self::Api, NumDecimals>, ManagedAddress> {
        SingleValueMapper::<_, _, ManagedAddress>::new_from_address(
            pool_address,
            StorageKey::new(TOTAL_BORROWED_AMOUNT_STORAGE_KEY),
        )
    }

    /// Validates sufficient collateral for a borrow operation.
    ///
    /// # Arguments
    /// - `ltv_base_amount`: LTV-weighted collateral in EGLD.
    /// - `borrowed_amount`: Current borrowed amount in EGLD.
    /// - `amount_to_borrow`: New borrow amount in EGLD.
    fn validate_borrow_collateral(
        &self,
        ltv_base_amount: &ManagedDecimal<Self::Api, NumDecimals>,
        borrowed_amount: &ManagedDecimal<Self::Api, NumDecimals>,
        amount_to_borrow: &ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        require!(
            ltv_base_amount >= &(borrowed_amount.clone() + amount_to_borrow.clone()),
            ERROR_INSUFFICIENT_COLLATERAL
        );
    }

    /// Validates borrow constraints and computes amounts in USD and EGLD.
    /// Ensures borrowing adheres to protocol rules.
    ///
    /// # Arguments
    /// - `ltv_base_amount`: LTV-weighted collateral in EGLD.
    /// - `borrow_token_id`: Token to borrow.
    /// - `amount_raw`: Raw borrow amount.
    /// - `borrow_positions`: Current borrow positions.
    /// - `cache`: Mutable storage cache.
    ///
    /// # Returns
    /// - Tuple of (USD value, price feed, decimal amount).
    fn validate_and_get_borrow_amounts(
        &self,
        ltv_base_amount: &ManagedDecimal<Self::Api, NumDecimals>,
        amount: &ManagedDecimal<Self::Api, NumDecimals>,
        borrow_positions: &ManagedVec<AccountPosition<Self::Api>>,
        feed: &PriceFeedShort<Self::Api>,
        cache: &mut Cache<Self>,
    ) {
        let egld_amount = self.get_token_egld_value(amount, &feed.price);
        let egld_total_borrowed = self.calculate_total_borrow_in_egld(borrow_positions, cache);

        self.validate_borrow_collateral(ltv_base_amount, &egld_total_borrowed, &egld_amount);
    }

    /// Validates an asset's borrowability under position constraints.
    ///
    /// # Arguments
    /// - `asset_config`: Borrowed asset configuration.
    /// - `borrow_token_id`: Token to borrow.
    /// - `nft_attributes`: NFT attributes.
    /// - `borrow_positions`: Current borrow positions.
    /// - `cache`: Mutable storage cache.
    fn validate_borrow_asset(
        &self,
        asset_config: &AssetConfig<Self::Api>,
        borrow_token_id: &EgldOrEsdtTokenIdentifier,
        nft_attributes: &AccountAttributes<Self::Api>,
        borrow_positions: &ManagedVec<AccountPosition<Self::Api>>,
        cache: &mut Cache<Self>,
    ) {
        // Check if borrowing is allowed in isolation mode
        if nft_attributes.is_isolated() {
            require!(
                asset_config.can_borrow_in_isolation(),
                ERROR_ASSET_NOT_BORROWABLE_IN_ISOLATION
            );
        }

        // Validate siloed borrowing constraints
        if asset_config.is_siloed_borrowing() {
            require!(
                borrow_positions.len() <= 1,
                ERROR_ASSET_NOT_BORROWABLE_IN_SILOED
            );
        }

        // Check if trying to borrow a different asset when there's a siloed position
        if borrow_positions.len() == 1 {
            let first_position = borrow_positions.get(0);
            let first_asset_config = cache.get_cached_asset_info(&first_position.asset_id);

            // If either the existing position or new borrow is siloed, they must be the same asset
            if first_asset_config.is_siloed_borrowing() || asset_config.is_siloed_borrowing() {
                require!(
                    borrow_token_id == &first_position.asset_id,
                    ERROR_ASSET_NOT_BORROWABLE_IN_SILOED
                );
            }
        }
    }

    /// Ensures a new borrow respects the isolated asset debt ceiling.
    ///
    /// # Arguments
    /// - `asset_config`: Collateral asset configuration.
    /// - `token_id`: Collateral token identifier.
    /// - `amount_to_borrow_in_dollars`: USD value of the borrow.
    fn validate_isolated_debt_ceiling(
        &self,
        asset_config: &AssetConfig<Self::Api>,
        token_id: &EgldOrEsdtTokenIdentifier,
        amount_to_borrow_in_dollars: ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        let current_debt = self.isolated_asset_debt_usd(token_id).get();
        let total_debt = current_debt + amount_to_borrow_in_dollars.clone();

        require!(
            total_debt <= asset_config.isolation_debt_ceiling_usd,
            ERROR_DEBT_CEILING_REACHED
        );
    }

    /// Processes a single borrow operation, including validations and position updates.
    ///
    /// # Arguments
    /// - `cache`: Mutable reference to the storage cache.
    /// - `account_payment`: The account NFT payment.
    /// - `caller`: The address initiating the borrow.
    /// - `borrowed_token`: The token and amount to borrow.
    /// - `account_attributes`: Attributes of the account position.
    /// - `e_mode`: The e-mode category, if enabled.
    /// - `collaterals`: Vector of collateral positions.
    /// - `borrows`: Mutable vector of borrow positions.
    /// - `borrow_index_mapper`: Mutable map for indexing borrow positions in bulk borrows.
    /// - `is_bulk_borrow`: Flag indicating if this is part of a bulk borrow operation.
    fn process_borrow(
        &self,
        cache: &mut Cache<Self>,
        account_nonce: u64,
        caller: &ManagedAddress,
        borrowed_token: &EgldOrEsdtTokenPayment<Self::Api>,
        account_attributes: &AccountAttributes<Self::Api>,
        e_mode: &Option<EModeCategory<Self::Api>>,
        borrows: &mut ManagedVec<AccountPosition<Self::Api>>,
        borrow_index_mapper: &mut ManagedMapEncoded<
            Self::Api,
            EgldOrEsdtTokenIdentifier<Self::Api>,
            usize,
        >,
        is_bulk_borrow: bool,
        ltv_collateral: &ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        // Basic validations
        self.validate_payment(borrowed_token);

        // Get and validate asset configuration
        let mut asset_config = cache.get_cached_asset_info(&borrowed_token.token_identifier);
        let feed = self.get_token_price(&borrowed_token.token_identifier, cache);

        self.validate_borrow_asset(
            &asset_config,
            &borrowed_token.token_identifier,
            account_attributes,
            borrows,
            cache,
        );

        // Apply e-mode configuration
        let asset_emode_config = self.get_token_e_mode_config(
            account_attributes.get_emode_id(),
            &borrowed_token.token_identifier,
        );
        self.ensure_e_mode_compatible_with_asset(&asset_config, account_attributes.get_emode_id());
        self.apply_e_mode_to_asset_config(&mut asset_config, e_mode, asset_emode_config);

        require!(asset_config.can_borrow(), ERROR_ASSET_NOT_BORROWABLE);

        let amount = self.to_decimal(borrowed_token.amount.clone(), feed.asset_decimals);

        // Validate borrow amounts and caps
        self.validate_and_get_borrow_amounts(ltv_collateral, &amount, borrows, &feed, cache);
        self.validate_borrow_cap(
            &asset_config,
            &amount,
            &borrowed_token.token_identifier,
            cache,
        );

        self.handle_isolated_debt(cache, &amount, account_attributes, &feed);

        // Handle the borrow position
        let updated_position = self.handle_borrow_position(
            account_nonce,
            &borrowed_token.token_identifier,
            amount,
            caller,
            &asset_config,
            account_attributes,
            &feed,
            cache,
        );

        // Update borrow positions for bulk borrows
        self.update_bulk_borrow_positions(
            borrows,
            borrow_index_mapper,
            updated_position,
            is_bulk_borrow,
        );
    }
}
