multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use crate::{errors::ERROR_INVALID_BORROW_INDEX, liq_math, liq_storage};

use common_structs::*;

#[multiversx_sc::module]
pub trait ViewModule: liq_math::MathModule + liq_storage::StorageModule {
    #[view(getCapitalUtilisation)]
    fn get_capital_utilisation(&self) -> BigUint {
        let borrowed_amount = self.borrowed_amount().get();
        let total_amount = self.supplied_amount().get();

        self.compute_capital_utilisation(&borrowed_amount, &total_amount)
    }

    #[view(getTotalCapital)]
    fn get_total_capital(&self) -> BigUint {
        let reserve_amount = self.reserves().get();
        let borrowed_amount = self.borrowed_amount().get();

        &reserve_amount + &borrowed_amount
    }

    #[view(getDebtInterest)]
    fn get_debt_interest(&self, amount: &BigUint, initial_borrow_index: &BigUint) -> BigUint {
        let borrow_index_diff = self.get_borrow_index_diff(initial_borrow_index);

        amount * &borrow_index_diff / BP
    }

    #[view(getDepositRate)]
    fn get_deposit_rate(&self) -> BigUint {
        let pool_params = self.pool_params().get();
        let capital_utilisation = self.get_capital_utilisation();
        let borrow_rate = self.get_borrow_rate();

        self.compute_deposit_rate(
            &capital_utilisation,
            &borrow_rate,
            &pool_params.reserve_factor,
        )
    }

    #[view(getBorrowRate)]
    fn get_borrow_rate(&self) -> BigUint {
        let pool_params = self.pool_params().get();
        let capital_utilisation = self.get_capital_utilisation();

        self.compute_borrow_rate(
            &pool_params.r_base,
            &pool_params.r_slope1,
            &pool_params.r_slope2,
            &pool_params.u_optimal,
            &capital_utilisation,
        )
    }

    fn get_borrow_index_diff(&self, initial_borrow_index: &BigUint) -> BigUint {
        let current_borrow_index = self.borrow_index().get();
        require!(
            &current_borrow_index >= initial_borrow_index,
            ERROR_INVALID_BORROW_INDEX
        );

        current_borrow_index - initial_borrow_index
    }
}
