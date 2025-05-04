multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use crate::{cache::Cache, rates, storage, view};

use common_errors::{ERROR_INVALID_ASSET, ERROR_INVALID_FLASHLOAN_REPAYMENT};
use common_structs::*;

/// The UtilsModule trait contains helper functions for updating interest indexes,
/// computing interest factors, and adjusting account positions with accrued interest.
#[multiversx_sc::module]
pub trait UtilsModule:
    rates::InterestRates
    + storage::Storage
    + common_events::EventsModule
    + view::ViewModule
    + common_math::SharedMathModule
{
    /// Splits a repayment into principal, interest, and overpayment components.
    ///
    /// **Scope**: Determines how a repayment amount is allocated to clear interest and principal,
    /// handling both partial and full repayments with potential overpayment.
    ///
    /// **Goal**: Accurately distribute repayment to reduce debt, ensuring fairness and transparency.
    ///
    /// **Process**:
    /// - If repayment exceeds total debt, returns principal, interest, and overpayment.
    /// - Otherwise, prioritizes interest, then principal, with no overpayment.
    ///
    /// # Arguments
    /// - `repayment`: Total repayment amount (`ManagedDecimal<Self::Api, NumDecimals>`).
    /// - `position`: Reference to the account position (`AccountPosition<Self::Api>`).
    /// - `cache`: Mutable reference to the pool state (`Cache<Self>`).
    ///
    /// # Returns
    /// - `(principal_repaid, interest_repaid, over_repaid)`: Tuple of `ManagedDecimal<Self::Api, NumDecimals>`:
    ///   - `principal_repaid`: Amount reducing principal.
    ///   - `interest_repaid`: Amount reducing interest.
    ///   - `over_repaid`: Excess amount beyond total debt.
    ///
    fn split_repay(
        &self,
        repayment: &ManagedDecimal<Self::Api, NumDecimals>,
        position: &AccountPosition<Self::Api>,
        cache: &mut Cache<Self>,
    ) -> (
        ManagedDecimal<Self::Api, NumDecimals>, // principal_repaid
        ManagedDecimal<Self::Api, NumDecimals>, // interest_repaid
        ManagedDecimal<Self::Api, NumDecimals>, // over_repaid
    ) {
        if repayment >= &position.get_total_amount() {
            // Full repayment with possible overpayment.
            let over_repaid = repayment.clone() - position.get_total_amount();
            // The entire outstanding debt is cleared.
            (
                position.principal_amount.clone(),
                position.interest_accrued.clone(),
                over_repaid,
            )
        } else {
            // Partial repayment: first cover interest, then principal.
            let interest_repaid = if repayment > &position.interest_accrued {
                position.interest_accrued.clone()
            } else {
                repayment.clone()
            };
            let principal_repaid = repayment.clone() - interest_repaid.clone();
            (principal_repaid, interest_repaid, cache.zero.clone())
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
    fn emit_market_update(
        &self,
        cache: &Cache<Self>,
        asset_price: &ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        self.update_market_state_event(
            cache.timestamp,
            &cache.supply_index,
            &cache.borrow_index,
            &cache.reserves,
            &cache.supplied,
            &cache.borrowed,
            &cache.revenue,
            &cache.pool_asset,
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
    fn send_asset(
        &self,
        cache: &Cache<Self>,
        amount: &ManagedDecimal<Self::Api, NumDecimals>,
        to: &ManagedAddress,
    ) -> EgldOrEsdtTokenPayment<Self::Api> {
        let payment = EgldOrEsdtTokenPayment::new(
            cache.pool_asset.clone(),
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

    /// Caps a withdrawal amount to prevent exceeding the position's total.
    ///
    /// **Scope**: Limits the amount a user can withdraw to their available balance.
    ///
    /// **Goal**: Prevent over-withdrawal, ensuring the contract remains solvent.
    ///
    /// # Arguments
    /// - `amount`: Mutable reference to the requested withdrawal amount (`ManagedDecimal<Self::Api, NumDecimals>`).
    /// - `position`: Reference to the account position (`AccountPosition<Self::Api>`).
    ///
    fn cap_withdrawal_amount(
        &self,
        amount: &mut ManagedDecimal<Self::Api, NumDecimals>,
        position: &AccountPosition<Self::Api>,
    ) {
        if *amount > position.get_total_amount() {
            *amount = position.get_total_amount();
        }
    }

    /// Calculates withdrawal amounts, including principal, interest, and total, with liquidation handling.
    ///
    /// **Scope**: Computes how a withdrawal affects a position, adjusting for interest and liquidation fees if applicable.
    ///
    /// **Goal**: Provide precise withdrawal details, ensuring correct accounting and fee application.
    ///
    /// **Process**:
    /// - Calculates extra interest accrued.
    /// - Splits withdrawal into principal and interest.
    /// - Adjusts total for liquidation fees if present.
    ///
    /// # Arguments
    /// - `amount`: Requested withdrawal amount (`ManagedDecimal<Self::Api, NumDecimals>`).
    /// - `position`: Reference to the account position (`AccountPosition<Self::Api>`).
    /// - `cache`: Mutable reference to the pool state (`Cache<Self>`).
    /// - `is_liquidation`: Flag indicating if this is a liquidation withdrawal (`bool`).
    /// - `protocol_fee_opt`: Optional protocol fee for liquidations (`Option<ManagedDecimal<Self::Api, NumDecimals>>`).
    ///
    /// # Returns
    /// - `(principal, interest, to_withdraw)`: Tuple of `ManagedDecimal<Self::Api, NumDecimals>`:
    ///   - `principal`: Amount reducing principal.
    ///   - `interest`: Amount reducing interest.
    ///   - `to_withdraw`: Total amount to transfer (net of fees).
    ///
    /// **Security Tip**: Relies on `split_repay` for safe arithmetic
    fn calc_withdrawal_amounts(
        &self,
        amount: &ManagedDecimal<Self::Api, NumDecimals>,
        position: &AccountPosition<Self::Api>,
        cache: &mut Cache<Self>,
        is_liquidation: bool,
        protocol_fee_opt: Option<ManagedDecimal<Self::Api, NumDecimals>>,
    ) -> (
        ManagedDecimal<Self::Api, NumDecimals>,
        ManagedDecimal<Self::Api, NumDecimals>,
        ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        let (principal, interest, _) = self.split_repay(amount, position, cache);
        let mut to_withdraw = amount.clone();
        if is_liquidation && protocol_fee_opt.is_some() {
            let protocol_fees = protocol_fee_opt.unwrap();
            cache.revenue += &protocol_fees;
            cache.reserves += &protocol_fees;
            to_withdraw -= &protocol_fees;
        }
        (principal, interest, to_withdraw)
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
        amount: &ManagedDecimal<Self::Api, NumDecimals>,
        required_repayment: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let repayment = if cache.pool_asset.is_egld() {
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
            repayment >= *required_repayment && amount < required_repayment,
            ERROR_INVALID_FLASHLOAN_REPAYMENT
        );

        repayment
    }
}
