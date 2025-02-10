use common_constants::{BP, DECIMAL_PRECISION};
use common_events::PoolParams;

multiversx_sc::imports!();

/// The InterestRateMath module provides functions for calculating market rates,
/// interest accrual, and capital utilization based on the pool parameters and current state.
#[multiversx_sc::module]
pub trait InterestRateMath {
    /// Computes the borrow rate based on current utilization and pool parameters.
    ///
    /// The borrow rate is determined by a two-part model:
    /// - When `u_current` is less than or equal to `u_optimal`, the rate is:
    ///   `borrow_rate = r_base + (u_current * r_slope1 / u_optimal)`
    /// - When `u_current` exceeds `u_optimal`, an extra penalty is applied:
    ///   `borrow_rate = r_base + r_slope1 + ((u_current - u_optimal) * r_slope2 / (1 - u_optimal))`
    /// The result is capped by `r_max`.
    ///
    /// # Parameters
    /// - `params`: The pool parameters (r_max, r_base, r_slope1, r_slope2, u_optimal, reserve_factor, decimals).
    /// - `u_current`: The current utilization ratio as a ManagedDecimal.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The computed borrow rate.
    fn compute_borrow_rate(
        &self,
        params: PoolParams<Self::Api>,
        u_current: ManagedDecimal<Self::Api, NumDecimals>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        // Represent 1.0 in ManagedDecimal using BP (Basis Points)
        let one_dec = ManagedDecimal::from_raw_units(BigUint::from(BP), DECIMAL_PRECISION);

        if u_current <= params.u_optimal {
            // Calculate utilization ratio: (u_current * r_slope1) / u_optimal
            let utilization_ratio = u_current.mul(params.r_slope1).div(params.u_optimal);

            // Compute borrow rate: r_base + utilization_ratio
            let borrow_rate_dec = params.r_base.add(utilization_ratio);

            // Rescale and convert back to BigUint
            borrow_rate_dec
        } else {
            // Calculate denominator: BP - u_optimal
            let denominator = one_dec.sub(params.u_optimal.clone());

            // Calculate numerator: (u_current - u_optimal) * r_slope2
            let numerator = u_current.sub(params.u_optimal).mul(params.r_slope2);

            // Compute intermediate rate: r_base + r_slope1
            let intermediate_rate = params.r_base.add(params.r_slope1);

            // Compute the final result: intermediate_rate + (numerator / denominator)
            let result = intermediate_rate.add(numerator.div(denominator));

            // Compare with r_max and return the minimum
            if result > params.r_max {
                params.r_max
            } else {
                result
            }
        }
    }

    /// Computes the deposit rate for suppliers based on current utilization.
    ///
    /// The deposit rate represents the yield suppliers earn and is calculated as:
    /// `deposit_rate = u_current * borrow_rate * (1 - reserve_factor)`,
    /// ensuring that the protocol's share is excluded.
    ///
    /// # Parameters
    /// - `u_current`: The current utilization ratio.
    /// - `borrow_rate`: The current borrow rate.
    /// - `reserve_factor`: The reserve factor representing the protocol's fee portion.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The computed deposit rate.
    fn compute_deposit_rate(
        &self,
        u_current: ManagedDecimal<Self::Api, NumDecimals>,
        borrow_rate: ManagedDecimal<Self::Api, NumDecimals>,
        reserve_factor: ManagedDecimal<Self::Api, NumDecimals>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        // Perform calculations using ManagedDecimal
        let one_dec = ManagedDecimal::from_raw_units(BigUint::from(BP), DECIMAL_PRECISION);

        let deposit_rate_dec = u_current
            .mul_with_precision(borrow_rate, DECIMAL_PRECISION)
            .mul_with_precision(one_dec.sub(reserve_factor), DECIMAL_PRECISION);

        deposit_rate_dec
    }

    /// Computes the capital utilization of the pool.
    ///
    /// Utilization is defined as the ratio of the borrowed amount to the total supplied amount,
    /// scaled by the base point (BP) for precision.
    ///
    /// # Parameters
    /// - `borrowed_amount`: The current borrowed amount.
    /// - `total_supplied`: The total supplied amount.
    /// - `zero`: A ManagedDecimal representing zero (for comparison).
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The computed utilization ratio.
    fn compute_capital_utilisation(
        &self,
        borrowed_amount: ManagedDecimal<Self::Api, NumDecimals>,
        total_supplied: ManagedDecimal<Self::Api, NumDecimals>,
        zero: ManagedDecimal<Self::Api, NumDecimals>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        if total_supplied == zero {
            zero
        } else {
            let utilization_ratio = borrowed_amount
                .mul(ManagedDecimal::from_raw_units(
                    BigUint::from(BP),
                    DECIMAL_PRECISION,
                ))
                .div(total_supplied);

            utilization_ratio
        }
    }

    /// Computes the interest accrued on a given position.
    ///
    /// The accrued interest is calculated using the formula:
    /// `interest = amount * (current_index / account_position_index) - amount`.
    ///
    /// # Parameters
    /// - `amount`: The principal amount of the asset.
    /// - `current_index`: The current market index (reflecting compounded interest).
    /// - `account_position_index`: The index at the time the position was last updated.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The interest accrued since the last update.

    fn compute_interest(
        &self,
        amount: ManagedDecimal<Self::Api, NumDecimals>, // Amount of the asset
        current_index: &ManagedDecimal<Self::Api, NumDecimals>, // Market index
        account_position_index: &ManagedDecimal<Self::Api, NumDecimals>, // Account position index
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let new_amount = amount
            .clone()
            .mul(current_index.clone())
            .div(account_position_index.clone());

        new_amount - amount
    }
}
