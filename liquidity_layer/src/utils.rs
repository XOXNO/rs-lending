multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use crate::{cache::Cache, rates, storage, view};

use common_constants::RAY_PRECISION;
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
    /// Calculates supplier rewards by deducting protocol fees from accrued interest.
    ///
    /// **Scope**: This function computes the rewards suppliers earn from interest paid by borrowers,
    /// after the protocol takes its share (reserve factor). It’s used during index updates to distribute profits.
    ///
    /// **Goal**: Ensure suppliers receive their fair share of interest while updating protocol revenue.
    ///
    /// **Formula**:
    /// - Accrued interest = `borrowed * (borrow_index / old_borrow_index - 1)`
    /// - Rewards = `accrued_interest * (1 - reserve_factor)`
    ///
    /// # Arguments
    /// - `cache`: Mutable reference to the pool state (`Cache<Self>`), containing borrow amounts, indexes, and params.
    /// - `old_borrow_index`: The borrow index before the current update (`ManagedDecimal<Self::Api, NumDecimals>`).
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: Net rewards for suppliers after protocol fees.
    ///
    /// **Security Tip**: No direct `require!` checks here; relies on upstream validation of `cache` state (e.g., in `global_sync`).
    fn calc_supplier_rewards(
        &self,
        cache: &mut Cache<Self>,
        old_borrow_index: ManagedDecimal<Self::Api, NumDecimals>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        // 1. Calculate new borrowed amount
        let accrued_interest =
            self.calc_interest(&cache.borrowed, &cache.borrow_index, &old_borrow_index);

        // 2. Calculate protocol's share
        let protocol_fee = self
            .mul_half_up(
                &accrued_interest,
                &cache.params.reserve_factor,
                RAY_PRECISION,
            )
            .rescale(cache.params.asset_decimals);
        // 3. Update reserves
        cache.revenue += &protocol_fee;

        // 4. Return suppliers' share
        accrued_interest - protocol_fee
    }

    /// Updates both borrow and supply indexes based on elapsed time since the last update.
    ///
    /// **Scope**: Synchronizes the global state of the pool by recalculating borrow and supply indexes,
    /// factoring in interest growth over time and distributing rewards.
    ///
    /// **Goal**: Keep the pool’s financial state current, ensuring accurate interest accrual and reward distribution.
    ///
    /// **Process**:
    /// 1. Computes time delta since last update.
    /// 2. Updates borrow index using growth factor.
    /// 3. Calculates supplier rewards.
    /// 4. Updates supply index with rewards.
    /// 5. Refreshes last update timestamp.
    ///
    /// # Arguments
    /// - `cache`: Mutable reference to the pool state (`Cache<Self>`), holding timestamps and indexes.
    ///
    /// **Security Tip**: Skips updates if `delta == 0`, preventing redundant computation. Protected by caller ensuring valid `cache`.
    fn global_sync(&self, cache: &mut Cache<Self>) {
        let delta = cache.timestamp - cache.last_timestamp;

        if delta > 0 {
            let factor = self.growth_factor(cache, delta);

            let old_borrow_index = self.update_borrow_index(cache, factor);

            let rewards = self.calc_supplier_rewards(cache, old_borrow_index);

            self.update_supply_index(rewards, cache);

            // Update the last used timestamp
            cache.last_timestamp = cache.timestamp;
        }
    }

    /// Calculates accrued interest for a position since its last update.
    ///
    /// **Scope**: Computes interest accrued on a principal amount based on the change in market index since the last recorded index.
    ///
    /// **Goal**: Provide an accurate interest calculation for updating positions or determining rewards/debts.
    ///
    /// **Formula**:
    /// - `interest = amount * (current_index / account_position_index - 1)`
    /// - Result is rescaled to match `amount`’s asset_decimals.
    ///
    /// # Arguments
    /// - `amount`: Principal amount (`ManagedDecimal<Self::Api, NumDecimals>`).
    /// - `current_index`: Latest market index (`ManagedDecimal<Self::Api, NumDecimals>`).
    /// - `account_position_index`: Position’s last recorded index (`ManagedDecimal<Self::Api, NumDecimals>`).
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: Accrued interest since the last update.
    ///
    /// **Security Tip**: No division-by-zero risk if `account_position_index` is non-zero, ensured by upstream position initialization.
    fn calc_interest(
        &self,
        amount: &ManagedDecimal<Self::Api, NumDecimals>, // Amount of the asset
        current_index: &ManagedDecimal<Self::Api, NumDecimals>, // Market index
        account_position_index: &ManagedDecimal<Self::Api, NumDecimals>, // Account position index
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let numerator = self.mul_half_up(amount, current_index, RAY_PRECISION);
        let new_amount = self
            .div_half_up(&numerator, account_position_index, RAY_PRECISION)
            .rescale(amount.scale());

        new_amount.sub(amount.clone())
    }

    /// Updates an account position with accrued interest based on current market indexes.
    ///
    /// **Scope**: Adjusts a user’s deposit or borrow position by calculating and adding accrued interest,
    /// updating timestamps and indexes accordingly.
    ///
    /// **Goal**: Ensure a position reflects the latest interest accrued, handling edge cases like zero amounts.
    ///
    /// **Process**:
    /// - Selects supply or borrow index based on position type.
    /// - For zero-amount positions, resets timestamp and index.
    /// - For non-zero amounts, calculates and adds interest.
    ///
    /// # Arguments
    /// - `position`: Mutable reference to the account position (`AccountPosition<Self::Api>`).
    /// - `cache`: Mutable reference to the pool state (`Cache<Self>`).
    ///
    /// **Security Tip**: Handles zero amounts safely by resetting state, protected by caller ensuring valid position type.
    fn position_sync(&self, position: &mut AccountPosition<Self::Api>, cache: &mut Cache<Self>) {
        let is_supply = position.position_type == AccountPositionType::Deposit;
        let index = if is_supply {
            cache.supply_index.clone()
        } else {
            cache.borrow_index.clone()
        };

        if position.get_total_amount().eq(&cache.zero) {
            position.last_update_timestamp = cache.timestamp;
            position.market_index = index.clone();
            return;
        }
        let accumulated_interest =
            self.calc_interest(&position.get_total_amount(), &index, &position.market_index);

        if accumulated_interest.gt(&cache.zero) {
            position.interest_accrued += &accumulated_interest;
            position.last_update_timestamp = cache.timestamp;
            position.market_index = index.clone();
        }
    }

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
            let remaining = repayment.clone() - interest_repaid.clone();
            let principal_repaid = if remaining > position.principal_amount.clone() {
                position.principal_amount.clone()
            } else {
                remaining
            };
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
    /// **Scope**: Extracts the payment amount (EGLD or ESDT) and ensures it matches the pool’s asset.
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

    /// Caps a withdrawal amount to prevent exceeding the position’s total.
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
        let extra = self.calc_interest(amount, &cache.supply_index, &position.market_index);
        let (principal, interest, _) = self.split_repay(amount, position, cache);
        let mut to_withdraw = amount.clone() + extra;
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
    /// **Goal**: Secure the flash loan process by enforcing repayment conditions, protecting the pool’s funds.
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
