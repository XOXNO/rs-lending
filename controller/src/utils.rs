multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use crate::cache::Cache;
use crate::{helpers, oracle, storage, ERROR_NO_POOL_FOUND};
pub use common_constants::{BPS_PRECISION, RAY_PRECISION, WAD_PRECISION};

use common_errors::*;
use common_structs::*;

/// Utility module providing core mathematical and position management functions.
///
/// This module contains essential utility functions for the lending protocol including:
/// - Position value calculations and health factor computations
/// - Collateral and debt valuation in EGLD terms
/// - Isolated asset debt tracking and management
/// - Bulk position updates for gas optimization
/// - Health factor validation for withdrawal safety
///
/// # Mathematical Foundation
/// The module implements critical financial calculations using high-precision
/// decimal arithmetic to ensure accuracy in lending operations:
/// - Ray precision (27 decimals) for internal calculations
/// - WAD precision (18 decimals) for token amounts
/// - Basis point conversions for risk parameters
///
/// # Security Considerations
/// All calculations use safe mathematical operations to prevent:
/// - Integer overflow/underflow attacks
/// - Precision loss in financial calculations
/// - Manipulation of health factor calculations
/// - Incorrect collateral valuation
#[multiversx_sc::module]
pub trait LendingUtilsModule:
    storage::Storage
    + oracle::OracleModule
    + common_events::EventsModule
    + common_math::SharedMathModule
    + helpers::MathsModule
    + common_rates::InterestRates
{
    /// Retrieves the liquidity pool address for a given asset.
    /// Ensures the asset has an associated pool; errors if not found.
    ///
    /// # Arguments
    /// - `asset`: The token identifier (EGLD or ESDT) of the asset.
    ///
    /// # Returns
    /// - `ManagedAddress`: The address of the liquidity pool.
    ///
    /// # Errors
    /// - `ERROR_NO_POOL_FOUND`: If no pool exists for the asset.
    fn pool_address(&self, asset: &EgldOrEsdtTokenIdentifier) -> ManagedAddress {
        let pool_address = self.pools_map(asset).get();
        require!(!pool_address.is_zero(), ERROR_NO_POOL_FOUND);
        pool_address
    }

    /// Calculates the current position amount by applying interest accrual to scaled amounts.
    ///
    /// **Purpose**: Converts a position's scaled amount to its current real value by applying
    /// the appropriate interest index. This accounts for interest accrual since the position
    /// was last updated.
    ///
    /// **How it works**:
    /// 1. Retrieves the latest market index from cache for the position's asset
    /// 2. For deposits: multiplies scaled_amount by supply_index
    /// 3. For borrows: multiplies scaled_amount by borrow_index  
    /// 4. Rescales result to asset's decimal precision
    ///
    /// **Mathematical formula**:
    /// ```
    /// // For deposit positions
    /// current_amount = scaled_amount * supply_index / RAY_PRECISION
    ///
    /// // For borrow positions  
    /// current_amount = scaled_amount * borrow_index / RAY_PRECISION
    /// ```
    ///
    /// **Interest index mechanics**:
    /// - **Supply index**: Tracks accumulated interest for depositors
    /// - **Borrow index**: Tracks accumulated interest for borrowers
    /// - Indices start at RAY (1e27) and increase over time
    /// - Scaled amounts remain constant; indices capture interest growth
    ///
    /// **Precision handling**:
    /// - Uses RAY_PRECISION (27 decimals) for internal calculations
    /// - Applies half-up rounding to prevent precision loss
    /// - Rescales final result to asset's native decimal precision
    ///
    /// **Security considerations**:
    /// - Always uses latest cached indices to prevent stale data
    /// - Half-up rounding prevents precision manipulation
    /// - Separate indices for deposits/borrows prevent cross-contamination
    ///
    /// # Arguments
    /// - `position`: Account position containing scaled amount and asset info
    /// - `feed`: Price feed data including asset decimal precision
    /// - `cache`: Mutable cache for efficient index retrieval
    ///
    /// # Returns
    /// Current position amount in asset's native decimal precision

    fn total_amount(
        &self,
        position: &AccountPosition<Self::Api>,
        feed: &PriceFeedShort<Self::Api>,
        cache: &mut Cache<Self>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let indexes = cache.cached_market_index(&position.asset_id);
        let index = if position.position_type == AccountPositionType::Deposit {
            indexes.supply_index_ray
        } else {
            indexes.borrow_index_ray
        };

        self.scaled_to_original(&position.scaled_amount_ray, &index, feed.asset_decimals)
    }

    /// Calculates current position amount with interest accrual in RAY precision.
    /// Multiplies scaled amount by appropriate index without decimal conversion.
    /// Returns high-precision value for internal calculations and aggregations.
    fn total_amount_ray(
        &self,
        position: &AccountPosition<Self::Api>,
        cache: &mut Cache<Self>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let indexes = cache.cached_market_index(&position.asset_id);
        let index = if position.position_type == AccountPositionType::Deposit {
            indexes.supply_index_ray
        } else {
            indexes.borrow_index_ray
        };

        self.scaled_to_original_ray(&position.scaled_amount_ray, &index)
    }

    /// Computes multiple collateral valuations for risk assessment and borrowing capacity.
    ///
    /// **Purpose**: Calculates three critical collateral metrics used throughout the protocol
    /// for health factor computation, liquidation decisions, and borrowing limit determination.
    ///
    /// **How it works**:
    /// 1. Iterates through all collateral positions
    /// 2. Gets current position amounts using interest indices
    /// 3. Converts amounts to EGLD value using oracle prices
    /// 4. Applies different risk weightings for each metric
    /// 5. Returns three distinct collateral values
    ///
    /// **Mathematical formulas**:
    /// ```
    /// // For each position:
    /// current_amount = scaled_amount * interest_index
    /// egld_value = current_amount * asset_price_in_egld
    ///
    /// // Aggregated values:
    /// total_collateral += egld_value
    /// weighted_collateral += egld_value * liquidation_threshold
    /// ltv_collateral += egld_value * loan_to_value
    /// ```
    ///
    /// **Collateral metric purposes**:
    /// - **Total collateral**: Unweighted sum, used for portfolio overview
    /// - **Weighted collateral**: Liquidation threshold weighted, used for health factor
    /// - **LTV collateral**: Loan-to-value weighted, used for borrowing capacity
    ///
    /// **Health factor calculation**:
    /// ```
    /// health_factor = weighted_collateral / total_debt
    /// ```
    /// Position becomes liquidatable when health_factor < 1.0
    ///
    /// **Borrowing capacity calculation**:
    /// ```
    /// max_new_borrow = ltv_collateral - current_debt
    /// ```
    ///
    /// **Risk weighting logic**:
    /// - Liquidation thresholds are typically 80-90% (conservative)
    /// - LTV ratios are typically 70-80% (more conservative)
    /// - This creates a safety buffer between borrowing limits and liquidation
    ///
    /// **Security considerations**:
    /// - Uses latest oracle prices to prevent stale price exploitation
    /// - Half-up rounding prevents precision manipulation
    /// - Separate weightings prevent borrowing beyond liquidation threshold
    ///
    /// # Arguments
    /// - `positions`: Collection of account deposit positions to evaluate
    /// - `cache`: Performance cache for price feeds and market indices
    ///
    /// # Returns
    /// Tuple containing (weighted_collateral, total_collateral, ltv_collateral) in EGLD terms
    fn calculate_collateral_values(
        &self,
        positions: &ManagedVec<AccountPosition<Self::Api>>,
        cache: &mut Cache<Self>,
    ) -> (
        ManagedDecimal<Self::Api, NumDecimals>,
        ManagedDecimal<Self::Api, NumDecimals>,
        ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        let mut weighted_collateral = self.ray_zero();
        let mut total_collateral = self.ray_zero();
        let mut ltv_collateral = self.ray_zero();

        for position in positions {
            let price_feed = self.token_price(&position.asset_id, cache);
            let amount = self.total_amount_ray(&position, cache);
            let amount_egld = self.token_egld_value_ray(&amount, &price_feed.price_wad);

            total_collateral += &amount_egld;
            weighted_collateral += self.mul_half_up(
                &amount_egld,
                &position.liquidation_threshold_bps,
                RAY_PRECISION,
            );
            ltv_collateral +=
                self.mul_half_up(&amount_egld, &position.loan_to_value_bps, RAY_PRECISION);
        }

        (weighted_collateral, total_collateral, ltv_collateral)
    }

    /// Calculates the total borrow value in EGLD for a set of positions.
    /// Sums the EGLD value of all borrowed assets.
    ///
    /// # Arguments
    /// - `positions`: Vector of account positions.
    /// - `cache`: Mutable reference to the storage cache.
    ///
    /// # Returns
    /// - Total borrow value in EGLD as a `ManagedDecimal`.
    fn calculate_total_borrow_in_egld(
        &self,
        positions: &ManagedVec<AccountPosition<Self::Api>>,
        cache: &mut Cache<Self>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        positions
            .iter()
            .fold(self.ray_zero(), |accumulator, position| {
                let price_feed = self.token_price(&position.asset_id, cache);
                let amount = self.total_amount_ray(&position, cache);
                accumulator + self.token_egld_value_ray(&amount, &price_feed.price_wad)
            })
    }

    /// Updates the global debt tracker for isolated assets in USD terms.
    ///
    /// **Purpose**: Maintains precise tracking of total debt borrowed against isolated
    /// collateral assets to enforce debt ceilings and prevent protocol insolvency.
    /// Isolated assets can only be used as collateral in isolation mode with debt limits.
    ///
    /// **How it works**:
    /// 1. Checks if adjustment amount is non-zero (optimization)
    /// 2. For increases: adds USD amount to debt tracker
    /// 3. For decreases: subtracts USD amount with safety bounds checking
    /// 4. Applies dust threshold cleanup (removes amounts < $1)
    /// 5. Emits debt ceiling update event for monitoring
    ///
    /// **Debt ceiling enforcement**:
    /// ```
    /// current_debt = isolated_asset_debt_usd[asset]
    /// debt_ceiling = asset_config.isolation_debt_ceiling_usd
    ///
    /// // Before new borrow:
    /// require(current_debt + new_borrow_usd <= debt_ceiling)
    /// ```
    ///
    /// **Decrease safety logic**:
    /// ```
    /// if debt > amount_to_decrease:
    ///     debt -= amount_to_decrease
    /// else:
    ///     debt = 0  // Prevent underflow
    /// ```
    ///
    /// **Dust threshold cleanup**:
    /// If remaining debt < $1 USD, it's set to zero to prevent
    /// accumulation of negligible debt amounts that could cause
    /// precision issues in calculations.
    ///
    /// **Security considerations**:
    /// - Prevents debt underflow through safe subtraction
    /// - Dust cleanup prevents precision manipulation
    /// - Event emission enables monitoring of debt ceiling usage
    /// - USD denomination provides stable debt measurement
    ///
    /// **Isolation mode mechanics**:
    /// - Isolated assets have separate debt ceilings
    /// - Prevents excessive concentration risk
    /// - Enables risk-adjusted borrowing against volatile collateral
    /// - Protects protocol from correlated asset risks
    ///
    /// # Arguments
    /// - `asset_id`: Token identifier of the isolated collateral asset
    /// - `amount_in_usd`: USD-denominated debt adjustment amount
    /// - `is_increase`: True for borrows, false for repayments
    ///
    /// # Returns
    /// Nothing - updates internal debt tracker and emits events
    fn adjust_isolated_debt_usd(
        &self,
        asset_id: &EgldOrEsdtTokenIdentifier,
        amount_in_usd: ManagedDecimal<Self::Api, NumDecimals>,
        is_increase: bool,
    ) {
        if amount_in_usd.eq(&self.wad_zero()) {
            return;
        }

        let debt_mapper = self.isolated_asset_debt_usd(asset_id);
        if is_increase {
            debt_mapper.update(|debt| *debt += amount_in_usd);
        } else {
            debt_mapper.update(|debt| {
                *debt -= if *debt > amount_in_usd {
                    amount_in_usd
                } else {
                    debt.clone()
                };
                // If dust remains under 1$ globally just erase the tracker
                if *debt < self.wad() {
                    *debt = self.wad_zero();
                }
            });
        }
        self.update_debt_ceiling_event(asset_id, debt_mapper.get());
    }

    /// Efficiently manages borrow position updates for gas optimization.
    ///
    /// **Purpose**: Provides gas-efficient updating of borrow positions when bulk
    /// borrowing is enabled. Maintains an indexed mapping for fast position lookups
    /// and updates without iterating through entire position lists.
    ///
    /// **How it works**:
    /// 1. Checks if bulk borrowing mode is enabled (early return if not)
    /// 2. Looks up existing position index in the mapping
    /// 3. For existing positions: updates in-place at known index
    /// 4. For new positions: appends to list and updates index mapping
    /// 5. Validates index consistency for security
    ///
    /// **Index mapping structure**:
    /// ```
    /// borrow_index_mapper: asset_id -> array_index + 1
    /// borrows: [position0, position1, position2, ...]
    ///
    /// // Index is stored as +1 to distinguish from empty (0)
    /// actual_index = stored_index - 1
    /// ```
    ///
    /// **Security validations**:
    /// - Verifies token ID consistency at index position
    /// - Ensures array bounds are respected
    /// - Prevents index tampering through validation
    ///
    /// **Gas optimization benefits**:
    /// - O(1) position lookups instead of O(n) iteration
    /// - Batch position updates in single transaction
    /// - Reduced storage operations for large position lists
    /// - Efficient memory management for growing positions
    ///
    /// **Error handling**:
    /// - ERROR_INVALID_BULK_BORROW_TICKER: Token mismatch at index
    /// - Index validation prevents out-of-bounds access
    /// - Graceful handling of new vs existing positions
    ///
    /// # Arguments
    /// - `borrows`: Mutable vector of borrow positions
    /// - `borrow_index_mapper`: Mutable mapping from asset to array index
    /// - `updated_position`: New position data to store
    /// - `is_bulk_borrow`: Flag enabling bulk borrow optimization
    ///
    /// # Returns
    /// Nothing - modifies borrows vector and index mapping in-place
    fn update_bulk_borrow_positions(
        &self,
        borrows: &mut ManagedVec<AccountPosition<Self::Api>>,
        borrow_index_mapper: &mut ManagedMapEncoded<Self::Api, EgldOrEsdtTokenIdentifier, usize>,
        updated_position: AccountPosition<Self::Api>,
        is_bulk_borrow: bool,
    ) {
        if !is_bulk_borrow {
            return;
        }

        let existing_borrow = borrow_index_mapper.contains(&updated_position.asset_id);
        if existing_borrow {
            let safe_index = borrow_index_mapper.get(&updated_position.asset_id);
            let index = safe_index - 1;
            let token_id = &borrows.get(index).asset_id.clone();
            require!(
                token_id == &updated_position.asset_id,
                ERROR_INVALID_BULK_BORROW_TICKER
            );
            let _ = borrows.set(index, updated_position);
        } else {
            let safe_index = borrows.len() + 1;
            borrow_index_mapper.put(&updated_position.asset_id, &safe_index);
            borrows.push(updated_position);
        }
    }

    /// Verifies that a position remains healthy after withdrawal operations.
    ///
    /// **Purpose**: Critical safety check that prevents withdrawals which would
    /// put a position at risk of liquidation. Calculates post-withdrawal health
    /// factor and ensures it meets minimum safety requirements.
    ///
    /// **How it works**:
    /// 1. Retrieves all borrow positions for the account (early return if none)
    /// 2. Retrieves all remaining deposit positions (post-withdrawal)
    /// 3. Calculates weighted collateral value using liquidation thresholds
    /// 4. Calculates total debt value in EGLD terms
    /// 5. Computes health factor and validates against minimum threshold
    ///
    /// **Health factor calculation**:
    /// ```
    /// health_factor = (total_collateral * liquidation_threshold) / total_debt
    ///
    /// // Position is safe when:
    /// health_factor >= 1.0 (normal operations)
    /// health_factor >= 1.0 + (1.0 / safety_factor) (with safety buffer)
    /// ```
    ///
    /// **Safety factor mechanics**:
    /// - When provided: adds additional safety buffer above 1.0
    /// - Example: safety_factor = 10 requires health_factor >= 1.1 (10% buffer)
    /// - Prevents positions from getting too close to liquidation threshold
    /// - Optional parameter for stricter safety requirements
    ///
    /// **Early termination optimization**:
    /// If no borrow positions exist, validation passes immediately since
    /// withdrawal cannot create liquidation risk without debt.
    ///
    /// **Security considerations**:
    /// - Uses post-withdrawal collateral values (not pre-withdrawal)
    /// - Includes all debt positions in calculation
    /// - Latest oracle prices prevent stale price manipulation
    /// - Safety factor provides additional protection buffer
    ///
    /// **Critical for protocol safety**:
    /// This function prevents users from withdrawing collateral that would
    /// make their positions liquidatable, protecting both users and protocol.
    ///
    /// # Arguments
    /// - `account_nonce`: NFT nonce identifying the position to validate
    /// - `cache`: Performance cache for price feeds and indices
    /// - `safety_factor`: Optional additional safety buffer (e.g., 10 for 10% buffer)
    ///
    /// # Returns
    /// Nothing - validates health factor or reverts transaction
    ///
    /// # Errors
    /// - `ERROR_HEALTH_FACTOR_WITHDRAW`: Health factor below minimum threshold
    fn validate_is_healthy(
        &self,
        account_nonce: u64,
        cache: &mut Cache<Self>,
        safety_factor: Option<ManagedDecimal<Self::Api, NumDecimals>>,
    ) {
        let borrow_positions = self.positions(account_nonce, AccountPositionType::Borrow);
        if borrow_positions.is_empty() {
            return;
        }

        let deposit_positions = self.positions(account_nonce, AccountPositionType::Deposit);
        let (collateral, _, _) =
            self.calculate_collateral_values(&deposit_positions.values().collect(), cache);
        let borrowed =
            self.calculate_total_borrow_in_egld(&borrow_positions.values().collect(), cache);
        let health_factor = self.compute_health_factor(&collateral, &borrowed);

        let min_health_factor = match safety_factor {
            Some(safety_factor_value) => self.ray() + (self.ray() / safety_factor_value),
            None => self.ray(),
        };

        require!(
            health_factor >= min_health_factor,
            ERROR_HEALTH_FACTOR_WITHDRAW
        );
    }
}
