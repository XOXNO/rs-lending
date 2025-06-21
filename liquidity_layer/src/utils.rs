multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use crate::{cache::Cache, storage, view};

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
    /// **Scope**: Synchronizes the global state of the pool by recalculating borrow and supply indexes,
    /// factoring in interest growth over time and distributing rewards.
    ///
    /// **Goal**: Keep the pool's financial state current, ensuring accurate interest accrual and reward distribution.
    ///
    /// **Process**:
    /// 1. Computes time delta (`delta`) since `cache.last_timestamp`.
    /// 2. If `delta > 0`:
    ///    a. Calculates the current `borrow_rate` using `cache.get_utilization()` and `cache.params`.
    ///    b. Computes the `borrow_factor` using `calculate_compounded_interest(borrow_rate, delta)`.
    ///    c. Updates `cache.borrow_index` via `update_borrow_index`, storing the `old_borrow_index`.
    ///    d. Calculates `rewards` for suppliers using `calc_supplier_rewards(cache, old_borrow_index)`.
    ///    e. Updates `cache.supply_index` via `update_supply_index(cache, rewards)`.
    ///    f. Sets `cache.last_timestamp` to the current `cache.timestamp`.
    ///
    /// # Arguments
    /// - `cache`: Mutable reference to the pool state (`Cache<Self>`), holding timestamps, indexes, and all relevant financial figures.
    ///
    /// **Security Tip**: Skips updates if `delta == 0`, preventing redundant computation. Protected by caller ensuring valid `cache`. The sequence of operations ensures that rewards are calculated based on interest accrued in the current period.
    fn global_sync(&self, cache: &mut Cache<Self>) {
        let delta = cache.timestamp - cache.last_timestamp;

        if delta > 0 {
            let borrow_rate = self.calc_borrow_rate(cache.get_utilization(), cache.params.clone());
            let borrow_factor = self.calculate_compounded_interest(borrow_rate.clone(), delta);
            let (new_borrow_index, old_borrow_index) =
                self.update_borrow_index(cache.borrow_index.clone(), borrow_factor.clone());

            // 3 raw split
            let (supplier_rewards_ray, protocol_fee_ray, new_bad_debt) = self
                .calc_supplier_rewards(
                    cache.params.clone(),
                    &cache.borrowed,
                    cache.bad_debt.clone(),
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
            cache.bad_debt = new_bad_debt;

            if protocol_fee_ray > self.ray_zero() {
                let fee_scaled = cache.scaled_supply(&protocol_fee_ray);
                cache.revenue += &fee_scaled;
                cache.supplied += &fee_scaled; // mint to total supply
            }

            cache.last_timestamp = cache.timestamp;
        }
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
            &cache.bad_debt,
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
        let repayment = if cache.params.asset_id.is_egld() {
            cache.get_decimal_value(&back_transfers.total_egld_amount)
        } else {
            require!(
                back_transfers.esdt_payments.len() == 1,
                ERROR_INVALID_FLASHLOAN_REPAYMENT
            );
            let payment = back_transfers.esdt_payments.get(0);
            require!(
                cache.is_same_asset(&EgldOrEsdtTokenIdentifier::esdt(
                    payment.token_identifier.clone()
                )),
                ERROR_INVALID_FLASHLOAN_REPAYMENT
            );

            cache.get_decimal_value(&payment.amount)
        };

        require!(
            repayment >= *required_repayment,
            ERROR_INVALID_FLASHLOAN_REPAYMENT
        );

        repayment
    }

    /// Determines the gross scaled and actual amounts for a withdrawal operation.
    ///
    /// **Scope**: Calculates how much of a user's position should be considered for withdrawal,
    /// based on their requested amount versus their total current supply (including interest).
    ///
    /// **Goal**: To provide a clear calculation of withdrawal amounts before applying any fees or further checks.
    ///
    /// **Process**:
    /// 1. Calculates the user's `current_supply_actual` (original value of their scaled position including interest).
    /// 2. If `requested_amount_actual` is greater than or equal to `current_supply_actual` (full withdrawal):
    ///    - `scaled_withdrawal_amount_gross` becomes the user's entire `position_scaled_amount`.
    ///    - `amount_to_withdraw_gross` becomes `current_supply_actual`.
    /// 3. Else (partial withdrawal):
    ///    - `scaled_withdrawal_amount_gross` becomes the scaled equivalent of `requested_amount_actual`.
    ///    - `amount_to_withdraw_gross` becomes `requested_amount_actual`.
    ///
    /// # Arguments
    /// - `cache`: A reference to the current pool state (`Cache<Self>`), used for index and scaling information.
    /// - `position_scaled_amount`: The scaled amount of the user's supply position.
    /// - `requested_amount_actual`: The actual amount the user has requested to withdraw.
    ///
    /// # Returns
    /// A tuple `(scaled_withdrawal_amount_gross, amount_to_withdraw_gross)`:
    /// - `scaled_withdrawal_amount_gross`: The portion of the user's scaled position to be withdrawn.
    /// - `amount_to_withdraw_gross`: The actual gross amount of tokens to be withdrawn, before any fees.
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
    /// **Scope**: Calculates the effect of a payment on a borrow position, identifying how much of the
    /// scaled debt is covered and if there's any overpayment.
    ///
    /// **Goal**: To provide a clear calculation of repayment effects before updating pool and position states.
    ///
    /// **Process**:
    /// 1. Calculates the user's `current_debt_actual` (original value of their scaled position including interest).
    /// 2. If `payment_amount_actual` is greater than or equal to `current_debt_actual` (full repayment or overpayment):
    ///    - `scaled_amount_to_repay` becomes the user's entire `position_scaled_amount`.
    ///    - `over_paid_amount_actual` is calculated as `payment_amount_actual - current_debt_actual`.
    /// 3. Else (partial repayment):
    ///    - `scaled_amount_to_repay` becomes the scaled equivalent of `payment_amount_actual`.
    ///    - `over_paid_amount_actual` is zero.
    ///
    /// # Arguments
    /// - `cache`: A reference to the current pool state (`Cache<Self>`), used for index, scaling information, and zero value.
    /// - `position_scaled_amount`: The scaled amount of the user's borrow position.
    /// - `payment_amount_actual`: The actual amount the user has paid.
    ///
    /// # Returns
    /// A tuple `(scaled_amount_to_repay, over_paid_amount_actual)`:
    /// - `scaled_amount_to_repay`: The portion of the user's scaled debt that is repaid.
    /// - `over_paid_amount_actual`: The actual amount overpaid by the user, if any.
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

    /// Calculates and applies any applicable liquidation fee, updating the net transfer amount directly
    /// and adding the fee to protocol revenue within the cache.
    ///
    /// **Scope**: Modifies the net withdrawal amount if a liquidation fee applies and updates `cache.revenue`
    /// with the fee amount.
    ///
    /// **Goal**: To encapsulate the logic for applying liquidation fees, directly adjusting the transfer amount
    /// and centralizing the revenue update.
    ///
    /// **Process**:
    /// 1. If `is_liquidation` is true and `protocol_fee_asset_decimals_opt` contains a fee:
    ///    a. Retrieves the `protocol_fee_asset_decimals`.
    ///    b. Requires that the initial `amount_to_transfer_net_asset_decimals` (gross withdrawal) is sufficient to cover the fee.
    ///    c. Calculates `protocol_fee_for_revenue_ray` by rescaling `protocol_fee_asset_decimals` to RAY precision.
    ///    d. Deducts `protocol_fee_asset_decimals` directly from the mutable `amount_to_transfer_net_asset_decimals`.
    ///    e. If `protocol_fee_for_revenue_ray` is greater than zero, adds it to `cache.revenue`.
    ///
    /// # Arguments
    /// - `cache`: A mutable reference to the current pool state (`Cache<Self>`), used for updating revenue.
    /// - `is_liquidation`: A boolean indicating if the withdrawal is part of a liquidation.
    /// - `protocol_fee_asset_decimals_opt`: An optional `ManagedDecimal` representing the liquidation fee in asset decimals.
    /// - `amount_to_transfer_net_asset_decimals`: A mutable reference to the gross amount to be withdrawn (in asset decimals).
    ///                                            This value will be reduced by the fee if applicable.
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

    /// Adds protocol revenue to the pool.
    ///
    /// **Scope**: Updates the pool's revenue and total supply by adding a portion of the incoming payment.
    ///
    /// **Goal**: To handle incoming payments that cover part of the bad debt, converting the remainder to scaled units.
    ///

    fn internal_add_protocol_revenue(
        &self,
        cache: &mut Cache<Self>,
        mut amount: ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        if amount == cache.zero {
            return;
        }

        if amount <= cache.bad_debt {
            // Entire incoming payment covers part (or all) of bad debt.
            cache.bad_debt -= &amount;
        } else {
            // Part of the payment clears bad debt, remainder is protocol revenue.
            amount -= &cache.bad_debt;
            cache.bad_debt = cache.zero.clone();

            // Convert remainder directly to scaled units – precision handled inside math helper.
            let fee_scaled = cache.scaled_supply(&amount);

            // Mint to treasury and total supply.
            cache.revenue += &fee_scaled;
            cache.supplied += &fee_scaled;
        }
    }
}
