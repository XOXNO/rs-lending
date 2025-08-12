multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use common_constants::RAY_PRECISION;

use crate::storage;

/// The ViewModule provides read-only endpoints for retrieving key market metrics and pool state information.
///
/// **Purpose**: Offers external visibility into the lending pool's financial state, interest rates,
/// and utilization metrics without requiring state modifications. These views are essential for:
/// - User interfaces displaying current pool conditions
/// - External integrations calculating potential returns
/// - Risk management systems monitoring pool health
/// - Analytics and reporting tools
///
/// **Mathematical Accuracy**: All calculations use the same formulas as the core protocol,
/// ensuring consistency between view data and actual transaction outcomes.
#[multiversx_sc::module]
pub trait ViewModule:
    storage::Storage + common_math::SharedMathModule + common_rates::InterestRates
{
    /// Returns current pool utilization ratio (borrowed_value / supplied_value).
    /// Used for interest rate calculations and pool health monitoring.
    /// Returns 0 if no supply exists.
    #[view(capitalUtilisation)]
    fn capital_utilisation(&self) -> ManagedDecimal<Self::Api, NumDecimals> {
        let parameters = self.parameters().get();
        let zero_wad = self.to_decimal(BigUint::zero(), parameters.asset_decimals);
        let supplied = self.supplied().get();
        let borrowed = self.borrowed().get();
        let total_borrowed_ray =
            self.mul_half_up(&borrowed, &self.borrow_index().get(), RAY_PRECISION);
        let total_supplied_ray =
            self.mul_half_up(&supplied, &self.supply_index().get(), RAY_PRECISION);
        if total_supplied_ray == zero_wad {
            self.ray_zero()
        } else {
            self.div_half_up(&total_borrowed_ray, &total_supplied_ray, RAY_PRECISION)
        }
    }

    /// Returns total asset balance held by pool contract.
    /// Represents immediately available liquidity for withdrawals and loans.
    /// Balance changes with deposits, withdrawals, borrows, and repayments.
    #[view(reserves)]
    fn reserves(&self) -> ManagedDecimal<Self::Api, NumDecimals> {
        let parameters = self.parameters().get();
        let pool_balance = self.blockchain().get_sc_balance(&parameters.asset_id, 0);
        self.to_decimal(pool_balance, parameters.asset_decimals)
    }

    /// Returns current annual percentage yield for suppliers.
    /// Calculated as: borrow_rate * utilization * (1 - reserve_factor).
    /// Higher utilization and borrow rates increase deposit yields.
    #[view(depositRate)]
    fn deposit_rate(&self) -> ManagedDecimal<Self::Api, NumDecimals> {
        let parameters = self.parameters().get();
        let utilization = self.capital_utilisation();
        let borrow_rate = self.calculate_borrow_rate(utilization.clone(), parameters.clone());
        self.calculate_deposit_rate(
            utilization,
            borrow_rate,
            parameters.reserve_factor_bps.clone(),
        )
    }

    /// Returns current annual percentage rate for borrowers.
    /// Uses piecewise linear rate model with kink point.
    /// Rates increase steeply above optimal utilization to protect liquidity.
    #[view(borrowRate)]
    fn borrow_rate(&self) -> ManagedDecimal<Self::Api, NumDecimals> {
        let parameters = self.parameters().get();
        let utilization = self.capital_utilisation();
        self.calculate_borrow_rate(utilization, parameters)
    }

    /// Returns milliseconds elapsed since last pool synchronization.
    /// Indicates accumulated interest awaiting index updates.
    /// Larger deltas mean more pending interest calculations.
    #[view(deltaTime)]
    fn delta_time(&self) -> u64 {
        self.blockchain().get_block_timestamp_ms() - self.last_timestamp().get()
    }

    /// Returns accumulated protocol revenue value in asset decimals.
    /// Revenue from interest spreads, fees, and liquidations.
    /// Stored as scaled tokens that appreciate with supply index.
    #[view(protocolRevenue)]
    fn protocol_revenue(&self) -> ManagedDecimal<Self::Api, NumDecimals> {
        let revenue_scaled = self.revenue().get();
        let supply_index = self.supply_index().get();

        self.scaled_to_original(
            &revenue_scaled,
            &supply_index,
            self.parameters().get().asset_decimals,
        )
    }

    /// Returns total value of all deposits including accrued interest.
    /// Includes user deposits, earned interest, and protocol revenue.
    /// Grows through deposits and supply index appreciation.
    #[view(suppliedAmount)]
    fn supplied_amount(&self) -> ManagedDecimal<Self::Api, NumDecimals> {
        let supplied_scaled = self.supplied().get();
        let supply_index = self.supply_index().get();

        self.scaled_to_original(
            &supplied_scaled,
            &supply_index,
            self.parameters().get().asset_decimals,
        )
    }

    /// Returns total debt owed by all borrowers including accrued interest.
    /// Includes principal and compound interest through borrow index.
    /// Grows through new borrows and interest accrual over time.
    #[view(borrowedAmount)]
    fn borrowed_amount(&self) -> ManagedDecimal<Self::Api, NumDecimals> {
        let borrowed_scaled = self.borrowed().get();
        let borrow_index = self.borrow_index().get();

        self.scaled_to_original(
            &borrowed_scaled,
            &borrow_index,
            self.parameters().get().asset_decimals,
        )
    }
}
