multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use crate::errors::*;
use common_structs::*;

use super::{contexts::base::StorageCache, liq_math, liq_storage, liq_utils, view};

#[multiversx_sc::module]
pub trait LiquidityModule:
    liq_storage::StorageModule
    + common_tokens::AccountTokenModule
    + liq_utils::UtilsModule
    + common_events::EventsModule
    + liq_math::MathModule
    + view::ViewModule
    + common_checks::ChecksModule
{
    #[only_owner]
    #[endpoint(updatePositionInterest)]
    fn update_collateral_with_interest(
        &self,
        deposit_position: AccountPosition<Self::Api>,
    ) -> AccountPosition<Self::Api> {
        let mut storage_cache = StorageCache::new(self);
        let account =
            self.internal_update_collateral_with_interest(deposit_position, &mut storage_cache);
        self.update_market_state_event(
            storage_cache.timestamp,
            &storage_cache.supply_index,
            &storage_cache.borrow_index,
            &storage_cache.reserves_amount,
            &storage_cache.supplied_amount,
            &storage_cache.borrowed_amount,
            &storage_cache.rewards_reserve,
            &storage_cache.pool_asset,
        );
        account
    }

    #[only_owner]
    #[endpoint(updatePositionDebt)]
    fn update_borrows_with_debt(
        &self,
        borrow_position: AccountPosition<Self::Api>,
    ) -> AccountPosition<Self::Api> {
        let mut storage_cache = StorageCache::new(self);
        let account = self.internal_update_borrows_with_debt(borrow_position, &mut storage_cache);
        self.update_market_state_event(
            storage_cache.timestamp,
            &storage_cache.supply_index,
            &storage_cache.borrow_index,
            &storage_cache.reserves_amount,
            &storage_cache.supplied_amount,
            &storage_cache.borrowed_amount,
            &storage_cache.rewards_reserve,
            &storage_cache.pool_asset,
        );
        account
    }

    #[only_owner]
    #[payable("*")]
    #[endpoint(supply)]
    fn supply(&self, deposit_position: AccountPosition<Self::Api>) -> AccountPosition<Self::Api> {
        let mut storage_cache = StorageCache::new(self);

        let (deposit_asset, deposit_amount) = self.call_value().single_fungible_esdt();
        let mut ret_deposit_position = deposit_position.clone();

        require!(
            deposit_asset == storage_cache.pool_asset,
            ERROR_INVALID_ASSET
        );

        self.update_interest_indexes(&mut storage_cache);

        if deposit_position.amount != 0 {
            ret_deposit_position =
                self.internal_update_collateral_with_interest(deposit_position, &mut storage_cache);
        }

        ret_deposit_position.amount += &deposit_amount;
        ret_deposit_position.timestamp = storage_cache.timestamp;
        ret_deposit_position.index = storage_cache.supply_index.clone();

        sc_print!("deposit_amount: {}", deposit_amount);
        storage_cache.reserves_amount += &deposit_amount;
        sc_print!("storage_cache.reserves_amount: {}", storage_cache.reserves_amount);
        storage_cache.supplied_amount += deposit_amount;

        self.update_market_state_event(
            storage_cache.timestamp,
            &storage_cache.supply_index,
            &storage_cache.borrow_index,
            &storage_cache.reserves_amount,
            &storage_cache.supplied_amount,
            &storage_cache.borrowed_amount,
            &storage_cache.rewards_reserve,
            &storage_cache.pool_asset,
        );

        ret_deposit_position
    }

    #[only_owner]
    #[payable("*")]
    #[endpoint(borrow)]
    fn borrow(
        &self,
        initial_caller: &ManagedAddress,
        borrow_amount: &BigUint,
        existing_borrow_position: AccountPosition<Self::Api>,
    ) -> AccountPosition<Self::Api> {
        let mut storage_cache = StorageCache::new(self);

        let mut ret_borrow_position = existing_borrow_position.clone();
        self.require_non_zero_address(initial_caller);

        require!(
            &storage_cache.reserves_amount >= borrow_amount,
            ERROR_INSUFFICIENT_LIQUIDITY
        );

        self.update_interest_indexes(&mut storage_cache);

        if ret_borrow_position.amount != 0 {
            ret_borrow_position = self
                .internal_update_borrows_with_debt(existing_borrow_position, &mut storage_cache);
        }

        ret_borrow_position.amount += borrow_amount;
        ret_borrow_position.timestamp = storage_cache.timestamp;
        ret_borrow_position.index = storage_cache.borrow_index.clone();

        storage_cache.borrowed_amount += borrow_amount;
        sc_print!("borrow_amount.borrowed_amount: {}", borrow_amount);
        storage_cache.reserves_amount -= borrow_amount;
        sc_print!("storage_cache.reserves_amount: {}", storage_cache.reserves_amount);

        self.tx()
            .to(initial_caller)
            .payment(EgldOrEsdtTokenPayment::new(
                storage_cache.pool_asset.clone(),
                0,
                borrow_amount.clone(),
            ))
            .transfer();

        self.update_market_state_event(
            storage_cache.timestamp,
            &storage_cache.supply_index,
            &storage_cache.borrow_index,
            &storage_cache.reserves_amount,
            &storage_cache.supplied_amount,
            &storage_cache.borrowed_amount,
            &storage_cache.rewards_reserve,
            &storage_cache.pool_asset,
        );

        ret_borrow_position
    }

    #[only_owner]
    #[payable("*")]
    #[endpoint(withdraw)]
    fn withdraw(
        &self,
        initial_caller: &ManagedAddress,
        amount: &BigUint,
        mut deposit_position: AccountPosition<Self::Api>,
        is_liquidation: bool,
        protocol_liquidation_fee: &BigUint,
    ) -> AccountPosition<Self::Api> {
        let mut storage_cache = StorageCache::new(self);

        self.require_non_zero_address(initial_caller);
        self.require_amount_greater_than_zero(amount);

        self.update_interest_indexes(&mut storage_cache);

        sc_print!("amount: {}", amount);
        // Unaccrued interest for the wanted amount
        let extra_interest =
            self.compute_interest(amount, &storage_cache.supply_index, &deposit_position.index);
        sc_print!("extra_interest: {}", extra_interest);
        let total_withdraw = amount + &extra_interest;
        sc_print!("total_withdraw: {}", total_withdraw);
        // Withdrawal amount = initial wanted amount + Unaccrued interest for that amount (this has to be paid back to the user that requested the withdrawal)
        let mut principal_amount = total_withdraw.clone();
        // Check if there is enough liquidity to cover the withdrawal

        sc_print!("storage_cache.reserves_amount: {}", storage_cache.reserves_amount);
        require!(
            &storage_cache.reserves_amount >= &principal_amount,
            ERROR_INSUFFICIENT_LIQUIDITY
        );

        // Update the reserves amount
        sc_print!("principal_amount:              {}", principal_amount);
        storage_cache.reserves_amount -= &principal_amount;
        sc_print!("storage_cache.reserves_amount: {}", storage_cache.reserves_amount);
        // If the total withdrawal amount is greater than the accumulated interest, we need to subtract the accumulated interest from the withdrawal amount
        if principal_amount >= deposit_position.accumulated_interest {
            principal_amount -= &deposit_position.accumulated_interest;
            deposit_position.accumulated_interest = BigUint::zero();
        }

        // If the accumulated interest is greater than the withdrawal amount, we need to subtract the withdrawal amount from the accumulated interest
        if deposit_position.accumulated_interest >= principal_amount {
            deposit_position.accumulated_interest -= principal_amount;
            principal_amount = BigUint::zero();
        }

        // Check if there is enough liquidity to cover the withdrawal after the interest was subtracted
        if principal_amount > BigUint::zero() {
            require!(
                storage_cache.supplied_amount >= principal_amount,
                ERROR_INSUFFICIENT_LIQUIDITY
            );
            deposit_position.amount -= &principal_amount;
            storage_cache.supplied_amount -= &principal_amount;
        }

        if !is_liquidation {
            self.tx()
                .to(initial_caller)
                .payment(EgldOrEsdtTokenPayment::new(
                    storage_cache.pool_asset.clone(),
                    0,
                    total_withdraw.clone(),
                ))
                .transfer();
        } else {
            let liquidation_fee = &total_withdraw * protocol_liquidation_fee / BP;
            let amount_after_fee = &total_withdraw - &liquidation_fee;

            storage_cache.rewards_reserve += &liquidation_fee;

            self.tx()
                .to(initial_caller)
                .payment(EgldOrEsdtTokenPayment::new(
                    storage_cache.pool_asset.clone(),
                    0,
                    amount_after_fee,
                ))
                .transfer();
        }

        sc_print!("storage_cache.reserves_amount: {}", storage_cache.reserves_amount);
        sc_print!("storage_cache.rewards_reserve: {}", storage_cache.rewards_reserve);
        self.update_market_state_event(
            storage_cache.timestamp,
            &storage_cache.supply_index,
            &storage_cache.borrow_index,
            &storage_cache.reserves_amount,
            &storage_cache.supplied_amount,
            &storage_cache.borrowed_amount,
            &storage_cache.rewards_reserve,
            &storage_cache.pool_asset,
        );
        deposit_position
    }

    #[only_owner]
    #[payable("*")]
    #[endpoint(repay)]
    fn repay(
        &self,
        initial_caller: ManagedAddress,
        borrow_position: AccountPosition<Self::Api>,
    ) -> AccountPosition<Self::Api> {
        let mut storage_cache = StorageCache::new(self);
        let (received_asset, received_amount) = self.call_value().egld_or_single_fungible_esdt();

        self.require_non_zero_address(&initial_caller);
        self.require_amount_greater_than_zero(&received_amount);
        require!(
            received_asset == storage_cache.pool_asset,
            ERROR_INVALID_ASSET
        );

        self.update_interest_indexes(&mut storage_cache);

        let mut ret_borrow_position =
            self.internal_update_borrows_with_debt(borrow_position, &mut storage_cache);

        let total_owed_with_interest = ret_borrow_position.get_total_amount();

        if received_amount >= total_owed_with_interest {
            // Full repayment
            let extra_amount = &received_amount - &total_owed_with_interest;
            if extra_amount > BigUint::zero() {
                self.tx()
                    .to(&initial_caller)
                    .payment(EgldOrEsdtTokenPayment::new(
                        received_asset,
                        0,
                        extra_amount.clone(),
                    ))
                    .transfer();
            }
            // Reduce borrowed by principal
            storage_cache.borrowed_amount -= &ret_borrow_position.amount;
            // Add full payment (principal + interest) to reserves
            sc_print!("total_owed_with_interest: {}", total_owed_with_interest);
            storage_cache.reserves_amount += &total_owed_with_interest;
            sc_print!("storage_cache.reserves_amount: {}", storage_cache.reserves_amount);

            ret_borrow_position.amount = BigUint::zero();
            ret_borrow_position.accumulated_interest = BigUint::zero();
        } else {
            // Partial repayment
            let total_debt = total_owed_with_interest.clone();

            // Calculate principal portion of the payment
            let principal_portion = &received_amount * &ret_borrow_position.amount / &total_debt;
            let interest_portion = &received_amount - &principal_portion;

            // Reduce position amounts
            ret_borrow_position.amount -= &principal_portion;
            ret_borrow_position.accumulated_interest -= &interest_portion;

            // Update storage
            storage_cache.borrowed_amount -= &principal_portion;
            sc_print!("storage_cache.received_amount: {}", received_amount);
            storage_cache.reserves_amount += &received_amount; // Full payment goes to reserves
            sc_print!("storage_cache.reserves_amount: {}", storage_cache.reserves_amount);
        }

        self.update_market_state_event(
            storage_cache.timestamp,
            &storage_cache.supply_index,
            &storage_cache.borrow_index,
            &storage_cache.reserves_amount,
            &storage_cache.supplied_amount,
            &storage_cache.borrowed_amount,
            &storage_cache.rewards_reserve,
            &storage_cache.pool_asset,
        );

        ret_borrow_position
    }

    #[endpoint(flashLoan)]
    fn flash_loan(
        &self,
        borrowed_token: &EgldOrEsdtTokenIdentifier,
        amount: &BigUint,
        contract_address: &ManagedAddress,
        endpoint: ManagedBuffer<Self::Api>,
        arguments: ManagedArgBuffer<Self::Api>,
        fees: &BigUint,
    ) {
        let mut storage_cache = StorageCache::new(self);

        require!(
            borrowed_token == &storage_cache.pool_asset,
            ERROR_INVALID_ASSET
        );

        require!(
            &storage_cache.reserves_amount >= amount,
            ERROR_FLASHLOAN_RESERVE_ASSET
        );

        // Calculate flash loan fee
        let flash_loan_fee = amount * fees / &BigUint::from(BP);

        // Calculate minimum required amount to be paid back
        let min_required_amount = amount + &flash_loan_fee;

        // TODO: Maybe before the execution drop the cache to save the new available liquidity
        // Does it make a difference? I tend to say no.
        // My concern is what if the call does another flashloan in a loop?
        let back_transfers = self
            .tx()
            .to(contract_address)
            .raw_call(endpoint)
            .arguments_raw(arguments)
            .payment(EgldOrEsdtTokenPayment::new(
                storage_cache.pool_asset.clone(),
                0,
                amount.clone(),
            ))
            .returns(ReturnsBackTransfers)
            .sync_call();

        let is_egld = borrowed_token == &EgldOrEsdtTokenIdentifier::egld();
        if is_egld {
            let amount = back_transfers.total_egld_amount;
            require!(
                amount >= min_required_amount,
                ERROR_INVALID_FLASHLOAN_REPAYMENT
            );
            let extra_amount = amount - min_required_amount;
            storage_cache.rewards_reserve += &extra_amount;
        } else {
            require!(
                back_transfers.esdt_payments.len() == 1,
                ERROR_INVALID_FLASHLOAN_REPAYMENT
            );
            let payment = back_transfers.esdt_payments.get(0);
            require!(
                &payment.token_identifier == &storage_cache.pool_asset,
                ERROR_INVALID_FLASHLOAN_REPAYMENT
            );
            let amount = payment.amount;
            require!(
                amount >= min_required_amount,
                ERROR_INVALID_FLASHLOAN_REPAYMENT
            );
            let extra_amount = amount - min_required_amount;
            storage_cache.rewards_reserve += &extra_amount;
        }

        self.update_market_state_event(
            storage_cache.timestamp,
            &storage_cache.supply_index,
            &storage_cache.borrow_index,
            &storage_cache.reserves_amount,
            &storage_cache.supplied_amount,
            &storage_cache.borrowed_amount,
            &storage_cache.rewards_reserve,
            &storage_cache.pool_asset,
        );
    }
}
