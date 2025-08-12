// Example showing how rescale_half_up works with different values

use common_math::SharedMathModule;
use multiversx_sc::types::{BigUint, ManagedDecimal};
use multiversx_sc_scenario::api::StaticApi;

pub struct MathTester;
impl multiversx_sc::contract_base::ContractBase for MathTester {
    type Api = StaticApi;
}
impl SharedMathModule for MathTester {}

#[test]
fn test_rescale_half_up_examples() {
    let tester = MathTester;

    // Example 1: 1.234567... rounds to 1.2346
    let value1 = ManagedDecimal::<StaticApi, usize>::from_raw_units(
        BigUint::from(1_234_567_890_123_456_789u64),
        18,
    );
    let result1 = tester.rescale_half_up(&value1, 4);
    println!("1.234567890123456789 -> {}", result1);
    assert_eq!(result1.into_raw_units(), &BigUint::from(12346u64)); // 1.2346

    // Example 2: 0.123456... rounds to 0.1235
    let value2 = ManagedDecimal::<StaticApi, usize>::from_raw_units(
        BigUint::from(123_456_789_012_345_678u64), // 0.123456789012345678
        18,
    );
    let result2 = tester.rescale_half_up(&value2, 4);
    println!("0.123456789012345678 -> {}", result2);
    assert_eq!(result2.into_raw_units(), &BigUint::from(1235u64)); // 0.1235

    // Example 3: 0.123449... rounds to 0.1234 (rounds down)
    let value3 = ManagedDecimal::<StaticApi, usize>::from_raw_units(
        BigUint::from(123_449_999_999_999_999u64), // 0.123449999999999999
        18,
    );
    let result3 = tester.rescale_half_up(&value3, 4);
    println!("0.123449999999999999 -> {}", result3);
    assert_eq!(result3.into_raw_units(), &BigUint::from(1234u64)); // 0.1234

    // Example 4: 0.123450... rounds to 0.1235 (exactly half rounds up)
    let value4 = ManagedDecimal::<StaticApi, usize>::from_raw_units(
        BigUint::from(123_450_000_000_000_000u64), // 0.123450000000000000
        18,
    );
    let result4 = tester.rescale_half_up(&value4, 4);
    println!("0.123450000000000000 -> {}", result4);
    assert_eq!(result4.into_raw_units(), &BigUint::from(1235u64)); // 0.1235
}
