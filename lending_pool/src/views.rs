use common_events::BP;

use crate::{oracle, storage};

multiversx_sc::imports!();

#[multiversx_sc::module]
pub trait ViewsModule:
    storage::LendingStorageModule + oracle::OracleModule + crate::math::LendingMathModule
{
    #[view(getCollateralAmountForToken)]
    fn get_collateral_amount_for_token(
        &self,
        account_position: u64,
        token_id: &EgldOrEsdtTokenIdentifier,
    ) -> BigUint {
        match self.deposit_positions(account_position).get(token_id) {
            Some(dp) => dp.get_total_amount(),
            None => BigUint::zero(),
        }
    }

    #[view(getBorrowAmountForToken)]
    fn get_borrow_amount_for_token(
        &self,
        account_position: u64,
        token_id: &EgldOrEsdtTokenIdentifier,
    ) -> BigUint {
        match self.borrow_positions(account_position).get(token_id) {
            Some(bp) => bp.get_total_amount(),
            None => BigUint::zero(),
        }
    }

    #[view(getAccountHealthFactor)]
    fn get_account_health_factor(&self, account_position: u64) -> BigUint {
        let collateral_in_dollars = self.get_liquidation_collateral_available(account_position);
        let borrowed_dollars = self.get_total_borrow_in_dollars(account_position);
        self.compute_health_factor(&collateral_in_dollars, &borrowed_dollars)
    }

    #[view(canBeLiquidated)]
    fn can_be_liquidated(&self, account_position: u64) -> bool {
        let health_factor = self.get_account_health_factor(account_position);
        health_factor < BigUint::from(BP)
    }

    #[view(getTotalBorrowInDollars)]
    fn get_total_borrow_in_dollars(&self, account_position: u64) -> BigUint {
        let mut total_borrow_in_dollars = BigUint::zero();
        let borrow_positions = self.borrow_positions(account_position);

        for bp in borrow_positions.values() {
            total_borrow_in_dollars +=
                self.get_token_amount_in_dollars(&bp.token_id, &bp.get_total_amount());
        }

        total_borrow_in_dollars
    }

    #[view(getTotalCollateralAvailable)]
    fn get_total_collateral_in_dollars(&self, account_position: u64) -> BigUint {
        let mut deposited_amount_in_dollars = BigUint::zero();
        let deposit_positions = self.deposit_positions(account_position);

        for dp in deposit_positions.values() {
            deposited_amount_in_dollars +=
                self.get_token_amount_in_dollars(&dp.token_id, &dp.get_total_amount());
        }

        deposited_amount_in_dollars
    }

    #[view(getLiquidationCollateralAvailable)]
    fn get_liquidation_collateral_available(&self, account_nonce: u64) -> BigUint {
        let mut weighted_liquidation_threshold_sum = BigUint::zero();
        let deposit_positions = self.deposit_positions(account_nonce);

        // Single iteration for both calculations
        for dp in deposit_positions.values() {
            let position_value_in_dollars =
                self.get_token_amount_in_dollars(&dp.token_id, &dp.get_total_amount());
            weighted_liquidation_threshold_sum +=
                &position_value_in_dollars * &dp.entry_liquidation_threshold / BigUint::from(BP);
        }

        weighted_liquidation_threshold_sum
    }

    #[view(getLtvCollateralInDollars)]
    fn get_ltv_collateral_in_dollars(&self, account_position: u64) -> BigUint {
        let mut weighted_collateral_in_dollars = BigUint::zero();
        let deposit_positions = self.deposit_positions(account_position);

        for dp in deposit_positions.values() {
            let position_value_in_dollars =
                self.get_token_amount_in_dollars(&dp.token_id, &dp.get_total_amount());

            weighted_collateral_in_dollars +=
                &position_value_in_dollars * &dp.entry_ltv / BigUint::from(BP);
        }

        weighted_collateral_in_dollars
    }
}
