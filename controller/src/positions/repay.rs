use common_constants::WAD_PRECISION;
use common_structs::{AccountAttributes, AccountPosition, AccountPositionType, PriceFeedShort};

use crate::{cache::Cache, helpers, oracle, proxy_pool, storage, utils, validation};

use super::{account, borrow, emode, update};

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

#[multiversx_sc::module]
pub trait PositionRepayModule:
    storage::Storage
    + validation::ValidationModule
    + oracle::OracleModule
    + common_events::EventsModule
    + utils::LendingUtilsModule
    + helpers::MathsModule
    + account::PositionAccountModule
    + borrow::PositionBorrowModule
    + update::PositionUpdateModule
    + common_math::SharedMathModule
    + common_rates::InterestRates
    + emode::EModeModule
{
    /// Updates isolated debt tracking post-repayment.
    ///
    /// **Purpose**: Reduces the tracked debt amount for isolated collateral positions
    /// after successful repayment, maintaining accurate debt ceiling accounting.
    ///
    /// **Methodology**:
    /// 1. Checks if position is in isolation mode
    /// 2. Converts repayment amount from EGLD to USD value
    /// 3. Decreases global isolated debt tracking for the collateral token
    ///
    /// **Security Considerations**:
    /// - Only processes debt reduction for confirmed isolated positions
    /// - Uses current EGLD/USD price for accurate value conversion
    /// - Maintains debt ceiling integrity across repayments
    ///
    /// **Mathematical Operations**:
    /// ```
    /// usd_value = repay_amount_egld * egld_usd_price / USD_PRECISION
    /// isolated_debt[token] -= usd_value
    /// ```
    ///
    /// # Arguments
    /// - `repay_amount`: Repayment amount in EGLD denomination
    /// - `cache`: Storage cache for EGLD/USD price access
    /// - `position_attributes`: Position attributes containing isolation token
    fn update_isolated_debt_after_repayment(
        &self,
        position: &AccountPosition<Self::Api>,
        repay_amount_egld: &ManagedDecimal<Self::Api, NumDecimals>,
        feed: &PriceFeedShort<Self::Api>,
        cache: &mut Cache<Self>,
        position_attributes: &AccountAttributes<Self::Api>,
    ) {
        if position_attributes.is_isolated() {
            // Compute current outstanding debt for this borrow position in EGLD (WAD)
            let current_debt_ray = self.total_amount_ray(position, cache);
            let current_debt_egld_ray =
                self.token_egld_value_ray(&current_debt_ray, &feed.price_wad);
            let current_debt_egld_wad = self.rescale_half_up(&current_debt_egld_ray, WAD_PRECISION);

            // Apply only the portion that actually reduces this borrow
            let applied_egld_wad = self.min(current_debt_egld_wad, repay_amount_egld.clone());

            // Convert applied repayment to USD and decrease the tracker
            let debt_usd_amount = self.egld_usd_value(&applied_egld_wad, &cache.egld_usd_price_wad);
            self.adjust_isolated_debt_usd(
                &position_attributes.isolated_token(),
                debt_usd_amount,
                false,
            );
        }
    }

    /// Clears all isolated debt for a position being fully repaid.
    ///
    /// **Purpose**: Removes all tracked isolated debt when a borrow position
    /// is completely closed, ensuring accurate debt ceiling accounting.
    ///
    /// **Methodology**:
    /// 1. Calculates total position debt including accrued interest
    /// 2. Converts debt to EGLD value using current price
    /// 3. Converts EGLD to USD for debt ceiling tracking
    /// 4. Removes full amount from isolated debt tracking
    ///
    /// **Security Considerations**:
    /// - Uses current market prices for accurate valuation
    /// - Includes accrued interest in debt calculation
    /// - Maintains debt ceiling integrity
    ///
    /// **Mathematical Operations**:
    /// ```
    /// total_debt = position.scaled_amount * borrow_index / RAY_PRECISION
    /// egld_value = total_debt * asset_price / RAY_PRECISION
    /// usd_value = egld_value * egld_usd_price / USD_PRECISION
    /// isolated_debt[token] -= usd_value
    /// ```
    ///
    /// # Arguments
    /// - `position`: Borrow position being cleared
    /// - `feed`: Price feed for debt valuation
    /// - `position_attributes`: Position attributes with isolation settings
    /// - `cache`: Storage cache for price and index access
    fn clear_position_isolated_debt(
        &self,
        position: &mut AccountPosition<Self::Api>,
        feed: &PriceFeedShort<Self::Api>,
        position_attributes: &AccountAttributes<Self::Api>,
        cache: &mut Cache<Self>,
    ) {
        if position_attributes.is_isolated() {
            let amount = self.total_amount_ray(position, cache);
            let egld_amount = self.token_egld_value_ray(&amount, &feed.price_wad);
            let debt_usd_amount = self.egld_usd_value(&egld_amount, &cache.egld_usd_price_wad);
            self.adjust_isolated_debt_usd(
                &position_attributes.isolated_token(),
                debt_usd_amount,
                false,
            );
        }
    }

    /// Manages the full repayment process.
    ///
    /// **Purpose**: Orchestrates the complete repayment flow including debt validation,
    /// isolated debt tracking updates, and position state management.
    ///
    /// **Methodology**:
    /// 1. Validates borrow position exists for the specified token
    /// 2. Updates isolated debt tracking if position is isolated
    /// 3. Executes repayment through liquidity pool contract
    /// 4. Emits position update event for monitoring
    /// 5. Updates or removes position based on remaining debt
    ///
    /// **Security Checks**:
    /// - Position existence validation prevents invalid repayments
    /// - Isolated debt tracking maintains ceiling accuracy
    /// - Cross-contract interaction with verified pool addresses
    ///
    /// **Mathematical Operations** (performed in pool):
    /// ```
    /// interest_accrued = position.scaled_amount * (borrow_index - position.last_index)
    /// total_debt = position.amount + interest_accrued
    /// repayment_applied = min(repay_amount, total_debt)
    /// new_debt = total_debt - repayment_applied
    /// ```
    ///
    /// # Arguments
    /// - `account_nonce`: Position NFT nonce for storage operations
    /// - `token_id`: Token identifier being repaid
    /// - `repay_amount`: Repayment amount in asset decimals
    /// - `caller`: Repayer's address for token transfer
    /// - `repay_amount_in_egld`: EGLD value for isolated debt tracking
    /// - `feed`: Price feed for valuation
    /// - `cache`: Storage cache for pool address lookup
    /// - `position_attributes`: Position attributes for isolation handling
    fn process_repayment(
        &self,
        account_nonce: u64,
        token_id: &EgldOrEsdtTokenIdentifier,
        repay_amount: &ManagedDecimal<Self::Api, NumDecimals>,
        caller: &ManagedAddress,
        repay_amount_in_egld: ManagedDecimal<Self::Api, NumDecimals>,
        feed: &PriceFeedShort<Self::Api>,
        cache: &mut Cache<Self>,
        position_attributes: &AccountAttributes<Self::Api>,
    ) {
        let mut borrow_position = self.validate_borrow_position_existence(account_nonce, token_id);

        // Update isolated debt tracker using the applied portion of this repayment,
        // computed internally only when the position is isolated.
        self.update_isolated_debt_after_repayment(
            &borrow_position,
            &repay_amount_in_egld,
            feed,
            cache,
            position_attributes,
        );

        let pool_address = cache.cached_pool_address(token_id);

        borrow_position = self
            .tx()
            .to(pool_address)
            .typed(proxy_pool::LiquidityPoolProxy)
            .repay(caller, borrow_position.clone(), feed.price_wad.clone())
            .egld_or_single_esdt(token_id, 0, repay_amount.into_raw_units())
            .returns(ReturnsResult)
            .sync_call();

        self.emit_position_update_event(
            cache,
            repay_amount,
            &borrow_position,
            feed.price_wad.clone(),
            caller,
            position_attributes,
        );

        self.update_or_remove_position(account_nonce, &borrow_position);
    }

    /// Ensures a borrow position exists for repayment.
    ///
    /// **Purpose**: Validates that a borrow position exists for the specified token
    /// before allowing repayment operations, preventing invalid transactions.
    ///
    /// **Methodology**:
    /// - Retrieves borrow positions mapping for the account
    /// - Attempts to find position for the specified token
    /// - Validates position exists with clear error messaging
    ///
    /// **Security Considerations**:
    /// - Prevents repayment attempts on non-existent debt
    /// - Provides clear error messaging for debugging
    /// - Uses unsafe unwrap only after existence validation
    ///
    /// # Arguments
    /// - `account_nonce`: Position NFT nonce for storage lookup
    /// - `token_id`: Borrowed token identifier to validate
    ///
    /// # Returns
    /// - `AccountPosition` containing the validated borrow position
    fn validate_borrow_position_existence(
        &self,
        account_nonce: u64,
        token_id: &EgldOrEsdtTokenIdentifier,
    ) -> AccountPosition<Self::Api> {
        let borrow_positions = self.positions(account_nonce, AccountPositionType::Borrow);
        let position = borrow_positions.get(token_id);
        require!(
            position.is_some(),
            "No borrow position exists for token {} in this account",
            token_id
        );
        unsafe { position.unwrap_unchecked() }
    }
}
