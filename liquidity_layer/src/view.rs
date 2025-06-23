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
    /// Retrieves the current capital utilization of the pool.
    ///
    /// **Purpose**: Calculates the percentage of supplied assets currently being borrowed,
    /// which is the primary driver of interest rates in the lending protocol.
    ///
    /// **Mathematical Formula**:
    /// ```
    /// total_borrowed_value = borrowed_scaled * current_borrow_index
    /// total_supplied_value = supplied_scaled * current_supply_index
    /// utilization = total_borrowed_value / total_supplied_value
    /// ```
    ///
    /// **Utilization Impact on Rates**:
    /// - Low utilization (0-80%): Gradual rate increases
    /// - High utilization (80%+): Steep rate increases (above kink point)
    /// - 100% utilization: Maximum borrow rates to incentivize repayment
    ///
    /// **Edge Cases**:
    /// - Zero supply: Returns 0% utilization (no funds to borrow)
    /// - Zero borrows: Returns 0% utilization (full liquidity available)
    ///
    /// # Returns
    /// - Utilization ratio as a decimal (0.0 to 1.0, where 1.0 = 100%)
    #[view(getCapitalUtilisation)]
    fn get_capital_utilisation(&self) -> ManagedDecimal<Self::Api, NumDecimals> {
        let params = self.params().get();
        let zero = self.to_decimal(BigUint::zero(), params.asset_decimals);
        let supplied = self.supplied().get();
        let borrowed = self.borrowed().get();
        let total_borrowed = self.mul_half_up(&borrowed, &self.borrow_index().get(), RAY_PRECISION);
        let total_supplied = self.mul_half_up(&supplied, &self.supply_index().get(), RAY_PRECISION);
        if total_supplied == zero {
            self.ray_zero()
        } else {
            self.div_half_up(&total_borrowed, &total_supplied, RAY_PRECISION)
        }
    }

    /// Retrieves the total actual balance of the asset held by the pool contract.
    ///
    /// **Purpose**: Returns the current liquidity available for withdrawals and new borrows,
    /// representing the pool's immediate cash position.
    ///
    /// **Calculation**:
    /// ```
    /// reserves = blockchain.get_sc_balance(asset_id)
    /// ```
    /// This is the raw token balance held by the smart contract.
    ///
    /// **Reserve Dynamics**:
    /// - Increases with: User deposits, loan repayments, flash loan fees
    /// - Decreases with: User withdrawals, new loans, flash loan disbursements
    /// - Should equal: Total supplied value - Total borrowed value + Accrued fees
    ///
    /// **Liquidity Constraints**:
    /// Available reserves limit:
    /// - Maximum withdrawal amounts
    /// - Maximum new borrow amounts
    /// - Flash loan capacity
    ///
    /// **Critical for**:
    /// - Liquidity monitoring
    /// - Maximum transaction sizing
    /// - Protocol solvency verification
    ///
    /// # Returns
    /// - Current asset balance of the pool contract in asset decimals
    #[view(getReserves)]
    fn get_reserves(&self) -> ManagedDecimal<Self::Api, NumDecimals> {
        let params = self.params().get();
        let pool_balance = self.blockchain().get_sc_balance(&params.asset_id, 0);
        self.to_decimal(pool_balance, params.asset_decimals)
    }

    /// Retrieves the current deposit rate for the pool.
    ///
    /// **Purpose**: Calculates the annual percentage yield (APY) that suppliers earn
    /// on their deposits, based on current pool utilization and borrower interest.
    ///
    /// **Mathematical Formula**:
    /// ```
    /// deposit_rate = borrow_rate * utilization * (1 - reserve_factor)
    /// ```
    ///
    /// **Rate Derivation Process**:
    /// 1. **Utilization Calculation**: `borrowed_value / supplied_value`
    /// 2. **Borrow Rate Lookup**: Rate based on utilization curve
    /// 3. **Revenue Sharing**: Split between suppliers and protocol
    /// 4. **Effective Rate**: `borrow_rate * utilization * supplier_share`
    ///
    /// **Economic Logic**:
    /// - Higher utilization → Higher borrow rates → Higher deposit rates
    /// - Reserve factor reduces supplier share (protocol revenue)
    /// - Rate automatically adjusts to market conditions
    ///
    /// **Example Calculation**:
    /// ```
    /// utilization = 80%
    /// borrow_rate = 10% APR
    /// reserve_factor = 20%
    /// deposit_rate = 10% * 80% * (1 - 20%) = 6.4% APR
    /// ```
    ///
    /// # Returns
    /// - Annual deposit rate as a decimal (e.g., 0.064 = 6.4% APR)
    #[view(getDepositRate)]
    fn get_deposit_rate(&self) -> ManagedDecimal<Self::Api, NumDecimals> {
        let params = self.params().get();
        let utilization = self.get_capital_utilisation();
        let borrow_rate = self.calc_borrow_rate(utilization.clone(), params.clone());
        self.calc_deposit_rate(utilization, borrow_rate, params.reserve_factor.clone())
    }

    /// Retrieves the current borrow rate for the pool.
    ///
    /// **Purpose**: Calculates the annual percentage rate (APR) that borrowers pay
    /// for loans, based on current pool utilization and rate curve parameters.
    ///
    /// **Mathematical Formula**:
    /// ```
    /// if utilization <= kink_point:
    ///     rate = base_rate + (utilization * slope1)
    /// else:
    ///     rate = base_rate + (kink_point * slope1) + ((utilization - kink_point) * slope2)
    /// ```
    ///
    /// **Rate Curve Properties**:
    /// - **Base Rate**: Minimum rate even at 0% utilization
    /// - **Slope1**: Gradual increase up to kink point (typically 80%)
    /// - **Kink Point**: Utilization threshold for steep rate increases
    /// - **Slope2**: Steep increase above kink to discourage over-borrowing
    ///
    /// **Economic Purpose**:
    /// - Low rates at low utilization encourage borrowing
    /// - Moderate rates at medium utilization maintain equilibrium
    /// - High rates at high utilization protect pool liquidity
    /// - Maximum rates near 100% utilization force repayment
    ///
    /// **Example Rate Curve**:
    /// ```
    /// Utilization:  0%    40%    80%    95%
    /// Borrow Rate:  2%    6%     10%    50%
    /// ```
    ///
    /// # Returns
    /// - Annual borrow rate as a decimal (e.g., 0.10 = 10% APR)
    #[view(getBorrowRate)]
    fn get_borrow_rate(&self) -> ManagedDecimal<Self::Api, NumDecimals> {
        let params = self.params().get();
        let utilization = self.get_capital_utilisation();
        self.calc_borrow_rate(utilization, params)
    }

    /// Retrieves the time delta since the last update.
    ///
    /// **Purpose**: Returns the elapsed time since the last global synchronization,
    /// indicating how much interest has accrued but not yet been applied to indexes.
    ///
    /// **Time Measurement**:
    /// ```
    /// delta_ms = current_block_timestamp * 1000 - last_timestamp
    /// delta_seconds = delta_ms / 1000
    /// ```
    ///
    /// **Interest Accrual Relationship**:
    /// - Larger deltas → More accumulated interest awaiting application
    /// - Zero delta → Indexes are fully up to date
    /// - Regular updates minimize compound interest approximation errors
    ///
    /// **Usage**:
    /// - Monitor pool update frequency
    /// - Calculate pending interest before transactions
    /// - Estimate gas costs for index updates
    ///
    /// # Returns
    /// - Time elapsed since last update in milliseconds
    #[view(getDeltaTime)]
    fn get_delta_time(&self) -> u64 {
        (self.blockchain().get_block_timestamp() * 1000u64) - self.last_timestamp().get()
    }

    /// Retrieves the protocol revenue accrued from borrow interest fees, scaled to the asset's decimals.
    ///
    /// **Purpose**: Returns the current value of protocol treasury holdings,
    /// representing accumulated revenue from various protocol operations.
    ///
    /// **Calculation**:
    /// ```
    /// revenue_actual = revenue_scaled * current_supply_index
    /// ```
    ///
    /// **Revenue Sources**:
    /// - Interest rate spreads (reserve factor percentage)
    /// - Flash loan fees
    /// - Strategy creation fees
    /// - Liquidation fees
    /// - Seized dust collateral
    ///
    /// **Revenue Appreciation**:
    /// Protocol revenue is stored as scaled supply tokens that appreciate
    /// with the supply index, ensuring the treasury earns interest on its holdings.
    ///
    /// **Claimable Amount**:
    /// The actual claimable amount may be limited by available pool reserves,
    /// ensuring user withdrawal capacity is preserved.
    ///
    /// # Returns
    /// - Current protocol revenue value in asset decimals
    #[view(getProtocolRevenue)]
    fn get_protocol_revenue(&self) -> ManagedDecimal<Self::Api, NumDecimals> {
        let revenue_scaled = self.revenue().get();
        let supply_index = self.supply_index().get();

        self.scaled_to_original(
            &revenue_scaled,
            &supply_index,
            self.params().get().asset_decimals,
        )
    }

    /// Retrieves the total amount supplied to the pool.
    ///
    /// **Purpose**: Returns the current total value of all user deposits including accrued interest,
    /// representing the total funds available for borrowing and protocol operations.
    ///
    /// **Calculation**:
    /// ```
    /// supplied_actual = supplied_scaled * current_supply_index
    /// ```
    ///
    /// **Value Components**:
    /// - Original user deposits (principal)
    /// - Accrued interest from borrower payments
    /// - Protocol revenue (treasury holdings)
    ///
    /// **Supply Growth Mechanism**:
    /// Total supplied value increases through:
    /// - New user deposits
    /// - Interest payments from borrowers
    /// - Flash loan and strategy fees
    /// - Supply index appreciation over time
    ///
    /// **Relationship to Reserves**:
    /// ```
    /// supplied_value = borrowed_value + available_reserves + protocol_revenue
    /// ```
    ///
    /// # Returns
    /// - Total supplied value including interest in asset decimals
    #[view(getSuppliedAmount)]
    fn get_supplied_amount(&self) -> ManagedDecimal<Self::Api, NumDecimals> {
        let supplied_scaled = self.supplied().get();
        let supply_index = self.supply_index().get();

        self.scaled_to_original(
            &supplied_scaled,
            &supply_index,
            self.params().get().asset_decimals,
        )
    }

    /// Retrieves the total amount borrowed from the pool.
    ///
    /// **Purpose**: Returns the current total debt owed by all borrowers including accrued interest,
    /// representing the total outstanding obligations to the pool.
    ///
    /// **Calculation**:
    /// ```
    /// borrowed_actual = borrowed_scaled * current_borrow_index
    /// ```
    ///
    /// **Debt Components**:
    /// - Original borrowed principal amounts
    /// - Compound interest accrued over time
    /// - Strategy fees included in debt positions
    ///
    /// **Debt Growth Mechanism**:
    /// Total borrowed value increases through:
    /// - New borrowing activity
    /// - Compound interest accrual
    /// - Strategy creation with upfront fees
    /// - Borrow index appreciation over time
    ///
    /// **Interest Generation**:
    /// The difference between borrowed and supplied values represents
    /// the interest being generated for suppliers and protocol revenue.
    ///
    /// **Utilization Calculation**:
    /// ```
    /// utilization = total_borrowed / total_supplied
    /// ```
    ///
    /// # Returns
    /// - Total borrowed value including interest in asset decimals
    #[view(getBorrowedAmount)]
    fn get_borrowed_amount(&self) -> ManagedDecimal<Self::Api, NumDecimals> {
        let borrowed_scaled = self.borrowed().get();
        let borrow_index = self.borrow_index().get();

        self.scaled_to_original(
            &borrowed_scaled,
            &borrow_index,
            self.params().get().asset_decimals,
        )
    }
}
