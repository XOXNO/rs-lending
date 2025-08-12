// Debug test to understand multiplication rounding

use common_math::SharedMathModule;
use multiversx_sc::types::{BigUint, ManagedDecimal};
use multiversx_sc_scenario::api::StaticApi;

pub struct MathTester;
impl multiversx_sc::contract_base::ContractBase for MathTester {
    type Api = StaticApi;
}
impl SharedMathModule for MathTester {}

#[test]
fn debug_mul_test() {
    let tester = MathTester;

    // 1.234 * 5.6 = ?
    let a = ManagedDecimal::<StaticApi, usize>::from_raw_units(BigUint::from(1234u64), 3); // 1.234
    let b = ManagedDecimal::<StaticApi, usize>::from_raw_units(BigUint::from(56u64), 1); // 5.6

    println!("a = {} (scale {})", a, a.scale());
    println!("b = {} (scale {})", b, b.scale());

    let result = tester.mul_half_up(&a, &b, 2);

    println!("result = {} (scale {})", result, result.scale());
    println!("result raw = {:?}", result.into_raw_units());

    // Manual calculation:
    // 1.234 * 5.6 = 6.9104
    // Rounded to 2 decimals with half-up: 6.91
    // Raw value should be 691
}
