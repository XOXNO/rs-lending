multiversx_sc::imports!();

use common_errors::{
    ERROR_FLASH_LOAN_ALREADY_ONGOING, ERROR_INVALID_ENDPOINT, ERROR_INVALID_SHARD,
    ERROR_POSITION_LIMIT_EXCEEDED,
};

use crate::{
    helpers, oracle, storage, utils, ERROR_AMOUNT_MUST_BE_GREATER_THAN_ZERO,
    ERROR_ASSET_NOT_SUPPORTED,
};
use common_structs::AccountPositionType;

/// Validation module providing security checks and constraint enforcement.
///
/// This module implements critical validation logic to ensure protocol safety:
/// - Payment validation for deposits, repayments, and withdrawals
/// - Asset support verification through liquidity pool existence
/// - Amount validation to prevent zero-value operations
/// - Flash loan security checks including shard validation and reentrancy protection
/// - Endpoint validation for flash loan callbacks
///
/// # Security Framework
/// All validation functions serve as the first line of defense against:
/// - Invalid or malicious transactions
/// - Unsupported asset operations
/// - Flash loan exploit attempts
/// - Cross-shard security vulnerabilities
/// - Reentrancy attacks
///
/// # Validation Hierarchy
/// 1. **Asset-level**: Verify asset is supported and has active liquidity pool
/// 2. **Amount-level**: Ensure amounts are positive and non-zero
/// 3. **Operation-level**: Validate specific operation constraints
/// 4. **Security-level**: Check for flash loan exploits and reentrancy
#[multiversx_sc::module]
pub trait ValidationModule:
    storage::Storage
    + utils::LendingUtilsModule
    + common_events::EventsModule
    + oracle::OracleModule
    + helpers::MathsModule
    + common_math::SharedMathModule
    + common_rates::InterestRates
{
    /// Performs comprehensive validation of payment data for protocol operations.
    ///
    /// **Purpose**: First-line validation for all payment-based operations including
    /// deposits, repayments, borrowing, and withdrawals. Ensures both asset support
    /// and payment validity before processing.
    ///
    /// **How it works**:
    /// 1. Validates asset has an active liquidity pool (require_asset_supported)
    /// 2. Validates payment amount is greater than zero (require_amount_greater_than_zero)
    /// 3. Returns early if both checks pass, reverts if either fails
    ///
    /// **Security validations**:
    /// - **Asset support**: Prevents operations on unsupported or delisted assets
    /// - **Amount validation**: Prevents zero-value attacks and dust operations
    /// - **Pool existence**: Ensures asset has active lending/borrowing functionality
    ///
    /// **Protocol safety**:
    /// This function serves as a critical security gate for all financial operations.
    /// Without proper validation, the protocol could:
    /// - Process operations on unsupported assets leading to undefined behavior
    /// - Accept zero-amount operations that could be used for exploit attempts
    /// - Interact with non-existent liquidity pools causing reverts or corruption
    ///
    /// **Usage pattern**:
    /// Called at the beginning of functions like deposit(), repay(), borrow(), etc.
    /// to ensure consistent validation across all protocol entry points.
    ///
    /// # Arguments
    /// - `payment`: EGLD or ESDT payment containing token identifier and amount
    ///
    /// # Returns
    /// Nothing - validates payment or reverts with specific error
    ///
    /// # Errors
    /// - `ERROR_ASSET_NOT_SUPPORTED`: Asset has no associated liquidity pool
    /// - `ERROR_AMOUNT_MUST_BE_GREATER_THAN_ZERO`: Payment amount is zero or negative

    fn validate_payment(&self, payment: &EgldOrEsdtTokenPayment<Self::Api>) {
        let _ = self.require_asset_supported(&payment.token_identifier);
        self.require_amount_greater_than_zero(&payment.amount);
    }

    /// Ensures an asset is supported by verifying its liquidity pool exists.
    ///
    /// # Arguments
    /// - `asset`: Token identifier (EGLD or ESDT) to check.
    ///
    /// # Returns
    /// - `ManagedAddress`: Pool address if the asset is supported.
    ///
    /// # Errors
    /// - `ERROR_ASSET_NOT_SUPPORTED`: If no pool exists for the asset.

    fn require_asset_supported(&self, asset: &EgldOrEsdtTokenIdentifier) -> ManagedAddress {
        let map = self.pools_map(asset);
        require!(!map.is_empty(), ERROR_ASSET_NOT_SUPPORTED);

        map.get()
    }

    /// Ensures an amount is greater than zero.
    /// Prevents zero-value operations like deposits or borrows.
    ///
    /// # Arguments
    /// - `amount`: The amount to validate as a `BigUint`.
    ///
    /// # Errors
    /// - `ERROR_AMOUNT_MUST_BE_GREATER_THAN_ZERO`: If the amount is zero or negative.

    fn require_amount_greater_than_zero(&self, amount: &BigUint) {
        require!(
            amount > &BigUint::zero(),
            ERROR_AMOUNT_MUST_BE_GREATER_THAN_ZERO
        );
    }

    // --- Helper Functions ---

    /// Validates cross-shard security for flash loan operations.
    ///
    /// **Purpose**: Ensures flash loans only occur within the same shard to prevent
    /// cross-shard timing attacks and maintain atomic transaction guarantees.
    /// Critical for preventing flash loan exploits that rely on shard-specific behavior.
    ///
    /// **How it works**:
    /// 1. Gets the destination contract's shard ID
    /// 2. Gets the current controller contract's shard ID
    /// 3. Compares shard IDs and reverts if different
    ///
    /// **Security rationale**:
    /// - **Atomic execution**: Flash loans must execute atomically within single shard
    /// - **Timing attack prevention**: Cross-shard calls introduce timing vulnerabilities
    /// - **State consistency**: Same-shard execution ensures consistent state access
    /// - **MEV protection**: Prevents cross-shard MEV extraction opportunities
    ///
    /// **Flash loan flow security**:
    /// ```
    /// 1. Controller initiates flash loan (same shard)
    /// 2. Funds sent to borrower contract (validated same shard)
    /// 3. Borrower executes arbitrary logic (same shard)
    /// 4. Borrower repays loan + fee (atomic in same shard)
    /// 5. Controller validates repayment (immediate verification)
    /// ```
    ///
    /// **Attack vector prevention**:
    /// Cross-shard flash loans could enable:
    /// - State inconsistency exploits between shards
    /// - Timing manipulation of price oracles
    /// - Complex multi-shard arbitrage with delayed settlement
    /// - Network congestion-based exploit opportunities
    ///
    /// # Arguments
    /// - `contract_address`: Destination contract address for flash loan callback
    ///
    /// # Returns
    /// Nothing - validates shard compatibility or reverts
    ///
    /// # Errors
    /// - `ERROR_INVALID_SHARD`: Destination contract is on different shard

    fn validate_flash_loan_shard(&self, contract_address: &ManagedAddress) {
        let destination_shard_id = self.blockchain().get_shard_of_address(contract_address);
        let current_shard_id = self
            .blockchain()
            .get_shard_of_address(&self.blockchain().get_sc_address());

        require!(
            destination_shard_id == current_shard_id,
            ERROR_INVALID_SHARD
        );
    }

    /// Validates flash loan callback endpoint for security compliance.
    ///
    /// **Purpose**: Ensures flash loan callbacks target legitimate contract endpoints
    /// and not built-in blockchain functions. Prevents exploitation of system-level
    /// functions that could bypass flash loan repayment requirements.
    ///
    /// **How it works**:
    /// 1. Checks endpoint name is not empty (must specify callback function)
    /// 2. Verifies endpoint is not a blockchain built-in function
    /// 3. Allows execution if both validation checks pass
    ///
    /// **Security validations**:
    /// - **Non-empty endpoint**: Prevents calling undefined or default functions
    /// - **No built-in functions**: Blocks calls to system-level blockchain functions
    /// - **Custom endpoint only**: Ensures callback goes to user-defined contract logic
    ///
    /// **Built-in function protection**:
    /// Built-in functions are system-level operations like:
    /// - Token transfers and minting
    /// - ESDT management operations
    /// - Staking and delegation functions
    /// - System configuration changes
    ///
    /// **Attack prevention**:
    /// Without validation, malicious contracts could:
    /// - Call built-in functions to manipulate token balances
    /// - Bypass custom repayment logic through system calls
    /// - Execute privileged operations outside contract control
    /// - Circumvent flash loan fee collection mechanisms
    ///
    /// **Flash loan callback pattern**:
    /// ```
    /// 1. Controller validates endpoint name
    /// 2. Controller sends flash loan to borrower
    /// 3. Controller calls borrower.validated_endpoint()
    /// 4. Borrower executes custom logic and repays loan
    /// 5. Controller verifies repayment completion
    /// ```
    ///
    /// # Arguments
    /// - `endpoint`: Managed buffer containing the callback endpoint name
    ///
    /// # Returns
    /// Nothing - validates endpoint or reverts
    ///
    /// # Errors
    /// - `ERROR_INVALID_ENDPOINT`: Endpoint is empty or a built-in function

    fn validate_flash_loan_endpoint(&self, endpoint: &ManagedBuffer<Self::Api>) {
        require!(
            !self.blockchain().is_builtin_function(endpoint) && !endpoint.is_empty(),
            ERROR_INVALID_ENDPOINT
        );
    }

    /// Prevents reentrancy attacks during flash loan operations.
    ///
    /// **Purpose**: Critical security mechanism that prevents nested flash loan calls
    /// which could be used to exploit protocol state inconsistencies or drain funds.
    /// Implements a simple but effective reentrancy guard pattern.
    ///
    /// **How it works**:
    /// 1. Checks if a flash loan is currently in progress
    /// 2. Reverts transaction if reentrancy is detected
    /// 3. Allows execution if no ongoing flash loan exists
    ///
    /// **Reentrancy attack pattern**:
    /// ```
    /// 1. Attacker initiates flash loan A
    /// 2. During flash loan A callback, attacker initiates flash loan B
    /// 3. Flash loan B could exploit inconsistent state from loan A
    /// 4. Attacker attempts to profit from state confusion
    /// ```
    ///
    /// **Protection mechanism**:
    /// - **State flag tracking**: Maintains flash_loan_ongoing boolean
    /// - **Atomic checking**: Validates state before allowing new flash loans
    /// - **Early termination**: Reverts immediately if reentrancy detected
    /// - **Clean state restoration**: Flag cleared after successful completion
    ///
    /// **Security benefits**:
    /// - Prevents flash loan callback chains that could confuse protocol state
    /// - Ensures atomic execution of individual flash loan operations
    /// - Eliminates timing attacks through nested flash loan calls
    /// - Protects against complex multi-step exploit patterns
    ///
    /// **Usage pattern**:
    /// Called at the beginning of flash loan functions before state modifications
    /// to ensure no ongoing flash loan operations could interfere.
    ///
    /// # Arguments
    /// - `flash_loan_ongoing`: Boolean flag indicating if flash loan is in progress
    ///
    /// # Returns
    /// Nothing - validates no reentrancy or reverts
    ///
    /// # Errors
    /// - `ERROR_FLASH_LOAN_ALREADY_ONGOING`: Reentrancy attempt detected

    fn reentrancy_guard(&self, flash_loan_ongoing: bool) {
        require!(!flash_loan_ongoing, ERROR_FLASH_LOAN_ALREADY_ONGOING);
    }

    /// Validates position count limits for bulk operations (multiple positions in one transaction).
    ///
    /// **Purpose**: Enforces governance-controlled limits on the number of positions
    /// per NFT while accounting for multiple new positions being created in a single transaction.
    ///
    /// **Bulk Operation Protection**: This prevents users from circumventing position limits
    /// by creating multiple new positions in a single transaction that would collectively
    /// exceed the configured limits.
    ///
    /// **How it works**:
    /// 1. Retrieves current position limits from governance storage
    /// 2. Counts existing positions of the specified type for the account
    /// 3. Counts how many NEW positions would be created from the payment list
    /// 4. Validates total (existing + new) would not exceed the configured limit
    /// 5. Reverts if limit would be exceeded, allows creation otherwise
    ///
    /// **Example scenarios**:
    /// - Limit: 10 supply positions
    /// - Current: 9 supply positions  
    /// - Bulk supply: 2 new assets
    /// - Total would be: 9 + 2 = 11 > 10 ❌ FAIL
    ///
    /// - Limit: 10 supply positions
    /// - Current: 8 supply positions
    /// - Bulk supply: 1 existing + 1 new asset
    /// - New positions: 1 (existing doesn't count)
    /// - Total would be: 8 + 1 = 9 <= 10 ✅ PASS
    ///
    /// # Arguments
    /// - `account_nonce`: Position NFT nonce to check
    /// - `position_type`: Type of position being created (Deposit or Borrow)
    /// - `payments`: Vector of payments that may create new positions
    ///
    /// # Returns
    /// Nothing - validates limits or reverts
    ///
    /// # Errors  
    /// - `ERROR_POSITION_LIMIT_EXCEEDED`: Position limit would be exceeded

    fn validate_bulk_position_limits(
        &self,
        account_nonce: u64,
        position_type: AccountPositionType,
        payments: &ManagedVec<EgldOrEsdtTokenPayment<Self::Api>>,
    ) {
        let limits = self.position_limits().get();
        let positions_map = self.positions(account_nonce, position_type.clone());
        let current_count = positions_map.len();

        let max_allowed = match position_type {
            AccountPositionType::Deposit => limits.max_supply_positions as usize,
            AccountPositionType::Borrow => limits.max_borrow_positions as usize,
            _ => return, // No limits for other types
        };

        // Count how many NEW positions would be created
        let mut new_positions_count = 0;
        for payment in payments {
            if !positions_map.contains_key(&payment.token_identifier) {
                new_positions_count += 1;
            }
        }

        let total_after_transaction = current_count + new_positions_count;
        require!(
            total_after_transaction <= max_allowed,
            ERROR_POSITION_LIMIT_EXCEEDED
        );
    }
}
