multiversx_sc::imports!();

use common_structs::BP;

#[multiversx_sc::module]
pub trait LendingMathModule {
    fn compute_health_factor(
        &self,
        weighted_collateral_in_dollars: &BigUint,
        borrowed_value_in_dollars: &BigUint,
    ) -> BigUint {
        let health_factor = weighted_collateral_in_dollars / borrowed_value_in_dollars;

        health_factor / BP
    }
}
