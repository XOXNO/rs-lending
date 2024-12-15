use common_structs::*;

multiversx_sc::imports!();

#[multiversx_sc::module]
pub trait MathModule {
    /// Computes the borrow rate based on the current utilization.
    ///
    /// # Parameters
    /// - `r_max`: The maximum borrow rate.
    /// - `r_base`: The base borrow rate.
    /// - `r_slope1`: The slope of the borrow rate before the optimal utilization.
    /// - `r_slope2`: The slope of the borrow rate after the optimal utilization.
    /// - `u_optimal`: The optimal utilization ratio.
    /// - `u_current`: The current utilization ratio.
    ///
    /// # Returns
    /// - `BigUint`: The computed borrow rate.
    fn compute_borrow_rate(
        &self,
        r_max: ManagedDecimal<Self::Api, NumDecimals>,
        r_base: ManagedDecimal<Self::Api, NumDecimals>,
        r_slope1: ManagedDecimal<Self::Api, NumDecimals>,
        r_slope2: ManagedDecimal<Self::Api, NumDecimals>,
        u_optimal: ManagedDecimal<Self::Api, NumDecimals>,
        u_current: ManagedDecimal<Self::Api, NumDecimals>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        // Represent 1.0 in ManagedDecimal using BP (Basis Points)
        let one_dec = ManagedDecimal::from_raw_units(BigUint::from(BP), DECIMAL_PRECISION);

        if u_current <= u_optimal {
            // Calculate utilization ratio: (u_current * r_slope1) / u_optimal
            let utilization_ratio = u_current.mul(r_slope1).div(u_optimal);

            // Compute borrow rate: r_base + utilization_ratio
            let borrow_rate_dec = r_base.add(utilization_ratio);

            // Rescale and convert back to BigUint
            borrow_rate_dec
        } else {
            // Calculate denominator: BP - u_optimal
            let denominator = one_dec.sub(u_optimal.clone());

            // Calculate numerator: (u_current - u_optimal) * r_slope2
            let numerator = u_current.sub(u_optimal).mul(r_slope2);

            // Compute intermediate rate: r_base + r_slope1
            let intermediate_rate = r_base.add(r_slope1);

            // Compute the final result: intermediate_rate + (numerator / denominator)
            let result = intermediate_rate.add(numerator.div(denominator));

            // Compare with r_max and return the minimum
            if result > r_max {
                r_max
            } else {
                result
            }
        }
    }

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

    fn compute_capital_utilisation(
        &self,
        borrowed_amount: ManagedDecimal<Self::Api, NumDecimals>,
        total_supplied: ManagedDecimal<Self::Api, NumDecimals>,
        decimals: usize,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let zero_dec = ManagedDecimal::from_raw_units(BigUint::zero(), decimals);

        if total_supplied == zero_dec {
            zero_dec
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

    fn compute_interest(
        &self,
        amount: ManagedDecimal<Self::Api, NumDecimals>,
        current_index: &ManagedDecimal<Self::Api, NumDecimals>, // Market index
        initial_index: &ManagedDecimal<Self::Api, NumDecimals>, // Account position index
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let new_amount = amount
            .clone()
            .mul(current_index.clone())
            .div(initial_index.clone());

        new_amount - amount
    }
}
