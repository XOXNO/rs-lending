use common_structs::BP;

multiversx_sc::imports!();

#[multiversx_sc::module]
pub trait MathModule {
    fn compute_borrow_rate(
        &self,
        r_max: &BigUint,
        r_base: &BigUint,
        r_slope1: &BigUint,
        r_slope2: &BigUint,
        u_optimal: &BigUint,
        u_current: &BigUint,
    ) -> BigUint {
        let bp = BigUint::from(BP);

        if u_current <= u_optimal {
            let utilisation_ratio = &(u_current * r_slope1) / u_optimal;
            r_base + &utilisation_ratio
        } else {
            let denominator = &bp - u_optimal;
            let numerator = &(u_current - u_optimal) * r_slope2;
            let result = (r_base + r_slope1) + numerator / denominator;

            if &result > r_max {
                r_max.clone()
            } else {
                result
            }
        }
    }

    fn compute_deposit_rate(
        &self,
        u_current: &BigUint,
        borrow_rate: &BigUint,
        reserve_factor: &BigUint,
    ) -> BigUint {
        let bp = BigUint::from(BP);

        (u_current * borrow_rate * (&bp - reserve_factor)) / (&bp * &bp)
    }

    fn compute_capital_utilisation(
        &self,
        borrowed_amount: &BigUint,
        total_reserves: &BigUint,
    ) -> BigUint {
        let bp = BigUint::from(BP);
        if *total_reserves == BigUint::zero() {
            BigUint::zero()
        } else {
            &(borrowed_amount * &bp) / total_reserves
        }
    }

    fn compute_interest(
        &self,
        amount: &BigUint,
        current_supply_index: &BigUint, // Market index
        initial_supply_index: &BigUint, // Account position index
    ) -> BigUint {
        let new_amount = (amount * current_supply_index) / initial_supply_index;
        new_amount - amount
    }
}
