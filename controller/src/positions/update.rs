use common_structs::{AccountAttributes, AccountPosition, AccountPositionType};

use super::account;
use crate::{cache::Cache, helpers, oracle, storage, utils, validation};
multiversx_sc::imports!();
multiversx_sc::derive_imports!();

#[multiversx_sc::module]
pub trait PositionUpdateModule:
    storage::Storage
    + validation::ValidationModule
    + oracle::OracleModule
    + common_events::EventsModule
    + utils::LendingUtilsModule
    + helpers::MathsModule
    + account::PositionAccountModule
    + common_math::SharedMathModule
    + common_rates::InterestRates
{
    /// Retrieves all borrow positions for an account with optional indexing.
    ///
    /// **Purpose**: Efficiently loads all borrow positions for health factor calculations
    /// and bulk operations, with optional index mapping for position tracking.
    ///
    /// **Methodology**:
    /// 1. Iterates through all borrow position keys for the account
    /// 2. Optionally creates index mapping for bulk operations
    /// 3. Loads position data from storage into vector format
    /// 4. Maintains position order consistency
    ///
    /// **Index Mapping**:
    /// - Maps asset identifiers to vector positions
    /// - Uses 1-based indexing to avoid zero-value issues
    /// - Enables efficient position updates in bulk operations
    ///
    /// **Performance Considerations**:
    /// - Single storage traversal for all positions
    /// - Optional indexing reduces overhead for simple operations
    /// - Unsafe unwrap used after existence validation
    ///
    /// # Arguments
    /// - `account_nonce`: Position NFT nonce for storage lookup
    /// - `should_return_map`: Flag to enable index mapping generation
    ///
    /// # Returns
    /// - Tuple containing (position vector, optional index mapping)
    fn borrow_positions(
        &self,
        account_nonce: u64,
        should_return_map: bool,
    ) -> (
        ManagedVec<AccountPosition<Self::Api>>,
        ManagedMapEncoded<Self::Api, EgldOrEsdtTokenIdentifier, usize>,
    ) {
        let borrow_positions_map = self.positions(account_nonce, AccountPositionType::Borrow);
        let mut updated_positions = ManagedVec::new();
        let mut position_index_map = ManagedMapEncoded::new();

        for (position_index, asset_id) in borrow_positions_map.keys().enumerate() {
            if should_return_map {
                let safe_index = position_index + 1; // Avoid zero index issues
                position_index_map.put(&asset_id, &safe_index);
            }

            updated_positions
                .push(unsafe { borrow_positions_map.get(&asset_id).unwrap_unchecked() });
        }

        (updated_positions, position_index_map)
    }

    /// Stores an updated position in storage.
    ///
    /// **Purpose**: Persists position state changes to blockchain storage,
    /// maintaining consistency across deposit and borrow position types.
    ///
    /// **Methodology**:
    /// - Uses position type to determine correct storage mapping
    /// - Updates position data under asset identifier key
    /// - Maintains atomic storage operations
    ///
    /// **Storage Structure**:
    /// ```
    /// positions[account_nonce][position_type][asset_id] = position
    /// ```
    ///
    /// **Security Considerations**:
    /// - Type-safe storage access prevents cross-contamination
    /// - Atomic updates ensure position consistency
    /// - Asset identifier as key enables efficient lookups
    ///
    /// # Arguments
    /// - `account_nonce`: Position NFT nonce for storage mapping
    /// - `position`: Updated position containing all current state
    fn store_updated_position(&self, account_nonce: u64, position: &AccountPosition<Self::Api>) {
        self.positions(account_nonce, position.position_type.clone())
            .insert(position.asset_id.clone(), position.clone());
    }
    /// Updates or removes a position in storage based on remaining balance.
    ///
    /// **Purpose**: Manages position lifecycle by either updating active positions
    /// or removing fully closed positions to maintain clean storage state.
    ///
    /// **Methodology**:
    /// 1. Checks if position can be removed (zero balance)
    /// 2. For removable positions: deletes from storage mapping
    /// 3. For active positions: updates with current state
    ///
    /// **Removal Criteria**:
    /// - Zero scaled amount (no remaining debt/deposit)
    /// - Position marked as removable by pool contract
    /// - Prevents storage bloat from empty positions
    ///
    /// **Security Considerations**:
    /// - Position removal only after confirmed zero balance
    /// - Prevents premature deletion of active positions
    /// - Maintains storage integrity across operations
    ///
    /// # Arguments
    /// - `account_nonce`: Position NFT nonce for storage operations
    /// - `position`: Position with updated state to validate for removal
    fn update_or_remove_position(&self, account_nonce: u64, position: &AccountPosition<Self::Api>) {
        if position.can_remove() {
            self.positions(account_nonce, position.position_type.clone())
                .remove(&position.asset_id);
        } else {
            self.store_updated_position(account_nonce, position);
        }
    }

    /// Emits an event for a position update.
    ///
    /// **Purpose**: Logs position state changes for off-chain monitoring,
    /// analytics, and user interface updates with comprehensive context.
    ///
    /// **Methodology**:
    /// - Formats position data for event emission
    /// - Includes price and caller information for context
    /// - Provides account attributes for filtering and analysis
    ///
    /// **Event Data**:
    /// - Amount: Operation amount in asset decimals
    /// - Position: Complete position state after update
    /// - Price: Current asset price for valuation
    /// - Caller: Address initiating the operation
    /// - Attributes: Account configuration and mode
    ///
    /// **Use Cases**:
    /// - User interface position updates
    /// - Off-chain analytics and monitoring
    /// - Interest accrual tracking
    /// - Liquidation monitoring
    ///
    /// # Arguments
    /// - `amount`: Operation amount for the update
    /// - `position`: Position state after the operation
    /// - `price`: Current asset price for context
    /// - `caller`: Address performing the operation
    /// - `attributes`: Account attributes and configuration
    fn emit_position_update_event(
        &self,
        cache: &mut Cache<Self>,
        amount: &ManagedDecimal<Self::Api, NumDecimals>,
        position: &AccountPosition<Self::Api>,
        price: ManagedDecimal<Self::Api, NumDecimals>,
        caller: &ManagedAddress<Self::Api>,
        attributes: &AccountAttributes<Self::Api>,
    ) {
        let position_type = position.position_type.clone();
        let market_index = cache.cached_market_index(&position.asset_id);
        let index = match position_type {
            AccountPositionType::Borrow => market_index.borrow_index_ray,
            AccountPositionType::Deposit => market_index.supply_index_ray,
            _ => {
                sc_panic!("Invalid position type");
            },
        };

        self.update_position_event(
            index,
            amount,
            position,
            OptionalValue::Some(price),
            OptionalValue::Some(caller),
            OptionalValue::Some(attributes),
        );
    }
}
