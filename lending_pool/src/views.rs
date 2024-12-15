use common_events::BP;

use crate::{oracle, storage, ERROR_HEALTH_FACTOR};

multiversx_sc::imports!();

#[multiversx_sc::module]
pub trait ViewsModule:
    storage::LendingStorageModule + oracle::OracleModule + crate::math::LendingMathModule
{
    /// Checks if an account position can be liquidated
    /// 
    /// # Arguments
    /// * `account_position` - NFT nonce of the account position
    /// 
    /// # Returns
    /// * `bool` - True if position can be liquidated (health factor < 100%)
    /// 
    /// # Example
    /// ```
    /// // Position 1: Healthy
    /// // Collateral: $150 weighted
    /// // Borrows: $100
    /// // Health Factor: 150% (15000 bp)
    /// can_be_liquidated(1) = false
    /// 
    /// // Position 2: Unhealthy
    /// // Collateral: $90 weighted
    /// // Borrows: $100
    /// // Health Factor: 90% (9000 bp)
    /// can_be_liquidated(2) = true
    /// ```
    #[view(canBeLiquidated)]
    fn can_be_liquidated(&self, account_position: u64) -> bool {
        let bp = BigUint::from(BP);
        let health_factor = self.get_health_factor(account_position);
        health_factor < bp
    }

    /// Gets the current health factor for an account position
    /// 
    /// # Arguments
    /// * `account_position` - NFT nonce of the account position
    /// 
    /// # Returns
    /// * `BigUint` - Health factor in basis points (10000 = 100%)
    /// 
    /// # Example
    /// ```
    /// // Position with:
    /// // Collateral: 100 EGLD @ $100 each = $10,000
    /// // Liquidation threshold: 80%
    /// // Weighted collateral: $8,000
    /// 
    /// // Borrows: 5000 USDC @ $1 each = $5,000
    /// 
    /// // Health Factor = $8,000 * 10000 / $5,000 = 16000 (160%)
    /// get_health_factor(1) = 16000
    /// ```
    #[view(getHealthFactor)]
    fn get_health_factor(&self, account_position: u64) -> BigUint {
        let collateral_in_dollars = self.get_liquidation_collateral_available(account_position);
        let borrowed_dollars = self.get_total_borrow_in_dollars(account_position);
        self.compute_health_factor(&collateral_in_dollars, &borrowed_dollars)
    }

    /// Calculates maximum amount of collateral that can be liquidated
    /// 
    /// # Arguments
    /// * `account_position` - NFT nonce of the account position
    /// * `collateral_asset` - Token identifier of collateral to liquidate
    /// * `in_usd` - Whether to return amount in USD (true) or token units (false)
    /// 
    /// # Returns
    /// * `BigUint` - Maximum liquidatable amount in USD or token units
    /// 
    /// # Errors
    /// * `ERROR_HEALTH_FACTOR` - If position is not liquidatable (HF >= 100%)
    /// 
    /// # Example
    /// ```
    /// // Position:
    /// // Collateral: 100 EGLD @ $100 each = $10,000
    /// // Borrows: 9000 USDC @ $1 each = $9,000
    /// // Health Factor: 90% (unhealthy)
    /// // Liquidation Bonus: 10%
    /// 
    /// // In USD:
    /// get_max_liquidate_amount_for_collateral(1, "EGLD-123456", true) = 5000
    /// // Can liquidate $5,000 worth of collateral
    /// 
    /// // In EGLD:
    /// get_max_liquidate_amount_for_collateral(1, "EGLD-123456", false) = 50
    /// // Can liquidate 50 EGLD
    /// ```
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

    /// Gets the collateral amount for a specific token
    /// 
    /// # Arguments
    /// * `account_position` - NFT nonce of the account position
    /// * `token_id` - Token identifier to check
    /// 
    /// # Returns
    /// * `BigUint` - Amount of token supplied as collateral
    /// 
    /// # Example
    /// ```
    /// // Position has:
    /// // - 100 EGLD supplied
    /// // - 1000 USDC supplied
    /// 
    /// get_collateral_amount_for_token(1, "EGLD-123456") = 100_000_000_000_000_000_000
    /// get_collateral_amount_for_token(1, "USDC-123456") = 1_000_000_000
    /// get_collateral_amount_for_token(1, "USDT-123456") = 0 // No USDT supplied
    /// ```
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

    /// Gets the borrowed amount for a specific token
    /// 
    /// # Arguments
    /// * `account_position` - NFT nonce of the account position
    /// * `token_id` - Token identifier to check
    /// 
    /// # Returns
    /// * `BigUint` - Amount of token borrowed
    /// 
    /// # Example
    /// ```
    /// // Position has:
    /// // - 50 EGLD borrowed
    /// // - 500 USDC borrowed
    /// 
    /// get_borrow_amount_for_token(1, "EGLD-123456") = 50_000_000_000_000_000_000
    /// get_borrow_amount_for_token(1, "USDC-123456") = 500_000_000
    /// get_borrow_amount_for_token(1, "USDT-123456") = 0 // No USDT borrowed
    /// ```
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

    /// Gets total value of borrowed assets in USD
    /// 
    /// # Arguments
    /// * `account_position` - NFT nonce of the account position
    /// 
    /// # Returns
    /// * `BigUint` - Total USD value of all borrowed assets
    /// 
    /// # Example
    /// ```
    /// // Position has:
    /// // - 50 EGLD borrowed @ $100 each = $5,000
    /// // - 500 USDC borrowed @ $1 each = $500
    /// 
    /// get_total_borrow_in_dollars(1) = 5_500_000_000 // $5,500
    /// ```
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

    /// Gets total value of collateral assets in USD
    /// 
    /// # Arguments
    /// * `account_position` - NFT nonce of the account position
    /// 
    /// # Returns
    /// * `BigUint` - Total USD value of all collateral assets (unweighted)
    /// 
    /// # Example
    /// ```
    /// // Position has:
    /// // - 100 EGLD supplied @ $100 each = $10,000
    /// // - 1000 USDC supplied @ $1 each = $1,000
    /// 
    /// get_total_collateral_in_dollars(1) = 11_000_000_000 // $11,000
    /// ```
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

    /// Gets total value of collateral available for liquidation in USD
    /// 
    /// # Arguments
    /// * `account_nonce` - NFT nonce of the account position
    /// 
    /// # Returns
    /// * `BigUint` - Total USD value of collateral weighted by liquidation thresholds
    /// 
    /// # Example
    /// ```
    /// // Position has:
    /// // - 100 EGLD @ $100 each = $10,000, threshold 80% = $8,000
    /// // - 1000 USDC @ $1 each = $1,000, threshold 85% = $850
    /// 
    /// get_liquidation_collateral_available(1) = 8_850_000_000 // $8,850
    /// ```
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

    /// Gets total value of collateral weighted by LTV ratios in USD
    /// 
    /// # Arguments
    /// * `account_position` - NFT nonce of the account position
    /// 
    /// # Returns
    /// * `BigUint` - Total USD value of collateral weighted by LTV ratios
    /// 
    /// # Example
    /// ```
    /// // Position has:
    /// // - 100 EGLD @ $100 each = $10,000, LTV 75% = $7,500
    /// // - 1000 USDC @ $1 each = $1,000, LTV 80% = $800
    /// 
    /// get_ltv_collateral_in_dollars(1) = 8_300_000_000 // $8,300
    /// ```
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
