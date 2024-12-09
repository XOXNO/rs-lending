use common_events::BP;

use crate::{oracle, storage, ERROR_HEALTH_FACTOR};

multiversx_sc::imports!();

#[multiversx_sc::module]
pub trait ViewsModule:
    storage::LendingStorageModule + oracle::OracleModule + crate::math::LendingMathModule
{
    #[view(canBeLiquidated)]
    fn can_be_liquidated(&self, account_position: u64) -> bool {
        let bp = BigUint::from(BP);
        let health_factor = self.get_health_factor(account_position);
        health_factor < bp
    }

    #[view(getHealthFactor)]
    fn get_health_factor(&self, account_position: u64) -> BigUint {
        let collateral_in_dollars = self.get_liquidation_collateral_available(account_position);
        let borrowed_dollars = self.get_total_borrow_in_dollars(account_position);
        self.compute_health_factor(&collateral_in_dollars, &borrowed_dollars)
    }

    #[view(getMaxLiquidateAmountForCollateral)]
    fn get_max_liquidate_amount_for_collateral(
        &self,
        account_position: u64,
        collateral_asset: &EgldOrEsdtTokenIdentifier,
        in_usd: bool,
    ) -> BigUint {
        let bp = BigUint::from(BP);

        let borrowed_dollars = self.get_total_borrow_in_dollars(account_position);
        let collateral_in_dollars = self.get_liquidation_collateral_available(account_position);
        let health_factor = self.compute_health_factor(&collateral_in_dollars, &borrowed_dollars);

        require!(health_factor < bp, ERROR_HEALTH_FACTOR);

        let asset_config = self.asset_config(collateral_asset).get();
        // Calculate collateral to receive with bonus
        let bonus_rate = self
            .calculate_dynamic_liquidation_bonus(&health_factor, asset_config.liquidation_bonus);

        let feed = self.get_token_price_data(collateral_asset);

        // Calculate liquidation amount using Dutch auction mechanism
        let liquidation_amount_usd = self.calculate_single_asset_liquidation_amount(
            &borrowed_dollars,
            &collateral_in_dollars,
            collateral_asset,
            &feed,
            account_position,
            OptionalValue::None,
            &bonus_rate,
        );
        // Convert USD value to collateral token amount
        let collateral_amount_before_bonus =
            self.compute_amount_in_tokens(&liquidation_amount_usd, &feed);

        if in_usd {
            liquidation_amount_usd
        } else {
            collateral_amount_before_bonus
        }
    }

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

    #[view(getTotalCollateralInDollars)]
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
            let asset_config = self.asset_config(&dp.token_id).get();

            weighted_collateral_in_dollars +=
                &position_value_in_dollars * &asset_config.ltv / BigUint::from(BP);
        }

        weighted_collateral_in_dollars
    }
}
