use crate::{cache::Cache, helpers, oracle, proxy_pool, storage, utils, validation};
use common_errors::{
    ERROR_ACCOUNT_ATTRIBUTES_MISMATCH, ERROR_ASSET_NOT_SUPPORTED_AS_COLLATERAL,
    ERROR_INVALID_NUMBER_OF_ESDT_TRANSFERS, ERROR_MIX_ISOLATED_COLLATERAL, ERROR_SUPPLY_CAP,
};
use common_structs::{
    AccountAttributes, AccountPosition, AccountPositionType, AssetConfig, PriceFeedShort,
};

use super::{account, emode, update};

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

#[multiversx_sc::module]
pub trait PositionDepositModule:
    storage::Storage
    + validation::ValidationModule
    + oracle::OracleModule
    + common_events::EventsModule
    + utils::LendingUtilsModule
    + helpers::MathsModule
    + account::PositionAccountModule
    + emode::EModeModule
    + common_math::SharedMathModule
    + update::PositionUpdateModule
    + common_rates::InterestRates
{
    /// Orchestrates deposit flow with e-mode validation, isolation constraints, and position updates.
    /// Validates each payment, checks supply caps, and calls liquidity pool for position scaling.
    /// Ensures compliance with risk parameters and market limits.
    fn process_deposit(
        &self,
        caller: &ManagedAddress,
        account_nonce: u64,
        position_attributes: AccountAttributes<Self::Api>,
        deposit_payments: &ManagedVec<EgldOrEsdtTokenPayment>,
        cache: &mut Cache<Self>,
    ) {
        let e_mode = self.e_mode_category(position_attributes.emode_id());
        self.ensure_e_mode_not_deprecated(&e_mode);

        // Validate position limits for all new positions in this transaction
        self.validate_bulk_position_limits(
            account_nonce,
            AccountPositionType::Deposit,
            deposit_payments,
        );

        for deposit_payment in deposit_payments {
            self.validate_payment(&deposit_payment);

            let mut asset_info = cache.cached_asset_info(&deposit_payment.token_identifier);
            let asset_emode_config = self.token_e_mode_config(
                position_attributes.emode_id(),
                &deposit_payment.token_identifier,
            );

            self.ensure_e_mode_compatible_with_asset(&asset_info, position_attributes.emode_id());
            self.apply_e_mode_to_asset_config(&mut asset_info, &e_mode, asset_emode_config);

            require!(
                asset_info.can_supply(),
                ERROR_ASSET_NOT_SUPPORTED_AS_COLLATERAL
            );

            self.validate_isolated_collateral(
                &deposit_payment.token_identifier,
                &asset_info,
                &position_attributes,
            );
            let price_feed = self.token_price(&deposit_payment.token_identifier, cache);
            self.validate_supply_cap(&asset_info, &deposit_payment, &price_feed, cache);

            self.update_deposit_position(
                account_nonce,
                &deposit_payment,
                &asset_info,
                caller,
                &position_attributes,
                &price_feed,
                cache,
            );
        }
    }

    /// Retrieves or creates a deposit position for a token.
    ///
    /// **Purpose**: Manages position lifecycle by either fetching existing deposit positions
    /// or initializing new ones with proper risk parameters.
    ///
    /// **Methodology**:
    /// - Attempts to retrieve existing position from storage
    /// - If none exists, creates new position with current asset configuration
    /// - Initializes position with liquidation parameters from asset config
    ///
    /// **Security Considerations**:
    /// - Uses latest asset risk parameters for new positions
    /// - Ensures consistent position structure across all deposits
    ///
    /// # Arguments
    /// - `account_nonce`: Position NFT nonce for storage mapping
    /// - `asset_info`: Deposited asset configuration containing risk parameters
    /// - `token_id`: Token identifier for position lookup
    ///
    /// # Returns
    /// - `AccountPosition` with current or newly initialized deposit position
    fn get_or_create_deposit_position(
        &self,
        account_nonce: u64,
        asset_info: &AssetConfig<Self::Api>,
        token_id: &EgldOrEsdtTokenIdentifier,
    ) -> AccountPosition<Self::Api> {
        self.positions(account_nonce, AccountPositionType::Deposit)
            .get(token_id)
            .unwrap_or_else(|| {
                AccountPosition::new(
                    AccountPositionType::Deposit,
                    token_id.clone(),
                    self.ray_zero(),
                    account_nonce,
                    asset_info.liquidation_threshold_bps.clone(),
                    asset_info.liquidation_bonus_bps.clone(),
                    asset_info.liquidation_fees_bps.clone(),
                    asset_info.loan_to_value_bps.clone(),
                )
            })
    }

    /// Updates a deposit position with a new deposit amount.
    ///
    /// **Purpose**: Executes the core deposit logic by updating position state,
    /// applying current risk parameters, and synchronizing with liquidity pools.
    ///
    /// **Methodology**:
    /// 1. Retrieves or creates position for the asset
    /// 2. Auto-upgrades risk parameters if they changed in asset config
    /// 3. Converts deposit amount to decimal format using asset decimals
    /// 4. Calls liquidity pool to update position with supply index scaling
    /// 5. Emits position update event for monitoring
    /// 6. Stores updated position in contract storage
    ///
    /// **Security Checks**:
    /// - Automatic risk parameter updates ensure latest safety margins
    /// - Position consistency validation across updates
    ///
    /// **Mathematical Operations**:
    /// - Decimal conversion: `amount * 10^(18 - asset_decimals)`
    /// - Supply index scaling in liquidity pool for interest accrual
    ///
    /// # Arguments
    /// - `account_nonce`: Position NFT nonce for storage mapping
    /// - `collateral`: Deposit payment containing token and amount
    /// - `asset_info`: Asset configuration with current risk parameters
    /// - `caller`: Depositor's address for event emission
    /// - `attributes`: NFT attributes for event logging
    /// - `feed`: Price feed for decimal conversion and valuation
    /// - `cache`: Mutable storage cache for pool addresses
    ///
    /// # Returns
    /// - `AccountPosition` with updated deposit position
    fn update_deposit_position(
        &self,
        account_nonce: u64,
        collateral: &EgldOrEsdtTokenPayment<Self::Api>,
        asset_info: &AssetConfig<Self::Api>,
        caller: &ManagedAddress,
        attributes: &AccountAttributes<Self::Api>,
        feed: &PriceFeedShort<Self::Api>,
        cache: &mut Cache<Self>,
    ) -> AccountPosition<Self::Api> {
        let mut position = self.get_or_create_deposit_position(
            account_nonce,
            asset_info,
            &collateral.token_identifier,
        );

        // Auto upgrade safe values when changed on demand
        if position.loan_to_value_bps != asset_info.loan_to_value_bps {
            position.loan_to_value_bps = asset_info.loan_to_value_bps.clone();
        }

        if position.liquidation_bonus_bps != asset_info.liquidation_bonus_bps {
            position.liquidation_bonus_bps = asset_info.liquidation_bonus_bps.clone();
        }

        if position.liquidation_fees_bps != asset_info.liquidation_fees_bps {
            position.liquidation_fees_bps = asset_info.liquidation_fees_bps.clone();
        }

        let amount_decimal = position.make_amount_decimal(&collateral.amount, feed.asset_decimals);

        self.update_market_position(
            &mut position,
            &collateral.amount,
            &collateral.token_identifier,
            feed,
            cache,
        );

        self.emit_position_update_event(
            cache,
            &amount_decimal,
            &position,
            feed.price_wad.clone(),
            caller,
            attributes,
        );

        // Update storage with the latest position
        self.store_updated_position(account_nonce, &position);

        position
    }

    /// Updates a market position via the liquidity pool.
    ///
    /// **Purpose**: Executes cross-contract call to liquidity pool for position updates,
    /// handling supply index calculations and interest accrual.
    ///
    /// **Methodology**:
    /// - Makes synchronous call to corresponding liquidity pool
    /// - Passes current position state and price for validation
    /// - Receives updated position with accrued interest and new scaled amounts
    ///
    /// **Security Considerations**:
    /// - Trusted cross-contract interaction with verified pool addresses
    /// - Price validation handled by liquidity pool contract
    ///
    /// **Mathematical Operations** (performed in pool):
    /// - Scaled amount calculation: `amount * supply_index / RAY_PRECISION`
    /// - Interest accrual based on time-weighted supply rate
    ///
    /// # Arguments
    /// - `position`: Current deposit position with existing scaled amounts
    /// - `amount`: Raw deposit amount to add to position
    /// - `token_id`: Token identifier for pool routing
    /// - `feed`: Price feed for pool-side validation
    /// - `cache`: Storage cache containing pool address mappings
    fn update_market_position(
        &self,
        position: &mut AccountPosition<Self::Api>,
        amount: &BigUint,
        token_id: &EgldOrEsdtTokenIdentifier,
        feed: &PriceFeedShort<Self::Api>,
        cache: &mut Cache<Self>,
    ) {
        *position = self
            .tx()
            .to(cache.cached_pool_address(token_id))
            .typed(proxy_pool::LiquidityPoolProxy)
            .supply(position.clone(), feed.price_wad.clone())
            .egld_or_single_esdt(token_id, 0, amount)
            .returns(ReturnsResult)
            .sync_call();
    }

    /// Validates deposit payments and handles NFT return.
    ///
    /// **Purpose**: Parses and validates incoming payments to separate position NFTs
    /// from collateral tokens, ensuring proper payment structure for deposits.
    ///
    /// **Methodology**:
    /// 1. Retrieves all transfer payments from call context
    /// 2. Validates caller address is not zero
    /// 3. Checks if first payment is account NFT:
    ///    - If NFT: validates account activity and attributes consistency
    ///    - If not NFT: handles accountless deposits or uses provided nonce
    /// 4. Optionally returns NFT to caller based on `return_nft` flag
    /// 5. Separates collateral payments from NFT payments
    ///
    /// **Security Checks**:
    /// - Zero address validation prevents invalid operations
    /// - Account activity validation ensures NFT exists in system
    /// - Attribute consistency check prevents tampering
    /// - Payment structure validation ensures proper token types
    ///
    /// # Arguments
    /// - `require_account_payment`: Whether NFT is mandatory for operation
    /// - `return_nft`: Whether to return NFT to caller after validation
    /// - `optional_account_nonce`: Optional account nonce for accountless operations
    ///
    /// # Returns
    /// - Tuple containing:
    ///   - `ManagedVec<EgldOrEsdtTokenPayment>`: Collateral payments to process
    ///   - `Option<EsdtTokenPayment>`: Account NFT payment if present
    ///   - `ManagedAddress`: Validated caller address
    ///   - `Option<AccountAttributes>`: Account attributes if NFT provided
    fn validate_supply_payment(
        &self,
        require_account_payment: bool,
        return_nft: bool,
        optional_account_nonce: OptionalValue<u64>,
    ) -> (
        ManagedVec<EgldOrEsdtTokenPayment<Self::Api>>,
        Option<EsdtTokenPayment<Self::Api>>,
        ManagedAddress,
        Option<AccountAttributes<Self::Api>>,
    ) {
        let caller = self.blockchain().get_caller();
        let payments = self.call_value().all_transfers();
        require!(!payments.is_empty(), ERROR_INVALID_NUMBER_OF_ESDT_TRANSFERS);

        self.require_non_zero_address(&caller);

        let first_payment = payments.get(0);
        let account_token = self.account().get_token_id();

        if account_token == first_payment.token_identifier {
            self.require_active_account(first_payment.token_nonce);

            let account_payment = first_payment.clone().unwrap_esdt();
            let account_attributes = self.nft_attributes(&account_payment);
            let stored_attributes = self.account_attributes(account_payment.token_nonce).get();

            require!(
                account_attributes == stored_attributes,
                ERROR_ACCOUNT_ATTRIBUTES_MISMATCH
            );

            if return_nft {
                // Refund NFT
                self.tx().to(&caller).payment(&account_payment).transfer();
            }

            (
                payments.slice(1, payments.len()).unwrap_or_default(),
                Some(account_payment),
                caller,
                Some(account_attributes),
            )
        } else {
            require!(
                !require_account_payment,
                ERROR_INVALID_NUMBER_OF_ESDT_TRANSFERS
            );

            match optional_account_nonce.into_option() {
                Some(account_nonce) => {
                    if account_nonce == 0 {
                        return (payments.clone(), None, caller, None);
                    }
                    self.require_active_account(account_nonce);
                    let stored_attributes = self.account_attributes(account_nonce).get();

                    return (
                        payments.clone(),
                        Some(EsdtTokenPayment::new(
                            account_token,
                            account_nonce,
                            BigUint::from(1u64),
                        )),
                        caller,
                        Some(stored_attributes),
                    );
                },
                None => (payments.clone(), None, caller, None),
            }
        }
    }

    /// Ensures isolated collateral constraints are met.
    ///
    /// **Purpose**: Enforces isolation mode rules to prevent mixing of isolated
    /// and non-isolated collaterals, maintaining risk isolation boundaries.
    ///
    /// **Methodology**:
    /// 1. Determines if either asset or position is isolated
    /// 2. If position is already isolated:
    ///    - Ensures new deposit matches existing isolated token
    ///    - Prevents switching between different isolated assets
    /// 3. If asset is isolated but position is not:
    ///    - Rejects deposit to prevent contamination
    /// 4. Allows non-isolated operations to proceed normally
    ///
    /// **Security Rationale**:
    /// - Isolated assets have specific risk profiles requiring separation
    /// - Mixing isolated collaterals could lead to correlation risks
    /// - Maintains predictable liquidation scenarios
    /// - Preserves debt ceiling effectiveness for isolated assets
    ///
    /// **Isolation Rules**:
    /// - One isolated asset per position maximum
    /// - Cannot mix isolated and non-isolated collaterals
    /// - Existing isolated positions can continue with same asset
    ///
    /// # Arguments
    /// - `token_id`: Token identifier being deposited
    /// - `asset_info`: Asset configuration containing isolation flag
    /// - `position_attributes`: NFT attributes with isolation state and token
    fn validate_isolated_collateral(
        &self,
        token_id: &EgldOrEsdtTokenIdentifier,
        asset_info: &AssetConfig<Self::Api>,
        position_attributes: &AccountAttributes<Self::Api>,
    ) {
        let is_isolated = asset_info.is_isolated() || position_attributes.is_isolated();
        if !is_isolated {
            return;
        }

        // Allow existing isolated positions to continue working even if asset becomes non-isolated
        if position_attributes.is_isolated() {
            // Position is isolated - ensure it's using the correct isolated token
            require!(
                position_attributes.isolated_token() == *token_id,
                ERROR_MIX_ISOLATED_COLLATERAL
            );
        } else if asset_info.is_isolated() {
            // Asset is isolated but position is not - not allowed
            require!(false, ERROR_MIX_ISOLATED_COLLATERAL);
        }
        // If neither is isolated, no further checks needed
    }

    /// Ensures a deposit respects the asset's supply cap.
    ///
    /// **Purpose**: Prevents excessive market concentration by enforcing maximum
    /// supply limits per asset, protecting against liquidity manipulation.
    ///
    /// **Methodology**:
    /// 1. Checks if asset has configured supply cap
    /// 2. Retrieves current total supply from liquidity pool
    /// 3. Converts scaled supply to actual amount using supply index
    /// 4. Validates new deposit won't exceed cap limit
    ///
    /// **Security Rationale**:
    /// - Prevents market manipulation through large deposits
    /// - Maintains liquidity diversity across assets
    /// - Protects against concentration risk
    /// - Ensures protocol stability under market stress
    ///
    /// **Mathematical Validation**:
    /// ```
    /// total_supplied = (total_supply_scaled * supply_index) / RAY_PRECISION
    /// require(total_supplied + deposit_amount <= supply_cap)
    /// ```
    ///
    /// # Arguments
    /// - `asset_info`: Asset configuration containing optional supply cap
    /// - `deposit_payment`: Deposit payment with amount to validate
    /// - `feed`: Price feed for decimal conversion
    /// - `cache`: Storage cache for pool address and market index access
    fn validate_supply_cap(
        &self,
        asset_info: &AssetConfig<Self::Api>,
        deposit_payment: &EgldOrEsdtTokenPayment,
        feed: &PriceFeedShort<Self::Api>,
        cache: &mut Cache<Self>,
    ) {
        match &asset_info.supply_cap_wad {
            Some(supply_cap) => {
                let pool = cache.cached_pool_address(&deposit_payment.token_identifier);
                let index = cache.cached_market_index(&deposit_payment.token_identifier);
                let total_supply_scaled = self.supplied(pool.clone()).get();
                let total_supplied = self.scaled_to_original(
                    &total_supply_scaled,
                    &index.supply_index_ray,
                    feed.asset_decimals,
                );

                require!(
                    total_supplied.into_raw_units() + &deposit_payment.amount <= *supply_cap,
                    ERROR_SUPPLY_CAP
                );
            },
            None => {
                // No supply cap set, do nothing
            },
        }
    }

    /// Updates position threshold (LTV or liquidation) parameters for an account.
    ///
    /// **Purpose**: Allows updating of risk parameters for existing positions,
    /// either for loan-to-value adjustments or liquidation threshold updates.
    ///
    /// **Methodology**:
    /// 1. Validates account exists and has deposit position for asset
    /// 2. Retrieves current e-mode configuration if applicable
    /// 3. Applies e-mode parameters to asset configuration
    /// 4. Updates either LTV parameters (safe) or liquidation thresholds (risky)
    /// 5. For risky updates: validates health factor remains above minimum
    /// 6. Emits position update event for monitoring
    ///
    /// **Security Considerations**:
    /// - Health factor validation prevents immediate liquidations
    /// - Requires minimum 5% health factor buffer for risky updates (safety factor of 20)
    /// - E-mode compatibility validation ensures proper parameter application
    ///
    /// **Risk Management**:
    /// - Safe updates: LTV, liquidation bonus, liquidation fees
    /// - Risky updates: liquidation threshold (requires health check)
    ///
    /// # Arguments
    /// - `account_nonce`: Position NFT nonce for validation and storage
    /// - `asset_id`: Asset identifier for position lookup
    /// - `has_risks`: Flag indicating if update affects liquidation threshold
    /// - `asset_config`: Updated asset configuration with new parameters
    /// - `cache`: Storage cache for health factor validation
    fn update_position_threshold(
        &self,
        account_nonce: u64,
        asset_id: &EgldOrEsdtTokenIdentifier<Self::Api>,
        has_risks: bool,
        asset_config: &mut AssetConfig<Self::Api>,
        controller_sc: &ManagedAddress,
        feed: &PriceFeedShort<Self::Api>,
        cache: &mut Cache<Self>,
    ) {
        self.require_active_account(account_nonce);
        let deposit_positions = self.positions(account_nonce, AccountPositionType::Deposit);
        let dp_option = deposit_positions.get(asset_id);
        if dp_option.is_none() {
            return;
        }

        let account_attributes = self.account_attributes(account_nonce).get();
        let e_mode_category = self.e_mode_category(account_attributes.emode_id());
        self.ensure_e_mode_not_deprecated(&e_mode_category);
        let asset_emode_config = self.token_e_mode_config(account_attributes.emode_id(), asset_id);
        self.apply_e_mode_to_asset_config(asset_config, &e_mode_category, asset_emode_config);

        let mut dp = unsafe { dp_option.unwrap_unchecked() };

        if has_risks {
            if dp.liquidation_threshold_bps != asset_config.liquidation_threshold_bps {
                dp.liquidation_threshold_bps = asset_config.liquidation_threshold_bps.clone();
            }
        } else {
            if dp.loan_to_value_bps != asset_config.loan_to_value_bps {
                dp.loan_to_value_bps = asset_config.loan_to_value_bps.clone();
            }

            if dp.liquidation_bonus_bps != asset_config.liquidation_bonus_bps {
                dp.liquidation_bonus_bps = asset_config.liquidation_bonus_bps.clone();
            }

            if dp.liquidation_fees_bps != asset_config.liquidation_fees_bps {
                dp.liquidation_fees_bps = asset_config.liquidation_fees_bps.clone();
            }
        }

        self.store_updated_position(account_nonce, &dp);

        if has_risks {
            self.validate_is_healthy(
                account_nonce,
                cache,
                Some(self.to_decimal(BigUint::from(20u64), 0usize)),
            );
        }

        self.emit_position_update_event(
            cache,
            &dp.zero_decimal(),
            &dp,
            feed.price_wad.clone(),
            &controller_sc,
            &account_attributes,
        );
    }
}
