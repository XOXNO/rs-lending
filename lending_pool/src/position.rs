multiversx_sc::imports!();
use common_constants::BP;
use common_events::{
    AccountPosition, AssetConfig, EModeCategory, EgldOrEsdtTokenPaymentNew, NftAccountAttributes,
};
use common_events::{AccountPositionType, PriceFeedShort};

use crate::contexts::base::StorageCache;
use crate::storage;
use crate::utils;
use crate::validation;
use crate::{math, views};
use crate::{oracle, ERROR_LIQUIDATED_AMOUNT_AFTER_FEES_LESS_THAN_MIN_AMOUNT_TO_RECEIVE};
use crate::{proxy_pool, ERROR_EMODE_CATEGORY_DEPRECATED};

#[multiversx_sc::module]
pub trait PositionModule:
    storage::LendingStorageModule
    + validation::ValidationModule
    + oracle::OracleModule
    + common_events::EventsModule
    + utils::LendingUtilsModule
    + math::LendingMathModule
    + views::ViewsModule
{
    /// Updates all borrow positions for an account with accumulated interest
    ///
    /// # Arguments
    /// * `account_position` - The NFT nonce representing the account position
    /// * `fetch_price` - Whether to fetch current price data for each asset
    ///
    /// # Returns
    /// * `ManagedVec<AccountPosition>` - Vector of updated borrow positions
    ///
    /// Updates each borrow position by calling the liquidity pool to calculate
    /// accumulated interest. Stores the updated positions in storage and returns them.
    fn update_borrows_with_debt(
        &self,
        account_position: u64,
        storage_cache: &mut StorageCache<Self>,
        fetch_price: bool,
    ) -> ManagedVec<AccountPosition<Self::Api>> {
        let borrow_positions = self.borrow_positions(account_position);
        let mut positions: ManagedVec<Self::Api, AccountPosition<Self::Api>> = ManagedVec::new();

        for bp in borrow_positions.values() {
            let asset_address = self.get_pool_address(&bp.token_id);
            let price = if fetch_price {
                let result = self.get_token_price_data(&bp.token_id, storage_cache);
                OptionalValue::Some(result.price)
            } else {
                OptionalValue::None
            };
            let latest_position = self
                .tx()
                .to(asset_address)
                .typed(proxy_pool::LiquidityPoolProxy)
                .update_position_with_interest(bp.clone(), price)
                .returns(ReturnsResult)
                .sync_call();

            positions.push(latest_position.clone());
            self.borrow_positions(account_position)
                .insert(bp.token_id, latest_position);
        }
        positions
    }

    /// Updates all collateral positions for an account with accumulated interest
    ///
    /// # Arguments
    /// * `account_position` - The NFT nonce representing the account position
    /// * `fetch_price` - Whether to fetch current price data for each asset
    ///
    /// # Returns
    /// * `ManagedVec<AccountPosition>` - Vector of updated collateral positions
    ///
    /// Updates each collateral position by calling the liquidity pool to calculate
    /// accumulated interest. Skips vault positions as they don't accrue interest.
    /// Stores the updated positions in storage and returns them.
    fn update_collateral_with_interest(
        &self,
        account_position: u64,
        storage_cache: &mut StorageCache<Self>,
        fetch_price: bool,
    ) -> ManagedVec<AccountPosition<Self::Api>> {
        let deposit_positions = self.deposit_positions(account_position);
        let mut positions: ManagedVec<Self::Api, AccountPosition<Self::Api>> = ManagedVec::new();
        for dp in deposit_positions.values() {
            let asset_address = self.get_pool_address(&dp.token_id);
            if !dp.is_vault {
                let price = if fetch_price {
                    let result = self.get_token_price_data(&dp.token_id, storage_cache);
                    OptionalValue::Some(result.price)
                } else {
                    OptionalValue::None
                };
                let latest_position = self
                    .tx()
                    .to(asset_address)
                    .typed(proxy_pool::LiquidityPoolProxy)
                    .update_position_with_interest(dp.clone(), price)
                    .returns(ReturnsResult)
                    .sync_call();

                self.deposit_positions(account_position)
                    .insert(dp.token_id, latest_position.clone());
                positions.push(latest_position);
            } else {
                positions.push(dp.clone());
            }
        }
        positions
    }

    /// Creates a new account position NFT
    ///
    /// # Arguments
    /// * `caller` - Address of the user creating the position
    /// * `is_isolated` - Whether this is an isolated position (can only have one collateral)
    /// * `is_vault` - Whether this is a vault position (no interest accrual)
    /// * `e_mode_category` - Optional e-mode category for specialized LTV and liquidation parameters
    ///
    /// # Returns
    /// * `(EsdtTokenPayment, NftAccountAttributes)` - The created NFT and its attributes
    ///
    /// Creates and sends a new NFT to the caller representing their lending position.
    /// The NFT attributes store the position type (isolated/vault) and e-mode settings.
    fn enter(
        &self,
        caller: &ManagedAddress,
        is_isolated: bool,
        is_vault: bool,
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
            is_vault,
        };
        let nft_token_payment = self
            .account_token()
            .nft_create_and_send::<NftAccountAttributes>(caller, amount, attributes);

        self.account_positions()
            .insert(nft_token_payment.token_nonce);
        self.account_attributes(nft_token_payment.token_nonce)
            .set(attributes.clone());

        (nft_token_payment, attributes.clone())
    }

    /// Gets or creates a supply position for a user
    ///
    /// # Arguments
    /// * `initial_caller` - Address of the user supplying assets
    /// * `is_isolated` - Whether this is an isolated position
    /// * `is_vault` - Whether this is a vault position
    /// * `e_mode_category` - Optional e-mode category
    /// * `account_nonce` - Optional existing NFT nonce to use
    ///
    /// # Returns
    /// * `(u64, NftAccountAttributes)` - NFT nonce and its attributes
    ///
    /// If account_nonce is provided, validates and uses existing position.
    /// Otherwise creates a new position with specified parameters.
    fn get_or_create_supply_position(
        &self,
        initial_caller: &ManagedAddress,
        is_isolated: bool,
        is_vault: bool,
        e_mode_category: OptionalValue<u8>,
        account_nonce: Option<u64>,
    ) -> (u64, NftAccountAttributes) {
        if let Some(nonce) = account_nonce {
            self.lending_account_in_the_market(nonce);

            let token_id = self.account_token().get_token_id();
            let attributes = self.get_account_attributes(nonce, &token_id);

            // Return NFT to owner after validation
            self.tx()
                .to(initial_caller)
                .single_esdt(&token_id, nonce, &BigUint::from(1u64))
                .transfer();
            (nonce, attributes)
        } else {
            let (payment, attributes) =
                self.enter(initial_caller, is_isolated, is_vault, e_mode_category);
            (payment.token_nonce, attributes)
        }
    }

    /// Retrieves existing deposit position or creates new one
    ///
    /// # Arguments
    /// * `account_nonce` - NFT nonce of the account position
    /// * `asset_info` - Configuration of the asset being deposited
    /// * `token_id` - Token identifier of the deposit
    /// * `is_vault` - Whether this is a vault position
    ///
    /// # Returns
    /// * `AccountPosition` - The existing or new deposit position
    ///
    /// If a position exists for the token, returns it.
    /// Otherwise creates a new position with zero balance and default parameters.
    fn get_existing_or_new_deposit_position_for_token(
        &self,
        account_nonce: u64,
        asset_info: &AssetConfig<Self::Api>,
        token_id: &EgldOrEsdtTokenIdentifier,
        is_vault: bool,
    ) -> AccountPosition<Self::Api> {
        let mut deposit_positions = self.deposit_positions(account_nonce);

        if let Some(position) = deposit_positions.get(token_id) {
            deposit_positions.remove(token_id);
            position
        } else {
            AccountPosition::new(
                AccountPositionType::Deposit,
                token_id.clone(),
                BigUint::zero(),
                BigUint::zero(),
                account_nonce,
                self.blockchain().get_block_timestamp(),
                BigUint::from(BP),
                asset_info.liquidation_threshold.clone(),
                asset_info.ltv.clone(),
                is_vault,
            )
        }
    }

    /// Updates supply position with new deposit
    ///
    /// # Arguments
    /// * `account_nonce` - NFT nonce of the account position
    /// * `token_id` - Token identifier of the deposit
    /// * `amount` - Amount being deposited
    /// * `asset_info` - Configuration of the asset
    /// * `is_vault` - Whether this is a vault position
    ///
    /// # Returns
    /// * `AccountPosition` - The updated position after deposit
    ///
    /// For vault positions, directly updates storage.
    /// For market positions, calls liquidity pool to handle deposit.
    /// Updates position storage and returns updated position.
    fn update_supply_position(
        &self,
        account_nonce: u64,
        collateral_payment: &EgldOrEsdtTokenPaymentNew<Self::Api>,
        asset_info: &AssetConfig<Self::Api>,
        is_vault: bool,
        asset_data_feed: &PriceFeedShort<Self::Api>,
    ) -> AccountPosition<Self::Api> {
        let mut deposit_position = self.get_existing_or_new_deposit_position_for_token(
            account_nonce,
            asset_info,
            &collateral_payment.token_identifier,
            is_vault,
        );

        if deposit_position.entry_ltv != asset_info.ltv {
            deposit_position.entry_ltv = asset_info.ltv.clone();
        }

        if is_vault {
            self.update_vault_position(
                &mut deposit_position,
                &collateral_payment.amount,
                &collateral_payment.token_identifier,
            );
        } else {
            self.update_market_position(
                &mut deposit_position,
                &collateral_payment.amount,
                &collateral_payment.token_identifier,
                &asset_data_feed,
            );
        }

        // Update storage with the latest position
        self.deposit_positions(account_nonce).insert(
            collateral_payment.token_identifier.clone(),
            deposit_position.clone(),
        );

        deposit_position
    }

    /// Updates market position through liquidity pool
    ///
    /// # Arguments
    /// * `position` - Current position to update
    /// * `amount` - Amount being deposited
    /// * `token_id` - Token identifier
    ///
    /// Calls liquidity pool to handle deposit, update interest indices,
    /// and return updated position. Used for non-vault positions.
    fn update_market_position(
        &self,
        position: &mut AccountPosition<Self::Api>,
        amount: &BigUint,
        token_id: &EgldOrEsdtTokenIdentifier,
        asset_data_feed: &PriceFeedShort<Self::Api>,
    ) {
        let pool_address = self.get_pool_address(token_id);

        *position = self
            .tx()
            .to(pool_address)
            .typed(proxy_pool::LiquidityPoolProxy)
            .supply(position.clone(), &asset_data_feed.price)
            .payment(EgldOrEsdtTokenPayment::new(
                token_id.clone(),
                0,
                amount.clone(),
            ))
            .returns(ReturnsResult)
            .sync_call();
    }

    /// Updates vault position directly in storage
    ///
    /// # Arguments
    /// * `position` - Current position to update
    /// * `amount` - Amount being deposited
    /// * `token_id` - Token identifier
    ///
    /// Updates vault supplied amount in storage and position balance.
    /// Used for vault positions that don't accrue interest.
    /// Emits event for tracking vault deposits.
    #[inline]
    fn update_vault_position(
        &self,
        position: &mut AccountPosition<Self::Api>,
        amount: &BigUint,
        token_id: &EgldOrEsdtTokenIdentifier,
    ) {
        let last_value = self.vault_supplied_amount(token_id).update(|am| {
            *am += amount;
            am.clone()
        });

        self.update_vault_supplied_amount_event(token_id, last_value);
        position.amount += amount;
    }

    /// Updates asset configuration for e-mode
    ///
    /// # Arguments
    /// * `asset_info` - Asset configuration to update
    /// * `e_mode_category_id` - E-mode category ID
    /// * `token_id` - Token identifier
    ///
    /// If position is in e-mode and asset supports it,
    /// updates LTV, liquidation threshold, and other parameters
    /// based on e-mode category settings.
    fn update_asset_config_for_e_mode(
        &self,
        asset_info: &mut AssetConfig<Self::Api>,
        e_mode_category_id: u8,
        token_id: &EgldOrEsdtTokenIdentifier,
        category: Option<EModeCategory<Self::Api>>,
    ) {
        if category.is_some() {
            require!(
                !category.unwrap().is_deprecated,
                ERROR_EMODE_CATEGORY_DEPRECATED
            );
        } else if e_mode_category_id > 0 {
            let category = self.e_mode_category().get(&e_mode_category_id).unwrap();
            require!(!category.is_deprecated, ERROR_EMODE_CATEGORY_DEPRECATED);
        };

        if !asset_info.is_isolated && asset_info.is_e_mode_enabled && e_mode_category_id > 0 {
            let category_data = self.e_mode_category().get(&e_mode_category_id).unwrap();
            let asset_emode_config = self
                .e_mode_assets(e_mode_category_id)
                .get(token_id)
                .unwrap();

            // Update all asset config parameters with e-mode values for that category
            asset_info.can_be_collateral = asset_emode_config.can_be_collateral;
            asset_info.can_be_borrowed = asset_emode_config.can_be_borrowed;
            asset_info.ltv = category_data.ltv;
            asset_info.liquidation_threshold = category_data.liquidation_threshold;
            asset_info.liquidation_bonus = category_data.liquidation_bonus;
        }
    }

    /// Processes withdrawal from a position
    ///
    /// # Arguments
    /// * `account_nonce` - NFT nonce of the account position
    /// * `withdraw_token_id` - Token to withdraw
    /// * `amount` - Amount to withdraw
    /// * `initial_caller` - Address initiating withdrawal
    /// * `is_liquidation` - Whether this is a liquidation withdrawal
    /// * `liquidation_fee` - Protocol fee for liquidation
    /// * `attributes` - Optional NFT attributes
    ///
    /// # Returns
    /// * `AccountPosition` - Updated position after withdrawal
    ///
    /// Handles both normal withdrawals and liquidations.
    /// For vault positions, updates storage directly.
    /// For market positions, processes through liquidity pool.
    /// Handles protocol fees for liquidations.
    fn internal_withdraw(
        &self,
        account_nonce: u64,
        withdraw_token_id: &EgldOrEsdtTokenIdentifier,
        mut amount: BigUint,
        initial_caller: &ManagedAddress,
        is_liquidation: bool,
        liquidation_fee: &BigUint,
        storage_cache: &mut StorageCache<Self>,
        attributes: OptionalValue<NftAccountAttributes>,
        min_amount_to_receive: Option<BigUint>,
    ) -> AccountPosition<Self::Api> {
        let pool_address = self.get_pool_address(withdraw_token_id);
        let mut dep_pos_map = self.deposit_positions(account_nonce);
        let dp_opt = dep_pos_map.get(withdraw_token_id);

        require!(
            dp_opt.is_some(),
            "Token {} is not available for this account",
            withdraw_token_id
        );

        let mut dp = dp_opt.unwrap();

        // Cap withdraw amount to available balance (principal + accumulated interest)
        if amount > dp.get_total_amount() {
            amount = dp.get_total_amount();
        }

        let liquidated_amount_after_fees = &(&amount - liquidation_fee);
        if min_amount_to_receive.is_some() && is_liquidation {
            require!(
                liquidated_amount_after_fees >= &min_amount_to_receive.unwrap(),
                ERROR_LIQUIDATED_AMOUNT_AFTER_FEES_LESS_THAN_MIN_AMOUNT_TO_RECEIVE
            );
        }

        let asset_data = self.get_token_price_data(withdraw_token_id, storage_cache);
        let deposit_position = if dp.is_vault {
            let last_value = self.vault_supplied_amount(withdraw_token_id).update(|am| {
                *am -= &amount;
                am.clone()
            });

            self.update_vault_supplied_amount_event(withdraw_token_id, last_value);
            dp.amount -= &amount;

            if is_liquidation {
                self.send().direct_esdt(
                    initial_caller,
                    &withdraw_token_id.clone().unwrap_esdt(),
                    0,
                    liquidated_amount_after_fees,
                );

                self.tx()
                    .to(pool_address)
                    .typed(proxy_pool::LiquidityPoolProxy)
                    .add_vault_liquidation_rewards(&asset_data.price)
                    .egld_or_single_esdt(withdraw_token_id, 0, liquidation_fee)
                    .returns(ReturnsResult)
                    .sync_call();

                dp.clone()
            } else {
                self.send().direct_esdt(
                    initial_caller,
                    &withdraw_token_id.clone().unwrap_esdt(),
                    0,
                    &amount,
                );
                dp.clone()
            }
        } else {
            self.tx()
                .to(pool_address)
                .typed(proxy_pool::LiquidityPoolProxy)
                .withdraw(
                    initial_caller,
                    &amount,
                    dp,
                    is_liquidation,
                    liquidation_fee,
                    &asset_data.price,
                )
                .returns(ReturnsResult)
                .sync_call()
        };

        self.update_position_event(
            &amount,
            &deposit_position,
            OptionalValue::Some(asset_data.price),
            OptionalValue::Some(initial_caller.clone()),
            attributes,
        );

        if deposit_position.amount == 0 {
            dep_pos_map.remove(withdraw_token_id);
        } else {
            dep_pos_map.insert(withdraw_token_id.clone(), deposit_position.clone());
        }

        deposit_position
    }

    /// Handles NFT after withdrawal operation
    ///
    /// # Arguments
    /// * `account_token` - NFT token payment
    /// * `initial_caller` - Address initiating withdrawal
    ///
    /// If no positions remain (no deposits or borrows),
    /// burns the NFT and removes from storage.
    /// Otherwise returns NFT to caller.
    fn handle_nft_after_withdraw(
        &self,
        account_token: EsdtTokenPayment<Self::Api>,
        initial_caller: &ManagedAddress,
    ) {
        let dep_pos_map = self.deposit_positions(account_token.token_nonce).len();
        let borrow_pos_map = self.borrow_positions(account_token.token_nonce).len();

        if dep_pos_map == 0 && borrow_pos_map == 0 {
            self.account_token()
                .nft_burn(account_token.token_nonce, &account_token.amount);
            self.account_positions()
                .swap_remove(&account_token.token_nonce);
            self.account_attributes(account_token.token_nonce).clear();
        } else {
            self.tx().to(initial_caller).esdt(account_token).transfer();
        }
    }

    /// Processes borrow operation
    ///
    /// # Arguments
    /// * `account_nonce` - NFT nonce of the account position
    /// * `asset_to_borrow` - Token to borrow
    /// * `amount` - Amount to borrow
    /// * `amount_in_usd` - USD value of borrow
    /// * `initial_caller` - Address initiating borrow
    /// * `asset_config` - Asset configuration
    /// * `nft_attributes` - Position NFT attributes
    /// * `collateral_positions` - Current collateral positions
    /// * `asset_data_feed` - Price data for the asset being borrowed
    ///
    /// # Returns
    /// * `AccountPosition` - Updated borrow position
    ///
    /// Creates or updates borrow position through liquidity pool.
    /// Handles isolated mode debt ceiling checks.
    /// Updates storage with new position.
    fn handle_borrow_position(
        &self,
        account_nonce: u64,
        asset_to_borrow: &EgldOrEsdtTokenIdentifier,
        amount: &BigUint,
        amount_in_usd: &BigUint,
        initial_caller: &ManagedAddress,
        asset_config: &AssetConfig<Self::Api>,
        nft_attributes: &NftAccountAttributes,
        collateral_positions: &ManagedVec<AccountPosition<Self::Api>>,
        asset_data_feed: &PriceFeedShort<Self::Api>,
    ) -> AccountPosition<Self::Api> {
        let pool_address = self.get_pool_address(asset_to_borrow);

        // Get or create borrow position
        let borrow_position = self.get_existing_or_new_borrow_position_for_token(
            account_nonce,
            asset_config,
            asset_to_borrow.clone(),
            nft_attributes.is_vault,
        );

        // Execute borrow
        let updated_position = self
            .tx()
            .to(pool_address)
            .typed(proxy_pool::LiquidityPoolProxy)
            .borrow(
                initial_caller,
                amount,
                borrow_position,
                &asset_data_feed.price,
            )
            .returns(ReturnsResult)
            .sync_call();

        // Handle isolated mode debt ceiling
        if nft_attributes.is_isolated {
            let collateral_token_id = collateral_positions.get(0).token_id;
            let collateral_config = self.asset_config(&collateral_token_id).get();

            self.validate_isolated_debt_ceiling(
                &collateral_config,
                &collateral_token_id,
                amount_in_usd,
            );
            self.update_isolated_debt_usd(
                &collateral_token_id,
                amount_in_usd,
                true, // is_increase
            );
        }

        // Update storage
        self.borrow_positions(account_nonce)
            .insert(asset_to_borrow.clone(), updated_position.clone());

        updated_position
    }

    /// Gets existing borrow position or creates new one
    ///
    /// # Arguments
    /// * `account_nonce` - NFT nonce of the account position
    /// * `asset_info` - Asset configuration for the borrowed token
    /// * `token_id` - Token identifier of the borrowed asset
    /// * `is_vault` - Whether this is a vault position
    ///
    /// # Returns
    /// * `AccountPosition` - The existing or new borrow position
    ///
    /// If a borrow position exists for the token, returns it.
    /// Otherwise creates a new position with zero balance and default parameters.
    /// Used in both normal borrowing and liquidation flows.
    fn get_existing_or_new_borrow_position_for_token(
        &self,
        account_nonce: u64,
        asset_info: &AssetConfig<Self::Api>,
        token_id: EgldOrEsdtTokenIdentifier,
        is_vault: bool,
    ) -> AccountPosition<Self::Api> {
        let mut borrow_positions = self.borrow_positions(account_nonce);

        if let Some(position) = borrow_positions.get(&token_id) {
            borrow_positions.remove(&token_id);
            position
        } else {
            AccountPosition::new(
                AccountPositionType::Borrow,
                token_id,
                BigUint::zero(),
                BigUint::zero(),
                account_nonce,
                self.blockchain().get_block_timestamp(),
                BigUint::from(BP),
                asset_info.liquidation_threshold.clone(),
                asset_info.ltv.clone(),
                is_vault,
            )
        }
    }

    /// Processes repayment for a borrow position through liquidity pool
    ///
    /// # Arguments
    /// * `account_nonce` - NFT nonce of the account position
    /// * `repay_token_id` - Token being repaid
    /// * `repay_amount` - Amount being repaid
    /// * `initial_caller` - Address initiating repayment
    /// * `borrow_position` - Current borrow position being repaid
    /// * `debt_token_price_data` - Price data for the debt token
    ///
    /// # Returns
    /// * `AccountPosition` - Updated position after repayment
    ///
    /// Calls liquidity pool to process repayment and update interest indices.
    /// If position is fully repaid (amount = 0), removes it from storage.
    /// Otherwise updates storage with new position details.
    fn handle_repay_position(
        &self,
        account_nonce: u64,
        repay_token_id: &EgldOrEsdtTokenIdentifier,
        repay_amount: &BigUint,
        initial_caller: &ManagedAddress,
        borrow_position: AccountPosition<Self::Api>,
        debt_token_price_data: &PriceFeedShort<Self::Api>,
    ) -> AccountPosition<Self::Api> {
        let asset_address = self.get_pool_address(repay_token_id);
        let updated_position = self
            .tx()
            .to(asset_address)
            .typed(proxy_pool::LiquidityPoolProxy)
            .repay(
                initial_caller,
                borrow_position,
                &debt_token_price_data.price,
            )
            .egld_or_single_esdt(repay_token_id, 0, repay_amount)
            .returns(ReturnsResult)
            .sync_call();

        // Update storage
        let mut borrow_positions = self.borrow_positions(account_nonce);
        if updated_position.amount.gt(&BigUint::from(0u64)) {
            borrow_positions.insert(repay_token_id.clone(), updated_position.clone());
        } else {
            borrow_positions.remove(repay_token_id);
        }

        updated_position
    }

    /// Updates isolated mode debt tracking after repayment
    ///
    /// # Arguments
    /// * `account_nonce` - NFT nonce of the account position
    /// * `principal_usd_amount` - USD value of principal being repaid
    ///
    /// For isolated positions (single collateral), updates the debt ceiling
    /// tracking for the collateral token. This ensures the debt ceiling
    /// is properly decreased when debt is repaid in isolated mode.
    fn handle_isolated_repay(&self, account_nonce: u64, principal_usd_amount: &BigUint) {
        let collaterals_map = self.deposit_positions(account_nonce);
        if collaterals_map.len() == 1 {
            let (collateral_token_id, _) = collaterals_map.iter().next().unwrap();
            let asset_config = self.asset_config(&collateral_token_id).get();

            if asset_config.is_isolated {
                self.update_isolated_debt_usd(
                    &collateral_token_id,
                    principal_usd_amount,
                    false, // is_decrease
                );
            }
        }
    }

    /// Processes complete repayment operation
    ///
    /// # Arguments
    /// * `account_nonce` - NFT nonce of the account position
    /// * `repay_token_id` - Token being repaid
    /// * `repay_amount` - Amount being repaid
    /// * `initial_caller` - Address initiating repayment
    /// * `repay_amount_in_egld` - Optional EGLD value of repayment (used in liquidations)
    /// * `debt_token_price_data` - Optional price data (used in liquidations)
    ///
    /// Orchestrates the entire repayment flow:
    /// 1. Validates position exists
    /// 2. Gets or uses provided price data
    /// 3. Calculates repayment amounts
    /// 4. Updates isolated mode debt if applicable
    /// 5. Processes repayment through liquidity pool
    /// 6. Emits position update event
    fn internal_repay(
        &self,
        account_nonce: u64,
        repay_token_id: &EgldOrEsdtTokenIdentifier,
        repay_amount: &BigUint,
        initial_caller: &ManagedAddress,
        repay_amount_in_egld: BigUint,
        debt_token_price_data: &PriceFeedShort<Self::Api>,
        storage_cache: &mut StorageCache<Self>,
    ) {
        // 1. Validate position exists
        let borrow_position = self.validate_repay_position(account_nonce, repay_token_id);

        // 3. Calculate repay amounts
        let principal_egld_amount = self.validate_and_get_repay_amounts(
            &borrow_position,
            &debt_token_price_data,
            repay_amount_in_egld,
        );

        // 4. Handle isolated mode debt update
        self.handle_isolated_repay(
            account_nonce,
            &self.get_token_amount_in_dollars_raw(
                &principal_egld_amount,
                &storage_cache.egld_price_feed,
            ),
        );

        // 5. Process repay and update position
        let updated_position = self.handle_repay_position(
            account_nonce,
            repay_token_id,
            repay_amount,
            initial_caller,
            borrow_position,
            &debt_token_price_data,
        );

        // 6. Emit event
        self.update_position_event(
            repay_amount,
            &updated_position,
            OptionalValue::Some(debt_token_price_data.price.clone()),
            OptionalValue::Some(initial_caller.clone()),
            OptionalValue::None,
        );
    }

    /// Calculates the protocol fee for a liquidation based on the bonus amount
    ///
    /// # Arguments
    /// * `collateral_amount_after_bonus` - Total collateral amount including the liquidation bonus
    /// * `collateral_amount_before_bonus` - Original collateral amount without the bonus
    /// * `asset_config` - Configuration of the collateral asset being liquidated
    /// * `health_factor` - Current health factor of the position
    ///
    /// # Returns
    /// * `BigUint` - Amount that goes to the protocol as fee
    fn calculate_liquidation_fees(
        &self,
        liq_bonus_amount: &BigUint,
        asset_config: &AssetConfig<Self::Api>,
        health_factor: &BigUint,
    ) -> BigUint {
        // Calculate dynamic protocol fee based on health factor
        let dynamic_fee = self.calculate_dynamic_protocol_fee(
            health_factor,
            asset_config.liquidation_base_fee.clone(),
        );

        // Calculate protocol's share of the bonus based on dynamic fee
        liq_bonus_amount * &dynamic_fee / &BigUint::from(BP)
    }

    /// Handles core liquidation logic
    ///
    /// # Arguments
    /// * `liquidatee_account_nonce` - NFT nonce of account being liquidated
    /// * `debt_payment` - Payment to cover debt
    /// * `collateral_to_receive` - Collateral token to receive
    /// * `initial_caller` - Address initiating liquidation
    /// * `asset_config_collateral` - Configuration of collateral asset
    ///
    /// # Returns
    /// * Tuple containing:
    ///   - health_factor: Current health factor
    ///   - debt_to_repay_amount: Amount of debt to repay
    ///   - collateral_amount_after_bonus: Collateral amount including bonus
    ///   - bonus_amount: Bonus amount
    ///   - repay_amount_in_egld: EGLD value of repayment
    ///   - debt_token_price_data: Price data for debt token
    ///
    /// Calculates liquidation amounts, handles excess payments,
    /// and determines collateral to receive including bonus.
    fn handle_liquidation(
        &self,
        liquidatee_account_nonce: u64,
        debt_payment: EgldOrEsdtTokenPayment<Self::Api>,
        collateral_to_receive: &EgldOrEsdtTokenIdentifier,
        initial_caller: &ManagedAddress,
        asset_config_collateral: &AssetConfig<Self::Api>,
        storage_cache: &mut StorageCache<Self>,
        nft_attributes: &NftAccountAttributes,
    ) -> (
        BigUint,                   // health_factor
        BigUint,                   // debt_to_repay_amount
        BigUint,                   // collateral_amount_after_bonus
        BigUint,                   // bonus_amount
        BigUint,                   // repay_amount_in_egld
        PriceFeedShort<Self::Api>, // debt_token_price_data
    ) {
        let debt_token_price_data =
            self.get_token_price_data(&debt_payment.token_identifier, storage_cache);
        let collateral_token_price_data =
            self.get_token_price_data(collateral_to_receive, storage_cache);
        let mut debt_payment_in_egld =
            self.get_token_amount_in_egld_raw(&debt_payment.amount, &debt_token_price_data);

        // Calculate liquidation bonus based on health factor
        let collateral_positions =
            self.update_collateral_with_interest(liquidatee_account_nonce, storage_cache, false);
        let borrow_positions =
            self.update_borrows_with_debt(liquidatee_account_nonce, storage_cache, false);

        let collateral_in_egld =
            self.get_liquidation_collateral_in_egld_vec(&collateral_positions, storage_cache);
        let borrowed_egld = self.get_total_borrow_in_egld_vec(&borrow_positions, storage_cache);

        let health_factor =
            self.validate_liquidation_health_factor(&collateral_in_egld, &borrowed_egld);

        let bonus_rate = self.calculate_dynamic_liquidation_bonus(
            &health_factor,
            &asset_config_collateral.liquidation_bonus,
            nft_attributes,
        );

        // Calculate liquidation amount using Dutch auction mechanism
        let liquidation_amount_egld = self.calculate_single_asset_liquidation_amount(
            &borrowed_egld,
            &collateral_in_egld,
            collateral_to_receive,
            &collateral_token_price_data,
            liquidatee_account_nonce,
            OptionalValue::Some(debt_payment_in_egld.clone()),
            &bonus_rate,
        );

        // Handle excess debt payment if any
        let (debt_to_repay_amount, excess_amount) =
            if debt_payment_in_egld > liquidation_amount_egld {
                let excess_in_egld = &debt_payment_in_egld - &liquidation_amount_egld;
                let excess_in_tokens =
                    self.compute_amount_in_tokens(&excess_in_egld, &debt_token_price_data);
                let used_tokens_for_debt = debt_payment.amount - &excess_in_tokens;
                debt_payment_in_egld = self
                    .get_token_amount_in_egld_raw(&used_tokens_for_debt, &debt_token_price_data);

                (used_tokens_for_debt, Some(excess_in_tokens))
            } else {
                (debt_payment.amount, None)
            };

        // Return excess if any
        if let Some(excess) = excess_amount {
            self.tx()
                .to(initial_caller)
                .payment(EgldOrEsdtTokenPayment::new(
                    debt_payment.token_identifier.clone(),
                    0,
                    excess,
                ))
                .transfer();
        }

        // Calculate collateral amounts
        let collateral_amount_before_bonus =
            self.compute_amount_in_tokens(&liquidation_amount_egld, &collateral_token_price_data);

        let collateral_amount_after_bonus = &collateral_amount_before_bonus
            * &(&BigUint::from(BP) + &bonus_rate)
            / &BigUint::from(BP);

        let bonus_amount = &collateral_amount_after_bonus - &collateral_amount_before_bonus;

        (
            health_factor,
            debt_to_repay_amount,
            collateral_amount_after_bonus,
            bonus_amount,
            debt_payment_in_egld,
            debt_token_price_data,
        )
    }

    /// Processes complete liquidation operation
    ///
    /// # Arguments
    /// * `liquidatee_account_nonce` - NFT nonce of account being liquidated
    /// * `debt_payment` - Payment to cover debt
    /// * `collateral_to_receive` - Collateral token to receive
    /// * `initial_caller` - Address initiating liquidation
    ///
    /// Orchestrates the entire liquidation flow:
    /// 1. Calculates liquidation amounts
    /// 2. Repays debt
    /// 3. Calculates and applies protocol fee
    /// 4. Transfers collateral to liquidator
    fn process_liquidation(
        &self,
        liquidatee_account_nonce: u64,
        debt_payment: EgldOrEsdtTokenPayment<Self::Api>,
        collateral_to_receive: &EgldOrEsdtTokenIdentifier,
        min_amount_to_receive: OptionalValue<BigUint>,
        initial_caller: &ManagedAddress,
    ) {
        let mut storage_cache = StorageCache::new(self);
        let asset_config_collateral = self.asset_config(collateral_to_receive).get();
        let nft_attributes = self.account_attributes(liquidatee_account_nonce).get();
        let (
            health_factor,
            debt_to_repay_amount,
            collateral_amount_after_bonus,
            bonus_amount,
            repay_amount_in_egld,
            debt_token_price_data,
        ) = self.handle_liquidation(
            liquidatee_account_nonce,
            debt_payment.clone(),
            collateral_to_receive,
            initial_caller,
            &asset_config_collateral,
            &mut storage_cache,
            &nft_attributes,
        );

        // Repay debt
        self.internal_repay(
            liquidatee_account_nonce,
            &debt_payment.token_identifier,
            &debt_to_repay_amount,
            initial_caller,
            repay_amount_in_egld,
            &debt_token_price_data,
            &mut storage_cache,
        );

        // Calculate protocol fee using pre-calculated values
        let protocol_fee_amount = self.calculate_liquidation_fees(
            &bonus_amount,
            &asset_config_collateral,
            &health_factor,
        );

        // Process withdrawal with protocol fee
        self.internal_withdraw(
            liquidatee_account_nonce,
            collateral_to_receive,
            collateral_amount_after_bonus,
            initial_caller,
            true,
            &protocol_fee_amount,
            &mut storage_cache,
            OptionalValue::Some(nft_attributes),
            min_amount_to_receive.into_option(),
        );
    }
}
