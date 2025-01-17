multiversx_sc::imports!();
use common_constants::BP;
use common_events::{
    AccountPosition, AssetConfig, EModeAssetConfig, EModeCategory, NftAccountAttributes,
};
use common_events::{AccountPositionType, PriceFeedShort};

use crate::contexts::base::StorageCache;
use crate::helpers;
use crate::oracle;
use crate::proxy_pool;
use crate::storage;
use crate::utils;
use crate::validation;

#[multiversx_sc::module]
pub trait PositionModule:
    storage::LendingStorageModule
    + validation::ValidationModule
    + oracle::OracleModule
    + common_events::EventsModule
    + utils::LendingUtilsModule
    + helpers::math::MathsModule
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
    fn update_debt(
        &self,
        account_position: u64,
        storage_cache: &mut StorageCache<Self>,
        fetch_price: bool,
        return_map: bool,
    ) -> (
        ManagedVec<AccountPosition<Self::Api>>,
        ManagedMap<Self::Api>,
    ) {
        let borrow_positions = self.borrow_positions(account_position);
        let mut positions: ManagedVec<Self::Api, AccountPosition<Self::Api>> = ManagedVec::new();
        let mut index_position = ManagedMap::new();
        for (index, token_id) in borrow_positions.keys().enumerate() {
            let mut bp = borrow_positions.get(&token_id).unwrap();
            let asset_address = self.get_pool_address(&bp.token_id);
            let price = if fetch_price {
                let result = self.get_token_price(&bp.token_id, storage_cache);
                OptionalValue::Some(result.price)
            } else {
                OptionalValue::None
            };

            self.update_position(&asset_address, &mut bp, price);

            if fetch_price {
                self.update_position_event(
                    &BigUint::zero(),
                    &bp,
                    OptionalValue::None,
                    OptionalValue::None,
                    OptionalValue::None,
                );
            }

            if return_map {
                index_position.put(
                    &bp.token_id.clone().into_name(),
                    &ManagedBuffer::new_from_bytes(&index.to_be_bytes()),
                );
            }

            self.borrow_positions(account_position)
                .insert(bp.token_id.clone(), bp.clone());

            positions.push(bp.clone());
        }
        (positions, index_position)
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
    fn update_interest(
        &self,
        account_position: u64,
        storage_cache: &mut StorageCache<Self>,
        fetch_price: bool,
    ) -> ManagedVec<AccountPosition<Self::Api>> {
        let positions_map = self.deposit_positions(account_position);
        let mut positions: ManagedVec<Self::Api, AccountPosition<Self::Api>> = ManagedVec::new();
        for mut dp in positions_map.values() {
            let asset_address = self.get_pool_address(&dp.token_id);
            if !dp.is_vault {
                let price = if fetch_price {
                    let result = self.get_token_price(&dp.token_id, storage_cache);
                    OptionalValue::Some(result.price)
                } else {
                    OptionalValue::None
                };
                self.update_position(&asset_address, &mut dp, price);

                if fetch_price {
                    self.update_position_event(
                        &BigUint::zero(),
                        &dp,
                        OptionalValue::None,
                        OptionalValue::None,
                        OptionalValue::None,
                    );
                }
                self.deposit_positions(account_position)
                    .insert(dp.token_id.clone(), dp.clone());

                positions.push(dp);
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
    /// * `caller` - Address of the user supplying assets
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
    fn get_or_create_account(
        &self,
        caller: &ManagedAddress,
        is_isolated: bool,
        is_vault: bool,
        e_mode_category: OptionalValue<u8>,
        account_nonce: Option<EgldOrEsdtTokenPayment<Self::Api>>,
    ) -> (u64, NftAccountAttributes) {
        if let Some(account) = account_nonce {
            self.require_active_account(account.token_nonce);

            let token_id = self.account_token().get_token_id();
            let attributes = self.nft_attributes(account.token_nonce, &token_id);

            // Return NFT to owner after validation
            self.tx()
                .to(caller)
                .single_esdt(&token_id, account.token_nonce, &BigUint::from(1u64))
                .transfer();
            (account.token_nonce, attributes)
        } else {
            let (payment, attributes) = self.enter(caller, is_isolated, is_vault, e_mode_category);
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
    fn get_or_create_deposit_position(
        &self,
        account_nonce: u64,
        asset_info: &AssetConfig<Self::Api>,
        token_id: &EgldOrEsdtTokenIdentifier,
        is_vault: bool,
    ) -> AccountPosition<Self::Api> {
        let mut positions = self.deposit_positions(account_nonce);

        if let Some(position) = positions.get(token_id) {
            positions.remove(token_id);
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
                asset_info.liquidation_base_bonus.clone(),
                asset_info.liquidation_max_fee.clone(),
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
        collateral: &EgldOrEsdtTokenPayment<Self::Api>,
        asset_info: &AssetConfig<Self::Api>,
        is_vault: bool,
        feed: &PriceFeedShort<Self::Api>,
        caller: &ManagedAddress,
        attributes: &NftAccountAttributes,
    ) -> AccountPosition<Self::Api> {
        let mut position = self.get_or_create_deposit_position(
            account_nonce,
            asset_info,
            &collateral.token_identifier,
            is_vault,
        );

        if position.entry_ltv != asset_info.ltv {
            position.entry_ltv = asset_info.ltv.clone();
        }

        if is_vault {
            self.increase_vault_position(
                &mut position,
                &collateral.amount,
                &collateral.token_identifier,
            );
        } else {
            self.update_market_position(
                &mut position,
                &collateral.amount,
                &collateral.token_identifier,
                &feed,
            );
        }

        self.update_position_event(
            &collateral.amount,
            &position,
            OptionalValue::Some(feed.price.clone()),
            OptionalValue::Some(caller),
            OptionalValue::Some(attributes),
        );

        // Update storage with the latest position
        self.deposit_positions(account_nonce)
            .insert(collateral.token_identifier.clone(), position.clone());

        position
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
        feed: &PriceFeedShort<Self::Api>,
    ) {
        let pool_address = self.get_pool_address(token_id);

        *position = self
            .tx()
            .to(pool_address)
            .typed(proxy_pool::LiquidityPoolProxy)
            .supply(position.clone(), &feed.price)
            .payment(EgldOrEsdtTokenPayment::new(
                token_id.clone(),
                0,
                amount.clone(),
            ))
            .returns(ReturnsResult)
            .sync_call();
    }

    /// Increase vault position directly in storage
    ///
    /// # Arguments
    /// * `position` - Current position to update
    /// * `amount` - Amount being deposited
    /// * `token_id` - Token identifier
    ///
    /// Increase vault supplied amount in storage and position balance.
    /// Used for vault positions that don't accrue interest.
    /// Emits event for tracking vault deposits.
    fn increase_vault_position(
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
        category: &Option<EModeCategory<Self::Api>>,
        asset_emode_config: Option<EModeAssetConfig>,
    ) {
        if let (Some(category), Some(asset_emode_config)) = (category, asset_emode_config) {
            // Update all asset config parameters with e-mode values for that category
            asset_info.can_be_collateral = asset_emode_config.can_be_collateral;
            asset_info.can_be_borrowed = asset_emode_config.can_be_borrowed;
            asset_info.ltv = category.ltv.clone();
            asset_info.liquidation_threshold = category.liquidation_threshold.clone();
            asset_info.liquidation_base_bonus = category.liquidation_bonus.clone();
        }
    }

    /// Processes withdrawal from a position
    ///
    /// # Arguments
    /// * `account_nonce` - NFT nonce of the account position
    /// * `withdraw_token_id` - Token to withdraw
    /// * `amount` - Amount to withdraw
    /// * `caller` - Address initiating withdrawal
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
    fn _withdraw(
        &self,
        account_nonce: u64,
        withdraw_token_id: &EgldOrEsdtTokenIdentifier,
        mut amount: BigUint,
        caller: &ManagedAddress,
        is_liquidation: bool,
        liquidation_fee: &BigUint,
        storage_cache: &mut StorageCache<Self>,
        attributes: &NftAccountAttributes,
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

        let asset_data = self.get_token_price(withdraw_token_id, storage_cache);
        let position = if dp.is_vault {
            // Cap withdraw amount to available balance
            if amount > dp.get_total_amount() {
                amount = dp.get_total_amount();
            }

            let last_value = self.vault_supplied_amount(withdraw_token_id).update(|am| {
                *am -= &amount;
                am.clone()
            });

            self.update_vault_supplied_amount_event(withdraw_token_id, last_value);

            dp.amount -= &amount;

            if is_liquidation {
                let liquidated_amount_after_fees = &(&amount - liquidation_fee);
                self.tx()
                    .to(caller)
                    .payment(EgldOrEsdtTokenPayment::new(
                        withdraw_token_id.clone(),
                        0,
                        liquidated_amount_after_fees.clone(),
                    ))
                    .transfer();

                self.tx()
                    .to(pool_address)
                    .typed(proxy_pool::LiquidityPoolProxy)
                    .add_external_protocol_revenue(&asset_data.price)
                    .egld_or_single_esdt(withdraw_token_id, 0, liquidation_fee)
                    .returns(ReturnsResult)
                    .sync_call();
            } else {
                self.tx()
                    .to(caller)
                    .payment(EgldOrEsdtTokenPayment::new(
                        withdraw_token_id.clone(),
                        0,
                        amount.clone(),
                    ))
                    .transfer();
            };

            dp
        } else {
            self.tx()
                .to(pool_address)
                .typed(proxy_pool::LiquidityPoolProxy)
                .withdraw(
                    caller,
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
            &position,
            OptionalValue::Some(asset_data.price),
            OptionalValue::Some(&caller),
            OptionalValue::Some(&attributes),
        );

        if position.get_total_amount().gt(&BigUint::zero()) {
            dep_pos_map.insert(withdraw_token_id.clone(), position.clone());
        } else {
            dep_pos_map.remove(withdraw_token_id);
        }

        position
    }

    /// Handles NFT after withdrawal operation
    ///
    /// # Arguments
    /// * `account_token` - NFT token payment
    /// * `caller` - Address initiating withdrawal
    ///
    /// If no positions remain (no deposits or borrows),
    /// burns the NFT and removes from storage.
    /// Otherwise returns NFT to caller.
    fn handle_nft_after_withdraw(
        &self,
        amount: &BigUint,
        token_nonce: u64,
        token_identifier: &TokenIdentifier,
        caller: &ManagedAddress,
    ) {
        let dep_pos_map = self.deposit_positions(token_nonce).len();
        let borrow_pos_map = self.borrow_positions(token_nonce).len();

        if dep_pos_map == 0 && borrow_pos_map == 0 {
            self.account_token().nft_burn(token_nonce, amount);
            self.account_positions().swap_remove(&token_nonce);
            self.account_attributes(token_nonce).clear();
        } else {
            self.tx()
                .to(caller)
                .single_esdt(token_identifier, token_nonce, amount)
                .transfer();
        }
    }

    /// Processes borrow operation
    ///
    /// # Arguments
    /// * `account_nonce` - NFT nonce of the account position
    /// * `asset_to_borrow` - Token to borrow
    /// * `amount` - Amount to borrow
    /// * `amount_in_usd` - USD value of borrow
    /// * `caller` - Address initiating borrow
    /// * `asset_config` - Asset configuration
    /// * `account` - Position NFT attributes
    /// * `collaterals` - Current collateral positions
    /// * `feed` - Price data for the asset being borrowed
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
        caller: &ManagedAddress,
        asset_config: &AssetConfig<Self::Api>,
        account: &NftAccountAttributes,
        collaterals: &ManagedVec<AccountPosition<Self::Api>>,
        feed: &PriceFeedShort<Self::Api>,
    ) -> AccountPosition<Self::Api> {
        let pool_address = self.get_pool_address(asset_to_borrow);

        // Get or create borrow position
        let mut borrow_position = self.get_or_create_borrow_position(
            account_nonce,
            asset_config,
            asset_to_borrow.clone(),
            account.is_vault,
        );

        // Execute borrow
        borrow_position = self
            .tx()
            .to(pool_address)
            .typed(proxy_pool::LiquidityPoolProxy)
            .borrow(caller, amount, borrow_position, &feed.price)
            .returns(ReturnsResult)
            .sync_call();

        // Handle isolated mode debt ceiling
        if account.is_isolated {
            let collateral_token_id = &collaterals.get(0).token_id;
            let collateral_config = self.asset_config(collateral_token_id).get();

            self.validate_isolated_debt_ceiling(
                &collateral_config,
                collateral_token_id,
                &amount_in_usd,
            );
            self.update_isolated_debt_usd(
                collateral_token_id,
                &amount_in_usd,
                true, // is_increase
            );
        }

        // Update storage
        self.borrow_positions(account_nonce)
            .insert(asset_to_borrow.clone(), borrow_position.clone());

        borrow_position
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
    fn get_or_create_borrow_position(
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
                asset_info.liquidation_base_bonus.clone(),
                asset_info.liquidation_max_fee.clone(),
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
    /// * `caller` - Address initiating repayment
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
        caller: &ManagedAddress,
        mut borrow_position: AccountPosition<Self::Api>,
        debt_token_price_data: &PriceFeedShort<Self::Api>,
        attributes: &NftAccountAttributes,
    ) {
        let asset_address = self.get_pool_address(repay_token_id);
        borrow_position = self
            .tx()
            .to(asset_address)
            .typed(proxy_pool::LiquidityPoolProxy)
            .repay(
                caller,
                borrow_position.clone(),
                &debt_token_price_data.price,
            )
            .egld_or_single_esdt(repay_token_id, 0, repay_amount)
            .returns(ReturnsResult)
            .sync_call();

        // Update storage
        let mut borrow_positions = self.borrow_positions(account_nonce);

        self.update_position_event(
            repay_amount,
            &borrow_position,
            OptionalValue::Some(debt_token_price_data.price.clone()),
            OptionalValue::Some(&caller),
            OptionalValue::Some(attributes),
        );

        if borrow_position.get_total_amount().gt(&BigUint::zero()) {
            borrow_positions.insert(repay_token_id.clone(), borrow_position);
        } else {
            borrow_positions.remove(repay_token_id);
        }
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
    fn handle_isolated_repay(
        &self,
        account_nonce: u64,
        position: &mut AccountPosition<Self::Api>,
        feed: &PriceFeedShort<Self::Api>,
        repay_amount: &BigUint,
        storage_cache: &mut StorageCache<Self>,
        attributes: &NftAccountAttributes,
    ) {
        if attributes.is_isolated {
            let collaterals_map = self.deposit_positions(account_nonce);
            let (collateral_token_id, _) = collaterals_map.iter().next().unwrap();

            // 3. Calculate repay amounts
            let asset_address = self.pools_map(&position.token_id).get();

            self.update_position(
                &asset_address,
                position,
                OptionalValue::Some(feed.price.clone()),
            );

            let principal_amount =
                self.validate_and_get_repay_amounts(&position, &feed, repay_amount);

            let debt_usd_amount = self
                .get_token_amount_in_dollars_raw(&principal_amount, &storage_cache.egld_price_feed);

            self.update_isolated_debt_usd(
                &collateral_token_id,
                &debt_usd_amount,
                false, // is_decrease
            );
        }
    }

    /// Processes complete repayment operation
    ///
    /// # Arguments
    /// * `account_nonce` - NFT nonce of the account position
    /// * `repay_token_id` - Token being repaid
    /// * `repay_amount` - Amount being repaid
    /// * `caller` - Address initiating repayment
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
    fn _repay(
        &self,
        account_nonce: u64,
        repay_token_id: &EgldOrEsdtTokenIdentifier,
        repay_amount: &BigUint,
        caller: &ManagedAddress,
        repay_amount_in_egld: BigUint,
        debt_token_price_data: &PriceFeedShort<Self::Api>,
        storage_cache: &mut StorageCache<Self>,
        attributes: &NftAccountAttributes,
    ) {
        // 1. Validate position exists
        let mut borrow_position = self.validate_borrow_position(account_nonce, repay_token_id);

        // 2. Handle isolated mode debt update
        self.handle_isolated_repay(
            account_nonce,
            &mut borrow_position,
            &debt_token_price_data,
            &repay_amount_in_egld,
            storage_cache,
            attributes,
        );

        // 3. Process repay and update position
        self.handle_repay_position(
            account_nonce,
            repay_token_id,
            repay_amount,
            caller,
            borrow_position,
            &debt_token_price_data,
            attributes,
        );
    }

    /// Handles core liquidation logic
    ///
    /// # Arguments
    /// * `liquidatee_account_nonce` - NFT nonce of account being liquidated
    /// * `debt_payment` - Payment to cover debt
    /// * `collateral_to_receive` - Collateral token to receive
    /// * `caller` - Address initiating liquidation
    /// * `asset_config_collateral` - Configuration of collateral asset
    ///
    /// Calculates liquidation amounts, handles excess payments,
    /// and determines collateral to receive including bonus.
    fn handle_liquidation(
        &self,
        account_nonce: u64,
        payments: &ManagedVec<EgldOrEsdtTokenPayment<Self::Api>>,
        caller: &ManagedAddress,
        storage_cache: &mut StorageCache<Self>,
    ) -> (
        ManagedVec<MultiValue2<EgldOrEsdtTokenPayment, BigUint>>, // Collateral seized
        ManagedVec<MultiValue3<EgldOrEsdtTokenPayment, BigUint, PriceFeedShort<Self::Api>>>, // Repaid tokens, egld value, price feed of each
    ) {
        let (debt_payment_in_egld, mut repaid_tokens) =
            self.sum_repayments(payments, storage_cache);

        let collaterals = self.update_interest(account_nonce, storage_cache, false);
        let (borrows, _) = self.update_debt(account_nonce, storage_cache, false, false);

        let (liquidation_collateral, total_collateral, _) =
            self.sum_collaterals(&collaterals, storage_cache);

        let (proportional_weighted, bonus_weighted) =
            self.proportion_of_weighted_seized(&total_collateral, &collaterals, storage_cache);

        let borrowed_egld = self.sum_borrows(&borrows, storage_cache);

        let health_factor = self.validate_can_liquidate(&liquidation_collateral, &borrowed_egld);

        // Calculate liquidation amount using Dutch auction mechanism
        let (max_debt_to_repay, bonus_rate) = self.calculate_max_debt_repayment(
            borrowed_egld,
            &total_collateral,
            liquidation_collateral,
            &proportional_weighted,
            &bonus_weighted,
            &health_factor,
            OptionalValue::Some(debt_payment_in_egld.clone()),
        );

        // Handle excess debt payment if any
        let refund_debt = if debt_payment_in_egld > max_debt_to_repay {
            let mut excess_in_egld = &debt_payment_in_egld - &max_debt_to_repay;
            let mut refunds = ManagedVec::new();

            for index in 0..repaid_tokens.len() {
                if excess_in_egld == BigUint::zero() {
                    break;
                }
                let (mut debt_payment, mut egld_asset_amount, price_feed) =
                    repaid_tokens.get(index).clone().into_tuple();

                if egld_asset_amount >= excess_in_egld {
                    // This flow is when the amount repaid is higher than the maximum possible
                    // We calculate how much we repaid in EGLD, then we convert to the original token
                    // We deduct from the repayment vec the amount and push to the refunds vec what needs to be sent back.
                    let excess_in_original =
                        self.compute_amount_in_tokens(&excess_in_egld, &price_feed);
                    debt_payment.amount -= &excess_in_original;
                    egld_asset_amount -= &excess_in_egld;
                    refunds.push(EgldOrEsdtTokenPayment::new(
                        debt_payment.token_identifier.clone(),
                        0,
                        excess_in_original,
                    ));
                    let _ = repaid_tokens
                        .set(index, (debt_payment, egld_asset_amount, price_feed).into());

                    excess_in_egld = BigUint::zero();
                } else {
                    // This flow is when the excess amount is more than the entire amount of this token, then refund the entire token sent
                    // it can happen only when there is a bulk repayment of different debts in the same position
                    refunds.push(debt_payment);
                    let _ = repaid_tokens.remove(index);
                    excess_in_egld -= egld_asset_amount;
                }
            }

            Some(refunds)
        } else {
            None
        };

        // Return excess if any
        if let Some(refunds) = refund_debt {
            self.tx()
                .to(caller)
                .payment(refunds)
                .transfer_if_not_empty();
        }

        let seized_collaterals = self.seize_collateral_proportionally(
            &collaterals,
            &total_collateral,
            &max_debt_to_repay,
            &bonus_rate,
            storage_cache,
        );

        (seized_collaterals, repaid_tokens)
    }

    /// Processes complete liquidation operation
    ///
    /// # Arguments
    /// * `liquidatee_account_nonce` - NFT nonce of account being liquidated
    /// * `debt_payment` - Payment to cover debt
    /// * `collateral_to_receive` - Collateral token to receive
    /// * `caller` - Address initiating liquidation
    ///
    /// Orchestrates the entire liquidation flow:
    /// 1. Calculates liquidation amounts
    /// 2. Repays debt
    /// 3. Calculates and applies protocol fee
    /// 4. Transfers collateral to liquidator
    fn process_liquidation(
        &self,
        liquidatee_account_nonce: u64,
        debt_payments: &ManagedVec<EgldOrEsdtTokenPayment<Self::Api>>,
        caller: &ManagedAddress,
    ) {
        let mut storage_cache = StorageCache::new(self);
        storage_cache.allow_unsafe_price = false;

        let account = self.account_attributes(liquidatee_account_nonce).get();

        let (seized_collaterals, repaid_tokens) = self.handle_liquidation(
            liquidatee_account_nonce,
            debt_payments,
            caller,
            &mut storage_cache,
        );

        for debt_payment_data in repaid_tokens {
            let (debt_payment, debt_egld_value, debt_price_feeed) = debt_payment_data.into_tuple();
            // // Repay debt
            self._repay(
                liquidatee_account_nonce,
                &debt_payment.token_identifier,
                &debt_payment.amount,
                caller,
                debt_egld_value,
                &debt_price_feeed,
                &mut storage_cache,
                &account,
            );
        }

        for collateral_data in seized_collaterals {
            let (seized_payment, protocol_fee) = collateral_data.into_tuple();
            sc_print!("Seized {}, Amount {}, Fee {}", (seized_payment.token_identifier), (seized_payment.amount), protocol_fee);
            // // Process withdrawal with protocol fee
            self._withdraw(
                liquidatee_account_nonce,
                &seized_payment.token_identifier,
                seized_payment.amount,
                caller,
                true,
                &protocol_fee,
                &mut storage_cache,
                &account,
            );
        }
    }
}
