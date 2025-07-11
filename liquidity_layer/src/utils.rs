multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use crate::{cache::Cache, storage, view};

use common_constants::RAY_PRECISION;
use common_errors::{
    ERROR_INVALID_ASSET, ERROR_INVALID_FLASHLOAN_REPAYMENT, ERROR_WITHDRAW_AMOUNT_LESS_THAN_FEE,
};

/// The `UtilsModule` trait provides a collection of helper functions supporting core liquidity pool operations.
///
/// **Scope**: Offers utilities for event emission, standardized asset transfers, payment retrieval and validation,
/// and flash loan repayment verification.
///
/// **Goal**: To encapsulate common, reusable logic, promoting clarity and consistency within the liquidity pool contract.
#[multiversx_sc::module]
pub trait UtilsModule:
    storage::Storage
    + common_events::EventsModule
    + view::ViewModule
    + common_math::SharedMathModule
    + common_rates::InterestRates
{
    /// Updates both borrow and supply indexes based on elapsed time since the last update.
    ///
    /// **Purpose**: Synchronizes the global state of the pool by recalculating borrow and supply indexes,
    /// ensuring accurate compound interest accrual and proportional reward distribution.
    ///
    /// **Mathematical Foundation**:
    /// This function implements the core interest accrual mechanism using compound interest formulas:
    ///
    /// **Compound Interest Formula**:
    /// ```
    /// compound_factor = (1 + annual_rate)^(time_delta / YEAR_IN_MS)
    /// new_borrow_index = old_borrow_index * compound_factor
    /// ```
    ///
    /// **Utilization-Based Rate Calculation**:
    /// ```
    /// utilization = total_borrowed_value / total_supplied_value
    /// borrow_rate = base_rate + (utilization * slope1) + max(0, (utilization - kink) * slope2)
    /// ```
    ///
    /// **Supply Index Update Formula**:
    /// ```
    /// supplier_rewards = borrowed_scaled * (new_borrow_index - old_borrow_index) * (1 - reserve_factor)
    /// new_supply_index = old_supply_index + (supplier_rewards / total_scaled_supplied)
    /// ```
    ///
    /// **Revenue Distribution**:
    /// ```
    /// total_interest = borrowed_scaled * (new_borrow_index - old_borrow_index)
    /// supplier_share = total_interest * (1 - reserve_factor)
    /// protocol_share = total_interest * reserve_factor
    /// ```
    ///
    /// **Process Flow**:
    /// 1. **Time Delta**: `delta = current_timestamp - last_update_timestamp`
    /// 2. **Rate Calculation**: Compute current borrow rate based on utilization
    /// 3. **Compound Factor**: Calculate interest growth factor for time period
    /// 4. **Borrow Index Update**: Apply compound growth to borrow index
    /// 5. **Interest Distribution**: Split accrued interest between suppliers and protocol
    /// 6. **Supply Index Update**: Distribute supplier rewards via index increase
    /// 7. **Protocol Revenue**: Add protocol share to revenue accumulator
    /// 8. **Timestamp Update**: Record current time as last update
    ///
    /// **Interest Accrual Properties**:
    /// - Continuous compounding approximation for small time intervals
    /// - Precise calculation for any time period length
    /// - Atomicity ensures no interest is lost or double-counted
    /// - Proportional distribution maintains fairness
    ///
    /// # Arguments
    /// - `cache`: Mutable pool state containing indexes, timestamps, and balances
    ///
    /// **Security Considerations**:
    /// - Zero delta check prevents redundant computation
    /// - Atomic state updates prevent inconsistencies
    /// - Overflow protection in compound interest calculations
    /// - Timestamp monotonicity enforcement
    fn global_sync(&self, cache: &mut Cache<Self>) {
        let delta = cache.timestamp - cache.last_timestamp;

        if delta > 0 {
            let borrow_rate = self.calc_borrow_rate(cache.get_utilization(), cache.params.clone());
            let borrow_factor = self.calculate_compounded_interest(borrow_rate.clone(), delta);
            let (new_borrow_index, old_borrow_index) =
                self.update_borrow_index(cache.borrow_index.clone(), borrow_factor.clone());

            // Calculate supplier rewards and protocol fees directly
            let (supplier_rewards_ray, protocol_fee_ray) = self.calc_supplier_rewards(
                cache.params.clone(),
                &cache.borrowed,
                &new_borrow_index,
                &old_borrow_index,
            );

            let new_supply_index = self.update_supply_index(
                cache.supplied.clone(),
                cache.supply_index.clone(),
                supplier_rewards_ray,
            );

            cache.supply_index = new_supply_index;
            cache.borrow_index = new_borrow_index;

            self.internal_add_protocol_revenue(cache, protocol_fee_ray);

            cache.last_timestamp = cache.timestamp;
        }
    }

    /// Applies bad debt immediately to supply index, socializing the loss among all suppliers.
    ///
    /// **Purpose**: Implements immediate loss socialization to prevent supplier flight and maintain
    /// pool stability during bad debt events. This mechanism is superior to traditional bad debt
    /// tracking as it eliminates race conditions and ensures fair loss distribution.
    ///
    /// **Problem Statement**:
    /// Traditional lending protocols track bad debt separately, creating opportunities for
    /// informed suppliers to withdraw before losses are realized, leaving remaining suppliers
    /// with disproportionate losses. This creates systemic instability during stress events.
    ///
    /// **Mathematical Foundation**:
    ///
    /// **Supply Index Reduction Formula**:
    /// ```
    /// total_supplied_value = total_scaled_supplied * current_supply_index
    /// loss_ratio = min(bad_debt_amount / total_supplied_value, 1.0)
    /// reduction_factor = 1 - loss_ratio
    /// new_supply_index = old_supply_index * reduction_factor
    /// ```
    ///
    /// **Loss Distribution Calculation**:
    /// ```
    /// // For each supplier:
    /// old_value = supplier_scaled_tokens * old_supply_index
    /// new_value = supplier_scaled_tokens * new_supply_index
    /// supplier_loss = old_value - new_value
    /// loss_percentage = bad_debt_amount / total_supplied_value
    /// ```
    ///
    /// **Proportionality Guarantee**:
    /// Every supplier loses exactly the same percentage of their holdings:
    /// ```
    /// supplier_loss_ratio = supplier_loss / supplier_old_value = loss_percentage (constant)
    /// ```
    ///
    /// **Minimum Index Protection**:
    /// To prevent total collapse while allowing significant losses:
    /// ```
    /// min_supply_index = 1e-27  // Very small but positive
    /// final_supply_index = max(calculated_new_index, min_supply_index)
    /// ```
    ///
    /// **Economic Properties**:
    /// 1. **Immediate Finality**: Losses are applied instantly, preventing withdrawal races
    /// 2. **Proportional Fairness**: All suppliers share losses proportionally
    /// 3. **No Arbitrage**: No opportunity to avoid losses through timing
    /// 4. **Stability Preservation**: Pool remains functional after loss events
    /// 5. **Simplified Accounting**: No separate bad debt tracking required
    ///
    /// **Implementation Details**:
    /// - Uses RAY precision (27 decimals) for accurate calculations
    /// - Caps bad debt to available value to prevent negative results
    /// - Maintains scaled token amounts (only index changes)
    /// - Preserves total scaled supply consistency
    ///
    /// # Arguments
    /// - `cache`: Mutable pool state for index updates
    /// - `bad_debt_amount`: Uncollectable debt amount in asset decimals
    ///
    /// **Security Considerations**:
    /// - Immediate application prevents gaming opportunities
    /// - Minimum index floor prevents total value destruction
    /// - Proportional distribution ensures fairness
    /// - Atomic operation prevents partial loss states
    fn apply_bad_debt_to_supply_index(
        &self,
        cache: &mut Cache<Self>,
        bad_debt_amount: &ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        // Calculate total supplied value in RAY precision
        let total_supplied_value_ray =
            self.mul_half_up(&cache.supplied, &cache.supply_index, RAY_PRECISION);
        // Convert bad debt to RAY precision
        let bad_debt_ray = bad_debt_amount.rescale(RAY_PRECISION);

        // Cap bad debt to available value (prevent negative results)
        let capped_bad_debt_ray = self.get_min(bad_debt_ray, total_supplied_value_ray.clone());

        // Calculate remaining value after bad debt
        let remaining_value_ray = total_supplied_value_ray.clone() - capped_bad_debt_ray;

        // Calculate reduction factor: remaining_value / total_value
        let reduction_factor = self.div_half_up(
            &remaining_value_ray,
            &total_supplied_value_ray,
            RAY_PRECISION,
        );

        // Apply reduction to supply index
        let new_supply_index =
            self.mul_half_up(&cache.supply_index, &reduction_factor, RAY_PRECISION);

        // Ensure minimum supply index (prevent total collapse but allow significant reduction)
        let min_supply_index = self.to_decimal(BigUint::from(1u64), RAY_PRECISION); // 1e-27, very small but > 0
        cache.supply_index = self.get_max(new_supply_index, min_supply_index);
    }

    /// Emits an event logging the current market state for transparency.
    ///
    /// **Scope**: Records key pool metrics like indexes, reserves, and asset price in an event.
    ///
    /// **Goal**: Provide auditors and users with a transparent snapshot of the market state after updates.
    ///
    /// # Arguments
    /// - `cache`: Reference to the pool state (`Cache<Self>`).
    /// - `asset_price`: Current price of the pool asset (`ManagedDecimal<Self::Api, NumDecimals>`).
    ///
    #[inline(always)]
    fn emit_market_update(
        &self,
        cache: &Cache<Self>,
        asset_price: &ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        let reserves = cache.get_reserves();
        self.update_market_state_event(
            cache.timestamp,
            &cache.supply_index,
            &cache.borrow_index,
            &reserves,
            &cache.supplied,
            &cache.borrowed,
            &cache.revenue,
            &cache.params.asset_id,
            asset_price,
        );
    }

    /// Transfers assets (EGLD or ESDT) to a specified address.
    ///
    /// **Scope**: Facilitates secure asset transfers from the contract to a recipient.
    ///
    /// **Goal**: Enable withdrawals, repayments, or reward distributions while ensuring safety.
    ///
    /// # Arguments
    /// - `cache`: Reference to the pool state (`Cache<Self>`), providing asset details.
    /// - `amount`: Amount to transfer (`ManagedDecimal<Self::Api, NumDecimals>`).
    /// - `to`: Recipient address (`ManagedAddress`).
    ///
    /// # Returns
    /// - `EgldOrEsdtTokenPayment<Self::Api>`: Payment object representing the transfer.
    ///
    /// **Security Tip**: Uses `transfer_if_not_empty` to avoid empty transfers, protected by caller validation of `amount`.
    #[inline]
    fn send_asset(
        &self,
        cache: &Cache<Self>,
        amount: &ManagedDecimal<Self::Api, NumDecimals>,
        to: &ManagedAddress,
    ) -> EgldOrEsdtTokenPayment<Self::Api> {
        let payment = EgldOrEsdtTokenPayment::new(
            cache.params.asset_id.clone(),
            0,
            amount.into_raw_units().clone(),
        );

        self.tx().to(to).payment(&payment).transfer_if_not_empty();

        payment
    }

    /// Retrieves and validates the payment amount from a transaction.
    ///
    /// **Scope**: Extracts the payment amount (EGLD or ESDT) and ensures it matches the pool's asset.
    ///
    /// **Goal**: Validate incoming payments to prevent asset mismatches during operations like deposits or repayments.
    ///
    /// # Arguments
    /// - `cache`: Reference to the pool state (`Cache<Self>`), containing the expected asset.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: Validated payment amount.
    ///
    /// **Security Tip**: Uses `require!` to enforce asset matching, protected by caller (e.g., `supply`) ensuring transaction context.
    fn get_payment_amount(&self, cache: &Cache<Self>) -> ManagedDecimal<Self::Api, NumDecimals> {
        let (asset, amount) = self.call_value().egld_or_single_fungible_esdt();

        require!(cache.is_same_asset(&asset), ERROR_INVALID_ASSET);

        cache.get_decimal_value(&amount)
    }

    /// Validates repayment of a flash loan, ensuring it meets requirements.
    ///
    /// **Scope**: Checks that a flash loan repayment matches the pool asset and exceeds the required amount.
    ///
    /// **Goal**: Secure the flash loan process by enforcing repayment conditions, protecting the pool's funds.
    ///
    /// **Process**:
    /// - Extracts repayment amount (EGLD or ESDT).
    /// - Validates asset and amount against requirements.
    ///
    /// # Arguments
    /// - `cache`: Reference to the pool state (`Cache<Self>`), containing asset details.
    /// - `back_transfers`: Repayment transfers from the transaction (`BackTransfers<Self::Api>`).
    /// - `amount`: Original loan amount (`ManagedDecimal<Self::Api, NumDecimals>`).
    /// - `required_repayment`: Minimum repayment including fees (`ManagedDecimal<Self::Api, NumDecimals>`).
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: Actual repayment amount.
    ///
    /// **Security Tip**: Multiple `require!` checks enforce asset and amount validity, protected by the flash loan flow structure.
    #[inline(always)]
    fn validate_flash_repayment(
        &self,
        cache: &Cache<Self>,
        back_transfers: &BackTransfers<Self::Api>,
        required_repayment: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        require!(
            back_transfers.payments.len() == 1,
            ERROR_INVALID_FLASHLOAN_REPAYMENT
        );
        let payment = back_transfers.payments.get(0);
        require!(
            cache.is_same_asset(&payment.token_identifier),
            ERROR_INVALID_FLASHLOAN_REPAYMENT
        );

        let repayment = cache.get_decimal_value(&payment.amount);

        require!(
            repayment >= *required_repayment,
            ERROR_INVALID_FLASHLOAN_REPAYMENT
        );

        repayment
    }

    /// Determines the gross scaled and actual amounts for a withdrawal operation.
    ///
    /// **Purpose**: Calculates the precise amounts to withdraw from a user's position,
    /// handling both full and partial withdrawals while preserving scaling precision.
    ///
    /// **Mathematical Process**:
    ///
    /// **Current Position Value Calculation**:
    /// ```
    /// current_supply_actual = position_scaled_amount * current_supply_index
    /// ```
    ///
    /// **Full vs Partial Withdrawal Logic**:
    /// ```
    /// if requested_amount >= current_supply_actual:
    ///     // Full withdrawal - user gets entire position value
    ///     scaled_to_withdraw = position_scaled_amount (entire position)
    ///     actual_to_withdraw = current_supply_actual (position value)
    /// else:
    ///     // Partial withdrawal - scale down the request
    ///     scaled_to_withdraw = requested_amount / current_supply_index
    ///     actual_to_withdraw = requested_amount
    /// ```
    ///
    /// **Scaling Precision Properties**:
    /// - Maintains exact scaled token arithmetic
    /// - Prevents rounding errors in position calculations
    /// - Ensures withdrawal accuracy regardless of supply index value
    /// - Preserves remaining position integrity
    ///
    /// **Interest Inclusion**:
    /// The withdrawal calculation automatically includes accrued interest
    /// through the current supply index, ensuring users receive their
    /// proportional share of pool earnings.
    ///
    /// **Edge Case Handling**:
    /// - Over-withdrawal protection: Caps at available position value
    /// - Zero position handling: Returns zero amounts safely
    /// - Index consistency: Uses current synchronized indexes
    ///
    /// # Arguments
    /// - `cache`: Current pool state with updated indexes
    /// - `position_scaled_amount`: User's scaled supply token balance
    /// - `requested_amount_actual`: Desired withdrawal amount in asset decimals
    ///
    /// # Returns
    /// - `scaled_withdrawal_amount_gross`: Scaled tokens to burn from position
    /// - `amount_to_withdraw_gross`: Actual asset amount to transfer (before fees)
    ///
    /// **Security Considerations**:
    /// - Prevents over-withdrawal through position capping
    /// - Maintains scaling precision to prevent exploitation
    /// - Uses synchronized indexes for accurate calculations
    #[inline(always)]
    fn determine_gross_withdrawal_amounts(
        &self,
        cache: &Cache<Self>,
        position_scaled_amount: &ManagedDecimal<Self::Api, NumDecimals>,
        requested_amount_actual: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> (
        ManagedDecimal<Self::Api, NumDecimals>, // scaled_withdrawal_amount_gross
        ManagedDecimal<Self::Api, NumDecimals>, // amount_to_withdraw_gross
    ) {
        let current_supply_actual = cache.original_supply(position_scaled_amount);

        if *requested_amount_actual >= current_supply_actual {
            // Full withdrawal
            (position_scaled_amount.clone(), current_supply_actual)
        } else {
            // Partial withdrawal
            let requested_scaled = cache.scaled_supply(requested_amount_actual);
            (requested_scaled, requested_amount_actual.clone())
        }
    }

    /// Determines the scaled amount to repay and any overpaid actual amount for a borrow position.
    ///
    /// **Purpose**: Calculates precise repayment allocation including interest and handles
    /// overpayment scenarios to ensure accurate debt reduction and fund protection.
    ///
    /// **Mathematical Process**:
    ///
    /// **Current Debt Calculation**:
    /// ```
    /// current_debt_actual = position_scaled_amount * current_borrow_index
    /// ```
    /// This includes both principal and all accrued interest up to the current moment.
    ///
    /// **Repayment Scenarios**:
    ///
    /// **Full Repayment or Overpayment**:
    /// ```
    /// if payment_amount >= current_debt_actual:
    ///     scaled_to_repay = position_scaled_amount (entire position)
    ///     overpayment = payment_amount - current_debt_actual
    ///     // User's debt completely cleared, excess refunded
    /// ```
    ///
    /// **Partial Repayment**:
    /// ```
    /// if payment_amount < current_debt_actual:
    ///     scaled_to_repay = payment_amount / current_borrow_index
    ///     overpayment = 0
    ///     remaining_scaled_debt = position_scaled_amount - scaled_to_repay
    /// ```
    ///
    /// **Interest Payment Mechanics**:
    /// Interest is automatically included in debt calculations through the borrow index:
    /// ```
    /// total_owed = principal_borrowed + accrued_interest
    /// accrued_interest = scaled_debt * (current_borrow_index - borrow_index_at_origination)
    /// ```
    ///
    /// **Proportional Debt Reduction**:
    /// For partial payments, the scaled debt reduction is proportional:
    /// ```
    /// payment_ratio = payment_amount / current_total_debt
    /// scaled_reduction = position_scaled_amount * payment_ratio
    /// ```
    ///
    /// **Overpayment Protection**:
    /// Prevents accidental loss of user funds by:
    /// - Calculating exact debt amount including interest
    /// - Automatically detecting overpayments
    /// - Ensuring refund of excess amounts
    /// - Clearing position completely when fully paid
    ///
    /// **Precision Handling**:
    /// Uses scaled arithmetic to maintain precision across:
    /// - Varying borrow index values
    /// - Different time periods since borrowing
    /// - Multiple partial repayments
    /// - Interest compounding effects
    ///
    /// # Arguments
    /// - `cache`: Current pool state with updated borrow index
    /// - `position_scaled_amount`: User's scaled debt token balance
    /// - `payment_amount_actual`: Repayment amount in asset decimals
    ///
    /// # Returns
    /// - `scaled_amount_to_repay`: Scaled debt tokens to burn from position
    /// - `over_paid_amount_actual`: Excess payment to refund to user
    ///
    /// **Security Considerations**:
    /// - Prevents overpayment loss through automatic refunds
    /// - Uses current borrow index for accurate debt calculation
    /// - Maintains scaling precision to prevent manipulation
    /// - Handles edge cases like zero payments safely
    #[inline(always)]
    fn determine_repayment_details(
        &self,
        cache: &Cache<Self>,
        position_scaled_amount: &ManagedDecimal<Self::Api, NumDecimals>,
        payment_amount_actual: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> (
        ManagedDecimal<Self::Api, NumDecimals>, // scaled_amount_to_repay
        ManagedDecimal<Self::Api, NumDecimals>, // over_paid_amount_actual
    ) {
        let current_debt_actual = cache.original_borrow(position_scaled_amount);

        if *payment_amount_actual >= current_debt_actual {
            // Full repayment or overpayment
            let over_paid = payment_amount_actual.clone() - current_debt_actual;
            (position_scaled_amount.clone(), over_paid)
        } else {
            // Partial repayment
            let payment_scaled = cache.scaled_borrow(payment_amount_actual);
            (payment_scaled, cache.zero.clone())
        }
    }

    /// Processes liquidation fees by deducting them from withdrawal amount and adding to protocol revenue.
    ///
    /// **Purpose**: Handles liquidation fee collection during collateral withdrawals,
    /// ensuring protocol revenue capture while maintaining user asset safety.
    ///
    /// **Liquidation Fee Economics**:
    /// Liquidation fees serve multiple purposes:
    /// - Compensate liquidators for gas costs and price risk
    /// - Generate protocol revenue from liquidation events
    /// - Create incentives for timely liquidation execution
    /// - Prevent excessive leverage through fee costs
    ///
    /// **Mathematical Process**:
    ///
    /// **Fee Validation and Deduction**:
    /// ```
    /// if is_liquidation && protocol_fee.is_some():
    ///     require(gross_withdrawal >= protocol_fee, "Insufficient withdrawal for fee")
    ///     net_transfer = gross_withdrawal - protocol_fee
    ///     protocol_revenue += protocol_fee (in scaled units)
    /// ```
    ///
    /// **Revenue Conversion**:
    /// ```
    /// fee_scaled = protocol_fee / current_supply_index
    /// cache.revenue += fee_scaled
    /// cache.supplied += fee_scaled  // Mint to total supply
    /// ```
    ///
    /// **Fee Sufficiency Check**:
    /// Critical validation to prevent negative transfers:
    /// ```
    /// require(amount_to_transfer >= liquidation_fee, ERROR_WITHDRAW_AMOUNT_LESS_THAN_FEE)
    /// ```
    ///
    /// **Liquidation Event Flow**:
    /// 1. Liquidator identifies undercollateralized position
    /// 2. Liquidator calls liquidation with fee parameter
    /// 3. Collateral withdrawal processes with fee deduction
    /// 4. Net amount transferred to liquidator
    /// 5. Fee amount added to protocol treasury
    ///
    /// **Fee Calculation Examples**:
    /// ```
    /// // 5% liquidation fee scenario:
    /// gross_withdrawal = 1000 USDC
    /// liquidation_fee = 50 USDC (5%)
    /// net_transfer = 950 USDC (to liquidator)
    /// protocol_revenue += 50 USDC (scaled)
    /// ```
    ///
    /// **Edge Case Handling**:
    /// - Non-liquidation withdrawals: No fee processing
    /// - Missing fee parameter: No fee deduction
    /// - Insufficient withdrawal amount: Transaction reverts
    /// - Zero fee: No-op processing
    ///
    /// **Revenue Accounting**:
    /// Fees are immediately converted to scaled supply tokens and added to:
    /// - Protocol revenue accumulator (for later claiming)
    /// - Total supplied amount (maintaining pool balance)
    ///
    /// # Arguments
    /// - `cache`: Mutable pool state for revenue updates
    /// - `is_liquidation`: Flag indicating liquidation context
    /// - `protocol_fee_asset_decimals_opt`: Optional liquidation fee amount
    /// - `amount_to_transfer_net_asset_decimals`: Mutable withdrawal amount to adjust
    ///
    /// **Security Considerations**:
    /// - Fee sufficiency validation prevents negative transfers
    /// - Revenue conversion maintains accurate pool accounting
    /// - Optional fee handling prevents forced fee scenarios
    /// - Scaled conversion preserves precision
    #[inline(always)]
    fn process_liquidation_fee_details(
        &self,
        cache: &mut Cache<Self>,
        is_liquidation: bool,
        protocol_fee_asset_decimals_opt: &Option<ManagedDecimal<Self::Api, NumDecimals>>,
        amount_to_transfer_net_asset_decimals: &mut ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        if is_liquidation {
            if let Some(protocol_fee_asset_decimals) = protocol_fee_asset_decimals_opt {
                require!(
                    *amount_to_transfer_net_asset_decimals >= *protocol_fee_asset_decimals,
                    ERROR_WITHDRAW_AMOUNT_LESS_THAN_FEE
                );

                *amount_to_transfer_net_asset_decimals -= protocol_fee_asset_decimals;

                self.internal_add_protocol_revenue(cache, protocol_fee_asset_decimals.clone());
            }
        }
    }

    /// Adds protocol revenue to the pool by minting scaled supply tokens to the treasury.
    ///
    /// **Purpose**: Converts protocol revenue from various sources into scaled supply tokens,
    /// effectively minting treasury shares that appreciate with the supply index.
    ///
    /// **Revenue Tokenization Process**:
    ///
    /// **Scaling Conversion**:
    /// ```
    /// fee_scaled = revenue_amount / current_supply_index
    /// ```
    /// This converts the actual revenue amount into scaled tokens at current rates.
    ///
    /// **Treasury Minting**:
    /// ```
    /// cache.revenue += fee_scaled         // Add to protocol treasury
    /// cache.supplied += fee_scaled        // Increase total supply
    /// ```
    ///
    /// **Revenue Appreciation Mechanism**:
    /// Protocol revenue is stored as scaled supply tokens that:
    /// - Appreciate in value as the supply index grows
    /// - Earn interest alongside user deposits
    /// - Maintain proportional value through market cycles
    /// - Can be claimed at any time by the protocol owner
    ///
    /// **Mathematical Properties**:
    /// ```
    /// // At revenue addition:
    /// treasury_value = fee_scaled * current_supply_index = revenue_amount
    ///
    /// // At future claim time:
    /// treasury_value = fee_scaled * future_supply_index > revenue_amount
    /// protocol_earnings = fee_scaled * (future_supply_index - current_supply_index)
    /// ```
    ///
    /// **Revenue Sources Integration**:
    /// This function handles revenue from multiple sources:
    /// - Interest rate spreads (reserve factor portion)
    /// - Flash loan fees
    /// - Strategy creation fees
    /// - Liquidation fees
    /// - Dust collateral seizures
    ///
    /// **Supply Index Impact**:
    /// Adding revenue does NOT dilute existing suppliers because:
    /// - Revenue represents value already earned by the pool
    /// - Scaled token minting maintains proportional ownership
    /// - Supply index reflects the new total value correctly
    ///
    /// **Zero Amount Handling**:
    /// Efficiently skips processing for zero amounts to save gas costs.
    ///
    /// # Arguments
    /// - `cache`: Mutable pool state for revenue and supply updates
    /// - `amount`: Revenue amount in asset decimals to add
    ///
    /// **Security Considerations**:
    /// - Zero check prevents unnecessary computation
    /// - Scaling conversion maintains precision
    /// - Treasury and supply updates are atomic
    /// - No dilution of existing user positions
    ///
    fn internal_add_protocol_revenue(
        &self,
        cache: &mut Cache<Self>,
        amount: ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        if amount == cache.zero {
            return;
        }

        // Convert directly to scaled units
        let fee_scaled = cache.scaled_supply(&amount);

        // Mint to treasury and total supply
        cache.revenue += &fee_scaled;
        cache.supplied += &fee_scaled;
    }
}
