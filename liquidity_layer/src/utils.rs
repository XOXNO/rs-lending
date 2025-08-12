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
    /// Updates borrow and supply indexes based on time elapsed and current utilization.
    /// Distributes accrued interest between suppliers and protocol based on reserve factor.
    /// Synchronizes all pool state for accurate interest calculations.
    fn global_sync(&self, cache: &mut Cache<Self>) {
        let delta_ms = cache.timestamp - cache.last_timestamp;

        if delta_ms > 0 {
            let borrow_rate =
                self.calculate_borrow_rate(cache.calculate_utilization(), cache.parameters.clone());
            let borrow_factor = self.calculate_compounded_interest(borrow_rate.clone(), delta_ms);
            let (new_borrow_index, old_borrow_index) =
                self.update_borrow_index(cache.borrow_index_ray.clone(), borrow_factor.clone());

            // Calculate supplier rewards and protocol fees directly
            let (supplier_rewards_ray, protocol_fee_ray) = self.calculate_supplier_rewards(
                cache.parameters.clone(),
                &cache.borrowed_ray,
                &new_borrow_index,
                &old_borrow_index,
            );

            let new_supply_index = self.update_supply_index(
                cache.supplied_ray.clone(),
                cache.supply_index_ray.clone(),
                supplier_rewards_ray,
            );

            cache.supply_index_ray = new_supply_index;
            cache.borrow_index_ray = new_borrow_index;

            self.internal_add_protocol_revenue(cache, protocol_fee_ray);

            cache.last_timestamp = cache.timestamp;
        }
    }

    /// Immediately socializes bad debt by reducing supply index proportionally.
    /// All suppliers share losses based on their scaled token holdings.
    /// Prevents supplier flight during insolvency events.
    fn apply_bad_debt_to_supply_index(
        &self,
        cache: &mut Cache<Self>,
        bad_debt_amount_ray: ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        // Calculate total supplied value in RAY precision
        let total_supplied_value_ray = cache.calculate_original_supply_ray(&cache.supplied_ray);
        // Convert bad debt to RAY precision
        // Cap bad debt to available value (prevent negative results)
        let capped_bad_debt_ray = self.min(bad_debt_amount_ray, total_supplied_value_ray.clone());

        // Calculate remaining value after bad debt
        let remaining_value_ray = total_supplied_value_ray.clone() - capped_bad_debt_ray;

        // Calculate reduction factor: remaining_value / total_value
        let reduction_factor_ray = self.div_half_up(
            &remaining_value_ray,
            &total_supplied_value_ray,
            RAY_PRECISION,
        );

        // Apply reduction to supply index
        let new_supply_index_ray = self.mul_half_up(
            &cache.supply_index_ray,
            &reduction_factor_ray,
            RAY_PRECISION,
        );

        // Ensure minimum supply index (prevent total collapse but allow significant reduction)
        let min_supply_index_ray = self.to_decimal(BigUint::from(1u64), RAY_PRECISION); // 1e-27, very small but > 0
        cache.supply_index_ray = self.max(new_supply_index_ray, min_supply_index_ray);
    }

    /// Emits market state event with current indexes, reserves, and asset price.
    /// Provides transparency for market participants and auditors.
    fn emit_market_update(
        &self,
        cache: &Cache<Self>,
        asset_price: &ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        let reserves = cache.calculate_reserves();
        self.update_market_state_event(
            cache.timestamp,
            &cache.supply_index_ray,
            &cache.borrow_index_ray,
            &reserves,
            &cache.supplied_ray,
            &cache.borrowed_ray,
            &cache.revenue_ray,
            &cache.parameters.asset_id,
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

    fn send_asset(
        &self,
        cache: &Cache<Self>,
        amount: &ManagedDecimal<Self::Api, NumDecimals>,
        to: &ManagedAddress,
    ) -> EgldOrEsdtTokenPayment<Self::Api> {
        let payment = EgldOrEsdtTokenPayment::new(
            cache.parameters.asset_id.clone(),
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
    fn payment_amount(&self, cache: &Cache<Self>) -> ManagedDecimal<Self::Api, NumDecimals> {
        let (asset, amount) = self.call_value().egld_or_single_fungible_esdt();

        require!(cache.is_same_asset(&asset), ERROR_INVALID_ASSET);

        cache.decimal_value(&amount)
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

        let repayment = cache.decimal_value(&payment.amount);

        require!(
            repayment >= *required_repayment,
            ERROR_INVALID_FLASHLOAN_REPAYMENT
        );

        repayment
    }

    /// Calculates scaled and actual amounts for withdrawal operation.
    /// Handles full withdrawals (capped at position value) and partial withdrawals.
    /// Returns scaled tokens to burn and actual amount to transfer.
    fn calculate_gross_withdrawal_amounts(
        &self,
        cache: &Cache<Self>,
        position_scaled_amount: &ManagedDecimal<Self::Api, NumDecimals>,
        requested_amount_actual: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> (
        ManagedDecimal<Self::Api, NumDecimals>, // scaled_withdrawal_amount_gross
        ManagedDecimal<Self::Api, NumDecimals>, // amount_to_withdraw_gross
    ) {
        let current_supply_actual = cache.calculate_original_supply(position_scaled_amount);

        if *requested_amount_actual >= current_supply_actual {
            // Full withdrawal
            (position_scaled_amount.clone(), current_supply_actual)
        } else {
            // Partial withdrawal
            let requested_scaled = cache.calculate_scaled_supply(requested_amount_actual);
            (requested_scaled, requested_amount_actual.clone())
        }
    }

    /// Calculates repayment allocation and overpayment refund.
    /// Full repayment clears position, partial repayment scales proportionally.
    /// Returns scaled debt to burn and overpayment to refund.
    fn calculate_repayment_details(
        &self,
        cache: &Cache<Self>,
        position_scaled_amount: &ManagedDecimal<Self::Api, NumDecimals>,
        payment_amount_actual: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> (
        ManagedDecimal<Self::Api, NumDecimals>, // scaled_amount_to_repay
        ManagedDecimal<Self::Api, NumDecimals>, // over_paid_amount_actual
    ) {
        let current_debt_actual = cache.calculate_original_borrow(position_scaled_amount);

        if *payment_amount_actual >= current_debt_actual {
            // Full repayment or overpayment
            let over_paid = payment_amount_actual.clone() - current_debt_actual;
            (position_scaled_amount.clone(), over_paid)
        } else {
            // Partial repayment
            let payment_scaled = cache.calculate_scaled_borrow(payment_amount_actual);
            (payment_scaled, cache.zero.clone())
        }
    }

    /// Deducts liquidation fees from withdrawal amount during liquidations.
    /// Adds fees to protocol revenue and reduces net transfer to liquidator.
    /// Only processes fees when is_liquidation flag is true.
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

    /// Converts revenue to scaled supply tokens and adds to protocol treasury.
    /// Mints treasury shares that appreciate with supply index.
    /// Revenue sources include fees, spreads, and liquidation penalties.
    fn internal_add_protocol_revenue(
        &self,
        cache: &mut Cache<Self>,
        amount: ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        if amount == cache.zero {
            return;
        }

        // Convert directly to scaled units
        let fee_scaled = cache.calculate_scaled_supply(&amount);

        // Mint to treasury and total supply
        cache.revenue_ray += &fee_scaled;
        cache.supplied_ray += &fee_scaled;
    }
}
